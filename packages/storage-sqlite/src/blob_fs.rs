//! Filesystem-based blob storage adapter.
//!
//! Stores blobs as plain files under a configurable root directory with
//! JSON sidecar files for metadata. Writes are atomic (write-to-temp then
//! rename) and checksums are SHA-256.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use chrono::Utc;
use life_engine_traits::blob::{
    BlobAdapterCapabilities, BlobInput, BlobKey, BlobMeta, BlobStorageAdapter,
};
use life_engine_traits::storage::{
    HealthCheck, HealthReport, HealthStatus, StorageError,
};
use sha2::{Digest, Sha256};

/// Filesystem blob adapter.
///
/// Layout on disk:
/// - Blob data: `{root}/{plugin_id}/{context}/{filename}`
/// - Metadata sidecar: `{root}/{plugin_id}/{context}/{filename}.meta.json`
pub struct FsBlobAdapter {
    root: PathBuf,
}

impl FsBlobAdapter {
    /// Create a new adapter rooted at `root`. The directory must already exist.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Full filesystem path for a blob key.
    fn blob_path(&self, key: &BlobKey) -> PathBuf {
        self.root.join(key.as_str())
    }

    /// Full filesystem path for the metadata sidecar.
    fn meta_path(&self, key: &BlobKey) -> PathBuf {
        let mut p = self.blob_path(key);
        let mut name = p
            .file_name()
            .unwrap_or_default()
            .to_os_string();
        name.push(".meta.json");
        p.set_file_name(name);
        p
    }

    /// Detect MIME type from the key's filename extension.
    fn detect_content_type(key: &BlobKey) -> String {
        let path = Path::new(key.as_str());
        mime_guess::from_path(path)
            .first_raw()
            .unwrap_or("application/octet-stream")
            .to_string()
    }

    /// Write `data` to `dest` atomically by writing to a sibling temp file
    /// then renaming.
    fn atomic_write(dest: &Path, data: &[u8]) -> Result<(), StorageError> {
        let parent = dest.parent().ok_or_else(|| StorageError::Internal {
            message: format!("no parent directory for {}", dest.display()),
        })?;
        std::fs::create_dir_all(parent).map_err(|e| StorageError::Internal {
            message: format!("mkdir {}: {e}", parent.display()),
        })?;

        // Write to a temp file in the same directory so rename is atomic.
        let mut tmp_path = dest.to_path_buf();
        let mut tmp_name = dest
            .file_name()
            .unwrap_or_default()
            .to_os_string();
        tmp_name.push(".tmp");
        tmp_path.set_file_name(tmp_name);

        std::fs::write(&tmp_path, data).map_err(|e| {
            StorageError::Internal {
                message: format!("write {}: {e}", tmp_path.display()),
            }
        })?;
        std::fs::rename(&tmp_path, dest).map_err(|e| {
            // Clean up the temp file on rename failure.
            let _ = std::fs::remove_file(&tmp_path);
            StorageError::Internal {
                message: format!("rename to {}: {e}", dest.display()),
            }
        })?;
        Ok(())
    }

    /// Read and deserialise the sidecar metadata for `key`.
    fn read_meta(&self, key: &BlobKey) -> Result<BlobMeta, StorageError> {
        let meta_path = self.meta_path(key);
        let bytes = std::fs::read(&meta_path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                not_found(key)
            } else {
                StorageError::Internal {
                    message: format!("read meta {}: {e}", meta_path.display()),
                }
            }
        })?;
        serde_json::from_slice(&bytes).map_err(|e| StorageError::Internal {
            message: format!("parse meta {}: {e}", meta_path.display()),
        })
    }
}

/// Convenience helper for `StorageError::NotFound` using blob key.
fn not_found(key: &BlobKey) -> StorageError {
    StorageError::NotFound {
        collection: "blob".into(),
        id: key.as_str().to_string(),
    }
}

