use crate::checksum_verifier::ChecksumVerifier;
use crate::config::{JpreConfig, PROJECT_DIRS};
use crate::error::ESResult;
use crate::foojay::{
    ArchiveType, ChecksumType, FoojayPackageInfo, FoojayPackageListInfo, FOOJAY_API,
};
use crate::http_client::new_http_client;
use crate::java_version::key::VersionKey;
use crate::java_version::JavaVersion;
use crate::tui::new_progress_bar;
use derive_more::Display;
use digest::Digest;
use error_stack::{Context, Report, ResultExt};
use indicatif::MultiProgress;
use owo_colors::{OwoColorize, Stream};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::LazyLock;
use tempfile::TempDir;
use tracing::warn;
use ureq::http::Response;
use ureq::Body;

#[derive(Debug, Display)]
pub struct JdkManagerError;

impl Context for JdkManagerError {}

static JDK_STORE_PATH: LazyLock<PathBuf> = LazyLock::new(|| PROJECT_DIRS.cache_dir().join("jdks"));
static JDK_DOWNLOADS_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| PROJECT_DIRS.cache_dir().join("downloads"));

// Why not '.jdk_marker'? Old jpre didn't emit the version number in the marker file, so we need to
// use a new marker file to ensure we know which version of the JDK is installed.
const JDK_VALID_MARKER_FILE_NAME: &str = ".jdk_marker_with_version";
// We'll inspect the legacy one and use it as a valid JDK, but when updating we'll always overwrite.
const LEGACY_JDK_MARKER_FILE_NAME: &str = ".jdk_marker";

fn jdk_path(jdk: &VersionKey) -> PathBuf {
    JDK_STORE_PATH.join(jdk.to_string())
}

pub static JDK_MANAGER: LazyLock<JdkManager> = LazyLock::new(JdkManager::new);

pub struct JdkManager {
    client: ureq::Agent,
}

impl JdkManager {
    pub fn new() -> Self {
        Self {
            client: new_http_client(),
        }
    }

    pub fn get_installed_jdks(&self) -> ESResult<Vec<VersionKey>, JdkManagerError> {
        if !JDK_STORE_PATH.exists() {
            return Ok(Vec::new());
        }
        let mut result = Vec::new();
        for ent in std::fs::read_dir(&*JDK_STORE_PATH)
            .change_context(JdkManagerError)
            .attach_printable_lazy(|| {
                format!("Could not read JDK store at {:?}", *JDK_STORE_PATH)
            })?
        {
            let ent = ent
                .change_context(JdkManagerError)
                .attach_printable_lazy(|| {
                    format!("Could not read entry in JDK store at {:?}", *JDK_STORE_PATH)
                })?;
            let file_name = ent.file_name();
            let Some(name) = file_name.to_str() else {
                continue;
            };
            let Ok(key) = VersionKey::from_str(name) else {
                continue;
            };
            let marker = ent.path().join(JDK_VALID_MARKER_FILE_NAME);
            let legacy_marker = ent.path().join(LEGACY_JDK_MARKER_FILE_NAME);
            if !marker.exists() && !legacy_marker.exists() {
                continue;
            }
            result.push(key);
        }
        Ok(result)
    }

    pub fn get_full_version(
        &self,
        jdk: &VersionKey,
    ) -> ESResult<Option<JavaVersion>, JdkManagerError> {
        self.get_full_version_from_path(&jdk_path(jdk))
    }

    pub fn get_full_version_from_path(
        &self,
        path: &Path,
    ) -> ESResult<Option<JavaVersion>, JdkManagerError> {
        let marker = path.join(JDK_VALID_MARKER_FILE_NAME);
        if !marker.exists() {
            return Ok(None);
        }
        let version = std::fs::read_to_string(&marker)
            .change_context(JdkManagerError)
            .attach_printable_lazy(|| format!("Could not read JDK version from {:?}", marker))?;
        let version = JavaVersion::from_str(&version)
            .change_context(JdkManagerError)
            .attach_printable_lazy(|| format!("Could not parse JDK version from {:?}", marker))?;
        Ok(Some(version))
    }

