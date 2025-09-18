use crate::command::{Context, JpreCommand};
use crate::context_id::get_context_path;
use crate::error::{ESResult, JpreError};
use crate::java_home_management::{clear_context_path, set_context_path_to_java_home};
use clap::Args;
use error_stack::ResultExt;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use tracing::debug;

/// Emit the Java home path.
#[derive(Debug, Args)]
pub struct JavaHome {}

impl JpreCommand for JavaHome {
    fn run(self, context: Context) -> ESResult<(), JpreError> {
        clear_context_path()?;

        debug!("Setting to default if necessary");
        if let Some(default) = context.config.default_jdk.clone() {
            set_context_path_to_java_home(&context, &default)?;
        }

        (|| -> std::io::Result<()> {
            let mut stdout = std::io::stdout();
            stdout.write_all(get_context_path().into_os_string().as_bytes())?;
            stdout.write_all(b"\n")?;
            stdout.flush()?;
            Ok(())
        })()
        .change_context(JpreError::Unexpected)
        .attach("Failed to write Java home path to stderr")?;

        Ok(())
    }
}
