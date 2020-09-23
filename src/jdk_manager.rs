use std::env::{var, var_os};
use std::error::Error;
use std::fs::{create_dir_all, File, OpenOptions};
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};

use console::Term;
use reqwest::blocking::Response;
use tempdir::TempDir;
use zip::read::read_zipfile_from_stream;

use crate::content_disposition_parser::parse_filename;

lazy_static! {
    static ref BASE_PATH: PathBuf = match var_os("XDG_CONFIG_HOME") {
        Some(val) => PathBuf::from(val),
        None => [
            var("HOME").expect("No HOME env var defined"),
            ".config/jpre".to_string()
        ].iter().collect()
    }.join("jdks");
}

const FINISHED_MARKER: &str = ".jdk_marker";

fn get_jdk_url(major: u8) -> String {
    return format!(
        "https://api.adoptopenjdk.net/v3/binary/latest/{}/ga/{}/{}/jdk/hotspot/normal/adoptopenjdk?project=jdk",
        major,
        std::env::consts::OS,
        match std::env::consts::ARCH {
            "x86" => "x86",
            "x86_64" => "x64",
            unknown => panic!("Unknown ARCH {}", unknown)
        },
    );
}

pub fn get_jdk_path(major: u8) -> Result<PathBuf, Box<dyn Error>> {
    let path = BASE_PATH.join(major.to_string());
    if path.join(FINISHED_MARKER).exists() {
        return Ok(path);
    }

    let response = reqwest::blocking::ClientBuilder::default()
        .connection_verbose(true)
        .build()?
        .get(&get_jdk_url(major))
        .send()?;
    if !response.status().is_success() {
        let status = response.status();
        let error = match response.text() {
            Ok(text) => text,
            Err(err) => err.to_string()
        };
        return Err(Box::from(format!("Failed to get JDK: {} ({})", status, error)));
    }

    let url = response.headers()
        .get(reqwest::header::CONTENT_DISPOSITION)
        .ok_or(Box::from("no content disposition"))
        .and_then(|value| {
            parse_filename(value.to_str()?)
        })
        .unwrap_or("<no filename>".to_string());
    eprintln!("Extracting {}", url);
    if path.exists() {
        std::fs::remove_dir_all(&path).map_err(|e|
            format!("Unable to clean JDK folder ({}): {}", path.display(), e)
        )?;
    }
    create_dir_all(&path).map_err(|e|
        format!("Unable to create directories to JDK folder ({}): {}", path.display(), e)
    )?;
    let temporary_dir = TempDir::new_in(&*BASE_PATH, "jdk-download")?;
    finish_extract(&path, response, url, &temporary_dir)
        .and_then(|_| temporary_dir.close().map_err(|e| Box::from(e)))?;
    return Ok(path);
}

fn finish_extract(path: &PathBuf, response: Response, url: String, temporary_dir: &TempDir) -> Result<(), Box<dyn Error>> {
    if url.ends_with(".tar.gz") {
        unarchive_tar_gz(temporary_dir.path(), response)
    } else if url.ends_with(".zip") {
        unarchive_zip(temporary_dir.path(), response)
    } else {
        return Err(Box::from(format!("Don't know how to handle {}", url)));
    }
    eprintln!();
    let dir_entries = temporary_dir.path().read_dir()?
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>, io::Error>>()?;
    let from_dir = if dir_entries.len() == 1 {
        &dir_entries[0]
    } else {
        temporary_dir.path()
    };

    std::fs::rename(from_dir, &path).map_err(|e|
        format!("Unable to move to JDK folder ({}): {}", path.display(), e)
    )?;

    File::create(path.join(FINISHED_MARKER))?;
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
        term.write(format!("Extracting {}", file.path().unwrap().display()).as_bytes()).unwrap();
        file.unpack_in(path).unwrap();
    }
}

fn unarchive_zip(path: &Path, mut response: Response) {
    let mut term = Term::stderr();
    loop {
        let mut zip_file = match read_zipfile_from_stream(&mut response) {
            Ok(Some(entry)) => entry,
            Ok(None) => break,
            Err(err) => panic!("Error reading zip: {}", err)
        };
        let name = zip_file.name();
        if name.starts_with("/") || name.contains("..") {
            panic!("Illegal zip file name: {}", name);
        }
        term.clear_line().unwrap();
        term.write(format!("Extracting {}", name).as_bytes()).unwrap();
        if zip_file.is_dir() {
            create_dir_all(zip_file.name()).unwrap();
            continue;
        }
        if !zip_file.is_file() {
            continue;
        }
        let mut options = OpenOptions::new();
        options
            .create(true)
            .write(true);
        #[cfg(unix)] {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(zip_file.unix_mode().unwrap_or(0o666));
        }
        let mut file = options.open(path.join(zip_file.name()))
            .expect("Failed to open file");
        io::copy(&mut zip_file, &mut file).expect("Unable to copy to file");
    }
}