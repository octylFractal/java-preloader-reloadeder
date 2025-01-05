use crate::config::PROJECT_DIRS;
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::LazyLock;

static JPRE_CONTEXT_ID: LazyLock<Option<String>> =
    LazyLock::new(|| std::env::var("JPRE_CONTEXT_ID").ok());

pub fn get_context_id() -> String {
    if let Some(context_id) = &*JPRE_CONTEXT_ID {
        return context_id.clone();
    }
    std::os::unix::process::parent_id().to_string()
}

pub fn get_context_path() -> PathBuf {
    PROJECT_DIRS
        .state_dir()
        .map(Cow::Borrowed)
        .unwrap_or_else(|| Cow::Owned(PROJECT_DIRS.cache_dir().join("state")))
        .join("java-home-by-pid")
        .join(get_context_id())
}
