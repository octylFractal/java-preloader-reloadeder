use crate::java_version::PreRelease;
use crate::string::SplittingExt;
use derive_more::Display;
use serde::{Deserialize, Deserializer, Serialize};
use std::num::ParseIntError;
use std::str::FromStr;
use thiserror::Error;

/// The key we use as what a user can install. Usually, this is the major version number of the JVM,
/// but it can also include other information such as if it is Early Access or General Availability.
#[derive(Debug, Clone, Display, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[display("{major}{}", match pre_release {
    PreRelease::None => String::new(),
    PreRelease::Other(s) => format!("-{}", s),
    PreRelease::Numeric(n) => format!("-{}", n),
})]
pub struct VersionKey {
    pub major: u32,
    pub pre_release: PreRelease,
}

#[derive(Debug, Error)]
pub enum VersionKeyParseError {
    #[error("Failed to parse major version number: {input}")]
    MajorNotNumeric {
        input: String,
        source: ParseIntError,
    },
}

impl FromStr for VersionKey {
    type Err = VersionKeyParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (major, pre) = s.split_optional('-');
        Ok(Self {
            major: major
                .parse()
                .map_err(|e| VersionKeyParseError::MajorNotNumeric {
                    input: s.to_string(),
                    source: e,
                })?,
            pre_release: pre
                .map(PreRelease::from_str)
                .transpose()
                .unwrap()
                .unwrap_or(PreRelease::None),
        })
    }
}

impl Serialize for VersionKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for VersionKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        VersionKey::from_str(&s).map_err(serde::de::Error::custom)
    }
}
