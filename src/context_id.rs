use crate::config::PROJECT_DIRS;
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::LazyLock;
use sysinfo::{get_current_pid, ProcessRefreshKind, RefreshKind, System};

static JPRE_CONTEXT_ID: LazyLock<Option<String>> =
    LazyLock::new(|| std::env::var("JPRE_CONTEXT_ID").ok());

static SYSTEM_PROCESSES_PID_ONLY: LazyLock<System> = LazyLock::new(|| {
    System::new_with_specifics(RefreshKind::new().with_processes(ProcessRefreshKind::new()))
});

pub fn get_context_id() -> String {
    if let Some(context_id) = &*JPRE_CONTEXT_ID {
        return context_id.clone();
    }
    let process = SYSTEM_PROCESSES_PID_ONLY
        .process(get_current_pid().unwrap())
        .expect("Could not find current process in system processes");
    process
        .parent()
        .expect("Could not find parent process")
        .as_u32()
        .to_string()
}

pub fn get_context_path() -> PathBuf {
    PROJECT_DIRS
        .state_dir()
        .map(Cow::Borrowed)
        .unwrap_or_else(|| Cow::Owned(PROJECT_DIRS.cache_dir().join("state")))
        .join("java-home-by-pid")
        .join(get_context_id())
}