    pub fn get_jdk_path(
        &self,
        config: &JpreConfig,
        jdk: &VersionKey,
    ) -> ESResult<PathBuf, JdkManagerError> {
        if !self.get_installed_jdks()?.into_iter().any(|k| &k == jdk) {
            self.download_jdk(config, jdk)?;
        }
        Ok(jdk_path(jdk))
    }

    /// Download a JDK, overwriting any existing JDK with the same version.
    pub fn download_jdk(
        &self,
        config: &JpreConfig,
        jdk: &VersionKey,
    ) -> ESResult<(), JdkManagerError> {
        let path = jdk_path(jdk);
        if path.exists() {
            std::fs::remove_dir_all(&path)
                .change_context(JdkManagerError)
                .attach_printable_lazy(|| {
                    format!("Could not remove JDK install folder at {:?}", path)
                })?;
        }
        std::fs::create_dir_all(&path)
            .change_context(JdkManagerError)
            .attach_printable_lazy(|| {
                format!("Could not create directory for JDK at {:?}", path)
            })?;
        let (list_info, info) = FOOJAY_API
            .get_latest_package_info_using_priority(config, jdk)
            .change_context(JdkManagerError)
            .attach_printable_lazy(|| {
                format!("Could not get latest JDK package info for {}", jdk)
            })?;

        let response = self
            .client
            .get(info.direct_download_uri.as_str())
            .call()
            .change_context(JdkManagerError)
            .attach_printable_lazy(|| {
                format!(
                    "Could not download JDK package from {}",
                    info.direct_download_uri
                )
            })?;
        std::fs::create_dir_all(&*JDK_DOWNLOADS_PATH)
            .change_context(JdkManagerError)
            .attach_printable_lazy(|| {
                format!(
                    "Could not create JDK downloads directory at {:?}",
                    JDK_DOWNLOADS_PATH
                )
            })?;
        let download_path = tempfile::NamedTempFile::new_in(&*JDK_DOWNLOADS_PATH)
            .change_context(JdkManagerError)
            .attach_printable_lazy(|| {
                format!(
                    "Could not create temporary file for JDK download in {:?}",
                    path
                )
            })?
            .into_temp_path();
        if let Err(e) = Self::download_jdk_to_file(&list_info, &info, response, &download_path) {
            let path = download_path.to_owned();
            if let Err(delete_err) = download_path.close() {
                warn!(
                    "Could not delete potentially invalid download at {:?}: {}",
                    path, delete_err
                );
            }
            return Err(e);
        }
        let unpack_dir = tempfile::tempdir_in(&*JDK_STORE_PATH)
            .change_context(JdkManagerError)
            .attach_printable("Could not create temporary directory for JDK unpacking")?;
        if let Err(e) = Self::unpack_jdk(&list_info, &download_path, unpack_dir.path()) {
            Self::cleanup_unpack_dir(unpack_dir);
            return Err(e);
        }
        let root = match Self::determine_jdk_root(unpack_dir.path())
            .change_context(JdkManagerError)
            .attach_printable("Could not determine JDK root directory")
        {
            Ok(root) => root,
            Err(e) => {
                Self::cleanup_unpack_dir(unpack_dir);
                return Err(e);
            }
        };

        if let Err(e) = std::fs::rename(&root, &path)
            .change_context(JdkManagerError)
            .attach_printable_lazy(|| format!("Could not move JDK from {:?} to {:?}", root, path))
        {
            Self::cleanup_unpack_dir(unpack_dir);
            return Err(e);
        }
        Self::cleanup_unpack_dir(unpack_dir);

        let marker_temp = tempfile::NamedTempFile::new_in(&path)
            .change_context(JdkManagerError)
            .attach_printable_lazy(|| {
                format!(
                    "Could not create temporary file for JDK marker in {:?}",
                    path
                )
            })?;
        std::fs::write(marker_temp.path(), list_info.java_version.to_string())
            .change_context(JdkManagerError)
            .attach_printable_lazy(|| {
                format!("Could not write JDK version to {:?}", marker_temp.path())
            })?;
        let marker_path = path.join(JDK_VALID_MARKER_FILE_NAME);
        std::fs::rename(marker_temp.path(), &marker_path)
            .change_context(JdkManagerError)
            .attach_printable_lazy(|| {
                format!(
                    "Could not move JDK marker from {:?} to {:?}",
                    marker_temp.path(),
                    marker_path
                )
            })?;

        Ok(())
    }

