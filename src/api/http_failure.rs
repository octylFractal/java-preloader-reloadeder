use std::fmt::Display;

use crate::api::def::JdkFetchError;

pub fn handle_response_fail(response: attohttpc::Response, message: impl Display) -> JdkFetchError {
    let status = response.status();
    match response.text() {
        Ok(upstream_error) => JdkFetchError::Upstream {
            message: format!("{}: {} ({})", message, status, upstream_error),
        },
        Err(error) => JdkFetchError::HttpIo(error),
    }
}
