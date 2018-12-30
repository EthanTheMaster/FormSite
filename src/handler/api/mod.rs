use handler::get_sql_connection;

use RequestResponse::*;

use fasthash::XXHasher;
use std::hash::Hasher;

use actix_web::{Path, Responder, HttpRequest, Json};
use actix_web::http::Cookie;

use serde_json;
use serde_json::Value;

use reqwest::Client;
use reqwest::header::CONTENT_LENGTH;

use rand::thread_rng;
use rand::Rng;

use mysql as my;

fn verify_captcha(captcha: &String) -> bool {
    let http_client = Client::new();

    let captcha_verify = http_client.post("https://www.google.com/recaptcha/api/siteverify")
        .query(
            &[
                ("secret", "6LciFH8UAAAAAHW21Ee5yY19cYFLmXuCFlcdDo2m"),
                ("response", &captcha)
            ])
        .header(CONTENT_LENGTH, "0")
        .send();

    match captcha_verify {
        Ok(mut response) => {
            let json = response.text().unwrap();
            let api_response = serde_json::from_str::<CaptchaResponse>(&json).unwrap();
            return api_response.success;
        },
        Err(_) => {
            return false;
        },
    }
}

pub fn get_form<'a>(form_id: u64) -> Result<FormSubmission, &'a str> {
    let pool = get_sql_connection();
    let mut stmt = pool.prep_exec("SELECT form_json FROM form_submissions WHERE id = :id", params!("id" => form_id));
    match stmt {
        Ok(ref mut results) => {
            match results.next() {
                None => {
                    return Err("Invalid Id");
                },
                Some(row) => {
                    let json_form_data = my::from_row::<String>(row.unwrap());
                    return Ok(serde_json::from_str::<FormSubmission>(&json_form_data).unwrap());
                },
            }
        },
        Err(_) => {
            return Err("An error occurred");
        },
    }
}

pub fn verify_survey_results_password(form_id: u64, password: &String) -> bool {
    let conn = get_sql_connection();
    let mut stmt = conn.prep_exec("SELECT form_json, password FROM form_submissions WHERE id = :form_id",
                                                                params!(
                                                                            "form_id" => form_id
                                                                        ));
    match stmt {
        Ok(ref mut results) => {
            match results.next() {
                None => {return false},
                Some(row) => {
                    let (form_json, survey_password) = my::from_row::<(String, Option<String>)>(row.unwrap());
                    let is_private = serde_json::from_str::<FormSubmission>(&form_json).unwrap().result_private;
                    if is_private {
                        return password == &survey_password.unwrap();
                    } else {
                        //Accept any password for surveys without a password
                        return true;
                    }
                },
            }
        },
        Err(_) => {return false},
    }
}

pub fn handle_api_get_form(info: Path<u64>) -> impl Responder {
    match get_form(info.into_inner()) {
        Ok(form) => {
            return ApiResponse::new(true, form).make_response();
        },
        Err(e) => {
            return ApiResponse::new(false, e).make_response()
        },
    }
}

pub fn handle_api_get_survey(info: (Path<u64>, HttpRequest)) -> impl Responder {
    let uuid_cookie = info.1.cookie("uuid");
    match uuid_cookie {
        None => {
            return ApiResponse::new(false, "Not a user!").make_response();
        },
        Some(cookie) => {
            let form_id = info.0.into_inner();
            let uuid = cookie.value();

            let conn = get_sql_connection();
            let mut stmt = conn.prep_exec("SELECT survey_json FROM survey_submissions WHERE form_id = :form_id AND uuid = :uuid",
                                          params!("form_id" => form_id,
                                                    "uuid" => uuid));
            match stmt {
                Ok(ref mut query) => {
                    //Get first sql row
                    let results = query.next();

                    match results {
                        None => {
                            return ApiResponse::new(false, "Invalid form id or uuid.").make_response();
                        },
                        Some(row) => {
                            let survey_json = my::from_row::<String>(row.unwrap());
                            return ApiResponse::new(true, serde_json::from_str::<Vec<UserSubmission>>(&survey_json).unwrap()).make_response();
                        },
                    }

                },
                Err(_) => {
                    return ApiResponse::new(false, "An error has occurred.").make_response();
                },
            }
        },
    }
}

