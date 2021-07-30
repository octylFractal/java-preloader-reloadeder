use std::collections::HashMap;
use std::ffi::CStr;
use std::fs::{create_dir_all, File};
use std::io;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use indicatif::{MultiProgress, ProgressDrawTarget};
use log::debug;
use once_cell::sync::Lazy;
use tempdir::TempDir;
use thiserror::Error;

use crate::api::def::{JdkFetchApi, JdkFetchError};
use crate::api::http_failure::handle_response_fail;
use crate::content_disposition_parser::parse_filename;
use crate::progress::new_progress_bar;

static BASE_PATH: Lazy<PathBuf> =
    Lazy::new(|| crate::config::PROJECT_DIRS.cache_dir().join("jdks"));
static BY_TTY: Lazy<PathBuf> = Lazy::new(|| std::env::temp_dir().join("jpre-by-tty"));

#[derive(Error, Debug)]
pub enum JdkManagerError {
    #[error("{message}")]
    Io {
        message: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{message}")]
    Toml {
        message: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("{message}")]
    Fetch {
        message: String,
        #[source]
        source: JdkFetchError,
    },
    #[error("unknown error: {message}")]
    Generic { message: String },
    #[error("{message}")]
    Sub {
        message: String,
        #[source]
        source: Box<JdkManagerError>,
    },
}

impl JdkManagerError {
    fn io<S: Into<String>>(message: S, error: std::io::Error) -> JdkManagerError {
        JdkManagerError::Io {
            message: message.into(),
            source: error,
        }
    }

    fn toml<S: Into<String>>(message: S, error: toml::de::Error) -> JdkManagerError {
        JdkManagerError::Toml {
            message: message.into(),
            source: error,
        }
    }

    fn fetch<S: Into<String>>(message: S, error: JdkFetchError) -> JdkManagerError {
        JdkManagerError::Fetch {
            message: message.into(),
            source: error,
        }
    }

    fn generic<S: Into<String>>(message: S) -> JdkManagerError {
        JdkManagerError::Generic {
            message: message.into(),
        }
    }

    fn sub<S: Into<String>>(message: S, error: JdkManagerError) -> JdkManagerError {
        JdkManagerError::Sub {
            message: message.into(),
            source: Box::from(error),
        }
    }
}

pub type JdkManagerResult<T> = Result<T, JdkManagerError>;

pub struct JdkManager<A: JdkFetchApi> {
    pub api: A,
}

const FINISHED_MARKER: &str = ".jdk_marker";

impl<A: JdkFetchApi> JdkManager<A> {
    pub fn get_symlink_location(&self) -> JdkManagerResult<PathBuf> {
        // Specifically check stderr, as stdout is likely to be redirected
        if !console::Term::stderr().features().is_attended() {
            return Err(JdkManagerError::generic("Not a TTY"));
        }
        let tty = unsafe { CStr::from_ptr(libc::ttyname(libc::STDERR_FILENO)) }
            .to_str()
            .expect("Filename is not UTF-8");
        let tty_as_name = tty.replace('/', "-");
        create_dir_all(&*BY_TTY)
            .map_err(|e| JdkManagerError::io("Failed to create by-tty directory", e))?;
        return Ok(BY_TTY.join(tty_as_name));
    }

    pub fn get_current_jdk(&self) -> JdkManagerResult<String> {
        let symlink = self.get_symlink_location()?;
        let actual = symlink
            .read_link()
            .map_err(|e| JdkManagerError::io("No current JDK detected", e))?;
        return actual
            .file_name()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse::<u8>().ok())
            .and_then(|m| self.get_jdk_version(m))
            .ok_or_else(|| JdkManagerError::generic("Not linked to an actual JDK"));
    }

    pub fn get_jdk_version(&self, major: u8) -> Option<String> {
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
        let config = std::fs::read_to_string(&release)
            .map_err(|e| JdkManagerError::io("Failed to read release file", e))
            .and_then(|data| {
                toml::from_str::<HashMap<String, String>>(data.as_str()).map_err(|e| {
                    JdkManagerError::toml(
                        format!(
                            "Failed to parse TOML from release file '{}'",
                            release.display()
                        ),
                        e,
                    )
                })
            });
        match config {
            Ok(map) => map.get("JAVA_VERSION").map(|v| v.clone()),
            Err(error) => {
                debug!("{:?}", error);
                None
            }
        }
    }

