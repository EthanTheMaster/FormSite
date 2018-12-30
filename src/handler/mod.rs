
use actix_web::{Path, Result, HttpRequest, Responder, Error, Body, HttpResponse};
use actix_web::http::StatusCode;

use std::fs::File;
use std::io::Read;


use mysql as my;
use serde_json;

use csv::Writer;

pub mod api;

use RequestResponse::*;

#[derive(Deserialize, Serialize, Debug)]
pub struct FileHandler {
    path: String
}

fn get_sql_connection() -> my::Pool {
    return my::Pool::new("mysql://root:password@db:3306/FormSite").unwrap();
}

//Retrieve file from filesystem and respond with the file's content
impl Responder for FileHandler {
    type Item = HttpResponse;
    type Error = Error;

    fn respond_to<S: 'static>(self, _req: &HttpRequest<S>) -> Result<Self::Item, Self::Error> {
        let mut file = File::open(self.path);
        match file {
            Ok(ref mut file) => {
                let mut buf: Vec<u8> = Vec::new();
                file.read_to_end(&mut buf);
                return Ok(HttpResponse::Ok().body(Body::from_slice(buf.as_ref())));
            },
            Err(_) => {
                return Ok(HttpResponse::NotFound().body("404"));
            },
        }
    }
}

impl FileHandler {
    fn new(path: String) -> FileHandler {
        FileHandler {path}
    }
}

//Response with file from the `assets` folder
pub fn handle_asset(path: Path<FileHandler>) -> impl Responder {
    FileHandler::new(String::from("web/assets/") + &path.path)
}

//Response with file from the `download` folder
pub fn handle_download(path: Path<FileHandler>) -> impl Responder {
    FileHandler::new(String::from("web/download/") + &path.path)
}

//Response with the index page
pub fn index(_req: &HttpRequest) -> impl Responder {
    FileHandler::new(String::from("web/index.html"))
}

//Go to the survey page
pub fn survey(_req: &HttpRequest) -> impl Responder {
    return FileHandler::new(String::from("web/survey.html"));
}

//Go to the results page for the survey
pub fn results(info: &HttpRequest) -> impl Responder {
    return FileHandler::new(String::from("web/results.html"));
}

//Download the results as a csv
pub fn download_results(info: (HttpRequest, Path<u64>)) -> impl Responder {
    let error_response = HttpResponse::build(StatusCode::from_u16(302).unwrap()).header("Location", "/").finish();

    let conn = get_sql_connection();

    let form_id = info.1.into_inner();

    //Verify results password
    let query = info.0.query();
    let password = query.get(&String::from("password"));
    let mut should_proceed = false;
    //Check that the password is correct
    match password {
        None => {
            return error_response;
        },
        Some(password) => {
            should_proceed =
                match conn.prep_exec("SELECT id FROM form_submissions WHERE password = :password", params!("password" => password)) {
                    Ok(ref mut results) => {
                        match results.next() {
                            None => {false},
                            Some(_) => {true},
                        }
                    },
                    Err(_) => {
                        false
                    },
                };
        },
    }
    if !should_proceed {
        return error_response;
    }
    //Password is correct ... generate the csv
    let rand_file_name = format!("{}{}.csv", form_id, password.unwrap());
    let mut writer = Writer::from_path(format!("web/download/{}", rand_file_name)).unwrap();
    //Get the form as csv template
    let form = api::get_form(form_id);
    match form {
        Ok(form) => {
            //Write the header row
            let questions: Vec<String> = form.questions.iter().map(|q| q.data.question.clone()).collect();
            if writer.write_record(questions).is_err() {
                return error_response;
            }

            //Write in all responses
            let retrieve = conn.prep_exec("SELECT survey_json FROM survey_submissions WHERE form_id = :form_id", params!("form_id" => form_id));
            match retrieve {
                Ok(results) => {
                    //Map each row/user response to a list of strings turning each mc selected to the option & frq response to a string
                    let csv_record: Vec<Vec<String>> = results.map(|row| {
                        let json = my::from_row::<String>(row.unwrap());
                        let submission = serde_json::from_str::<Vec<UserSubmission>>(&json).unwrap();
                        //Map each user submission to the corresponding string
                        return submission.iter().enumerate().map(|(idx, response)| {
                            let question_type = &form.questions.get(idx).unwrap().question_type;
                            let mut output = String::from("");
                            if question_type == "mc" {
                                if response.selected.unwrap() != -1 {
                                    output = form.questions.get(idx).as_ref().unwrap() //get the idx-th question
                                                .data.options.as_ref().unwrap() //get the options
                                                .get(response.selected.unwrap() as usize).unwrap().clone(); // get selected choice
                                }
                            }
                            if question_type == "frq" {
                                output = response.response.as_ref().unwrap().clone();
                            }
                            return output;
                        }).collect();
                    }).collect();
                    //Write each record
                    for record in csv_record.iter() {
                        //Write each record
                        if writer.write_record(record).is_err() {
                            return error_response;
                        }
                    }
                    //Generate the file and redirect the browser to the download link
                    match writer.flush() {
                        Ok(_) => {
                            return HttpResponse::build(StatusCode::from_u16(302).unwrap())
                                                .header("Location", format!("/download/{}", rand_file_name))
                                                .finish()
                        },
                        Err(_) => {
                            return error_response;
                        },
                    }

                },
                Err(_) => {
                    return error_response;
                },
            }
        },
        Err(_) => {
            return error_response;
        },
    }
}