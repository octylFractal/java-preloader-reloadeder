use derive_more::Display;
use error_stack::Report;
use std::error::Error;

pub type ESResult<T, E> = Result<T, Report<E>>;

#[derive(Debug, Display)]
pub enum JpreError {
    /// Error from user input.
    #[display("User error")]
    UserError,
    /// Error from APIs, OS, etc.
    #[display("An unexpected error occurred")]
    Unexpected,
}

impl Error for JpreError {}

/// Message for the user. Attached when the error is a [`JpreError::UserError`].
#[derive(Debug)]
pub struct UserMessage {
    pub message: String,
}
