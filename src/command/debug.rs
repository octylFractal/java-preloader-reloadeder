use crate::command::{Context, JpreCommand};
use crate::context_id::get_context_id;
use crate::error::{ESResult, JpreError};
use clap::{Args, Subcommand};
use owo_colors::{OwoColorize, Stream};

/// Debug commands.
#[derive(Debug, Args)]
pub struct Debug {
    #[clap(subcommand)]
    subcommand: DebugSubcommand,
}

/// Debug subcommands.
#[derive(Debug, Subcommand)]
enum DebugSubcommand {
    /// Show context ID.
    ContextId,
}

impl JpreCommand for Debug {
    fn run(self, _context: Context) -> ESResult<(), JpreError> {
        match self.subcommand {
            DebugSubcommand::ContextId => {
                println!(
                    "Context ID: {}",
                    get_context_id().if_supports_color(Stream::Stdout, |s| s.red())
                );
            }
        }
        Ok(())
    }
}
