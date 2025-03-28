use crate::command::{Context, JpreCommand};
use crate::error::{ESResult, JpreError, UserMessage};
use crate::foojay::FOOJAY_API;
use clap::Args;
use error_stack::{Report, ResultExt};
use itertools::Itertools;
use std::collections::HashSet;

/// Set the distribution(s) to use.
#[derive(Debug, Args)]
pub struct SetDistributions {
    /// The distribution(s) to use.
    #[clap(required = true, num_args = 1..)]
    distributions: Vec<String>,
}

impl JpreCommand for SetDistributions {
    fn run(self, mut context: Context) -> ESResult<(), JpreError> {
        if self.distributions == context.config.distributions {
            eprintln!(
                "Distribution(s) already set to '{}'",
                self.distributions.join(", ")
            );
            return Ok(());
        }
        eprintln!(
            "Validating distribution(s) '{}'...",
            self.distributions.join(", ")
        );
        let distributions = FOOJAY_API
            .list_distributions()
            .change_context(JpreError::Unexpected)
            .attach_printable("Failed to list distributions")?;
        let all_names = distributions
            .iter()
            .flat_map(|i| &i.synonyms)
            .map(String::as_str)
            .collect::<HashSet<_>>();
        let mut missing_names = self
            .distributions
            .iter()
            .map(String::as_str)
            .filter(|i| !all_names.contains(*i))
            .collect::<Vec<_>>();
        if !missing_names.is_empty() {
            missing_names.sort();
            return Err(Report::new(JpreError::UserError)
                .attach(UserMessage {
                    message: format!("Distribution(s) '{}' not found", missing_names.join(", ")),
                })
                .attach(UserMessage {
                    message: format!(
                        "Available distributions: {}",
                        distributions.into_iter().map(|i| i.name).join(", ")
                    ),
                }));
        }

        context.config.edit_config(|doc| {
            let mut distributions = toml_edit::Array::new();
            for distribution in &self.distributions {
                distributions.push(toml_edit::Value::from(distribution));
            }
            distributions.fmt();

            doc["distributions"] = toml_edit::value(distributions);
        })?;

        eprintln!("Distribution(s) set to '{}'", self.distributions.join(", "));
        Ok(())
    }
}
