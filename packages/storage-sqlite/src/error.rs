//! Storage error types.

use life_engine_traits::{EngineError, Severity};
use thiserror::Error;

/// Errors that can occur during storage operations.
#[derive(Debug, Error)]
pub enum StorageError {
    /// Database error.
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// Serialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Record not found.
    #[error("record not found: {0}")]
    NotFound(String),

    /// Unable to decrypt database — wrong key or corrupted file.
    #[error("unable to decrypt database: {0}")]
    DecryptionFailed(String),

    /// Database file permission denied.
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// Invalid configuration.
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    /// Initialization failed.
    #[error("initialization failed: {0}")]
    InitFailed(String),
}

impl EngineError for StorageError {
    fn code(&self) -> &str {
        match self {
            StorageError::Database(_) => "STORAGE_001",
            StorageError::Serialization(_) => "STORAGE_002",
            StorageError::NotFound(_) => "STORAGE_003",
            StorageError::DecryptionFailed(_) => "STORAGE_004",
            StorageError::PermissionDenied(_) => "STORAGE_005",
            StorageError::InvalidConfig(_) => "STORAGE_006",
            StorageError::InitFailed(_) => "STORAGE_007",
        }
    }

    fn severity(&self) -> Severity {
        match self {
            StorageError::NotFound(_) => Severity::Warning,
            _ => Severity::Fatal,
        }
    }

    fn source_module(&self) -> &str {
        "storage-sqlite"
    }
}
