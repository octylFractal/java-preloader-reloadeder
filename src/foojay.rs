use crate::config::JpreConfig;
use crate::error::ESResult;
use crate::http_client::new_http_client;
use crate::java_version::key::VersionKey;
use crate::java_version::{JavaVersion, PreRelease};
use derive_more::Display;
use error_stack::{Report, ResultExt};
use serde::Deserialize;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::error::Error;
use std::sync::LazyLock;
use tracing::debug;
use url::Url;

const FOOJAY_BASE_URL: &str = "https://api.foojay.io/disco/v3.0";

#[derive(Debug, Display)]
pub enum FoojayDiscoApiError {
    #[display("Foojay Disco API error")]
    Api,
    #[display("Invalid distribution")]
    InvalidDistribution,
}

impl Error for FoojayDiscoApiError {}

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

fn detected_foojay_os(libc: &str) -> &'static str {
    match std::env::consts::OS {
        "macos" => "macos",
        "linux" => {
            if libc == "musl" {
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
            .attach_with(|| format!("Distribution: {}", distribution))?
            .versions
            .into_iter()
            .map(|v| v.into())
            .collect())
    }

    pub fn get_latest_package_info_using_priority(
        &self,
        config: &JpreConfig,
        jdk: &VersionKey,
    ) -> ESResult<(FoojayPackageListInfo, FoojayPackageInfo), [FoojayDiscoApiError]> {
        let mut iter = config
            .distributions
            .iter()
            .map(|dist| self.get_latest_package_info(config, dist, jdk));
        let first = iter.next().expect("always at least one distribution");
        if let Ok((list_info, info)) = first {
            return Ok((list_info, info));
        }
        let mut errors = vec![first.unwrap_err()];
        for result in iter {
            match result {
                Ok((list_info, info)) => return Ok((list_info, info)),
                Err(e) => errors.push(e),
            }
        }
        let mut report = Report::new(FoojayDiscoApiError::Api)
            .expand()
            .attach("Failed to get latest package info");
        for error in errors {
            report.push(error);
        }
        Err(report)
    }

    pub fn get_latest_package_info(
        &self,
        config: &JpreConfig,
        distribution: &str,
        jdk: &VersionKey,
    ) -> ESResult<(FoojayPackageListInfo, FoojayPackageInfo), FoojayDiscoApiError> {
        let arch = config
            .forced_architecture
            .clone()
            .unwrap_or_else(|| detected_foojay_arch().to_string());
        let libc = config.forced_libc.clone();
        let os = config
            .forced_os
            .clone()
            .unwrap_or_else(|| detected_foojay_os(&libc).to_string());
        let url = {
            let mut params = vec![
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
                ("distribution", distribution.to_string()),
                ("operating_system", os.clone()),
                ("architecture", arch),
            ];
            if os == "linux" || os == "linux-musl" {
                params.push(("libc_type", libc.clone()));
            }

            Url::parse_with_params(&format!("{}/packages", FOOJAY_BASE_URL), &params).unwrap()
        };
        self.call_foojay_api::<FoojayPackageListInfo>(url)?
            .into_iter()
            .find_map(|p| -> Option<ESResult<_, FoojayDiscoApiError>> {
                if !p.latest_build_available {
                    return None;
                }
                if let ArchiveType::Unknown(archive_type) = &p.archive_type {
                    debug!("Unknown archive type: {}", archive_type);
                    return None;
                }
                self.call_foojay_api_single(p.links.pkg_info_uri.clone())
                    .map(|mut info: FoojayPackageInfo| {
                        if matches!(info.checksum_type, ChecksumType::Unknown(ref ct) if ct.is_empty()) {
                            try_fill_checksum(&mut info);
                        }
                        if let ChecksumType::Unknown(checksum_type) = &info.checksum_type {
                            debug!("Unknown checksum type: {}", checksum_type);
                            None
                        } else {
                            Some((p, info))
                        }
                    })
                    .transpose()
            })
            .ok_or_else(|| {
                Report::new(FoojayDiscoApiError::Api).attach(format!(
                    "No latest package available for JDK {} in distribution {}",
                    jdk, distribution
                ))
            })?
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
            .into_body()
            .read_json()
            .change_context(FoojayDiscoApiError::Api)?;

        if status_code.is_success() {
            Ok(data.result)
        } else {
            match data.message.as_str() {
                "Requested distribution not found" => {
                    Err(Report::new(FoojayDiscoApiError::InvalidDistribution))
                }
                _ => Err(Report::new(FoojayDiscoApiError::Api)
                    .attach(format!("Unknown message: {}", data.message))
                    .attach(format!("Status code: {}", status_code))),
            }
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

/// Attempt to fill in the missing checksum data using known checksum URL patterns.
fn try_fill_checksum(info: &mut FoojayPackageInfo) {
    for suffix in &["sha256", "sha256.text"] {
        let url = format!("{}.{}", info.direct_download_uri, suffix);
        let Ok(response) = ureq::get(&url).call() else {
            continue;
        };
        if !response.status().is_success() {
            continue;
        }
        let Ok(checksum) = response.into_body().read_to_string() else {
            continue;
        };
        let checksum = checksum.trim();
        if checksum.len() == 64 {
            info.checksum = checksum.to_string();
            info.checksum_type = ChecksumType::Sha256;
            return;
        }
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

#[derive(Debug, Clone, Deserialize)]
pub enum ArchiveType {
    #[serde(rename = "tar.gz")]
    TarGz,
    #[serde(rename = "zip")]
    Zip,
    #[serde(untagged)]
    Unknown(String),
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

#[derive(Debug, Clone, Deserialize)]
pub enum ChecksumType {
    #[serde(rename = "sha256")]
    Sha256,
    #[serde(untagged)]
    Unknown(String),
}
