use crate::command::{Context, JpreCommand};
use crate::error::{ESResult, JpreError, UserMessage};
use crate::foojay::FOOJAY_API;
use clap::Args;
use error_stack::{Report, ResultExt};
use itertools::Itertools;
use std::collections::HashSet;

/// Set the distribution to use.
#[derive(Debug, Args)]
pub struct SetDistribution {
    /// The distribution to use.
    #[clap(name = "distribution")]
    distribution: String,
}

impl JpreCommand for SetDistribution {
    fn run(self, mut context: Context) -> ESResult<(), JpreError> {
        if self.distribution == context.config.distribution {
            eprintln!("Distribution already set to '{}'", self.distribution);
            return Ok(());
        }
        eprintln!("Validating distribution '{}'...", self.distribution);
        let mut distributions = FOOJAY_API
            .list_distributions()
            .change_context(JpreError::Unexpected)
            .attach_printable("Failed to list distributions")?;
        let all_names = distributions
            .iter()
            .flat_map(|i| &i.synonyms)
            .collect::<HashSet<_>>();
        if !all_names.contains(&self.distribution) {
            distributions.sort();
            return Err(Report::new(JpreError::UserError)
                .attach(UserMessage {
                    message: format!("Distribution '{}' not found", context.config.distribution),
                })
                .attach(UserMessage {
                    message: format!(
                        "Available distributions: {}",
                        distributions.into_iter().map(|i| i.name).join(", ")
                    ),
                }));
        }
        context.config.distribution = self.distribution.clone();
        context
            .config
            .save()
            .change_context(JpreError::Unexpected)
            .attach_printable("Failed to save config")?;
        eprintln!("Distribution set to '{}'", self.distribution);
        Ok(())
    }
}
