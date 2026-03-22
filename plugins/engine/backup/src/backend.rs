//! Storage backend trait and implementations for backup targets.
//!
//! Provides a unified `BackupBackend` trait implemented by local
//! filesystem, S3-compatible, and WebDAV storage backends.

pub mod local;
pub mod s3;
pub mod webdav;

use async_trait::async_trait;

/// Metadata about a stored backup file.
#[derive(Debug, Clone)]
pub struct StoredBackup {
    /// The key/path identifying this backup.
    pub key: String,
    /// Size in bytes.
    pub size: u64,
    /// Last modified timestamp (ISO 8601).
    pub last_modified: String,
}

/// Trait for backup storage backends.
///
/// All implementations store and retrieve opaque byte blobs keyed by
/// a string path. The backup engine handles serialization, compression,
/// and encryption before calling these methods.
#[async_trait]
pub trait BackupBackend: Send + Sync {
    /// Store a backup blob at the given key.
    async fn put(&self, key: &str, data: &[u8]) -> anyhow::Result<()>;

    /// Retrieve a backup blob by key.
    async fn get(&self, key: &str) -> anyhow::Result<Vec<u8>>;

    /// Delete a backup by key. Returns true if it existed.
    async fn delete(&self, key: &str) -> anyhow::Result<bool>;

    /// List all backups matching the given prefix.
    async fn list(&self, prefix: &str) -> anyhow::Result<Vec<StoredBackup>>;

    /// Check if a backup exists at the given key.
    async fn exists(&self, key: &str) -> anyhow::Result<bool>;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify BackupBackend is object-safe.
    fn _assert_object_safe(_: &dyn BackupBackend) {}

    #[test]
    fn stored_backup_construction() {
        let sb = StoredBackup {
            key: "backups/full-2026-03-22.enc".into(),
            size: 1024,
            last_modified: "2026-03-22T00:00:00Z".into(),
        };
        assert_eq!(sb.key, "backups/full-2026-03-22.enc");
        assert_eq!(sb.size, 1024);
    }
}
