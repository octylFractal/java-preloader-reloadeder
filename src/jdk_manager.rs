use std::fs::{create_dir_all, File, OpenOptions};
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result, Context};
use config::Source;
use console::Term;
use log::debug;
use once_cell::sync::Lazy;
use reqwest::blocking::Response;
use tempdir::TempDir;
use zip::read::read_zipfile_from_stream;

use crate::adoptjdk;
use crate::content_disposition_parser::parse_filename;
use crate::reqwest_failure::handle_response_fail;

static BASE_PATH: Lazy<PathBuf> = Lazy::new(|| crate::config::APP_HOME.join("jdks"));

const FINISHED_MARKER: &str = ".jdk_marker";

pub fn get_jdk_version(major: u8) -> Option<String> {
    let path = BASE_PATH.join(major.to_string());
    if !path.join(FINISHED_MARKER).exists() {
        debug!("No finished marker exists in JDK {}", major);
        return None;
    }
    let release = path.join("release");
    if !path.join("release").exists() {
        debug!("No release file exists in JDK {}", major);
        return None;
    }
    let config = config::File::from(release)
        .format(config::FileFormat::Toml)
        .collect()
        .ok()?;
    return config
        .get("JAVA_VERSION")
        .and_then(|v| v.clone().into_str().ok());
}

pub fn get_all_jdk_majors() -> Result<Vec<u8>> {
    return BASE_PATH
        .read_dir().context("Failed to read base directory")?
        .map(|res| {
            res
                .map(|e| {
                    e.path()
                        .file_name()
                        // This should be impossible
                        .expect("cannot be missing file name")
                        .to_str()
                        // I don't really know if I should handle non-UTF-8
                        .expect("Non-UTF8 filename encountered")
                        .to_string()
                })
                .context("Failed to read directory entry")
        })
        .filter_map(|res| {
            match res {
                // map the parse error to None, otherwise get Some(Ok(u8))
                Ok(file_name) => file_name.parse::<u8>().ok().map(Ok),
                // map the actual errors back in
                Err(err) => Some(Err(err)),
            }
        })
        .collect();
}

pub fn map_available_jdk_versions(majors: &Vec<u8>) -> Vec<(u8, String)> {
    let mut vec: Vec<(u8, String)> = majors
        .iter()
        .filter_map(|jdk_major| get_jdk_version(*jdk_major).map(|version| (*jdk_major, version)))
        .collect();
    vec.sort_by_key(|v| v.0);
    return vec;
}

pub fn get_jdk_path(major: u8) -> Result<PathBuf> {
    let path = BASE_PATH.join(major.to_string());
    if path.join(FINISHED_MARKER).exists() {
        return Ok(path);
    }

    update_jdk(major)?;
    return Ok(path);
}

pub fn update_jdk(major: u8) -> Result<()> {
    let path = BASE_PATH.join(major.to_string());
    let response = adoptjdk::get_latest_jdk_binary(major)?;
    if !response.status().is_success() {
        return Err(handle_response_fail(response, "Failed to get JDK binary"));
    }

    let url = response
        .headers()
        .get(reqwest::header::CONTENT_DISPOSITION)
        .ok_or_else(|| anyhow!("no content disposition"))
        .and_then(|value| parse_filename(value.to_str()?))
        .unwrap_or("<no filename>".to_string());
    eprintln!("Extracting {}", url);
    if path.exists() {
        std::fs::remove_dir_all(&path)
            .with_context(|| format!("Unable to clean JDK folder ({})", path.display()))?;
    }
    create_dir_all(&path)
        .with_context(||
            format!("Unable to create directories to JDK folder ({})", path.display())
        )?;
    let temporary_dir = TempDir::new_in(&*BASE_PATH, "jdk-download")
        .context("Failed to create temporary directory")?;
    finish_extract(&path, response, url, &temporary_dir)
        .and_then(|_| temporary_dir.close().context("Failed to cleanup temp dir"))?;
    return Ok(());
}

fn finish_extract(
    path: &PathBuf,
    response: Response,
    url: String,
    temporary_dir: &TempDir,
) -> Result<()> {
    if url.ends_with(".tar.gz") {
        unarchive_tar_gz(temporary_dir.path(), response)
    } else if url.ends_with(".zip") {
        unarchive_zip(temporary_dir.path(), response)
    } else {
        return Err(anyhow!("Don't know how to handle {}", url));
    }
    eprintln!();
    let dir_entries = temporary_dir
        .path()
        .read_dir().context("Failed to read temp dir")?
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>, io::Error>>()
        .context("Failed to read temp dir entry")?;
    let from_dir = if dir_entries.len() == 1 {
        &dir_entries[0]
    } else {
        temporary_dir.path()
    };

    std::fs::rename(from_dir, &path)
        .with_context(|| format!("Unable to move to JDK folder ({})", path.display()))?;

    File::create(path.join(FINISHED_MARKER)).context("Unable to create marker")?;
    Ok(())
}

fn unarchive_tar_gz(path: &Path, mut response: Response) {
    let mut term = Term::stderr();
    let gz_decode = libflate::gzip::Decoder::new(&mut response).unwrap();
    let mut archive = tar::Archive::new(gz_decode);
    archive.set_preserve_permissions(true);
    archive.set_overwrite(true);
    for entry in archive.entries().unwrap() {
        let mut file = entry.unwrap();
        term.clear_line().unwrap();
        term.write(format!("Extracting {}", file.path().unwrap().display()).as_bytes())
            .unwrap();
        file.unpack_in(path).unwrap();
    }
}

fn unarchive_zip(path: &Path, mut response: Response) {
    let mut term = Term::stderr();
    loop {
        let mut zip_file = match read_zipfile_from_stream(&mut response) {
            Ok(Some(entry)) => entry,
            Ok(None) => break,
            Err(err) => panic!("Error reading zip: {}", err),
        };
        let name = zip_file.name();
        if name.starts_with("/") || name.contains("..") {
            panic!("Illegal zip file name: {}", name);
        }
        term.clear_line().unwrap();
        term.write(format!("Extracting {}", name).as_bytes())
            .unwrap();
        if zip_file.is_dir() {
            create_dir_all(zip_file.name()).unwrap();
            continue;
        }
        if !zip_file.is_file() {
            continue;
        }
        let mut options = OpenOptions::new();
        options.create(true).write(true);
        #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(zip_file.unix_mode().unwrap_or(0o666));
            }
        let mut file = options
            .open(path.join(zip_file.name()))
            .expect("Failed to open file");
        io::copy(&mut zip_file, &mut file).expect("Unable to copy to file");
    }
}
