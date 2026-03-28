//! Blob storage host functions for WASM plugins.
//!
//! These host functions allow plugins to store, retrieve, and delete binary
//! blobs. All keys are automatically prefixed with the calling plugin's ID
//! so plugins can only access their own blobs.

use std::sync::Arc;

use async_trait::async_trait;
use life_engine_traits::Capability;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::capability::ApprovedCapabilities;
use crate::error::PluginError;

/// Maximum blob size that a plugin can store (10 MiB).
const MAX_BLOB_SIZE: usize = 10 * 1024 * 1024;

/// Metadata about a stored blob.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobMeta {
    /// The blob's storage key.
    pub key: String,
    /// Size in bytes.
    pub size_bytes: u64,
    /// MIME content type.
    pub content_type: String,
}

/// Trait for blob storage backends used by host functions.
///
/// This is a simplified interface scoped to what the host function layer needs.
/// The full `BlobStorageAdapter` trait in the traits crate provides a richer API.
#[async_trait]
pub trait BlobBackend: Send + Sync {
    /// Store a blob, returning metadata.
    async fn store(
        &self,
        key: &str,
        data: Vec<u8>,
        content_type: Option<String>,
    ) -> Result<BlobMeta, String>;

    /// Retrieve a blob's data and metadata.
    async fn retrieve(&self, key: &str) -> Result<(Vec<u8>, BlobMeta), String>;

    /// Delete a blob by key.
    async fn delete(&self, key: &str) -> Result<(), String>;
}

/// Context passed to blob host functions, containing the plugin's identity,
/// approved capabilities, and a reference to the blob storage backend.
#[derive(Clone)]
pub struct BlobHostContext {
    /// The plugin ID making the blob call.
    pub plugin_id: String,
    /// The plugin's approved capabilities.
    pub capabilities: ApprovedCapabilities,
    /// Shared reference to the blob storage backend.
    pub blob_storage: Arc<dyn BlobBackend>,
}

/// Request payload for storing a blob.
#[derive(Debug, Deserialize, Serialize)]
pub struct BlobStoreRequest {
    /// The blob key (without plugin_id prefix — the host adds it).
    pub key: String,
    /// Base64-encoded blob data.
    pub data_base64: String,
    /// Optional MIME content type.
    pub content_type: Option<String>,
}

/// Request payload for retrieving a blob.
#[derive(Debug, Deserialize, Serialize)]
pub struct BlobRetrieveRequest {
    /// The blob key (without plugin_id prefix).
    pub key: String,
}

/// Response payload for a retrieved blob.
#[derive(Debug, Deserialize, Serialize)]
pub struct BlobRetrieveResponse {
    /// Base64-encoded blob data.
    pub data_base64: String,
    /// Size in bytes.
    pub size_bytes: u64,
    /// Content type.
    pub content_type: String,
}

/// Request payload for deleting a blob.
#[derive(Debug, Deserialize, Serialize)]
pub struct BlobDeleteRequest {
    /// The blob key (without plugin_id prefix).
    pub key: String,
}

/// Prefixes a user-provided key with the plugin ID for scoping.
fn scoped_key(plugin_id: &str, user_key: &str) -> String {
    format!("{plugin_id}/blobs/{user_key}")
}

