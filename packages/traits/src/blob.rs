//! Blob storage adapter trait and supporting types.
//!
//! Defines the `BlobStorageAdapter` trait for binary object storage and
//! the types used by its API: `BlobKey`, `BlobInput`, `BlobMeta`, and
//! `BlobAdapterCapabilities`. Error and health types are imported from
//! [`crate::storage`].

use std::collections::HashMap;
use std::fmt;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::storage::{HealthReport, StorageError};

// ---------------------------------------------------------------------------
// BlobKey
// ---------------------------------------------------------------------------

/// A validated blob storage key following the `{plugin_id}/{context}/{filename}` format.
///
/// Rejects keys that contain `..`, start with `/`, have empty segments,
/// or have fewer than three segments.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlobKey(String);

impl BlobKey {
    /// Create a new `BlobKey`, validating format rules.
    pub fn new(key: impl Into<String>) -> Result<Self, StorageError> {
        let key = key.into();

        if key.starts_with('/') {
            return Err(StorageError::ValidationFailed {
                message: "blob key must not start with '/'".into(),
                field: Some("key".into()),
            });
        }

        if key.contains("..") {
            return Err(StorageError::ValidationFailed {
                message: "blob key must not contain '..'".into(),
                field: Some("key".into()),
            });
        }

        let segments: Vec<&str> = key.split('/').collect();

        if segments.len() < 3 {
            return Err(StorageError::ValidationFailed {
                message: "blob key must have at least 3 segments (plugin_id/context/filename)"
                    .into(),
                field: Some("key".into()),
            });
        }

        if segments.iter().any(|s| s.is_empty()) {
            return Err(StorageError::ValidationFailed {
                message: "blob key must not contain empty segments".into(),
                field: Some("key".into()),
            });
        }

        Ok(Self(key))
    }

    /// Return the key as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BlobKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for BlobKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// Blob data types
// ---------------------------------------------------------------------------

/// Input for a blob store operation.
#[derive(Debug, Clone)]
pub struct BlobInput {
    /// The raw blob bytes.
    pub data: Vec<u8>,
    /// Optional MIME content type (e.g. `image/png`).
    pub content_type: Option<String>,
    /// Arbitrary key-value metadata attached to the blob.
    pub metadata: HashMap<String, String>,
}

/// Metadata about a stored blob.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobMeta {
    /// The blob's storage key.
    pub key: String,
    /// Size of the blob data in bytes.
    pub size_bytes: u64,
    /// MIME content type.
    pub content_type: String,
    /// SHA-256 hex digest of the blob data.
    pub checksum: String,
    /// When the blob was first created.
    pub created_at: DateTime<Utc>,
    /// Arbitrary key-value metadata.
    pub metadata: HashMap<String, String>,
}

/// Capabilities reported by a blob storage adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobAdapterCapabilities {
    /// Whether the adapter supports streaming I/O.
    pub streaming: bool,
    /// Maximum blob size in bytes, if enforced.
    pub max_blob_size: Option<u64>,
    /// Supported checksum algorithms (e.g. `["sha256"]`).
    pub checksum_algorithms: Vec<String>,
    /// Whether the adapter supports encryption at rest.
    pub encryption: bool,
    /// Whether the adapter supports server-side copy.
    pub server_side_copy: bool,
}

impl Default for BlobAdapterCapabilities {
    fn default() -> Self {
        Self {
            streaming: false,
            max_blob_size: None,
            checksum_algorithms: vec!["sha256".to_string()],
            encryption: false,
            server_side_copy: false,
        }
    }
}

// ---------------------------------------------------------------------------
// BlobStorageAdapter trait
// ---------------------------------------------------------------------------

/// Async trait for blob storage backend implementations.
///
/// All adapters must be `Send + Sync` so they can be shared across async tasks.
#[async_trait]
pub trait BlobStorageAdapter: Send + Sync {
    /// Store a blob at the given key, returning its metadata.
    async fn store(&self, key: BlobKey, input: BlobInput) -> Result<BlobMeta, StorageError>;

    /// Retrieve a blob's data and metadata by key.
    async fn retrieve(&self, key: BlobKey) -> Result<(Vec<u8>, BlobMeta), StorageError>;

    /// Delete a blob by key.
    async fn delete(&self, key: BlobKey) -> Result<(), StorageError>;

    /// Check whether a blob exists at the given key.
    async fn exists(&self, key: BlobKey) -> Result<bool, StorageError>;

