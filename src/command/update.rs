use crate::command::{Context, JpreCommand};
use crate::error::{ESResult, JpreError, UserMessage};
use crate::foojay::FOOJAY_API;
use crate::java_version::key::VersionKey;
use crate::jdk_manager::JDK_MANAGER;
use crate::tui::jdk_color;
use clap::Args;
use error_stack::{Report, ResultExt};
use owo_colors::{OwoColorize, Stream};
use std::str::FromStr;
use tracing::warn;

/// Update installed Java versions.
#[derive(Debug, Args)]
pub struct UpdateInstalled {
    /// Check only, do not download new updates.
    #[clap(short, long)]
    check: bool,
    /// The JDK to update. Version key, 'all', or 'default'.
    target: UpdateTarget,
    /// Force update even if the version is the same.
    #[clap(short, long)]
    force: bool,
}

#[derive(Debug, Clone)]
enum UpdateTarget {
    All,
    Default,
    VersionKey(VersionKey),
}

impl FromStr for UpdateTarget {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "all" => Ok(UpdateTarget::All),
            "default" => Ok(UpdateTarget::Default),
            _ => VersionKey::from_str(s)
                .map(UpdateTarget::VersionKey)
                .map_err(|_| {
                    "Invalid update target, expected 'all', 'default', or a version key".to_string()
                }),
        }
    }
}

impl JpreCommand for UpdateInstalled {
    fn run(self, context: Context) -> ESResult<(), JpreError> {
        let mut installed = JDK_MANAGER
            .get_installed_jdks()
            .change_context(JpreError::Unexpected)
            .attach("Failed to get installed JDKs")?;

        let retain_fn: Box<dyn Fn(&VersionKey) -> bool> = match self.target {
            UpdateTarget::All => Box::new(|_| true),
            UpdateTarget::Default => {
                let Some(default) = context.config.default_jdk.clone() else {
                    return Err(
                        Report::new(JpreError::UserError).attach_opaque(UserMessage {
                            message: "No default JDK set".to_string(),
                        }),
                    );
                };
                Box::new(move |jdk| jdk == &default)
            }
            UpdateTarget::VersionKey(key) => Box::new(move |jdk| jdk == &key),
        };
        installed.retain(retain_fn);

        installed.sort();

        eprintln!("Checking updates for installed JDKs...");
        for jdk in installed {
            eprintln!(
                "Checking for updates for {}",
                jdk.if_supports_color(Stream::Stderr, |s| s.color(jdk_color()))
            );
            let full_version = match JDK_MANAGER.get_full_version(&jdk) {
                Ok(full_version) => full_version,
                Err(err) => {
                    warn!("Failed to get full version for {}: {:?}", jdk, err);
                    continue;
                }
            };

            if let Some(full_version) = full_version {
                let latest_info_result = FOOJAY_API
                    .get_latest_package_info_using_priority(&context.config, &jdk)
                    .change_context(JpreError::Unexpected)
                    .attach("Failed to get latest package info");
                let (list_info, _) = match latest_info_result {
                    Ok(info) => info,
                    Err(err) => {
                        warn!("Failed to get latest package info for {}: {:?}", jdk, err);
                        continue;
                    }
                };
                let latest = list_info.java_version;
                let do_update = if latest.compare(&full_version) == std::cmp::Ordering::Greater {
                    eprintln!(
                        "  New version available: {}",
                        latest.if_supports_color(Stream::Stderr, |s| s.color(jdk_color()))
                    );
                    true
                } else {
                    eprintln!(
                        "  Already up-to-date: {}",
                        full_version.if_supports_color(Stream::Stderr, |s| s.color(jdk_color()))
                    );
                    if self.force {
                        eprintln!("  Forcing re-install...");
                    }
                    self.force
                };
                if do_update && !self.check {
                    Self::update_jdk(&context, &jdk)?;
                }
            } else {
                warn!("No full version found for {}", jdk);
                if !self.check {
                    warn!("Re-installing JDK {}", jdk);
                    Self::update_jdk(&context, &jdk)?;
                }
            }
        }

        Ok(())
    }
}

impl UpdateInstalled {
    fn update_jdk(context: &Context, jdk: &VersionKey) -> Result<(), Report<JpreError>> {
        JDK_MANAGER
            .download_jdk(&context.config, jdk)
            .change_context(JpreError::Unexpected)
            .attach("Failed to update JDK")?;
        Ok(())
    }
}
