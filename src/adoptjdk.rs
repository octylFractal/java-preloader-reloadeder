use serde::Deserialize;

use crate::http_failure::handle_response_fail;
use anyhow::{anyhow, Context, Result};

const BASE_URL: &str = "https://api.adoptopenjdk.net/v3";

fn get_jdk_url(major: u8) -> Result<String> {
    let arch = {
        let env_arch = std::env::consts::ARCH;
        match env_arch {
            "x86" => Ok(env_arch),
            "x86_64" => Ok("x64"),
            _ => Err(anyhow!("Unknown ARCH {}", env_arch)),
        }
    }?;
    let os = {
        let env_os = std::env::consts::OS;
        match env_os {
            "linux" => Ok(env_os),
            "macos" => Ok("mac"),
            _ => Err(anyhow!("Unknown OS {}", env_os)),
        }
    }?;
    return Ok(format!(
        "{}/binary/latest/{}/ga/{}/{}/jdk/hotspot/normal/adoptopenjdk?project=jdk",
        BASE_URL,
        major,
        std::env::consts::OS,
        arch,
    ));
}

pub fn get_latest_jdk_binary(major: u8) -> Result<attohttpc::Response> {
    attohttpc::get(&get_jdk_url(major)?)
        .send()
        .context("Failed to get latest JDK binary")
}

fn get_latest_jdk_version_url(major: u8) -> String {
    // grabs a 1 item page containing the most recent version of [major, major+1)
    return format!(
        "{}/info/release_versions\
        ?page=0\
        &page_size=1\
        &release_type=ga\
        &sort_method=DEFAULT\
        &sort_order=DESC\
        &vendor=adoptopenjdk\
        &version=%5B{}%2C{}%29",
        BASE_URL,
        major,
        major + 1,
    );
}

#[derive(Deserialize)]
struct JdkVersionsPage {
    versions: Vec<JdkVersion>,
}

#[derive(Deserialize)]
struct JdkVersion {
    openjdk_version: String,
}

pub fn get_latest_jdk_version(major: u8) -> Result<String> {
    let response = attohttpc::get(&get_latest_jdk_version_url(major))
        .send()
        .context("Failed to get latest JDK version")?;
    if !response.is_success() {
        return Err(handle_response_fail(
            response,
            "Failed to get latest JDK version",
        ));
    }
    let mut page: JdkVersionsPage = response
        .json()
        .context("Failed to get JSON from Adopt API")?;
    let base_version = page.versions.remove(0).openjdk_version;
    let fixed_version = match base_version.find('-').or_else(|| base_version.find('+')) {
        Some(index) => (&base_version[..index]).to_string(),
        None => base_version,
    };
    return Ok(fixed_version.to_string());
}
