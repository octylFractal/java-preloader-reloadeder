use crate::command::{Context, JpreCommand};
use crate::error::{ESResult, JpreError};
use crate::jdk_manager::JDK_MANAGER;
use crate::tui::jdk_color;
use clap::Args;
use error_stack::ResultExt;
use owo_colors::{OwoColorize, Stream};

/// List all installed Java versions.
#[derive(Debug, Args)]
pub struct ListInstalled {}

impl JpreCommand for ListInstalled {
    fn run(self, _context: Context) -> ESResult<(), JpreError> {
        let mut installed = JDK_MANAGER
            .get_installed_jdks()
            .change_context(JpreError::Unexpected)
            .attach_printable("Failed to get installed JDKs")?;

        installed.sort();

        eprintln!("Installed JDKs:");
        for jdk in installed {
            let full = JDK_MANAGER
                .get_full_version(&jdk)
                .change_context(JpreError::Unexpected)
                .attach_printable_lazy(|| format!("Failed to get full version for JDK {}", jdk))?;
            println!(
                "- {} (full: {})",
                jdk.if_supports_color(Stream::Stdout, |s| s.color(jdk_color())),
                full.map(|f| f.to_string())
                    .unwrap_or_else(|| "<unknown>".to_string())
                    .if_supports_color(Stream::Stdout, |s| s.color(jdk_color()))
            );
        }

        Ok(())
    }
}