#[async_trait]
impl BlobStorageAdapter for FsBlobAdapter {
    async fn store(&self, key: BlobKey, input: BlobInput) -> Result<BlobMeta, StorageError> {
        let checksum = hex::encode(Sha256::digest(&input.data));
        let content_type = input
            .content_type
            .unwrap_or_else(|| Self::detect_content_type(&key));

        let now = Utc::now();

        // Preserve original created_at when overwriting.
        let created_at = self
            .read_meta(&key)
            .ok()
            .map(|m| m.created_at)
            .unwrap_or(now);

        let meta = BlobMeta {
            key: key.as_str().to_string(),
            size_bytes: input.data.len() as u64,
            content_type,
            checksum,
            created_at,
            metadata: input.metadata,
        };

        let meta_bytes = serde_json::to_vec_pretty(&meta).map_err(|e| {
            StorageError::Internal {
                message: format!("serialise meta: {e}"),
            }
        })?;

        // Write blob data atomically, then sidecar atomically.
        let blob_path = self.blob_path(&key);
        let meta_path = self.meta_path(&key);

        Self::atomic_write(&blob_path, &input.data)?;
        if let Err(e) = Self::atomic_write(&meta_path, &meta_bytes) {
            // Roll back blob if sidecar write fails.
            let _ = std::fs::remove_file(&blob_path);
            return Err(e);
        }

        Ok(meta)
    }

    async fn retrieve(&self, key: BlobKey) -> Result<(Vec<u8>, BlobMeta), StorageError> {
        let blob_path = self.blob_path(&key);
        let data = std::fs::read(&blob_path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                not_found(&key)
            } else {
                StorageError::Internal {
                    message: format!("read blob {}: {e}", blob_path.display()),
                }
            }
        })?;

        let meta = self.read_meta(&key)?;

        // Verify checksum on read.
        let actual = hex::encode(Sha256::digest(&data));
        if actual != meta.checksum {
            return Err(StorageError::Internal {
                message: format!(
                    "checksum mismatch for {}: expected {}, got {actual}",
                    key, meta.checksum
                ),
            });
        }

        Ok((data, meta))
    }

    async fn delete(&self, key: BlobKey) -> Result<(), StorageError> {
        let blob_path = self.blob_path(&key);
        let meta_path = self.meta_path(&key);

        if !blob_path.exists() {
            return Err(not_found(&key));
        }

        std::fs::remove_file(&blob_path).map_err(|e| StorageError::Internal {
            message: format!("delete blob {}: {e}", blob_path.display()),
        })?;
        // Best-effort remove sidecar — if blob existed, sidecar should too.
        let _ = std::fs::remove_file(&meta_path);

        Ok(())
    }

    async fn exists(&self, key: BlobKey) -> Result<bool, StorageError> {
        Ok(self.blob_path(&key).exists())
    }

    async fn copy(&self, source: BlobKey, dest: BlobKey) -> Result<BlobMeta, StorageError> {
        let src_blob = self.blob_path(&source);
        if !src_blob.exists() {
            return Err(not_found(&source));
        }

        let data = std::fs::read(&src_blob).map_err(|e| StorageError::Internal {
            message: format!("read blob {}: {e}", src_blob.display()),
        })?;
        let src_meta = self.read_meta(&source)?;

        let now = Utc::now();
        let new_meta = BlobMeta {
            key: dest.as_str().to_string(),
            size_bytes: src_meta.size_bytes,
            content_type: src_meta.content_type,
            checksum: src_meta.checksum,
            created_at: now,
            metadata: src_meta.metadata,
        };

        let meta_bytes = serde_json::to_vec_pretty(&new_meta).map_err(|e| {
            StorageError::Internal {
                message: format!("serialise meta: {e}"),
            }
        })?;

        let dest_blob = self.blob_path(&dest);
        let dest_meta = self.meta_path(&dest);

        Self::atomic_write(&dest_blob, &data)?;
        if let Err(e) = Self::atomic_write(&dest_meta, &meta_bytes) {
            let _ = std::fs::remove_file(&dest_blob);
            return Err(e);
        }

        Ok(new_meta)
    }

    async fn list(&self, prefix: &str) -> Result<Vec<BlobMeta>, StorageError> {
        let search_dir = self.root.join(prefix);

        // The prefix may refer to a partial directory path. Walk the deepest
        // existing ancestor and filter.
        let walk_dir = if search_dir.is_dir() {
            search_dir.clone()
        } else {
            search_dir
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| self.root.clone())
        };

        if !walk_dir.exists() {
            return Ok(vec![]);
        }

        let mut results = Vec::new();
        collect_metas(&walk_dir, &self.root, prefix, &mut results)?;
        Ok(results)
    }

    async fn metadata(&self, key: BlobKey) -> Result<BlobMeta, StorageError> {
        self.read_meta(&key)
    }

    async fn health(&self) -> Result<HealthReport, StorageError> {
        let dir_exists = self.root.is_dir();
        let writable = if dir_exists {
            let probe = self.root.join(".health_probe");
            let ok = std::fs::write(&probe, b"ok").is_ok();
            let _ = std::fs::remove_file(&probe);
            ok
        } else {
            false
        };

        let status = if dir_exists && writable {
            HealthStatus::Healthy
        } else if dir_exists {
            HealthStatus::Degraded
        } else {
            HealthStatus::Unhealthy
        };

        Ok(HealthReport {
            status,
            message: None,
            checks: vec![
                HealthCheck {
                    name: "root_exists".into(),
                    status: if dir_exists {
                        HealthStatus::Healthy
                    } else {
                        HealthStatus::Unhealthy
                    },
                    message: None,
                },
                HealthCheck {
                    name: "root_writable".into(),
                    status: if writable {
                        HealthStatus::Healthy
                    } else {
                        HealthStatus::Unhealthy
                    },
                    message: None,
                },
            ],
        })
    }

    fn capabilities(&self) -> BlobAdapterCapabilities {
        BlobAdapterCapabilities {
            streaming: false,
            max_blob_size: None,
            checksum_algorithms: vec!["sha256".into()],
            encryption: false,
            server_side_copy: true,
        }
    }
}

