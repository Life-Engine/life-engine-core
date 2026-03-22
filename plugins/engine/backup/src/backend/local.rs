//! Local filesystem backup backend.

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use super::{BackupBackend, StoredBackup};

/// Local filesystem backup storage.
pub struct LocalBackend {
    /// Base directory for backups.
    base_dir: PathBuf,
}

impl LocalBackend {
    /// Create a new local backend writing to the given directory.
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    fn full_path(&self, key: &str) -> PathBuf {
        self.base_dir.join(key)
    }
}

#[async_trait]
impl BackupBackend for LocalBackend {
    async fn put(&self, key: &str, data: &[u8]) -> anyhow::Result<()> {
        let path = self.full_path(key);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&path, data).await?;
        Ok(())
    }

    async fn get(&self, key: &str) -> anyhow::Result<Vec<u8>> {
        let path = self.full_path(key);
        let data = tokio::fs::read(&path).await?;
        Ok(data)
    }

    async fn delete(&self, key: &str) -> anyhow::Result<bool> {
        let path = self.full_path(key);
        if path.exists() {
            tokio::fs::remove_file(&path).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn list(&self, prefix: &str) -> anyhow::Result<Vec<StoredBackup>> {
        let search_dir = self.full_path(prefix);
        let dir = if search_dir.is_dir() {
            &search_dir
        } else {
            search_dir
                .parent()
                .unwrap_or(Path::new(&self.base_dir))
        };

        let mut results = Vec::new();

        if !dir.exists() {
            return Ok(results);
        }

        let mut entries = tokio::fs::read_dir(dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                let key = path
                    .strip_prefix(&self.base_dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();

                if key.starts_with(prefix) || prefix.is_empty() {
                    let metadata = entry.metadata().await?;
                    let modified = metadata
                        .modified()
                        .ok()
                        .and_then(|t| {
                            t.duration_since(std::time::UNIX_EPOCH)
                                .ok()
                                .map(|d| {
                                    chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                                        .unwrap_or_default()
                                        .to_rfc3339()
                                })
                        })
                        .unwrap_or_default();

                    results.push(StoredBackup {
                        key,
                        size: metadata.len(),
                        last_modified: modified,
                    });
                }
            }
        }

        results.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(results)
    }

    async fn exists(&self, key: &str) -> anyhow::Result<bool> {
        Ok(self.full_path(key).exists())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn put_and_get_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let backend = LocalBackend::new(tmp.path());

        backend.put("test.bin", b"hello world").await.unwrap();
        let data = backend.get("test.bin").await.unwrap();
        assert_eq!(data, b"hello world");
    }

    #[tokio::test]
    async fn put_creates_subdirectories() {
        let tmp = TempDir::new().unwrap();
        let backend = LocalBackend::new(tmp.path());

        backend
            .put("sub/dir/test.bin", b"nested data")
            .await
            .unwrap();
        let data = backend.get("sub/dir/test.bin").await.unwrap();
        assert_eq!(data, b"nested data");
    }

    #[tokio::test]
    async fn delete_existing_file() {
        let tmp = TempDir::new().unwrap();
        let backend = LocalBackend::new(tmp.path());

        backend.put("to-delete.bin", b"delete me").await.unwrap();
        assert!(backend.exists("to-delete.bin").await.unwrap());

        let deleted = backend.delete("to-delete.bin").await.unwrap();
        assert!(deleted);
        assert!(!backend.exists("to-delete.bin").await.unwrap());
    }

    #[tokio::test]
    async fn delete_nonexistent_returns_false() {
        let tmp = TempDir::new().unwrap();
        let backend = LocalBackend::new(tmp.path());

        let deleted = backend.delete("nope.bin").await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn list_files() {
        let tmp = TempDir::new().unwrap();
        let backend = LocalBackend::new(tmp.path());

        backend.put("backup-001.enc", b"data1").await.unwrap();
        backend.put("backup-002.enc", b"data2").await.unwrap();
        backend.put("other.txt", b"other").await.unwrap();

        let all = backend.list("").await.unwrap();
        assert_eq!(all.len(), 3);

        let backups = backend.list("backup-").await.unwrap();
        assert_eq!(backups.len(), 2);
    }

    #[tokio::test]
    async fn list_empty_directory() {
        let tmp = TempDir::new().unwrap();
        let backend = LocalBackend::new(tmp.path());

        let result = backend.list("").await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn exists_true_and_false() {
        let tmp = TempDir::new().unwrap();
        let backend = LocalBackend::new(tmp.path());

        assert!(!backend.exists("test.bin").await.unwrap());
        backend.put("test.bin", b"data").await.unwrap();
        assert!(backend.exists("test.bin").await.unwrap());
    }

    #[tokio::test]
    async fn get_nonexistent_fails() {
        let tmp = TempDir::new().unwrap();
        let backend = LocalBackend::new(tmp.path());

        let result = backend.get("nope.bin").await;
        assert!(result.is_err());
    }
}
