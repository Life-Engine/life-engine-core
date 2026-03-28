//! In-memory mock implementation of `BlobStorageAdapter` for testing.

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::Utc;
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;

use life_engine_traits::blob::{
    BlobAdapterCapabilities, BlobInput, BlobKey, BlobMeta, BlobStorageAdapter,
};
use life_engine_traits::storage::{HealthCheck, HealthReport, HealthStatus, StorageError};

/// In-memory blob storage for testing.
///
/// Stores blobs as `(Vec<u8>, BlobMeta)` keyed by the string representation
/// of `BlobKey`. All operations are guarded by a `tokio::sync::RwLock`.
pub struct MockBlobStorageAdapter {
    data: RwLock<HashMap<String, (Vec<u8>, BlobMeta)>>,
}

impl MockBlobStorageAdapter {
    /// Create a new empty mock blob store.
    pub fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for MockBlobStorageAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute the SHA-256 hex digest of a byte slice.
fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Guess MIME type from a key's filename extension, defaulting to
/// `application/octet-stream`.
fn guess_content_type(key: &str) -> String {
    let filename = key.rsplit('/').next().unwrap_or(key);
    mime_guess::from_path(filename)
        .first_or_octet_stream()
        .to_string()
}

#[async_trait]
impl BlobStorageAdapter for MockBlobStorageAdapter {
    async fn store(&self, key: BlobKey, input: BlobInput) -> Result<BlobMeta, StorageError> {
        let now = Utc::now();
        let checksum = sha256_hex(&input.data);
        let size_bytes = input.data.len() as u64;

        let content_type = input
            .content_type
            .unwrap_or_else(|| guess_content_type(key.as_str()));

        let mut data = self.data.write().await;

        // Preserve original created_at if overwriting.
        let created_at = data
            .get(key.as_str())
            .map(|(_, meta)| meta.created_at)
            .unwrap_or(now);

        let meta = BlobMeta {
            key: key.as_str().to_string(),
            size_bytes,
            content_type,
            checksum,
            created_at,
            metadata: input.metadata,
        };

        data.insert(key.as_str().to_string(), (input.data, meta.clone()));
        Ok(meta)
    }

    async fn retrieve(&self, key: BlobKey) -> Result<(Vec<u8>, BlobMeta), StorageError> {
        let data = self.data.read().await;
        data.get(key.as_str())
            .cloned()
            .ok_or_else(|| StorageError::NotFound {
                collection: "blob".to_string(),
                id: key.as_str().to_string(),
            })
    }

    async fn delete(&self, key: BlobKey) -> Result<(), StorageError> {
        let mut data = self.data.write().await;
        if data.remove(key.as_str()).is_none() {
            return Err(StorageError::NotFound {
                collection: "blob".to_string(),
                id: key.as_str().to_string(),
            });
        }
        Ok(())
    }

    async fn exists(&self, key: BlobKey) -> Result<bool, StorageError> {
        let data = self.data.read().await;
        Ok(data.contains_key(key.as_str()))
    }

    async fn copy(&self, source: BlobKey, dest: BlobKey) -> Result<BlobMeta, StorageError> {
        let mut data = self.data.write().await;
        let (bytes, _source_meta) =
            data.get(source.as_str())
                .cloned()
                .ok_or_else(|| StorageError::NotFound {
                    collection: "blob".to_string(),
                    id: source.as_str().to_string(),
                })?;

        let now = Utc::now();
        let checksum = sha256_hex(&bytes);

        let new_meta = BlobMeta {
            key: dest.as_str().to_string(),
            size_bytes: bytes.len() as u64,
            content_type: _source_meta.content_type.clone(),
            checksum,
            created_at: now,
            metadata: _source_meta.metadata.clone(),
        };

        data.insert(dest.as_str().to_string(), (bytes, new_meta.clone()));
        Ok(new_meta)
    }

    async fn list(&self, prefix: &str) -> Result<Vec<BlobMeta>, StorageError> {
        let data = self.data.read().await;
        let results: Vec<BlobMeta> = data
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(_, (_, meta))| meta.clone())
            .collect();
        Ok(results)
    }

    async fn metadata(&self, key: BlobKey) -> Result<BlobMeta, StorageError> {
        let data = self.data.read().await;
        data.get(key.as_str())
            .map(|(_, meta)| meta.clone())
            .ok_or_else(|| StorageError::NotFound {
                collection: "blob".to_string(),
                id: key.as_str().to_string(),
            })
    }

