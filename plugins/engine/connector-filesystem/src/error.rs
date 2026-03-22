use thiserror::Error;

/// Errors specific to the filesystem connector plugin.
#[derive(Debug, Error)]
pub enum FilesystemConnectorError {
    #[error("no watch paths configured")]
    NoWatchPaths,
    #[error("scan failed: {0}")]
    ScanFailed(String),
    #[error("path not found: {0}")]
    PathNotFound(String),
}
