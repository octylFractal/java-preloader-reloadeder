use crate::error::{ESResult, JpreError};
use crate::java_version::key::VersionKey;
use crate::java_version::PreRelease;
use directories::ProjectDirs;
use error_stack::ResultExt;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::LazyLock;

pub static PROJECT_DIRS: LazyLock<ProjectDirs> = LazyLock::new(|| {
    ProjectDirs::from("net", "octyl", "jpre").expect("Could not determine project directories")
});

static CONFIG_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| PROJECT_DIRS.preference_dir().join("config.toml"));

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JpreConfig {
    /// The default JDK to use in a new context.
    #[serde(default)]
    pub default_jdk: Option<VersionKey>,
    /// The distribution to use when downloading a JDK. Must be a valid Foojay distribution.
    #[serde(default = "default_distribution")]
    pub distribution: String,
    /// Architecture to force when downloading a JDK. If not set, the system's architecture will be
    /// used if it can be mapped.
    #[serde(default)]
    pub forced_architecture: Option<String>,
    /// OS to force when downloading a JDK. If not set, the system's OS will be used if it can be
    /// mapped.
    #[serde(default)]
    pub forced_os: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Jpre02Config {
    #[serde(default)]
    pub default_jdk: Option<u32>,
}

impl JpreConfig {
    pub(super) fn load() -> ESResult<JpreConfig, JpreError> {
        std::fs::create_dir_all(PROJECT_DIRS.config_dir())
            .change_context(JpreError::Unexpected)
            .attach_printable_lazy(|| {
                format!(
                    "Could not create config directory at {:?}",
                    PROJECT_DIRS.config_dir()
                )
            })?;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&*CONFIG_PATH)
            .change_context(JpreError::Unexpected)
            .attach_printable_lazy(|| {
                format!("Could not open config file at {:?}", *CONFIG_PATH)
            })?;
        let contents = std::fs::read_to_string(&*CONFIG_PATH)
            .change_context(JpreError::Unexpected)
            .attach_printable_lazy(|| {
                format!("Could not read config file at {:?}", *CONFIG_PATH)
            })?;
        let config = toml::from_str(&contents);
        match config {
            Ok(config) => Ok(config),
            Err(e) => {
                // Try to load the old config format.
                let Ok(old_config) = toml::from_str::<Jpre02Config>(&contents) else {
                    return Err(e)
                        .change_context(JpreError::Unexpected)
                        .attach_printable_lazy(|| {
                            format!("Could not parse config file at {:?}", *CONFIG_PATH)
                        });
                };
                let new_config = JpreConfig {
                    default_jdk: old_config.default_jdk.map(|v| VersionKey {
                        major: v,
                        pre_release: PreRelease::None,
                    }),
                    distribution: default_distribution(),
                    forced_architecture: None,
                    forced_os: None,
                };
                new_config.save()?;
                Ok(new_config)
            }
        }
    }

    pub fn save(&self) -> ESResult<(), JpreError> {
        let contents = toml::to_string(self)
            .change_context(JpreError::Unexpected)
            .attach_printable("Could not serialize config to TOML")?;
        std::fs::write(&*CONFIG_PATH, contents)
            .change_context(JpreError::Unexpected)
            .attach_printable_lazy(|| {
                format!("Could not write config file to {:?}", *CONFIG_PATH)
            })?;
        Ok(())
    }
}

fn default_distribution() -> String {
    "temurin".to_string()
}