    async fn health(&self) -> Result<HealthReport, StorageError> {
        Ok(HealthReport {
            status: HealthStatus::Healthy,
            message: Some("mock blob adapter".to_string()),
            checks: vec![HealthCheck {
                name: "memory".to_string(),
                status: HealthStatus::Healthy,
                message: None,
            }],
        })
    }

    fn capabilities(&self) -> BlobAdapterCapabilities {
        BlobAdapterCapabilities {
            streaming: false,
            max_blob_size: Some(10 * 1024 * 1024), // 10 MB
            checksum_algorithms: vec!["sha256".to_string()],
            encryption: false,
            server_side_copy: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key(s: &str) -> BlobKey {
        BlobKey::new(s).expect("valid test key")
    }

    #[tokio::test]
    async fn mock_blob_store_and_retrieve() {
        let store = MockBlobStorageAdapter::new();
        let key = test_key("plugin-a/docs/readme.txt");
        let data = b"Hello, world!".to_vec();
        let input = BlobInput {
            data: data.clone(),
            content_type: Some("text/plain".to_string()),
            metadata: HashMap::new(),
        };

        let meta = store.store(key.clone(), input).await.unwrap();
        assert_eq!(meta.size_bytes, 13);
        assert_eq!(meta.content_type, "text/plain");
        assert_eq!(meta.checksum, sha256_hex(&data));

        let (retrieved_data, retrieved_meta) = store.retrieve(key).await.unwrap();
        assert_eq!(retrieved_data, data);
        assert_eq!(retrieved_meta.checksum, meta.checksum);
    }

    #[tokio::test]
    async fn mock_blob_store_overwrites_preserving_created_at() {
        let store = MockBlobStorageAdapter::new();
        let key = test_key("plugin-a/docs/file.bin");

        let input1 = BlobInput {
            data: vec![1, 2, 3],
            content_type: Some("application/octet-stream".to_string()),
            metadata: HashMap::new(),
        };
        let meta1 = store.store(key.clone(), input1).await.unwrap();

        let input2 = BlobInput {
            data: vec![4, 5, 6],
            content_type: Some("application/octet-stream".to_string()),
            metadata: HashMap::new(),
        };
        let meta2 = store.store(key.clone(), input2).await.unwrap();

        assert_eq!(meta2.created_at, meta1.created_at);
        assert_eq!(meta2.size_bytes, 3);
        assert_ne!(meta2.checksum, meta1.checksum);
    }

    #[tokio::test]
    async fn mock_blob_delete() {
        let store = MockBlobStorageAdapter::new();
        let key = test_key("plugin-a/docs/file.bin");
        let input = BlobInput {
            data: vec![1],
            content_type: None,
            metadata: HashMap::new(),
        };
        store.store(key.clone(), input).await.unwrap();

        store.delete(key.clone()).await.unwrap();

        let result = store.retrieve(key).await;
        assert!(matches!(result.unwrap_err(), StorageError::NotFound { .. }));
    }

    #[tokio::test]
    async fn mock_blob_delete_not_found() {
        let store = MockBlobStorageAdapter::new();
        let key = test_key("plugin-a/docs/nope.bin");
        let result = store.delete(key).await;
        assert!(matches!(result.unwrap_err(), StorageError::NotFound { .. }));
    }

    #[tokio::test]
    async fn mock_blob_exists() {
        let store = MockBlobStorageAdapter::new();
        let key = test_key("plugin-a/docs/file.bin");
        assert!(!store.exists(key.clone()).await.unwrap());

        let input = BlobInput {
            data: vec![1],
            content_type: None,
            metadata: HashMap::new(),
        };
        store.store(key.clone(), input).await.unwrap();
        assert!(store.exists(key).await.unwrap());
    }

    #[tokio::test]
    async fn mock_blob_copy() {
        let store = MockBlobStorageAdapter::new();
        let src = test_key("plugin-a/docs/original.txt");
        let dest = test_key("plugin-a/docs/copy.txt");

        let data = b"copy me".to_vec();
        let input = BlobInput {
            data: data.clone(),
            content_type: Some("text/plain".to_string()),
            metadata: HashMap::new(),
        };
        store.store(src.clone(), input).await.unwrap();

        let copy_meta = store.copy(src.clone(), dest.clone()).await.unwrap();
        assert_eq!(copy_meta.key, dest.as_str());
        assert_eq!(copy_meta.size_bytes, data.len() as u64);

        // Both should exist independently
        let (src_data, _) = store.retrieve(src).await.unwrap();
        let (dest_data, _) = store.retrieve(dest).await.unwrap();
        assert_eq!(src_data, dest_data);
    }

    #[tokio::test]
    async fn mock_blob_copy_not_found() {
        let store = MockBlobStorageAdapter::new();
        let src = test_key("plugin-a/docs/nope.txt");
        let dest = test_key("plugin-a/docs/copy.txt");
        let result = store.copy(src, dest).await;
        assert!(matches!(result.unwrap_err(), StorageError::NotFound { .. }));
    }

    #[tokio::test]
    async fn mock_blob_list_by_prefix() {
        let store = MockBlobStorageAdapter::new();

        for name in ["a.txt", "b.txt", "c.txt"] {
            let key = test_key(&format!("plugin-a/docs/{name}"));
            let input = BlobInput {
                data: vec![1],
                content_type: None,
                metadata: HashMap::new(),
            };
            store.store(key, input).await.unwrap();
        }

        // Also add one in a different prefix
        let other = test_key("plugin-b/docs/d.txt");
        let input = BlobInput {
            data: vec![2],
            content_type: None,
            metadata: HashMap::new(),
        };
        store.store(other, input).await.unwrap();

        let results = store.list("plugin-a/docs/").await.unwrap();
        assert_eq!(results.len(), 3);

        let results_b = store.list("plugin-b/").await.unwrap();
        assert_eq!(results_b.len(), 1);

        let empty = store.list("nonexistent/").await.unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn mock_blob_metadata_without_data() {
        let store = MockBlobStorageAdapter::new();
        let key = test_key("plugin-a/docs/file.bin");
        let input = BlobInput {
            data: vec![10, 20, 30],
            content_type: Some("application/octet-stream".to_string()),
            metadata: HashMap::from([("author".to_string(), "test".to_string())]),
        };
        store.store(key.clone(), input).await.unwrap();

        let meta = store.metadata(key).await.unwrap();
        assert_eq!(meta.size_bytes, 3);
        assert_eq!(meta.content_type, "application/octet-stream");
        assert_eq!(meta.metadata.get("author").unwrap(), "test");
    }

    #[tokio::test]
    async fn mock_blob_metadata_not_found() {
        let store = MockBlobStorageAdapter::new();
        let key = test_key("plugin-a/docs/nope.bin");
        let result = store.metadata(key).await;
        assert!(matches!(result.unwrap_err(), StorageError::NotFound { .. }));
    }

    #[tokio::test]
    async fn mock_blob_key_validation() {
        // Valid keys
        assert!(BlobKey::new("plugin-a/photos/image.png").is_ok());
        assert!(BlobKey::new("system/backup/db.tar.gz").is_ok());
        assert!(BlobKey::new("plugin-a/deep/nested/path/file.txt").is_ok());

        // Invalid keys
        assert!(BlobKey::new("file.txt").is_err()); // too few segments
        assert!(BlobKey::new("a/b").is_err()); // too few segments
        assert!(BlobKey::new("/a/b/c").is_err()); // leading slash
        assert!(BlobKey::new("a/../b/c").is_err()); // double dot
        assert!(BlobKey::new("a//b/c").is_err()); // empty segment
    }

    #[tokio::test]
    async fn mock_blob_health() {
        let store = MockBlobStorageAdapter::new();
        let report = store.health().await.unwrap();
        assert_eq!(report.status, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn mock_blob_capabilities() {
        let store = MockBlobStorageAdapter::new();
        let caps = store.capabilities();
        assert!(caps.server_side_copy);
        assert_eq!(caps.checksum_algorithms, vec!["sha256"]);
    }

    #[tokio::test]
    async fn mock_blob_content_type_detection() {
        let store = MockBlobStorageAdapter::new();
        let key = test_key("plugin-a/docs/photo.png");
        let input = BlobInput {
            data: vec![1, 2, 3],
            content_type: None, // should auto-detect
            metadata: HashMap::new(),
        };
        let meta = store.store(key, input).await.unwrap();
        assert_eq!(meta.content_type, "image/png");
    }
}
