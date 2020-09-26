use reqwest::blocking::Response;
use std::error::Error;
use std::fmt::Display;

pub fn handle_response_fail(response: Response, message: impl Display) -> Box<dyn Error> {
    let status = response.status();
    let error = match response.text() {
        Ok(text) => text,
        Err(err) => err.to_string(),
    };
    return Box::from(format!("{}: {} ({})", message, status, error));
}
