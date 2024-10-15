use crate::command::Context;
use crate::context_id::get_context_path;
use crate::error::{ESResult, JpreError};
use crate::java_version::key::VersionKey;
use crate::jdk_manager::JDK_MANAGER;
use error_stack::ResultExt;
use tracing::debug;

pub fn clear_context_path() -> ESResult<(), JpreError> {
    let path = get_context_path();
    debug!("Removing Java home path file '{:?}'", path);
    match std::fs::remove_file(&path) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e)
            .change_context(JpreError::Unexpected)
            .attach_printable_lazy(|| format!("Failed to remove Java home path file '{:?}'", path)),
    }
}

pub fn set_context_path_to_java_home(
    context: &Context,
    jdk: &VersionKey,
) -> ESResult<(), JpreError> {
    debug!("Setting Java home path to JDK '{}'", jdk);
    let jdk = JDK_MANAGER
        .get_jdk_path(&context.config, jdk)
        .change_context(JpreError::Unexpected)
        .attach_printable_lazy(|| format!("Failed to get path for JDK {}", jdk))?;
    let path = get_context_path();
    let parent = path.parent().unwrap();
    debug!("Creating directories to '{}'", parent.display());
    std::fs::create_dir_all(parent)
        .change_context(JpreError::Unexpected)
        .attach_printable_lazy(|| {
            format!("Failed to create directories to {}", parent.display())
        })?;
    clear_context_path()?;
    debug!(
        "Creating symlink from '{}' to '{}'",
        jdk.display(),
        path.display()
    );
    std::os::unix::fs::symlink(&jdk, &path)
        .change_context(JpreError::Unexpected)
        .attach_printable_lazy(|| {
            format!(
                "Failed to create symlink from {} to {}",
                jdk.display(),
                path.display()
            )
        })?;

    Ok(())
}
