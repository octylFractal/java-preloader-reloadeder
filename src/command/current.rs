use crate::command::{Context, JpreCommand};
use crate::context_id::get_context_path;
use crate::error::{ESResult, JpreError};
use crate::jdk_manager::JDK_MANAGER;
use clap::Args;
use error_stack::ResultExt;

/// Emit the full current Java version.
#[derive(Debug, Args)]
pub struct Current {}

impl JpreCommand for Current {
    fn run(self, _context: Context) -> ESResult<(), JpreError> {
        let path = get_context_path();
        if !path.exists() {
            println!("<unknown>");
            return Ok(());
        }
        let link_target = std::fs::read_link(&path)
            .change_context(JpreError::Unexpected)
            .attach_printable_lazy(|| format!("Failed to read link target of {:?}", path))?;
        let full_version = JDK_MANAGER
            .get_full_version_from_path(&link_target)
            .change_context(JpreError::Unexpected)
            .attach_printable_lazy(|| format!("Failed to get full version of {:?}", link_target))?;

        println!(
            "{}",
            full_version
                .map(|v| v.to_string())
                .unwrap_or("<unknown>".to_string())
        );

        Ok(())
    }
}
