use life_engine_plugin_sdk::prelude::*;
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
    #[error("unknown action: {0}")]
    UnknownAction(String),
}

impl EngineError for FilesystemConnectorError {
    fn code(&self) -> &str {
        match self {
            Self::NoWatchPaths => "FILESYSTEM_001",
            Self::ScanFailed(_) => "FILESYSTEM_002",
            Self::PathNotFound(_) => "FILESYSTEM_003",
            Self::UnknownAction(_) => "FILESYSTEM_004",
        }
    }

    fn severity(&self) -> Severity {
        match self {
            Self::UnknownAction(_) => Severity::Fatal,
            Self::NoWatchPaths => Severity::Fatal,
            Self::ScanFailed(_) => Severity::Retryable,
            Self::PathNotFound(_) => Severity::Fatal,
        }
    }

    fn source_module(&self) -> &str {
        "connector-filesystem"
    }
}
