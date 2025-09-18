use crate::command::{Context, JpreCommand};
use crate::error::{ESResult, JpreError, UserMessage};
use crate::java_home_management::set_context_path_to_java_home;
use crate::java_version::key::VersionKey;
use crate::tui::jdk_color;
use clap::Args;
use error_stack::Report;
use owo_colors::{OwoColorize, Stream};
use std::str::FromStr;

/// Use a JDK in the current context.
#[derive(Debug, Args)]
pub struct UseJdk {
    /// The JDK to use. Version key or 'default'.
    jdk: UseTarget,
}

#[derive(Debug, Clone)]
enum UseTarget {
    Default,
    VersionKey(VersionKey),
}

impl FromStr for UseTarget {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "default" => Ok(UseTarget::Default),
            _ => VersionKey::from_str(s)
                .map(UseTarget::VersionKey)
                .map_err(|_| {
                    "Invalid use target, expected 'default', or a version key".to_string()
                }),
        }
    }
}

impl JpreCommand for UseJdk {
    fn run(self, context: Context) -> ESResult<(), JpreError> {
        let jdk = match self.jdk {
            UseTarget::Default => context.config.default_jdk.clone().ok_or_else(|| {
                Report::new(JpreError::UserError).attach_opaque(UserMessage {
                    message: "No default JDK set".to_string(),
                })
            })?,
            UseTarget::VersionKey(jdk) => jdk,
        };
        set_context_path_to_java_home(&context, &jdk)?;

        eprintln!(
            "Using JDK {}",
            jdk.if_supports_color(Stream::Stderr, |s| s.color(jdk_color()))
        );
        Ok(())
    }
}
