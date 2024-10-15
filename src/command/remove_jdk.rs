use crate::command::{Context, JpreCommand};
use crate::error::{ESResult, JpreError};
use crate::java_version::key::VersionKey;
use crate::jdk_manager::JDK_MANAGER;
use clap::Args;
use error_stack::ResultExt;

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
            .attach_printable_lazy(|| format!("Failed to get path for JDK {}", self.jdk))?;
        std::fs::remove_dir_all(&path)
            .change_context(JpreError::Unexpected)
            .attach_printable_lazy(|| format!("Failed to remove JDK at {}", path.display()))?;
        eprintln!("Removed JDK {}", self.jdk);
        Ok(())
    }
}
