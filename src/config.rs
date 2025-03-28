use crate::error::{ESResult, JpreError, UserMessage};
use crate::java_version::key::VersionKey;
use crate::java_version::PreRelease;
use directories::ProjectDirs;
use error_stack::ResultExt;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::LazyLock;
use toml::de::Error;

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
    /// The legacy distribution option.
    #[serde(default)]
    distribution: Option<String>,
    /// The distribution(s) to use when downloading a JDK. Must be a valid Foojay distribution.
    #[serde(default = "default_distribution")]
    pub distributions: Vec<String>,
    /// Architecture to force when downloading a JDK. If not set, the system's architecture will be
    /// used if it can be mapped.
    #[serde(default)]
    pub forced_architecture: Option<String>,
    /// OS to force when downloading a JDK. If not set, the system's OS will be used if it can be
    /// mapped.
    #[serde(default)]
    pub forced_os: Option<String>,
}

impl JpreConfig {
    pub(super) fn load() -> ESResult<JpreConfig, JpreError> {
        std::fs::create_dir_all(CONFIG_PATH.parent().unwrap())
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
        let (contents, config) = Self::read_config()?;
        match config {
            Ok(mut config) => {
                if let Some(distribution) = config.distribution {
                    config.distributions = vec![distribution];
                    config.distribution = None;
                }
                if config.distributions.is_empty() {
                    return Err(JpreError::UserError).attach(UserMessage {
                        message: "No distributions set in config".to_string(),
                    });
                }
                Ok(config)
            }
            Err(e) => {
                // Try to load the old config format.
                let Ok(old_config) = toml::from_str::<toml::Table>(&contents) else {
                    return Err(e)
                        .change_context(JpreError::Unexpected)
                        .attach_printable_lazy(|| {
                            format!("Could not parse config file at {:?}", *CONFIG_PATH)
                        });
                };
                if let Some(toml::Value::Integer(major)) = old_config.get("default_jdk") {
                    if old_config.keys().len() != 1 {
                        return Err(e)
                            .change_context(JpreError::Unexpected)
                            .attach_printable_lazy(|| {
                                format!("Could not parse config file at {:?}", *CONFIG_PATH)
                            });
                    }

                    let mut new_config = toml_edit::DocumentMut::new();
                    new_config["default_jdk"] = toml_edit::value(
                        VersionKey {
                            major: *major as u32,
                            pre_release: PreRelease::None,
                        }
                        .to_string(),
                    );
                    let mut distributions = toml_edit::Array::new();
                    distributions.push("temurin");
                    new_config["distributions"] = toml_edit::value(distributions);

                    // Ensure whatever is in the config is valid.
                    toml::from_str::<JpreConfig>(&new_config.to_string())
                        .expect("New config is invalid");

                    std::fs::write(&*CONFIG_PATH, new_config.to_string())
                        .change_context(JpreError::Unexpected)
                        .attach_printable_lazy(|| {
                            format!("Could not write config file at {:?}", *CONFIG_PATH)
                        })?;

                    return Self::read_config()?
                        .1
                        .change_context(JpreError::Unexpected)
                        .attach_printable_lazy(|| {
                            format!("Could not parse config file at {:?}", *CONFIG_PATH)
                        });
                }
                Err(e)
                    .change_context(JpreError::Unexpected)
                    .attach_printable_lazy(|| {
                        format!("Could not parse config file at {:?}", *CONFIG_PATH)
                    })
            }
        }
    }

    fn read_config() -> ESResult<(String, Result<JpreConfig, Error>), JpreError> {
        let contents = std::fs::read_to_string(&*CONFIG_PATH)
            .change_context(JpreError::Unexpected)
            .attach_printable_lazy(|| {
                format!("Could not read config file at {:?}", *CONFIG_PATH)
            })?;
        let config = toml::from_str::<JpreConfig>(&contents);
        Ok((contents, config))
    }

    pub fn edit_config<F: FnOnce(&mut toml_edit::DocumentMut)>(
        &mut self,
        editor: F,
    ) -> ESResult<(), JpreError> {
        let contents = Self::read_config()?.0;
        let mut config = toml_edit::DocumentMut::from_str(&contents)
            .change_context(JpreError::Unexpected)
            .attach_printable_lazy(|| {
                format!("Could not parse config file at {:?}", *CONFIG_PATH)
            })?;
        editor(&mut config);

        // Ensure whatever is in the config is valid, and update ourselves to it.
        *self = toml::from_str::<JpreConfig>(&config.to_string())
            .unwrap_or_else(|e| panic!("Edited config is invalid: {}", e));

        std::fs::write(&*CONFIG_PATH, config.to_string())
            .change_context(JpreError::Unexpected)
            .attach_printable_lazy(|| {
                format!("Could not write config file at {:?}", *CONFIG_PATH)
            })?;
        Ok(())
    }
}

fn default_distribution() -> Vec<String> {
    vec!["temurin".to_string()]
}