    /// Copy a blob from source to destination, returning the new metadata.
    async fn copy(&self, source: BlobKey, dest: BlobKey) -> Result<BlobMeta, StorageError>;

    /// List blobs whose keys start with the given prefix.
    async fn list(&self, prefix: &str) -> Result<Vec<BlobMeta>, StorageError>;

    /// Retrieve metadata for a blob without downloading the data.
    async fn metadata(&self, key: BlobKey) -> Result<BlobMeta, StorageError>;

    /// Report the adapter's current health status.
    async fn health(&self) -> Result<HealthReport, StorageError>;

    /// Report the adapter's capabilities (sync, no I/O).
    fn capabilities(&self) -> BlobAdapterCapabilities;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- BlobKey validation tests --

    #[test]
    fn blob_key_accepts_valid_key() {
        let key = BlobKey::new("plugin-a/photos/image.png");
        assert!(key.is_ok());
        assert_eq!(key.unwrap().as_str(), "plugin-a/photos/image.png");
    }

    #[test]
    fn blob_key_accepts_deeply_nested_key() {
        let key = BlobKey::new("plugin-a/photos/2026/03/image.png");
        assert!(key.is_ok());
    }

    #[test]
    fn blob_key_rejects_double_dot() {
        let key = BlobKey::new("plugin-a/../secrets/file.txt");
        assert!(key.is_err());
        let err = key.unwrap_err();
        assert!(matches!(err, StorageError::ValidationFailed { .. }));
    }

    #[test]
    fn blob_key_rejects_leading_slash() {
        let key = BlobKey::new("/plugin-a/photos/image.png");
        assert!(key.is_err());
        let err = key.unwrap_err();
        assert!(matches!(err, StorageError::ValidationFailed { .. }));
    }

    #[test]
    fn blob_key_rejects_empty_segments() {
        let key = BlobKey::new("plugin-a//file.txt");
        assert!(key.is_err());
        let err = key.unwrap_err();
        assert!(matches!(err, StorageError::ValidationFailed { .. }));
    }

    #[test]
    fn blob_key_rejects_fewer_than_three_segments() {
        let key = BlobKey::new("plugin-a/file.txt");
        assert!(key.is_err());
        let err = key.unwrap_err();
        assert!(matches!(err, StorageError::ValidationFailed { .. }));
    }

    #[test]
    fn blob_key_rejects_single_segment() {
        let key = BlobKey::new("file.txt");
        assert!(key.is_err());
    }

    // -- BlobInput tests --

    #[test]
    fn blob_input_construction() {
        let input = BlobInput {
            data: vec![1, 2, 3],
            content_type: Some("image/png".to_string()),
            metadata: HashMap::from([("author".to_string(), "test".to_string())]),
        };
        assert_eq!(input.data, vec![1, 2, 3]);
        assert_eq!(input.content_type, Some("image/png".to_string()));
        assert_eq!(input.metadata.get("author").unwrap(), "test");
    }

    // -- BlobMeta tests --

    #[test]
    fn blob_meta_fields() {
        let now = Utc::now();
        let meta = BlobMeta {
            key: "plugin-a/photos/image.png".to_string(),
            size_bytes: 1024,
            content_type: "image/png".to_string(),
            checksum: "abc123".to_string(),
            created_at: now,
            metadata: HashMap::new(),
        };
        assert_eq!(meta.key, "plugin-a/photos/image.png");
        assert_eq!(meta.size_bytes, 1024);
        assert_eq!(meta.content_type, "image/png");
        assert_eq!(meta.checksum, "abc123");
        assert_eq!(meta.created_at, now);
        assert!(meta.metadata.is_empty());
    }

    // -- BlobAdapterCapabilities tests --

    #[test]
    fn blob_adapter_capabilities_defaults() {
        let caps = BlobAdapterCapabilities::default();
        assert!(!caps.streaming);
        assert_eq!(caps.max_blob_size, None);
        assert_eq!(caps.checksum_algorithms, vec!["sha256".to_string()]);
        assert!(!caps.encryption);
        assert!(!caps.server_side_copy);
    }

    // -- Trait object compilation test --

    #[test]
    fn blob_storage_adapter_trait_is_object_safe() {
        fn _assert_object_safe(_: &dyn BlobStorageAdapter) {}
        fn _assert_boxed(_: Box<dyn BlobStorageAdapter>) {}
    }
}
