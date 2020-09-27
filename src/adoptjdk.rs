use serde::Deserialize;

use crate::reqwest_failure::handle_response_fail;
use anyhow::{anyhow, Context, Result};
use once_cell::sync::Lazy;

const BASE_URL: &str = "https://api.adoptopenjdk.net/v3";
static HTTP_CLIENT: Lazy<reqwest::blocking::Client> = Lazy::new(|| {
    reqwest::blocking::ClientBuilder::default()
        .connection_verbose(true)
        .build()
        .expect("Unable to build reqwest client")
});

fn get_jdk_url(major: u8) -> Result<String> {
    let arch = match std::env::consts::ARCH {
        "x86" => Ok("x86"),
        "x86_64" => Ok("x64"),
        unknown => Err(anyhow!("Unknown ARCH {}", unknown)),
    }?;
    return Ok(format!(
        "{}/binary/latest/{}/ga/{}/{}/jdk/hotspot/normal/adoptopenjdk?project=jdk",
        BASE_URL,
        major,
        std::env::consts::OS,
        arch,
    ));
}

pub fn get_latest_jdk_binary(major: u8) -> Result<reqwest::blocking::Response> {
    return Ok(HTTP_CLIENT
        .get(&get_jdk_url(major)?)
        .send()
        .context("Failed to get latest binary from Adopt API")?);
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
    let response = HTTP_CLIENT
        .get(&get_latest_jdk_version_url(major))
        .send()
        .context("Failed to get latest JDK version from Adopt API")?;
    if !response.status().is_success() {
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
