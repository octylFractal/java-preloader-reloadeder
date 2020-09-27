use std::fmt::Display;

use anyhow::{anyhow, Error, Context};

pub fn handle_response_fail(response: attohttpc::Response, message: impl Display) -> Error {
    let status = response.status();
    match response.text().context("Unable to de-serialize error from HTTP response") {
        Ok(upstream_error) => anyhow!("{}: {} ({})", message, status, upstream_error),
        Err(error) => error
    }
}
