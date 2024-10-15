use derive_more::Display;
use error_stack::Context;

pub type ESResult<T, E> = error_stack::Result<T, E>;

#[derive(Debug, Display)]
pub enum JpreError {
    /// Error from user input.
    #[display("User error")]
    UserError,
    /// Error from APIs, OS, etc.
    #[display("An unexpected error occurred")]
    Unexpected,
}

impl Context for JpreError {}

/// Message for the user. Attached when the error is a [`JpreError::UserError`].
#[derive(Debug)]
pub struct UserMessage {
    pub message: String,
}
