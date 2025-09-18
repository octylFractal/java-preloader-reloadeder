use crate::command::{Context, JpreCommand};
use crate::error::{ESResult, JpreError};
use crate::java_version::key::VersionKey;
use crate::jdk_manager::JDK_MANAGER;
use crate::tui::jdk_color;
use clap::Args;
use error_stack::ResultExt;
use owo_colors::{OwoColorize, Stream};

/// Set the default JDK to use.
#[derive(Debug, Args)]
pub struct SetDefault {
    /// The JDK to use.
    jdk: VersionKey,
}

impl JpreCommand for SetDefault {
    fn run(self, mut context: Context) -> ESResult<(), JpreError> {
        if context
            .config
            .default_jdk
            .as_ref()
            .is_some_and(|i| i == &self.jdk)
        {
            eprintln!(
                "Default JDK already set to '{}'",
                self.jdk
                    .if_supports_color(Stream::Stderr, |s| s.color(jdk_color()))
            );
            return Ok(());
        }
        eprintln!(
            "Validating JDK '{}'...",
            self.jdk
                .if_supports_color(Stream::Stderr, |s| s.color(jdk_color()))
        );
        JDK_MANAGER
            .get_jdk_path(&context.config, &self.jdk)
            .change_context(JpreError::Unexpected)
            .attach_with(|| format!("Failed to get path for JDK {}", self.jdk))?;

        context.config.edit_config(|doc| {
            doc["default_jdk"] = toml_edit::value(self.jdk.to_string());
        })?;

        eprintln!(
            "Default JDK set to '{}'",
            self.jdk
                .if_supports_color(Stream::Stderr, |s| s.color(jdk_color()))
        );
        Ok(())
    }
}
