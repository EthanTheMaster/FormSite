#[macro_use]
extern crate serde_derive;

extern crate actix_web;
extern crate serde_json;
extern crate serde;
extern crate futures;
extern crate fasthash;
extern crate reqwest;
extern crate rand;
extern crate csv;

#[macro_use]
extern crate mysql;

mod handler;
mod RequestResponse;

use actix_web::{App, server};
use actix_web::http::Method;

fn main() {
    println!("SERVER IS RUNNING ON 0.0.0.0:8080");
    server::new(|| {
        App::new()
            .resource("/", |r| r.method(Method::GET).f(handler::index))
            .resource("/{form_id}/results", |r| r.method(Method::GET).f(handler::results))
            .resource("/{form_id}/download", |r| r.method(Method::GET).with(handler::download_results))
            .resource("/{form_id}/", |r| r.method(Method::GET).f(handler::survey))
            .resource("/{form_id}", |r| r.method(Method::GET).f(handler::survey))
            .scope("/api", |scope| {
                scope
                    .resource("/submit", |r| {
                        r.method(Method::POST).with(handler::api::handle_form_submit)
                    })
                    .resource("/getform/{form_id}", |r| {
                        r.method(Method::GET).with(handler::api::handle_api_get_form)
                    })
                    .resource("/submitsurvey", |r| {
                        r.method(Method::POST).with(handler::api::handle_survey_submit)
                    })
                    .resource("/getsurvey/{form_id}", |r| {
                        r.method(Method::GET).with(handler::api::handle_api_get_survey)
                    })
                    .resource("/getresults/{form_id}", |r| {
                        r.method(Method::GET).with(handler::api::handle_api_get_results)
                    })
            })
            .resource("/assets/{path:.*}", |r| r.method(Method::GET).with(handler::handle_asset))
            .resource("/download/{path:.*}", |r| r.method(Method::GET).with(handler::handle_download))
    })
        .bind("0.0.0.0:8080")
        .unwrap()
        .run();
}