//Take in JSON data of form
pub fn handle_form_submit(submission: Json<Value>) -> impl Responder {
    let json_content = serde_json::to_string(&submission.0).unwrap();
    let deserialized = serde_json::from_value::<FormSubmission>(submission.0);

    let mut hasher: XXHasher = Default::default();
    hasher.write(json_content.as_bytes());
    let hash = hasher.finish();

    //Did form deserialize correctly?
    match deserialized {
        Ok(submission) => {
            //From valid verification
            let is_valid = submission.is_valid();
            if let Err(e) = is_valid {
                return ApiResponse::new(false, e).make_response();
            }
            //Captcha verification
            if !verify_captcha(&submission.captcha) {
                return ApiResponse::new(false, "Invalid Captcha").make_response();
            }

            //Add to database
            let pool = get_sql_connection();

            //Generate a random password
            let rand_num: u64 = thread_rng().gen::<u64>();
            let password = format!("{:X}", rand_num);
            pool.prep_exec("INSERT INTO form_submissions (id, form_json, password) VALUES (:id, :form_json, :password)",
                           params!(
                    "id" => hash,
                    "form_json" => serde_json::to_string::<FormSubmission>(&submission).unwrap(),
                    "password" => password.clone()
                )).iter();

            let response = FormSubmitResponse {
                result_private: submission.result_private,
                password,
                form_id: format!("{}", hash),
            };
            return ApiResponse::new(true, response).make_response();
        },
        Err(e) => {
            return ApiResponse::new(false, format!("{:?}", e)).make_response()
        },
    }

}

pub fn handle_survey_submit(info: (Json<Value>, HttpRequest)) -> impl Responder {
    let submission = info.0;
    let req = info.1;

    //Make sure json submitted follows correct form
    let deserialized = serde_json::from_value::<SurveySubmission>(submission.0);
    match deserialized {
        Ok(survey_submission) => {
            println!("{:?}", survey_submission);
            //Verify the captcha
            if !verify_captcha(&survey_submission.captcha) {
                return ApiResponse::new(false, "Invalid Captcha").make_response()
            }

            //Get uuid / set uuid if there is none
            let uuid_cookie = req.cookie("uuid");
            let mut uuid: String = String::from("");
            match uuid_cookie {
                None => {
                    //Generate a UUID
                    uuid = format!("{}", thread_rng().gen::<u64>());
                    //println!("MADE UUID: {}", uuid);
                },
                Some(cookie) => {
                    uuid = String::from(cookie.value());
                    //println!("EXISTING UUID: {}", uuid);
                },
            }

            let conn = get_sql_connection();
            //Parse form id string as u64
            let mut form_id: u64 = 0;
            match survey_submission.form_id.parse::<u64>() {
                Ok(id) => {
                    form_id = id;
                },
                Err(_) => {
                    return ApiResponse::new(false, "Invalid form id").make_response();
                }
            }
            //Grab the form data and verify that the submission follows the specs
            match get_form(form_id) {
                Ok(form) => {
                    let is_valid_form = survey_submission.is_valid(&form);
                    match is_valid_form {
                        Ok(_) => {},
                        Err(e) => {
                            return ApiResponse::new(false, e).make_response()
                        },
                    }
                },
                Err(e) => {
                    return ApiResponse::new(false, e).make_response()
                },
            }
            //Load submission into DB
            let stmt = conn.prep_exec("SELECT form_id FROM survey_submissions WHERE form_id = :form_id AND uuid = :uuid",
                                      params!("form_id" => form_id,
                                            "uuid" => uuid.clone()));
            //Has the user submitted before? If so update the user's row. If not create a new row
            if stmt.is_ok() {
                if stmt.unwrap().next().is_some() {
                    conn.prep_exec("UPDATE survey_submissions SET survey_json = :survey_json WHERE form_id = :form_id AND uuid = :uuid",
                                   params!("form_id" => form_id,
                                            "uuid" => uuid.clone(),
                                            "survey_json" => serde_json::to_string(&survey_submission.user_submission).unwrap()));

                } else {
                    conn.prep_exec("INSERT INTO survey_submissions (form_id, uuid, survey_json) VALUES (:form_id, :uuid, :survey_json)",
                                   params!("form_id" => form_id,
                                            "uuid" => uuid.clone(),
                                            "survey_json" => serde_json::to_string(&survey_submission.user_submission).unwrap()));

                }
            } else {
                return ApiResponse::new(false, "An error has occurred").make_response();
            }

            //Generate success response and add uuid cookie to reload user's data
            let mut server_response = ApiResponse::new(true, "Boom").make_response();
            let mut uuid_cookie = Cookie::new("uuid", uuid);
            uuid_cookie.make_permanent();
            server_response.add_cookie(&uuid_cookie);

            return server_response;
        },
        Err(_) => {
            return ApiResponse::new(false, "The submission could not be parsed correctly").make_response()
        }
    }
}

