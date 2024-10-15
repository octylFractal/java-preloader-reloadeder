use crate::config::JpreConfig;
use crate::error::{ESResult, JpreError};
use enum_dispatch::enum_dispatch;

pub(super) mod current;
pub(super) mod debug;
pub(super) mod get_context_id;
pub(super) mod java_home;
pub(super) mod list_distributions;
pub(super) mod list_installed;
pub(super) mod list_versions;
pub(super) mod remove_jdk;
pub(super) mod set_default;
pub(super) mod set_distribution;
pub(super) mod update;
pub(super) mod use_jdk;

#[enum_dispatch]
pub trait JpreCommand {
    fn run(self, context: Context) -> ESResult<(), JpreError>;
}

pub struct Context {
    pub config: JpreConfig,
}
