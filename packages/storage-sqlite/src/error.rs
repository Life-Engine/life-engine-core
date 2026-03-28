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

    /// Optimistic concurrency conflict — the record version has changed.
    #[error("concurrency conflict: record {id} expected version {expected} but was modified")]
    ConcurrencyConflict {
        /// The record identifier.
        id: String,
        /// The version the caller expected.
        expected: u64,
    },

    /// Schema validation failed for a canonical or private collection write.
    #[error("validation failed for collection '{collection}': {message}")]
    ValidationFailed {
        /// The collection name.
        collection: String,
        /// Human-readable description of the validation failure.
        message: String,
    },

    /// Write targets an unknown collection (neither canonical nor declared private).
    #[error("unknown collection: {0}")]
    UnknownCollection(String),

    /// Key rotation (PRAGMA rekey) failed.
    #[error("key rotation failed: {0}")]
    RekeyFailed(String),

    /// Per-credential encryption or decryption failed.
    #[error("credential encryption error for credential '{credential_id}': {message}")]
    CredentialEncryption {
        /// The credential identifier.
        credential_id: String,
        /// Human-readable description of the failure.
        message: String,
    },

    /// I/O error during backup or restore.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
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
            StorageError::ConcurrencyConflict { .. } => "STORAGE_008",
            StorageError::ValidationFailed { .. } => "STORAGE_009",
            StorageError::UnknownCollection(_) => "STORAGE_010",
            StorageError::RekeyFailed(_) => "STORAGE_011",
            StorageError::CredentialEncryption { .. } => "STORAGE_012",
            StorageError::Io(_) => "STORAGE_013",
        }
    }

    fn severity(&self) -> Severity {
        match self {
            StorageError::NotFound(_) => Severity::Warning,
            StorageError::ConcurrencyConflict { .. } => Severity::Retryable,
            _ => Severity::Fatal,
        }
    }

    fn source_module(&self) -> &str {
        "storage-sqlite"
    }
}
