use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use directories_next::ProjectDirs;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

/// Environment variable name for the `JAVA_HOME` that the current environment's jpre is using.
pub const JPRE_JAVA_HOME: &str = "JPRE_JAVA_HOME";

pub static PROJECT_DIRS: Lazy<ProjectDirs> =
    Lazy::new(|| ProjectDirs::from("net", "octyl", "jpre").expect("No project dirs derived?"));

static CONFIG_FILE: Lazy<PathBuf> = Lazy::new(|| PROJECT_DIRS.config_dir().join("config.toml"));

#[derive(Debug, Serialize, Deserialize)]
pub struct Configuration {
    default_jdk: Option<u8>,
}

impl Configuration {
    pub fn new() -> Result<Configuration> {
        let ser = std::fs::read_to_string(&*CONFIG_FILE)
            .or_else(|err| match err.kind() {
                // map missing file to empty string
                std::io::ErrorKind::NotFound => Ok("".to_string()),
                // otherwise still fail
                _ => Err(err),
            })
            .context("failed to read from config file")?;
        toml::from_str(ser.as_str()).context("failed to de-serialize as TOML")
    }

    pub fn resolve_default(&self) -> Result<u8> {
        self.default_jdk
            .ok_or_else(|| anyhow!("No default JDK specified"))
    }

    pub fn set_default(&mut self, jdk: u8) {
        self.default_jdk = Some(jdk);
    }

    pub fn save(&self) -> Result<()> {
        let ser = toml::to_string(self).expect("Failed to serialize as TOML");
        std::fs::create_dir_all(CONFIG_FILE.parent().expect("no parent directory?"))
            .context("Failed to create config file directories")?;
        std::fs::write(&*CONFIG_FILE, ser).context("Failed to write to config file")
    }
}
