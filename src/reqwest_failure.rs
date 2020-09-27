use std::fmt::Display;

use anyhow::{anyhow, Error};
use reqwest::blocking::Response;

pub fn handle_response_fail(response: Response, message: impl Display) -> Error {
    let status = response.status();
    let error = match response.text() {
        Ok(text) => text,
        Err(err) => err.to_string(),
    };
    return anyhow!("{}: {} ({})", message, status, error);
}
