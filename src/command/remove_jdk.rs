use crate::command::{Context, JpreCommand};
use crate::error::{ESResult, JpreError};
use crate::java_version::key::VersionKey;
use crate::jdk_manager::JDK_MANAGER;
use crate::tui::jdk_color;
use clap::Args;
use error_stack::ResultExt;
use owo_colors::{OwoColorize, Stream};

/// Remove an installed JDK.
#[derive(Debug, Args)]
pub struct RemoveJdk {
    /// The JDK to remove.
    jdk: VersionKey,
}

impl JpreCommand for RemoveJdk {
    fn run(self, context: Context) -> ESResult<(), JpreError> {
        let path = JDK_MANAGER
            .get_jdk_path(&context.config, &self.jdk)
            .change_context(JpreError::Unexpected)
            .attach_with(|| format!("Failed to get path for JDK {}", self.jdk))?;
        std::fs::remove_dir_all(&path)
            .change_context(JpreError::Unexpected)
            .attach_with(|| format!("Failed to remove JDK at {}", path.display()))?;
        eprintln!(
            "Removed JDK {}",
            self.jdk
                .if_supports_color(Stream::Stderr, |s| s.color(jdk_color()))
        );
        Ok(())
    }
}