    pub fn get_all_jdk_majors(&self) -> JdkManagerResult<Vec<u8>> {
        let read_dir_result = BASE_PATH.read_dir();
        if let Err(read_dir_error) = &read_dir_result {
            if read_dir_error.kind() == std::io::ErrorKind::NotFound {
                // ignore if we can't find the dir
                return Ok(Vec::new());
            }
        }
        return read_dir_result
            .map_err(|e| JdkManagerError::io("Failed to read base directory", e))?
            .map(|res| {
                res.map(|e| {
                    e.path()
                        .file_name()
                        // This should be impossible
                        .expect("cannot be missing file name")
                        .to_str()
                        // I don't really know if I should handle non-UTF-8
                        .expect("Non-UTF8 filename encountered")
                        .to_string()
                })
                .map_err(|e| JdkManagerError::io("Failed to read directory entry", e))
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

    pub fn map_available_jdk_versions(&self, majors: &Vec<u8>) -> Vec<(u8, String)> {
        let mut vec: Vec<(u8, String)> = majors
            .iter()
            .filter_map(|jdk_major| {
                self.get_jdk_version(*jdk_major)
                    .map(|version| (*jdk_major, version))
            })
            .collect();
        vec.sort_by_key(|v| v.0);
        return vec;
    }

    pub fn symlink_jdk_path(&self, major: u8) -> JdkManagerResult<()> {
        let path = self
            .get_jdk_path(major)
            .map_err(|e| JdkManagerError::sub("Failed to get JDK path", e))?;
        let symlink = self
            .get_symlink_location()
            .map_err(|e| JdkManagerError::sub("Failed to get symlink location", e))?;
        if symlink.symlink_metadata().is_ok() {
            std::fs::remove_file(&symlink)
                .map_err(|e| JdkManagerError::io("Failed to remove old symlink", e))?;
        }
        std::os::unix::fs::symlink(path, symlink)
            .map_err(|e| JdkManagerError::io("Failed to make new symlink", e))?;
        Ok(())
    }

    pub fn get_jdk_path(&self, major: u8) -> JdkManagerResult<PathBuf> {
        let path = BASE_PATH.join(major.to_string());
        if path.join(FINISHED_MARKER).exists() {
            return Ok(path);
        }

        self.update_jdk(major)?;
        return Ok(path);
    }

    pub fn update_jdk(&self, major: u8) -> JdkManagerResult<()> {
        let path = BASE_PATH.join(major.to_string());
        let response = self
            .api
            .get_latest_jdk_binary(major)
            .map_err(|e| JdkManagerError::fetch("Failed to get latest JDK binary", e))?;
        if !response.is_success() {
            return Err(JdkManagerError::fetch(
                "",
                handle_response_fail(response, "Failed to get JDK binary"),
            ));
        }

        let url = response
            .headers()
            .get(attohttpc::header::CONTENT_DISPOSITION)
            .ok_or_else(|| JdkManagerError::generic("no content disposition"))
            .map(|value| parse_filename(value.to_str().unwrap()).unwrap())?;
        eprintln!("Extracting {}", url);
        if path.exists() {
            std::fs::remove_dir_all(&path).map_err(|e| {
                JdkManagerError::io(
                    format!("Unable to clean JDK folder ({})", path.display()),
                    e,
                )
            })?;
        }
        create_dir_all(&path).map_err(|e| {
            JdkManagerError::io(
                format!(
                    "Unable to create directories to JDK folder ({})",
                    path.display()
                ),
                e,
            )
        })?;
        let temporary_dir = TempDir::new_in(&*BASE_PATH, "jdk-download")
            .map_err(|e| JdkManagerError::io("Failed to create temporary directory", e))?;
        self.finish_extract(&path, response, url, &temporary_dir)
            .and_then(|_| {
                if temporary_dir.path().exists() {
                    temporary_dir
                        .close()
                        .map_err(|e| JdkManagerError::io("Failed to cleanup temp dir", e))
                } else {
                    Ok(())
                }
            })?;
        return Ok(());
    }

    fn finish_extract(
        &self,
        path: &PathBuf,
        response: attohttpc::Response,
        url: String,
        temporary_dir: &TempDir,
    ) -> JdkManagerResult<()> {
        if url.ends_with(".tar.gz") {
            let expected_size = response.headers().get("Content-length").and_then(|len| {
                len.to_str()
                    .ok()
                    .and_then(|len_str| len_str.parse::<u64>().ok())
            });
            self.unarchive_tar_gz(temporary_dir.path(), expected_size, response)
        } else {
            return Err(JdkManagerError::generic(format!(
                "Don't know how to handle {}",
                url
            )));
        }
        eprintln!();
        let dir_entries = temporary_dir
            .path()
            .read_dir()
            .map_err(|e| JdkManagerError::io("Failed to read temp dir", e))?
            .map(|res| res.map(|e| e.path()))
            .filter(|r| match r {
                Ok(p) => match p.file_name() {
                    Some(name) => !name.to_string_lossy().starts_with("."),
                    _ => true,
                },
                _ => true,
            })
            .collect::<Result<Vec<_>, io::Error>>()
            .map_err(|e| JdkManagerError::io("Failed to read temp dir entry", e))?;
        let from_dir = if dir_entries.len() == 1 {
            if std::env::consts::OS == "macos" {
                let x = &dir_entries[0];
                x.join("Contents/Home")
            } else {
                (&dir_entries[0]).to_path_buf()
            }
        } else {
            temporary_dir.path().to_path_buf()
        };

        std::fs::rename(from_dir, &path).map_err(|e| {
            JdkManagerError::io(
                format!("Unable to move to JDK folder ({})", path.display()),
                e,
            )
        })?;

        File::create(path.join(FINISHED_MARKER))
            .map_err(|e| JdkManagerError::io("Unable to create marker", e))?;
        Ok(())
    }

    fn unarchive_tar_gz(
        &self,
        path: &Path,
        expected_size: Option<u64>,
        reader: impl Read + Send + 'static,
    ) {
        let all_bars = MultiProgress::with_draw_target(ProgressDrawTarget::stderr());
        let download_bar = all_bars.add(new_progress_bar(expected_size));
        download_bar.set_message("Download progress");
        let writing_bar = all_bars.add(new_progress_bar(None));

        let static_path = path.to_path_buf();
        let _ = std::thread::spawn(move || {
            let gz_decode =
                libflate::gzip::Decoder::new(BufReader::new(download_bar.wrap_read(reader)))
                    .unwrap();
            let mut archive = tar::Archive::new(BufReader::new(writing_bar.wrap_read(gz_decode)));
            archive.set_preserve_permissions(true);
            archive.set_overwrite(true);
            for entry in archive.entries().unwrap() {
                let mut file = entry.unwrap();
                writing_bar.set_message(format!("Extracting {}", file.path().unwrap().display()));
                file.unpack_in(&static_path).unwrap();
            }
            download_bar.finish();
            writing_bar.abandon_with_message("Done extracting!");
        });

        all_bars.join().unwrap();
    }
}