/// Stores a blob on behalf of a plugin.
///
/// Automatically prefixes the key with the plugin's ID. Requires the
/// `storage:blob:write` capability.
pub async fn host_blob_store(
    ctx: &BlobHostContext,
    input: &[u8],
) -> Result<Vec<u8>, PluginError> {
    if !ctx.capabilities.has(Capability::StorageBlobWrite) {
        warn!(
            plugin_id = %ctx.plugin_id,
            "storage:blob:write capability violation"
        );
        return Err(PluginError::RuntimeCapabilityViolation(format!(
            "plugin '{}' lacks capability 'storage:blob:write'",
            ctx.plugin_id
        )));
    }

    let request: BlobStoreRequest = serde_json::from_slice(input).map_err(|e| {
        PluginError::ExecutionFailed(format!(
            "failed to deserialize blob store request from plugin '{}': {e}",
            ctx.plugin_id
        ))
    })?;

    let key = scoped_key(&ctx.plugin_id, &request.key);

    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD
        .decode(&request.data_base64)
        .map_err(|e| {
            PluginError::ExecutionFailed(format!(
                "invalid base64 data from plugin '{}': {e}",
                ctx.plugin_id
            ))
        })?;

    if data.len() > MAX_BLOB_SIZE {
        warn!(
            plugin_id = %ctx.plugin_id,
            key = %key,
            size = data.len(),
            max = MAX_BLOB_SIZE,
            "blob exceeds size limit"
        );
        return Err(PluginError::ExecutionFailed(format!(
            "blob from plugin '{}' exceeds maximum size ({} bytes > {} bytes)",
            ctx.plugin_id,
            data.len(),
            MAX_BLOB_SIZE,
        )));
    }

    debug!(
        plugin_id = %ctx.plugin_id,
        key = %key,
        size = data.len(),
        "storing blob"
    );

    let meta = ctx
        .blob_storage
        .store(&key, data, request.content_type)
        .await
        .map_err(|e| {
            PluginError::ExecutionFailed(format!(
                "blob store failed for plugin '{}': {e}",
                ctx.plugin_id
            ))
        })?;

    serde_json::to_vec(&meta).map_err(|e| {
        PluginError::ExecutionFailed(format!(
            "failed to serialize blob metadata for plugin '{}': {e}",
            ctx.plugin_id
        ))
    })
}

/// Retrieves a blob on behalf of a plugin.
///
/// Automatically prefixes the key with the plugin's ID. Requires the
/// `storage:blob:read` capability.
pub async fn host_blob_retrieve(
    ctx: &BlobHostContext,
    input: &[u8],
) -> Result<Vec<u8>, PluginError> {
    if !ctx.capabilities.has(Capability::StorageBlobRead) {
        warn!(
            plugin_id = %ctx.plugin_id,
            "storage:blob:read capability violation"
        );
        return Err(PluginError::RuntimeCapabilityViolation(format!(
            "plugin '{}' lacks capability 'storage:blob:read'",
            ctx.plugin_id
        )));
    }

    let request: BlobRetrieveRequest = serde_json::from_slice(input).map_err(|e| {
        PluginError::ExecutionFailed(format!(
            "failed to deserialize blob retrieve request from plugin '{}': {e}",
            ctx.plugin_id
        ))
    })?;

    let key = scoped_key(&ctx.plugin_id, &request.key);

    debug!(
        plugin_id = %ctx.plugin_id,
        key = %key,
        "retrieving blob"
    );

    let (data, meta) = ctx.blob_storage.retrieve(&key).await.map_err(|e| {
        PluginError::ExecutionFailed(format!(
            "blob retrieve failed for plugin '{}': {e}",
            ctx.plugin_id
        ))
    })?;

    use base64::Engine;
    let response = BlobRetrieveResponse {
        data_base64: base64::engine::general_purpose::STANDARD.encode(&data),
        size_bytes: meta.size_bytes,
        content_type: meta.content_type,
    };

    serde_json::to_vec(&response).map_err(|e| {
        PluginError::ExecutionFailed(format!(
            "failed to serialize blob response for plugin '{}': {e}",
            ctx.plugin_id
        ))
    })
}

