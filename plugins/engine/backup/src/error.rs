use life_engine_plugin_sdk::prelude::*;
use thiserror::Error;

/// Errors specific to the backup plugin.
#[derive(Debug, Error)]
pub enum BackupError {
    #[error("backup not configured")]
    NotConfigured,
    #[error("backup failed: {0}")]
    BackupFailed(String),
    #[error("restore failed: {0}")]
    RestoreFailed(String),
    #[error("encryption error: {0}")]
    EncryptionError(String),
    #[error("unknown action: {0}")]
    UnknownAction(String),
}

impl EngineError for BackupError {
    fn code(&self) -> &str {
        match self {
            Self::NotConfigured => "BACKUP_001",
            Self::BackupFailed(_) => "BACKUP_002",
            Self::RestoreFailed(_) => "BACKUP_003",
            Self::EncryptionError(_) => "BACKUP_004",
            Self::UnknownAction(_) => "BACKUP_005",
        }
    }

    fn severity(&self) -> Severity {
        match self {
            Self::UnknownAction(_) => Severity::Fatal,
            Self::NotConfigured => Severity::Fatal,
            Self::BackupFailed(_) => Severity::Retryable,
            Self::RestoreFailed(_) => Severity::Fatal,
            Self::EncryptionError(_) => Severity::Fatal,
        }
    }

    fn source_module(&self) -> &str {
        "backup"
    }
}
