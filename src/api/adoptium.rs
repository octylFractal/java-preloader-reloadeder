use serde::Deserialize;

use crate::api::def::{JdkFetchApi, JdkFetchError, JdkFetchResult};
use crate::api::http_failure::handle_response_fail;
use std::collections::HashSet;

#[derive(Clone)]
pub struct AdoptiumApi {
    pub base_url: String,
    pub vendor: String,
}

impl AdoptiumApi {
    fn get_jdk_url(&self, major: u8) -> JdkFetchResult<String> {
        let arch = get_arch_name()?;
        let os = get_os_name()?;
        Ok(format!(
            "{}/binary/latest/{}/ga/{}/{}/jdk/hotspot/normal/{}?project=jdk",
            &self.base_url, major, os, arch, &self.vendor
        ))
    }

    fn get_latest_jdk_version_url(&self, major: u8) -> JdkFetchResult<String> {
        let os_name = get_os_name()?;
        let arch_name = get_arch_name()?;

        // grabs a 10 item page containing the most recent version of [major, major+1)
        // EAs CAN get mixed in here, unfortunately
        return Ok(format!(
            "{}/info/release_versions\
            ?architecture={}\
            &OS={}\
            &page=0\
            &page_size=10\
            &release_type=ga\
            &sort_method=DEFAULT\
            &sort_order=DESC\
            &vendor={}\
            &version=%5B{}%2C{}%29",
            &self.base_url,
            arch_name,
            os_name,
            &self.vendor,
            major,
            major + 1,
        ));
    }

    fn get_jdk_version_list_url(&self, page: u8) -> JdkFetchResult<String> {
        let os_name = get_os_name()?;
        let arch_name = get_arch_name()?;

        return Ok(format!(
            "{}/info/release_names\
            ?architecture={}\
            &OS={}\
            &image_type=jdk\
            &project=jdk\
            &release_type=ga\
            &sort_method=DEFAULT\
            &sort_order=DESC\
            &vendor={}\
            &page={}\
            &page_size=20",
            &self.base_url, arch_name, os_name, &self.vendor, page,
        ));
    }
}

impl JdkFetchApi for AdoptiumApi {
    fn get_latest_jdk_binary(&self, major: u8) -> JdkFetchResult<attohttpc::Response> {
        attohttpc::get(&self.get_jdk_url(major)?)
            .send()
            .map_err(JdkFetchError::HttpIo)
    }

    fn get_latest_jdk_version(&self, major: u8) -> JdkFetchResult<Option<String>> {
        let url = self.get_latest_jdk_version_url(major)?;
        let response = attohttpc::get(&url).send().map_err(JdkFetchError::HttpIo)?;
        if !response.is_success() {
            if response.status() == attohttpc::StatusCode::NOT_FOUND {
                return Ok(None);
            }
            return Err(handle_response_fail(
                response,
                "Failed to get latest JDK version",
            ));
        }
        let page: JdkVersionsPage = response.json().map_err(JdkFetchError::HttpIo)?;
        let ga_only_versions = page
            .versions
            .iter()
            .filter(|v| v.pre.is_none())
            .collect::<Vec<_>>();
        if ga_only_versions.is_empty() {
            return Err(JdkFetchError::Generic {
                message: String::from("No versions returned from Adopt API"),
            });
        }
        let base_version = &ga_only_versions[0].openjdk_version;
        let fixed_version = match base_version.find('-').or_else(|| base_version.find('+')) {
            Some(index) => (&base_version[..index]).to_string(),
            None => base_version.to_string(),
        };
        Ok(Some(fixed_version))
    }

    fn get_available_jdk_versions(&self) -> JdkFetchResult<HashSet<String>> {
        let mut versions = HashSet::<String>::new();

        let mut i = 0;
        loop {
            let url = self.get_jdk_version_list_url(i)?;
            i += 1;

            let response = attohttpc::get(&url).send().map_err(JdkFetchError::HttpIo)?;

            if response.status() == attohttpc::StatusCode::NOT_FOUND {
                break;
            }
            if !response.is_success() {
                return Err(handle_response_fail(
                    response,
                    "Failed to get available JDK versions",
                ));
            }

            let page: JdkNamesPage = response.json().map_err(JdkFetchError::HttpIo)?;
            for name in page.releases {
                let mut version_part = name.as_str();

                let start_index = version_part.find(|c: char| c.is_digit(10));
                let start_index = unwrap_or_err(name.as_str(), start_index)?;
                version_part = &version_part[start_index..];

                let end_index = version_part.find(|c: char| !c.is_digit(10));
                let end_index = unwrap_or_err(name.as_str(), end_index)?;
                version_part = &version_part[..end_index];

                versions.insert(version_part.to_string());
            }
        }

        return Ok(versions);

        fn unwrap_or_err(current_version: &str, i: Option<usize>) -> JdkFetchResult<usize> {
            i.ok_or(JdkFetchError::Generic {
                message: format!("Failed to find major JDK version from: {}", current_version),
            })
        }
    }
}

#[derive(Debug, Deserialize)]
struct JdkVersionsPage {
    versions: Vec<JdkVersion>,
}

#[derive(Debug, Deserialize)]
struct JdkVersion {
    openjdk_version: String,
    #[serde(default)]
    pre: Option<Prerelease>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Prerelease {
    EA,
}

#[derive(Debug, Deserialize)]
struct JdkNamesPage {
    releases: Vec<String>,
}

fn get_os_name() -> JdkFetchResult<&'static str> {
    let env_os = std::env::consts::OS;
    match env_os {
        "linux" => Ok(env_os),
        "macos" => Ok("mac"),
        _ => Err(JdkFetchError::Incompatible {
            message: format!("Unsupported OS: {}", env_os),
        }),
    }
}

fn get_arch_name() -> JdkFetchResult<&'static str> {
    let env_arch = std::env::consts::ARCH;
    match env_arch {
        "x86" | "aarch64" => Ok(env_arch),
        "x86_64" => Ok("x64"),
        _ => Err(JdkFetchError::Incompatible {
            message: format!("Unsupported architecture: {}", env_arch),
        }),
    }
}
