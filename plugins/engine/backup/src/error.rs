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
}
