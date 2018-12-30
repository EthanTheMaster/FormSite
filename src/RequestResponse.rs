use serde::Serialize;

use serde_json;

use actix_web::HttpResponse;

//Form Submission Structure
#[derive(Deserialize, Serialize, Debug)]
pub struct FormSubmission {
    pub questions: Vec<Question>,
    pub result_private: bool,
    #[serde(skip_serializing, default = "String::new")]
    pub captcha: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Question {
    pub question_type: String,
    pub data: QuestionData
}

#[derive(Deserialize, Serialize, Debug)]
pub struct QuestionData {
    pub question: String,
    pub required: bool,
    //Question specific fields
    pub options: Option<Vec<String>>
}

#[derive(Deserialize, Serialize, Debug)]
pub struct SurveySubmission {
    //form_id is String as JS does not support u64
    #[serde(skip_serializing)]
    pub form_id: String,
    #[serde(skip_serializing)]
    pub captcha: String,
    pub user_submission: Vec<UserSubmission>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct UserSubmission {
    //FRQ
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<String>,
    //MC
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected: Option<i64>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct CaptchaResponse {
    pub success: bool,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ResultsResponse {
    pub mc_questions: Vec<String>,
    pub mc_labels: Vec<Vec<String>>,
    pub mc_results: Vec<Vec<u64>>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct FormSubmitResponse {
    pub result_private: bool,
    pub password: String,
    pub form_id: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub message: T
}

impl FormSubmission {
    pub fn is_valid(&self) -> Result<(), &str> {
        if self.questions.len() == 0 {
            return Err("Form is empty");
        }

        for question in self.questions.iter() {
            if !(question.question_type.eq("frq") || question.question_type.eq("mc")) {
                return Err("There is not a valid question type");
            }
            if question.data.question.trim().eq("") {
                return Err("There is an empty question");
            }

            let options = &question.data.options;
            if question.question_type.eq("mc") {
                match options {
                    Some(vec) => {
                        if vec.len() == 0 {
                            return Err("There are no options for a multiple choice question");
                        }
                    },
                    None => {
                        return Err("There are no options for a multiple choice question")
                    }
                }
            }
        }

        Ok(())
    }
}

impl SurveySubmission {
    pub fn is_valid(&self, form: &FormSubmission) -> Result<(), &str> {
        let questions = &form.questions;
        let submissions = &self.user_submission;

        if questions.len() != submissions.len() {
            return Err("Number of responses does not match number of questions");
        }

        for (question, submission) in questions.iter().zip(submissions) {
            //Check for FRQ's
            if question.question_type.eq(&String::from("frq")) {
                match &submission.response {
                    Some(response) => {
                        let trimmed: &str = response.trim();

                        //Don't allow empty values for required FRQ
                        if trimmed == "" && question.data.required {
                            return Err("Not all required questions were answered");
                        }
                    },
                    None => {
                        if question.data.required {
                            return Err("Not all required questions were answered");
                        }
                    }
                }
            }

            //Check for MC
            if question.question_type.eq(&String::from("mc")) {
                match &submission.selected {
                    Some(selected) => {
                        //Make sure user answered required questions
                        if selected == &-1 && question.data.required {
                            return Err("Not all required questions were answered");
                        }

                        //Make sure bad data is not submitted
                        if *selected < -1 || *selected >= question.data.options.as_ref().unwrap().len() as i64 {
                            return Err("Invalid item selected");
                        }
                    },
                    None => {
                        if question.data.required {
                            return Err("Not all required questions were answered");
                        }
                    }
                }

            }
        }

        Ok(())
    }
}

impl<T: Serialize> ApiResponse<T> {
    pub fn new(success: bool, message: T) -> ApiResponse<T> {
        ApiResponse {
            success,
            message,
        }
    }

    pub fn make_response(&self) -> HttpResponse {
        HttpResponse::Ok().content_type("application/json").body(serde_json::to_string(&self).unwrap())
    }
}