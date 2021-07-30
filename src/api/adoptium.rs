use serde::Deserialize;

use crate::api::def::{JdkFetchApi, JdkFetchError, JdkFetchResult};
use crate::api::http_failure::handle_response_fail;

#[derive(Clone)]
pub struct AdoptiumApi {
    pub base_url: String,
    pub vendor: String,
}

impl AdoptiumApi {
    fn get_jdk_url(&self, major: u8) -> JdkFetchResult<String> {
        let arch = {
            let env_arch = std::env::consts::ARCH;
            match env_arch {
                "x86" => Ok(env_arch),
                "x86_64" => Ok("x64"),
                _ => Err(JdkFetchError::Generic {
                    message: format!("Unknown ARCH {}", env_arch),
                }),
            }
        }?;
        let os = {
            let env_os = std::env::consts::OS;
            match env_os {
                "linux" => Ok(env_os),
                "macos" => Ok("mac"),
                _ => Err(JdkFetchError::Generic {
                    message: format!("Unknown OS {}", env_os),
                }),
            }
        }?;
        return Ok(format!(
            "{}/binary/latest/{}/ga/{}/{}/jdk/hotspot/normal/{}?project=jdk",
            &self.base_url, major, os, arch, &self.vendor
        ));
    }

    fn get_latest_jdk_version_url(&self, major: u8) -> String {
        // grabs a 10 item page containing the most recent version of [major, major+1)
        // EAs CAN get mixed in here, unfortunately
        return format!(
            "{}/info/release_versions\
            ?page=0\
            &page_size=10\
            &release_type=ga\
            &sort_method=DEFAULT\
            &sort_order=DESC\
            &vendor={}\
            &version=%5B{}%2C{}%29",
            &self.base_url,
            &self.vendor,
            major,
            major + 1,
        );
    }
}

impl JdkFetchApi for AdoptiumApi {
    fn get_latest_jdk_binary(&self, major: u8) -> JdkFetchResult<attohttpc::Response> {
        attohttpc::get(&self.get_jdk_url(major)?)
            .send()
            .map_err(JdkFetchError::HttpIo)
    }

    fn get_latest_jdk_version(&self, major: u8) -> JdkFetchResult<Option<String>> {
        let response = attohttpc::get(&self.get_latest_jdk_version_url(major))
            .send()
            .map_err(JdkFetchError::HttpIo)?;
        if !response.is_success() {
            if response.status().as_u16() == 404 {
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
        return Ok(Some(fixed_version));
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
