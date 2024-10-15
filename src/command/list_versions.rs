use crate::command::{Context, JpreCommand};
use crate::error::{ESResult, JpreError, UserMessage};
use crate::foojay::{FoojayDiscoApiError, FOOJAY_API};
use crate::java_version::PreRelease;
use clap::ArgAction;
use clap::Args;

/// List all available version keys.
#[derive(Debug, Args)]
pub struct ListVersions {
    /// The distribution to list versions for.
    /// Defaults to the current distribution.
    #[clap()]
    distribution: Option<String>,
    /// Show pre-release versions.
    #[clap(long, action = ArgAction::Set, default_value = "false", default_missing_value = "true", num_args = 0..=1)]
    pre_release: bool,
    /// Show General Availability versions. Defaults to `true`.
    #[clap(long, action = ArgAction::Set, default_value = "true", default_missing_value = "true", num_args = 0..=1)]
    ga: bool,
}

impl JpreCommand for ListVersions {
    fn run(self, context: Context) -> ESResult<(), JpreError> {
        let distribution = self
            .distribution
            .as_ref()
            .unwrap_or(&context.config.distribution);
        eprintln!("Listing versions for distribution '{}'...", distribution);
        let result = FOOJAY_API.list_dist_version_keys(distribution);
        let mut major_versions = match result {
            Ok(result) => Vec::from_iter(result),
            Err(err)
                if matches!(
                    err.current_context(),
                    FoojayDiscoApiError::InvalidDistribution
                ) =>
            {
                return Err(err
                    .change_context(JpreError::UserError)
                    .attach(UserMessage {
                        message: format!("Distribution '{}' not found", distribution),
                    }));
            }
            Err(err) => {
                return Err(err
                    .change_context(JpreError::Unexpected)
                    .attach_printable("Failed to list versions"))
            }
        };
        major_versions.sort();
        for version in major_versions {
            if !self.pre_release && version.pre_release != PreRelease::None {
                continue;
            }
            if !self.ga && version.pre_release == PreRelease::None {
                continue;
            }
            println!("- {}", version);
        }
        Ok(())
    }
}
