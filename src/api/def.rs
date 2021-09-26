use std::collections::HashSet;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum JdkFetchError {
    #[error("error in HTTP I/O")]
    HttpIo(#[from] attohttpc::Error),
    #[error("error from upstream: {message}")]
    Upstream { message: String },
    #[error("{message}")]
    Incompatible { message: String },
    #[error("unknown error: {message}")]
    Generic { message: String },
}

pub type JdkFetchResult<T> = Result<T, JdkFetchError>;

pub trait JdkFetchApi {
    fn get_latest_jdk_binary(&self, major: u8) -> JdkFetchResult<attohttpc::Response>;

    /// Get the latest JDK version, returning `None` if the API doesn't know about this major
    /// version.
    fn get_latest_jdk_version(&self, major: u8) -> JdkFetchResult<Option<String>>;

    fn get_available_jdk_versions(&self) -> JdkFetchResult<HashSet<String>>;
}