/// Recursively collect `BlobMeta` for blobs whose key starts with `prefix`.
fn collect_metas(
    dir: &Path,
    root: &Path,
    prefix: &str,
    out: &mut Vec<BlobMeta>,
) -> Result<(), StorageError> {
    let entries = std::fs::read_dir(dir).map_err(|e| StorageError::Internal {
        message: format!("readdir {}: {e}", dir.display()),
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| StorageError::Internal {
            message: format!("readdir entry: {e}"),
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_metas(&path, root, prefix, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("json")
            && path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".meta.json"))
        {
            // This is a sidecar file — derive the blob key from it.
            let meta_rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy();
            let blob_key_str = meta_rel.trim_end_matches(".meta.json");
            if blob_key_str.starts_with(prefix) {
                let bytes =
                    std::fs::read(&path).map_err(|e| StorageError::Internal {
                        message: format!("read meta {}: {e}", path.display()),
                    })?;
                if let Ok(meta) = serde_json::from_slice::<BlobMeta>(&bytes) {
                    out.push(meta);
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn adapter(dir: &TempDir) -> FsBlobAdapter {
        FsBlobAdapter::new(dir.path())
    }

    fn key(s: &str) -> BlobKey {
        BlobKey::new(s).unwrap()
    }

    fn sample_input(data: &[u8]) -> BlobInput {
        BlobInput {
            data: data.to_vec(),
            content_type: None,
            metadata: HashMap::new(),
        }
    }

    // 1. Store/retrieve round-trip with checksum verification
    #[tokio::test]
    async fn store_retrieve_round_trip() {
        let dir = TempDir::new().unwrap();
        let a = adapter(&dir);
        let k = key("plugin-a/docs/hello.txt");
        let data = b"hello world";

        let meta = a.store(k.clone(), sample_input(data)).await.unwrap();
        assert_eq!(meta.size_bytes, data.len() as u64);
        assert_eq!(
            meta.checksum,
            hex::encode(Sha256::digest(data))
        );
        assert_eq!(meta.content_type, "text/plain");

        let (retrieved_data, retrieved_meta) = a.retrieve(k).await.unwrap();
        assert_eq!(retrieved_data, data);
        assert_eq!(retrieved_meta.checksum, meta.checksum);
    }

    // 2. Atomic write — no partial files on failure (temp file cleaned up)
    #[tokio::test]
    async fn atomic_write_no_partial_files() {
        let dir = TempDir::new().unwrap();
        let a = adapter(&dir);
        let k = key("plugin-a/docs/file.bin");

        a.store(k.clone(), sample_input(b"data")).await.unwrap();

        // Verify no .tmp files remain.
        let blob_dir = dir.path().join("plugin-a/docs");
        for entry in std::fs::read_dir(&blob_dir).unwrap() {
            let name = entry.unwrap().file_name();
            let name = name.to_str().unwrap();
            assert!(
                !name.ends_with(".tmp"),
                "temp file left behind: {name}"
            );
        }
    }

    // 3. Metadata sidecar created and readable
    #[tokio::test]
    async fn metadata_sidecar_created() {
        let dir = TempDir::new().unwrap();
        let a = adapter(&dir);
        let k = key("plugin-a/imgs/photo.png");

        let mut input = sample_input(b"\x89PNG");
        input.content_type = Some("image/png".into());
        input.metadata.insert("author".into(), "test".into());

        let stored = a.store(k.clone(), input).await.unwrap();
        let read_meta = a.metadata(k).await.unwrap();
        assert_eq!(stored, read_meta);
        assert_eq!(read_meta.content_type, "image/png");
        assert_eq!(read_meta.metadata.get("author").unwrap(), "test");
    }

    // 4. MIME type detection from extension
    #[tokio::test]
    async fn mime_type_detection() {
        let dir = TempDir::new().unwrap();
        let a = adapter(&dir);

        let png_key = key("plugin-a/files/image.png");
        let meta = a.store(png_key, sample_input(b"bytes")).await.unwrap();
        assert_eq!(meta.content_type, "image/png");

        let unknown_key = key("plugin-a/files/data.xyz123");
        let meta = a
            .store(unknown_key, sample_input(b"bytes"))
            .await
            .unwrap();
        assert_eq!(meta.content_type, "application/octet-stream");
    }

    // 5. Delete removes both file and sidecar
    #[tokio::test]
    async fn delete_removes_file_and_sidecar() {
        let dir = TempDir::new().unwrap();
        let a = adapter(&dir);
        let k = key("plugin-a/docs/rm.txt");

        a.store(k.clone(), sample_input(b"bye")).await.unwrap();
        assert!(a.exists(k.clone()).await.unwrap());

        a.delete(k.clone()).await.unwrap();
        assert!(!a.exists(k.clone()).await.unwrap());

        // Sidecar should also be gone.
        let meta_path = a.meta_path(&k);
        assert!(!meta_path.exists());
    }

    // 6. Exists returns correct state
    #[tokio::test]
    async fn exists_returns_correct_state() {
        let dir = TempDir::new().unwrap();
        let a = adapter(&dir);
        let k = key("plugin-a/docs/check.txt");

        assert!(!a.exists(k.clone()).await.unwrap());
        a.store(k.clone(), sample_input(b"x")).await.unwrap();
        assert!(a.exists(k).await.unwrap());
    }

    // 7. Copy creates independent copy
    #[tokio::test]
    async fn copy_creates_independent_copy() {
        let dir = TempDir::new().unwrap();
        let a = adapter(&dir);
        let src = key("plugin-a/docs/original.txt");
        let dst = key("plugin-a/docs/copy.txt");

        a.store(src.clone(), sample_input(b"content"))
            .await
            .unwrap();

        let copy_meta = a.copy(src.clone(), dst.clone()).await.unwrap();
        assert_eq!(copy_meta.key, dst.as_str());

        // Both exist independently.
        let (src_data, _) = a.retrieve(src.clone()).await.unwrap();
        let (dst_data, _) = a.retrieve(dst.clone()).await.unwrap();
        assert_eq!(src_data, dst_data);

        // Deleting source leaves copy intact.
        a.delete(src).await.unwrap();
        assert!(a.exists(dst).await.unwrap());
    }

    // 8. List by prefix
    #[tokio::test]
    async fn list_by_prefix() {
        let dir = TempDir::new().unwrap();
        let a = adapter(&dir);

        a.store(key("plugin-a/docs/a.txt"), sample_input(b"a"))
            .await
            .unwrap();
        a.store(key("plugin-a/docs/b.txt"), sample_input(b"b"))
            .await
            .unwrap();
        a.store(key("plugin-a/imgs/c.png"), sample_input(b"c"))
            .await
            .unwrap();

        let docs = a.list("plugin-a/docs").await.unwrap();
        assert_eq!(docs.len(), 2);

        let all = a.list("plugin-a").await.unwrap();
        assert_eq!(all.len(), 3);

        let empty = a.list("plugin-b").await.unwrap();
        assert!(empty.is_empty());
    }

    // 9. Health check
    #[tokio::test]
    async fn health_check_healthy() {
        let dir = TempDir::new().unwrap();
        let a = adapter(&dir);

        let report = a.health().await.unwrap();
        assert_eq!(report.status, HealthStatus::Healthy);
        assert_eq!(report.checks.len(), 2);
    }

    #[tokio::test]
    async fn health_check_unhealthy_missing_dir() {
        let a = FsBlobAdapter::new("/tmp/nonexistent_blob_root_xyz");
        let report = a.health().await.unwrap();
        assert_eq!(report.status, HealthStatus::Unhealthy);
    }

    // -- Additional edge cases --

    #[tokio::test]
    async fn delete_nonexistent_returns_not_found() {
        let dir = TempDir::new().unwrap();
        let a = adapter(&dir);
        let result = a.delete(key("plugin-a/docs/nope.txt")).await;
        assert!(matches!(
            result,
            Err(StorageError::NotFound { .. })
        ));
    }

    #[tokio::test]
    async fn retrieve_nonexistent_returns_not_found() {
        let dir = TempDir::new().unwrap();
        let a = adapter(&dir);
        let result = a.retrieve(key("plugin-a/docs/nope.txt")).await;
        assert!(matches!(
            result,
            Err(StorageError::NotFound { .. })
        ));
    }

    #[tokio::test]
    async fn copy_nonexistent_source_returns_not_found() {
        let dir = TempDir::new().unwrap();
        let a = adapter(&dir);
        let result = a
            .copy(
                key("plugin-a/docs/nope.txt"),
                key("plugin-a/docs/dest.txt"),
            )
            .await;
        assert!(matches!(
            result,
            Err(StorageError::NotFound { .. })
        ));
    }

    #[tokio::test]
    async fn overwrite_preserves_created_at() {
        let dir = TempDir::new().unwrap();
        let a = adapter(&dir);
        let k = key("plugin-a/docs/over.txt");

        let first = a.store(k.clone(), sample_input(b"v1")).await.unwrap();
        let second = a.store(k.clone(), sample_input(b"v2")).await.unwrap();

        assert_eq!(first.created_at, second.created_at);
        assert_eq!(second.size_bytes, 2);
    }

    #[tokio::test]
    async fn capabilities_reports_expected_values() {
        let dir = TempDir::new().unwrap();
        let a = adapter(&dir);
        let caps = a.capabilities();
        assert!(!caps.streaming);
        assert!(!caps.encryption);
        assert!(caps.server_side_copy);
        assert_eq!(caps.checksum_algorithms, vec!["sha256".to_string()]);
    }

    #[tokio::test]
    async fn metadata_nonexistent_returns_not_found() {
        let dir = TempDir::new().unwrap();
        let a = adapter(&dir);
        let result = a.metadata(key("plugin-a/docs/nope.txt")).await;
        assert!(matches!(
            result,
            Err(StorageError::NotFound { .. })
        ));
    }
}
