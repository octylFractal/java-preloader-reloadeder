use crate::command::{Context, JpreCommand};
use crate::error::{ESResult, JpreError};
use crate::foojay::FOOJAY_API;
use clap::Args;
use error_stack::ResultExt;

/// List all available distributions.
#[derive(Debug, Args)]
pub struct ListDistributions {
    /// Show synonyms.
    #[clap(long, action = clap::ArgAction::Set, default_value = "false", default_missing_value = "true", num_args = 0..=1)]
    synonyms: bool,
}

impl JpreCommand for ListDistributions {
    fn run(self, _context: Context) -> ESResult<(), JpreError> {
        eprintln!("Listing distributions...");
        let mut distributions = Vec::from_iter(
            FOOJAY_API
                .list_distributions()
                .change_context(JpreError::Unexpected)
                .attach("Failed to list distributions")?,
        );
        distributions.sort();
        for distribution in distributions {
            println!("- {}", distribution.name);
            if !self.synonyms {
                continue;
            }
            println!("  Synonyms:");
            for synonym in distribution.synonyms {
                println!("  - {}", synonym);
            }
        }
        if !self.synonyms {
            println!();
            println!("(Use --synonyms to show synonyms)");
        }
        Ok(())
    }
}