    fn cleanup_unpack_dir(unpack_dir: TempDir) {
        let path = unpack_dir.path().to_owned();
        if let Err(delete_err) = unpack_dir.close() {
            warn!(
                "Could not delete invalid download dir at {:?}: {}",
                path, delete_err
            );
        }
    }

    fn download_jdk_to_file(
        list_info: &FoojayPackageListInfo,
        info: &FoojayPackageInfo,
        response: Response<Body>,
        download_path: &Path,
    ) -> ESResult<(), JdkManagerError> {
        let mut file = std::fs::File::create(download_path)
            .change_context(JdkManagerError)
            .attach_printable_lazy(|| {
                format!(
                    "Could not create file for JDK download at {:?}",
                    download_path
                )
            })?;
        let mut checksum_verifier = ChecksumVerifier::new(
            &info.checksum,
            match info.checksum_type {
                ChecksumType::Sha256 => Box::new(sha2::Sha256::new()),
                ChecksumType::Unknown(ref ct) => {
                    unreachable!(
                        "JDKs listed should not contain unknown checksum type {}",
                        ct
                    )
                }
            },
            &mut file,
        );
        let progress_bar = new_progress_bar(
            response.body().content_length(),
        )
        .with_message(
            format!("Downloading JDK {}", list_info.java_version)
                .if_supports_color(Stream::Stderr, |s| s.green())
                .to_string(),
        );
        std::io::copy(
            &mut response.into_body().into_reader(),
            &mut progress_bar.wrap_write(&mut checksum_verifier),
        )
        .change_context(JdkManagerError)
        .attach_printable_lazy(|| format!("Could not write JDK package to {:?}", download_path))?;
        if !checksum_verifier.verify() {
            return Err(Report::new(JdkManagerError)
                .attach_printable(format!("Checksum failed for {}", info.direct_download_uri)));
        }
        progress_bar.abandon_with_message(
            format!("Downloaded JDK {} archive", list_info.java_version)
                .if_supports_color(Stream::Stderr, |s| s.green())
                .to_string(),
        );
        Ok(())
    }