pub fn handle_api_get_results(info: (Path<u64>, HttpRequest)) -> impl Responder {
    let conn = get_sql_connection();
    let form_id = info.0.into_inner();

    //Make sure that the password is correct is the results should be private
    let query = info.1.query();
    let password = query.get(&String::from("password"));
    let mut should_proceed = false;
    match password {
        None => {
            should_proceed = verify_survey_results_password(form_id, &String::new());
        },
        Some(password) => {
            should_proceed = verify_survey_results_password(form_id, password);
        },
    }
    if !should_proceed {
        return ApiResponse::new(false, "The results are private").make_response();
    }

    //Get the form and generate the labels
    let form_result = get_form(form_id);
    let mut mc_questions: Vec<String> = Vec::new();
    let mut mc_labels: Vec<Vec<String>> = Vec::new();
    let mut mc_results: Vec<Vec<u64>> = Vec::new();
    match form_result {
        Ok(form) => {
            for question in form.questions.iter() {
                let data = &question.data;
                match data.options {
                    None => {continue},
                    Some(ref labels) => {
                        mc_questions.push(data.question.clone());
                        mc_labels.push(labels.clone());
                        mc_results.push(vec![0; labels.len()]);
                    },
                }
            }
        },
        Err(e) => {
            return ApiResponse::new(false, e).make_response();
        },
    }

    //Go through all submissions and tally results
    let stmt = conn.prep_exec("SELECT survey_json FROM survey_submissions WHERE form_id = :form_id",
                              params!("form_id" => form_id));
    match stmt {
        Ok(query) => {
            for row in query {
                let survey_json = my::from_row::<String>(row.unwrap());
                //Tally up the MC results
                let mut ith_mc: usize = 0; //the submission's mc responses should align with labels
                let submission = serde_json::from_str::<Vec<UserSubmission>>(&survey_json).unwrap();
                for response in submission.iter() {
                    match response.selected {
                        None => {continue;},
                        Some(selected) => {
                            //Don't count empty responses
                            if selected != -1 {
                                *mc_results.get_mut(ith_mc).unwrap().get_mut(selected as usize).unwrap() += 1;
                            }
                            ith_mc += 1;
                        },
                    }
                }
            }

            let results_response = ResultsResponse {
                mc_questions,
                mc_labels,
                mc_results,
            };
            return ApiResponse::new(true, results_response).make_response();
        },
        Err(_) => {
            return ApiResponse::new(false, "An error has occurred").make_response();
        },
    }
}