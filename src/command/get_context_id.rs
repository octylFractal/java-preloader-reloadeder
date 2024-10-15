use crate::command::{Context, JpreCommand};
use crate::context_id::get_context_id;
use crate::error::{ESResult, JpreError};
use clap::Args;

/// Emit the current context ID, usually to set a var indicating the context for all child
/// processes. This is necessary sometimes to ensure the correct context is used in shell
/// formatting.
#[derive(Debug, Args)]
pub struct GetContextId {}

impl JpreCommand for GetContextId {
    fn run(self, _context: Context) -> ESResult<(), JpreError> {
        println!("{}", get_context_id());
        Ok(())
    }
}