/// Deletes a blob on behalf of a plugin.
///
/// Automatically prefixes the key with the plugin's ID. Requires the
/// `storage:blob:delete` capability.
pub async fn host_blob_delete(
    ctx: &BlobHostContext,
    input: &[u8],
) -> Result<Vec<u8>, PluginError> {
    if !ctx.capabilities.has(Capability::StorageBlobDelete) {
        warn!(
            plugin_id = %ctx.plugin_id,
            "storage:blob:delete capability violation"
        );
        return Err(PluginError::RuntimeCapabilityViolation(format!(
            "plugin '{}' lacks capability 'storage:blob:delete'",
            ctx.plugin_id
        )));
    }

    let request: BlobDeleteRequest = serde_json::from_slice(input).map_err(|e| {
        PluginError::ExecutionFailed(format!(
            "failed to deserialize blob delete request from plugin '{}': {e}",
            ctx.plugin_id
        ))
    })?;

    let key = scoped_key(&ctx.plugin_id, &request.key);

    debug!(
        plugin_id = %ctx.plugin_id,
        key = %key,
        "deleting blob"
    );

    ctx.blob_storage.delete(&key).await.map_err(|e| {
        PluginError::ExecutionFailed(format!(
            "blob delete failed for plugin '{}': {e}",
            ctx.plugin_id
        ))
    })?;

    Ok(b"{}".to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashSet;
    use std::sync::Mutex;

    // --- Mock blob storage ---

    struct MockBlobStorage {
        store_calls: Mutex<Vec<(String, Vec<u8>)>>,
        retrieve_result: Mutex<Option<(Vec<u8>, BlobMeta)>>,
        delete_calls: Mutex<Vec<String>>,
    }

    impl MockBlobStorage {
        fn new() -> Self {
            Self {
                store_calls: Mutex::new(vec![]),
                retrieve_result: Mutex::new(None),
                delete_calls: Mutex::new(vec![]),
            }
        }

        fn with_retrieve_result(data: Vec<u8>, meta: BlobMeta) -> Self {
            Self {
                store_calls: Mutex::new(vec![]),
                retrieve_result: Mutex::new(Some((data, meta))),
                delete_calls: Mutex::new(vec![]),
            }
        }
    }

    #[async_trait]
    impl BlobBackend for MockBlobStorage {
        async fn store(
            &self,
            key: &str,
            data: Vec<u8>,
            content_type: Option<String>,
        ) -> Result<BlobMeta, String> {
            self.store_calls
                .lock()
                .unwrap()
                .push((key.to_string(), data.clone()));
            Ok(BlobMeta {
                key: key.to_string(),
                size_bytes: data.len() as u64,
                content_type: content_type.unwrap_or_else(|| "application/octet-stream".into()),
            })
        }

        async fn retrieve(&self, key: &str) -> Result<(Vec<u8>, BlobMeta), String> {
            self.retrieve_result
                .lock()
                .unwrap()
                .clone()
                .ok_or_else(|| format!("blob not found: {key}"))
        }

        async fn delete(&self, key: &str) -> Result<(), String> {
            self.delete_calls
                .lock()
                .unwrap()
                .push(key.to_string());
            Ok(())
        }
    }

    // --- Helpers ---

    fn make_capabilities(caps: &[Capability]) -> ApprovedCapabilities {
        let set: HashSet<Capability> = caps.iter().copied().collect();
        ApprovedCapabilities::new(set)
    }

    fn make_context(
        plugin_id: &str,
        caps: &[Capability],
        blob_storage: Arc<dyn BlobBackend>,
    ) -> BlobHostContext {
        BlobHostContext {
            plugin_id: plugin_id.to_string(),
            capabilities: make_capabilities(caps),
            blob_storage,
        }
    }

    fn make_store_bytes(key: &str, data: &[u8]) -> Vec<u8> {
        use base64::Engine;
        serde_json::to_vec(&BlobStoreRequest {
            key: key.to_string(),
            data_base64: base64::engine::general_purpose::STANDARD.encode(data),
            content_type: Some("image/png".to_string()),
        })
        .unwrap()
    }

    fn make_retrieve_bytes(key: &str) -> Vec<u8> {
        serde_json::to_vec(&BlobRetrieveRequest {
            key: key.to_string(),
        })
        .unwrap()
    }

    fn make_delete_bytes(key: &str) -> Vec<u8> {
        serde_json::to_vec(&BlobDeleteRequest {
            key: key.to_string(),
        })
        .unwrap()
    }

    // --- Tests ---

    #[tokio::test]
    async fn store_succeeds_with_blob_write_capability() {
        let storage = Arc::new(MockBlobStorage::new());
        let ctx = make_context("test-plugin", &[Capability::StorageBlobWrite], storage.clone());

        let input = make_store_bytes("photos/image.png", b"fake-png-data");
        let result = host_blob_store(&ctx, &input).await;

        assert!(result.is_ok(), "blob store should succeed: {result:?}");

        let calls = storage.store_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "test-plugin/blobs/photos/image.png");
        assert_eq!(calls[0].1, b"fake-png-data");
    }

    #[tokio::test]
    async fn store_without_blob_write_returns_capability_error() {
        let storage = Arc::new(MockBlobStorage::new());
        let ctx = make_context("test-plugin", &[Capability::StorageBlobRead], storage);

        let input = make_store_bytes("photos/image.png", b"data");
        let result = host_blob_store(&ctx, &input).await;

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::RuntimeCapabilityViolation(_)));
        assert!(err.to_string().contains("storage:blob:write"));
    }

    #[tokio::test]
    async fn blob_key_auto_scoped_by_plugin_id() {
        let storage = Arc::new(MockBlobStorage::new());
        let ctx = make_context("my-plugin", &[Capability::StorageBlobWrite], storage.clone());

        let input = make_store_bytes("docs/file.pdf", b"pdf-data");
        let _ = host_blob_store(&ctx, &input).await;

        let calls = storage.store_calls.lock().unwrap();
        assert!(
            calls[0].0.starts_with("my-plugin/"),
            "key should be prefixed with plugin_id"
        );
    }

    #[tokio::test]
    async fn retrieve_succeeds_with_blob_read_capability() {
        let meta = BlobMeta {
            key: "test-plugin/blobs/photos/image.png".into(),
            size_bytes: 13,
            content_type: "image/png".into(),
        };
        let storage = Arc::new(MockBlobStorage::with_retrieve_result(
            b"fake-png-data".to_vec(),
            meta,
        ));
        let ctx = make_context("test-plugin", &[Capability::StorageBlobRead], storage);

        let input = make_retrieve_bytes("photos/image.png");
        let result = host_blob_retrieve(&ctx, &input).await;

        assert!(result.is_ok(), "blob retrieve should succeed: {result:?}");
        let response: BlobRetrieveResponse =
            serde_json::from_slice(&result.unwrap()).unwrap();
        assert_eq!(response.size_bytes, 13);
        assert_eq!(response.content_type, "image/png");
    }

    #[tokio::test]
    async fn retrieve_without_blob_read_returns_capability_error() {
        let storage = Arc::new(MockBlobStorage::new());
        let ctx = make_context("test-plugin", &[Capability::StorageBlobWrite], storage);

        let input = make_retrieve_bytes("photos/image.png");
        let result = host_blob_retrieve(&ctx, &input).await;

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::RuntimeCapabilityViolation(_)));
        assert!(err.to_string().contains("storage:blob:read"));
    }

    #[tokio::test]
    async fn delete_succeeds_with_blob_delete_capability() {
        let storage = Arc::new(MockBlobStorage::new());
        let ctx = make_context("test-plugin", &[Capability::StorageBlobDelete], storage.clone());

        let input = make_delete_bytes("photos/image.png");
        let result = host_blob_delete(&ctx, &input).await;

        assert!(result.is_ok(), "blob delete should succeed: {result:?}");

        let calls = storage.delete_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], "test-plugin/blobs/photos/image.png");
    }

    #[tokio::test]
    async fn delete_without_blob_delete_returns_capability_error() {
        let storage = Arc::new(MockBlobStorage::new());
        let ctx = make_context("test-plugin", &[Capability::StorageBlobRead], storage);

        let input = make_delete_bytes("photos/image.png");
        let result = host_blob_delete(&ctx, &input).await;

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::RuntimeCapabilityViolation(_)));
        assert!(err.to_string().contains("storage:blob:delete"));
    }

    #[tokio::test]
    async fn store_rejects_oversized_blob() {
        let storage = Arc::new(MockBlobStorage::new());
        let ctx = make_context("test-plugin", &[Capability::StorageBlobWrite], storage);

        // Create data exceeding MAX_BLOB_SIZE (10 MiB + 1 byte)
        let oversized = vec![0u8; MAX_BLOB_SIZE + 1];
        let input = make_store_bytes("huge-file.bin", &oversized);
        let result = host_blob_store(&ctx, &input).await;

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ExecutionFailed(_)));
        assert!(err.to_string().contains("exceeds maximum size"));
    }
}