    fn unpack_jdk(
        list_info: &FoojayPackageListInfo,
        download_path: &Path,
        unpack_dir: &Path,
    ) -> ESResult<(), JdkManagerError> {
        let all_bars = MultiProgress::new();
        let archive_size = std::fs::metadata(download_path)
            .change_context(JdkManagerError)
            .attach_printable_lazy(|| {
                format!(
                    "Could not get metadata for JDK download at {:?}",
                    download_path
                )
            })?
            .len();
        let archive_bar = all_bars.add(new_progress_bar(Some(archive_size)));
        let writing_bar = all_bars.add(new_progress_bar(None));
        match list_info.archive_type {
            ArchiveType::TarGz => {
                let gz_decode = flate2::read::GzDecoder::new(
                    archive_bar.wrap_read(
                        std::fs::File::open(download_path)
                            .change_context(JdkManagerError)
                            .attach_printable_lazy(|| {
                                format!("Could not open JDK download at {:?}", download_path)
                            })?,
                    ),
                );
                let mut archive = tar::Archive::new(writing_bar.wrap_read(gz_decode));
                archive.set_preserve_permissions(true);
                archive.set_overwrite(true);
                for entry in archive.entries().unwrap() {
                    let mut file = entry.unwrap();
                    let archive_path = file.path().unwrap().into_owned();
                    writing_bar.set_message(
                        format!(
                            "Extracting {}",
                            archive_path
                                .display()
                                .if_supports_color(Stream::Stderr, |s| s.cyan())
                        )
                        .if_supports_color(Stream::Stderr, |s| s.green())
                        .to_string(),
                    );
                    if !file.unpack_in(unpack_dir).unwrap() {
                        warn!("Not extracting file with unsafe path: {:?}", archive_path);
                    }
                }
            }
            ArchiveType::Zip => {
                let mut archive = zip::ZipArchive::new(
                    archive_bar.wrap_read(
                        std::fs::File::open(download_path)
                            .change_context(JdkManagerError)
                            .attach_printable_lazy(|| {
                                format!("Could not open JDK download at {:?}", download_path)
                            })?,
                    ),
                )
                .change_context(JdkManagerError)
                .attach_printable_lazy(|| {
                    format!(
                        "Could not read JDK download as ZIP archive at {:?}",
                        download_path
                    )
                })?;
                for i in 0..archive.len() {
                    let mut file = archive.by_index(i).unwrap();
                    let Some(archive_path) = file.enclosed_name() else {
                        warn!("Not extracting file with unsafe path: {:?}", file.name());
                        continue;
                    };
                    writing_bar.set_message(
                        format!(
                            "Extracting {}",
                            file.name().if_supports_color(Stream::Stderr, |s| s.cyan())
                        )
                        .if_supports_color(Stream::Stderr, |s| s.green())
                        .to_string(),
                    );
                    let mut extracted_file = std::fs::File::create(unpack_dir.join(&archive_path))
                        .change_context(JdkManagerError)
                        .attach_printable_lazy(|| {
                            format!(
                                "Could not create file for extracted JDK at {:?}",
                                unpack_dir.join(&archive_path)
                            )
                        })?;
                    std::io::copy(&mut file, &mut extracted_file)
                        .change_context(JdkManagerError)
                        .attach_printable_lazy(|| {
                            format!(
                                "Could not write extracted JDK file to {:?}",
                                unpack_dir.join(archive_path)
                            )
                        })?;
                }
            }
            ArchiveType::Unknown(ref at) => {
                unreachable!("JDKs listed should not contain unknown archive type {}", at)
            }
        }
        archive_bar.finish();
        writing_bar.abandon_with_message(
            "Done extracting!"
                .if_supports_color(Stream::Stderr, |s| s.green())
                .to_string(),
        );
        Ok(())
    }

    fn determine_jdk_root(unpack_dir: &Path) -> ESResult<PathBuf, JdkManagerError> {
        let entries = std::fs::read_dir(unpack_dir)
            .change_context(JdkManagerError)
            .attach_printable_lazy(|| {
                format!("Could not read JDK unpack directory at {:?}", unpack_dir)
            })?
            .map(|r| r.map(|e| e.path()))
            .filter(|r| match r {
                Ok(p) => match p.file_name() {
                    Some(name) => !name.to_string_lossy().starts_with('.'),
                    _ => true,
                },
                _ => true,
            })
            .collect::<Result<Vec<_>, std::io::Error>>()
            .change_context(JdkManagerError)
            .attach_printable_lazy(|| {
                format!("Could not read JDK unpack directory at {:?}", unpack_dir)
            })?;
        let base_dir = if entries.len() == 1 {
            entries[0].to_owned()
        } else {
            unpack_dir.to_owned()
        };
        let possible_home = if std::env::consts::OS == "macos" {
            let contents_home = base_dir.join("Contents/Home");
            if contents_home.exists() {
                contents_home
            } else {
                base_dir
            }
        } else {
            base_dir
        };
        if possible_home.join("bin/java").exists() {
            Ok(possible_home)
        } else {
            Err(Report::new(JdkManagerError).attach_printable(format!(
                "Could not find JDK root directory in {:?}, tried {:?}",
                unpack_dir, possible_home
            )))
        }
    }
}
