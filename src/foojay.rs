use crate::config::JpreConfig;
use crate::error::ESResult;
use crate::java_version::key::VersionKey;
use crate::java_version::{JavaVersion, PreRelease};
use derive_more::Display;
use error_stack::{Context, Report, ResultExt};
use serde::Deserialize;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::LazyLock;
use url::Url;
use crate::http_client::new_http_client;

const FOOJAY_BASE_URL: &str = "https://api.foojay.io/disco/v3.0";

#[derive(Debug, Display)]
pub enum FoojayDiscoApiError {
    #[display("Foojay Disco API error")]
    Api,
    #[display("Invalid distribution")]
    InvalidDistribution,
}

impl Context for FoojayDiscoApiError {}

pub static FOOJAY_API: LazyLock<FoojayDiscoApi> = LazyLock::new(FoojayDiscoApi::new);

fn detected_foojay_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86" => "x86",
        "x86_64" => "x64",
        "aarch64" => "arm64",
        _ => panic!(
            "Unsupported architecture: {}, try setting forced_architecture in the config",
            std::env::consts::ARCH
        ),
    }
}

fn detected_foojay_os() -> &'static str {
    match std::env::consts::OS {
        "macos" => "macos",
        "linux" => {
            if cfg!(target_env = "musl") {
                "linux-musl"
            } else {
                "linux"
            }
        }
        _ => panic!(
            "Unsupported OS: {}, try setting forced_os in the config",
            std::env::consts::OS
        ),
    }
}

pub struct FoojayDiscoApi {
    client: ureq::Agent,
}

impl FoojayDiscoApi {
    pub fn new() -> Self {
        Self {
            client: new_http_client(),
        }
    }

    /// List all distributions, including synonyms.
    pub fn list_distributions(
        &self,
    ) -> ESResult<Vec<FoojayDistributionListInfo>, FoojayDiscoApiError> {
        let url = Url::parse_with_params(
            &format!("{}/distributions", FOOJAY_BASE_URL),
            &[("include_versions", "false"), ("include_synonyms", "true")],
        )
        .unwrap();
        Ok(self
            .call_foojay_api::<FoojayDistributionListInfo>(url)?
            .into_iter()
            .collect())
    }

    pub fn list_dist_version_keys(
        &self,
        distribution: &str,
    ) -> ESResult<HashSet<VersionKey>, FoojayDiscoApiError> {
        let url = Url::parse_with_params(
            &format!("{}/distributions/{}", FOOJAY_BASE_URL, distribution),
            &[("latest_per_update", "true")],
        )
        .unwrap();
        Ok(self
            .call_foojay_api_single::<FoojayDistributionInfo>(url)
            .attach_printable_lazy(|| format!("Distribution: {}", distribution))?
            .versions
            .into_iter()
            .map(|v| v.into())
            .collect())
    }

    pub fn get_latest_package_info(
        &self,
        config: &JpreConfig,
        jdk: &VersionKey,
    ) -> ESResult<(FoojayPackageListInfo, FoojayPackageInfo), FoojayDiscoApiError> {
        let arch = config
            .forced_architecture
            .clone()
            .unwrap_or_else(|| detected_foojay_arch().to_string());
        let os = config
            .forced_os
            .clone()
            .unwrap_or_else(|| detected_foojay_os().to_string());
        let url = Url::parse_with_params(
            &format!("{}/packages", FOOJAY_BASE_URL),
            &[
                // We don't want to handle JREs yet.
                ("package_type", "jdk".to_string()),
                // JavaFX can be nice to have bundled.
                ("with_javafx_if_available", "true".to_string()),
                // We need to be able to download it.
                ("directly_downloadable", "true".to_string()),
                ("jdk_version", jdk.major.to_string()),
                (
                    "release_status",
                    match &jdk.pre_release {
                        PreRelease::None => "ga".to_string(),
                        PreRelease::Numeric(v) => v.to_string(),
                        PreRelease::Other(v) => v.clone(),
                    },
                ),
                ("distribution", config.distribution.clone()),
                ("operating_system", os),
                ("architecture", arch),
            ],
        )
        .unwrap();
        let list_info = self.call_foojay_api::<FoojayPackageListInfo>(url)?
            .into_iter()
            .find(|p| p.latest_build_available)
            .ok_or_else(|| {
                Report::new(FoojayDiscoApiError::Api).attach_printable("No latest build available")
            })?;
        let info = self.call_foojay_api_single(list_info.links.pkg_info_uri.clone())?;
        Ok((list_info, info))
    }

    fn call_foojay_api<T: for<'a> Deserialize<'a>>(
        &self,
        url: Url,
    ) -> ESResult<Vec<T>, FoojayDiscoApiError> {
        let response = self
            .client
            .get(url.as_str())
            .call()
            .change_context(FoojayDiscoApiError::Api)?;
        let status_code = response.status();
        let data: FoojayResult<T> = response
            .into_json()
            .change_context(FoojayDiscoApiError::Api)?;

        match status_code {
            200..=299 => Ok(data.result),
            _ => match data.message.as_str() {
                "Requested distribution not found" => {
                    Err(Report::new(FoojayDiscoApiError::InvalidDistribution))
                }
                _ => Err(Report::new(FoojayDiscoApiError::Api)
                    .attach_printable(format!("Unknown message: {}", data.message)))
                .attach_printable(format!("Status code: {}", status_code)),
            },
        }
    }

    fn call_foojay_api_single<T: for<'a> Deserialize<'a>>(
        &self,
        url: Url,
    ) -> ESResult<T, FoojayDiscoApiError> {
        let result: Vec<T> = self.call_foojay_api(url)?;
        assert_eq!(result.len(), 1, "Expected exactly one result");
        Ok(result.into_iter().next().unwrap())
    }
}

#[derive(Debug, Deserialize)]
struct FoojayResult<T> {
    message: String,
    result: Vec<T>,
}

#[derive(Debug, Deserialize)]
pub struct FoojayDistributionListInfo {
    pub name: String,
    pub synonyms: Vec<String>,
}

impl PartialEq for FoojayDistributionListInfo {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for FoojayDistributionListInfo {}

impl PartialOrd for FoojayDistributionListInfo {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FoojayDistributionListInfo {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

#[derive(Debug, Deserialize)]
struct FoojayDistributionInfo {
    versions: Vec<JavaVersion>,
}

#[derive(Debug, Deserialize)]
pub struct FoojayPackageListInfo {
    pub archive_type: ArchiveType,
    pub java_version: JavaVersion,
    pub latest_build_available: bool,
    pub links: FoojayPackageLinks,
}

#[derive(Debug, Copy, Clone, Deserialize)]
pub enum ArchiveType {
    #[serde(rename = "tar.gz")]
    TarGz,
    #[serde(rename = "zip")]
    Zip,
}

#[derive(Debug, Deserialize)]
pub struct FoojayPackageLinks {
    pub pkg_info_uri: Url,
}

#[derive(Debug, Deserialize)]
pub struct FoojayPackageInfo {
    pub direct_download_uri: Url,
    pub checksum: String,
    pub checksum_type: ChecksumType,
}

#[derive(Debug, Copy, Clone, Deserialize)]
pub enum ChecksumType {
    #[serde(rename = "sha256")]
    Sha256,
}
