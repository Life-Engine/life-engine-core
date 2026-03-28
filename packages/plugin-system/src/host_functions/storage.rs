//! Storage read and write host functions for WASM plugins.
//!
//! These host functions allow plugins to interact with the storage layer.
//! Each function checks the plugin's approved capabilities before delegating
//! to the `StorageBackend` trait.

use std::sync::Arc;

use life_engine_traits::{Capability, StorageBackend};
use life_engine_types::{PipelineMessage, StorageMutation, StorageQuery};
use tracing::{debug, warn};

use crate::capability::ApprovedCapabilities;
use crate::error::PluginError;

/// Context passed to storage host functions, containing the plugin's identity,
/// approved capabilities, and a reference to the storage backend.
#[derive(Clone)]
pub struct StorageHostContext {
    /// The plugin ID making the storage call.
    pub plugin_id: String,
    /// The plugin's approved capabilities.
    pub capabilities: ApprovedCapabilities,
    /// Shared reference to the storage backend.
    pub storage: Arc<dyn StorageBackend>,
}

/// Executes a storage read operation on behalf of a plugin.
///
/// Deserializes the query from JSON bytes, checks the `StorageRead` capability,
/// scopes the query to the calling plugin's ID, delegates to the storage backend,
/// and serializes the results back to JSON bytes.
pub async fn host_storage_read(
    ctx: &StorageHostContext,
    input: &[u8],
) -> Result<Vec<u8>, PluginError> {
    // Check capability
    if !ctx.capabilities.has(Capability::StorageRead) {
        warn!(
            plugin_id = %ctx.plugin_id,
            "storage:doc:read capability violation"
        );
        return Err(PluginError::RuntimeCapabilityViolation(format!(
            "plugin '{}' lacks capability 'storage:doc:read'",
            ctx.plugin_id
        )));
    }

    // Deserialize the query from WASM input
    let mut query: StorageQuery = serde_json::from_slice(input).map_err(|e| {
        PluginError::ExecutionFailed(format!(
            "failed to deserialize storage query from plugin '{}': {e}",
            ctx.plugin_id
        ))
    })?;

    // Scope the query to the calling plugin's ID
    query.plugin_id = ctx.plugin_id.clone();

    debug!(
        plugin_id = %ctx.plugin_id,
        collection = %query.collection,
        "executing storage read"
    );

    // Delegate to the storage backend
    let results: Vec<PipelineMessage> = ctx.storage.execute(query).await.map_err(|e| {
        PluginError::ExecutionFailed(format!(
            "storage read failed for plugin '{}': {e}",
            ctx.plugin_id
        ))
    })?;

    // Serialize results back to JSON bytes
    serde_json::to_vec(&results).map_err(|e| {
        PluginError::ExecutionFailed(format!(
            "failed to serialize storage read results for plugin '{}': {e}",
            ctx.plugin_id
        ))
    })
}

/// Executes a storage write (mutation) operation on behalf of a plugin.
///
/// Deserializes the mutation from JSON bytes, checks the `StorageWrite` capability,
/// scopes the mutation to the calling plugin's ID, delegates to the storage backend,
/// and returns an empty success response.
pub async fn host_storage_write(
    ctx: &StorageHostContext,
    input: &[u8],
) -> Result<Vec<u8>, PluginError> {
    // Check capability
    if !ctx.capabilities.has(Capability::StorageWrite) {
        warn!(
            plugin_id = %ctx.plugin_id,
            "storage:doc:write capability violation"
        );
        return Err(PluginError::RuntimeCapabilityViolation(format!(
            "plugin '{}' lacks capability 'storage:doc:write'",
            ctx.plugin_id
        )));
    }

    // Deserialize the mutation from WASM input
    let mutation: StorageMutation = serde_json::from_slice(input).map_err(|e| {
        PluginError::ExecutionFailed(format!(
            "failed to deserialize storage mutation from plugin '{}': {e}",
            ctx.plugin_id
        ))
    })?;

    // Re-scope the mutation to the calling plugin's ID for safety
    let scoped_mutation = scope_mutation(mutation, &ctx.plugin_id);

    debug!(
        plugin_id = %ctx.plugin_id,
        mutation_type = mutation_type_name(&scoped_mutation),
        "executing storage write"
    );

    // Delegate to the storage backend
    ctx.storage.mutate(scoped_mutation).await.map_err(|e| {
        PluginError::ExecutionFailed(format!(
            "storage write failed for plugin '{}': {e}",
            ctx.plugin_id
        ))
    })?;

    // Return empty JSON object as success acknowledgement
    Ok(b"{}".to_vec())
}

/// Executes a storage delete operation on behalf of a plugin.
///
/// Deserializes the delete mutation from JSON bytes, checks the `StorageDelete`
/// capability, scopes the mutation to the calling plugin's ID, delegates to the
/// storage backend, and returns an empty success response.
pub async fn host_storage_delete(
    ctx: &StorageHostContext,
    input: &[u8],
) -> Result<Vec<u8>, PluginError> {
    // Check capability
    if !ctx.capabilities.has(Capability::StorageDelete) {
        warn!(
            plugin_id = %ctx.plugin_id,
            "storage:doc:delete capability violation"
        );
        return Err(PluginError::RuntimeCapabilityViolation(format!(
            "plugin '{}' lacks capability 'storage:doc:delete'",
            ctx.plugin_id
        )));
    }

    // Deserialize the mutation from WASM input
    let mutation: StorageMutation = serde_json::from_slice(input).map_err(|e| {
        PluginError::ExecutionFailed(format!(
            "failed to deserialize storage delete from plugin '{}': {e}",
            ctx.plugin_id
        ))
    })?;

    // Ensure the mutation is actually a Delete variant
    let scoped_mutation = match mutation {
        StorageMutation::Delete {
            collection, id, ..
        } => StorageMutation::Delete {
            plugin_id: ctx.plugin_id.clone(),
            collection,
            id,
        },
        _ => {
            return Err(PluginError::ExecutionFailed(format!(
                "host_storage_delete received non-delete mutation from plugin '{}'",
                ctx.plugin_id
            )));
        }
    };

    debug!(
        plugin_id = %ctx.plugin_id,
        "executing storage delete"
    );

    // Delegate to the storage backend
    ctx.storage.mutate(scoped_mutation).await.map_err(|e| {
        PluginError::ExecutionFailed(format!(
            "storage delete failed for plugin '{}': {e}",
            ctx.plugin_id
        ))
    })?;

    // Return empty JSON object as success acknowledgement
    Ok(b"{}".to_vec())
}

/// Replaces the `plugin_id` in a `StorageMutation` with the calling plugin's ID,
/// ensuring plugins cannot write data as another plugin.
fn scope_mutation(mutation: StorageMutation, plugin_id: &str) -> StorageMutation {
    match mutation {
        StorageMutation::Insert {
            collection, data, ..
        } => StorageMutation::Insert {
            plugin_id: plugin_id.to_string(),
            collection,
            data,
        },
        StorageMutation::Update {
            collection,
            id,
            data,
            expected_version,
            ..
        } => StorageMutation::Update {
            plugin_id: plugin_id.to_string(),
            collection,
            id,
            data,
            expected_version,
        },
        StorageMutation::Delete {
            collection, id, ..
        } => StorageMutation::Delete {
            plugin_id: plugin_id.to_string(),
            collection,
            id,
        },
    }
}

/// Returns a human-readable name for the mutation type (for logging).
fn mutation_type_name(mutation: &StorageMutation) -> &'static str {
    match mutation {
        StorageMutation::Insert { .. } => "insert",
        StorageMutation::Update { .. } => "update",
        StorageMutation::Delete { .. } => "delete",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashSet;
    use std::sync::Mutex;

    use async_trait::async_trait;
    use chrono::Utc;
    use life_engine_traits::EngineError;
    use life_engine_types::{
        CdmType, MessageMetadata, PipelineMessage, Task, TaskPriority, TaskStatus, TypedPayload,
    };
    use uuid::Uuid;

    // --- Mock storage backend ---

    struct MockStorage {
        /// Records of execute calls: (collection, plugin_id).
        execute_calls: Mutex<Vec<(String, String)>>,
        /// Records of mutate calls: (plugin_id, mutation type name).
        mutate_calls: Mutex<Vec<(String, String)>>,
        /// What to return from execute.
        execute_result: Vec<PipelineMessage>,
    }

    impl MockStorage {
        fn new(execute_result: Vec<PipelineMessage>) -> Self {
            Self {
                execute_calls: Mutex::new(vec![]),
                mutate_calls: Mutex::new(vec![]),
                execute_result,
            }
        }
    }

    #[async_trait]
    impl StorageBackend for MockStorage {
        async fn execute(
            &self,
            query: StorageQuery,
        ) -> Result<Vec<PipelineMessage>, Box<dyn EngineError>> {
            self.execute_calls
                .lock()
                .unwrap()
                .push((query.collection.clone(), query.plugin_id.clone()));
            Ok(self.execute_result.clone())
        }

        async fn mutate(&self, op: StorageMutation) -> Result<(), Box<dyn EngineError>> {
            let (plugin_id, op_type) = match &op {
                StorageMutation::Insert { plugin_id, .. } => {
                    (plugin_id.clone(), "insert".to_string())
                }
                StorageMutation::Update { plugin_id, .. } => {
                    (plugin_id.clone(), "update".to_string())
                }
                StorageMutation::Delete { plugin_id, .. } => {
                    (plugin_id.clone(), "delete".to_string())
                }
            };
            self.mutate_calls
                .lock()
                .unwrap()
                .push((plugin_id, op_type));
            Ok(())
        }

        async fn init(
            _config: toml::Value,
            _key: [u8; 32],
        ) -> Result<Self, Box<dyn EngineError>> {
            Ok(MockStorage::new(vec![]))
        }
    }

    // --- Helper functions ---

    fn make_capabilities(caps: &[Capability]) -> ApprovedCapabilities {
        let set: HashSet<Capability> = caps.iter().copied().collect();
        ApprovedCapabilities::new(set)
    }

    fn make_context(
        plugin_id: &str,
        caps: &[Capability],
        storage: Arc<dyn StorageBackend>,
    ) -> StorageHostContext {
        StorageHostContext {
            plugin_id: plugin_id.to_string(),
            capabilities: make_capabilities(caps),
            storage,
        }
    }

    fn sample_pipeline_message() -> PipelineMessage {
        PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: Uuid::new_v4(),
                source: "test".into(),
                timestamp: Utc::now(),
                auth_context: None,
                warnings: vec![],
            },
            payload: TypedPayload::Cdm(Box::new(CdmType::Task(Task {
                id: Uuid::new_v4(),
                title: "Test Task".into(),
                description: None,
                status: TaskStatus::Pending,
                priority: TaskPriority::Medium,
                due_date: None,
                completed_at: None,
                tags: vec![],
                assignee: None,
                parent_id: None,
                source: "test".into(),
                source_id: "t-1".into(),
                extensions: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }))),
        }
    }

    fn make_query_bytes(collection: &str) -> Vec<u8> {
        let query = StorageQuery {
            collection: collection.to_string(),
            plugin_id: "should-be-overwritten".to_string(),
            filters: vec![],
            sort: vec![],
            limit: None,
            offset: None,
        };
        serde_json::to_vec(&query).unwrap()
    }

    fn make_insert_bytes(collection: &str, msg: &PipelineMessage) -> Vec<u8> {
        let mutation = StorageMutation::Insert {
            plugin_id: "should-be-overwritten".to_string(),
            collection: collection.to_string(),
            data: msg.clone(),
        };
        serde_json::to_vec(&mutation).unwrap()
    }

    // --- Tests ---

    #[tokio::test]
    async fn read_succeeds_with_storage_read_capability() {
        let msg = sample_pipeline_message();
        let storage = Arc::new(MockStorage::new(vec![msg.clone()]));
        let ctx = make_context("test-plugin", &[Capability::StorageRead], storage.clone());

        let input = make_query_bytes("tasks");
        let result = host_storage_read(&ctx, &input).await;

        assert!(result.is_ok(), "read should succeed: {result:?}");

        let output: Vec<PipelineMessage> = serde_json::from_slice(&result.unwrap()).unwrap();
        assert_eq!(output.len(), 1);

        // Verify the storage backend was called with the correct plugin_id
        let calls = storage.execute_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "tasks");
        assert_eq!(calls[0].1, "test-plugin"); // scoped to calling plugin
    }

    #[tokio::test]
    async fn write_succeeds_with_storage_write_capability() {
        let storage = Arc::new(MockStorage::new(vec![]));
        let ctx = make_context("test-plugin", &[Capability::StorageWrite], storage.clone());

        let msg = sample_pipeline_message();
        let input = make_insert_bytes("tasks", &msg);
        let result = host_storage_write(&ctx, &input).await;

        assert!(result.is_ok(), "write should succeed: {result:?}");
        assert_eq!(result.unwrap(), b"{}");

        // Verify the storage backend was called with the correct plugin_id
        let calls = storage.mutate_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "test-plugin"); // scoped to calling plugin
        assert_eq!(calls[0].1, "insert");
    }

    #[tokio::test]
    async fn read_without_storage_read_returns_capability_error() {
        let storage = Arc::new(MockStorage::new(vec![]));
        // Plugin has storage:write but NOT storage:read
        let ctx = make_context("test-plugin", &[Capability::StorageWrite], storage);

        let input = make_query_bytes("tasks");
        let result = host_storage_read(&ctx, &input).await;

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::RuntimeCapabilityViolation(_)));
        assert!(err.to_string().contains("storage:doc:read"));
        assert!(err.to_string().contains("test-plugin"));
    }

    #[tokio::test]
    async fn write_without_storage_write_returns_capability_error() {
        let storage = Arc::new(MockStorage::new(vec![]));
        // Plugin has storage:read but NOT storage:write
        let ctx = make_context("test-plugin", &[Capability::StorageRead], storage);

        let msg = sample_pipeline_message();
        let input = make_insert_bytes("tasks", &msg);
        let result = host_storage_write(&ctx, &input).await;

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::RuntimeCapabilityViolation(_)));
        assert!(err.to_string().contains("storage:doc:write"));
        assert!(err.to_string().contains("test-plugin"));
    }

    #[tokio::test]
    async fn plugin_id_is_scoped_in_query() {
        let storage = Arc::new(MockStorage::new(vec![]));
        let ctx = make_context("my-plugin", &[Capability::StorageRead], storage.clone());

        // Send a query with a different plugin_id — it should be overwritten
        let query = StorageQuery {
            collection: "contacts".to_string(),
            plugin_id: "malicious-plugin".to_string(),
            filters: vec![],
            sort: vec![],
            limit: None,
            offset: None,
        };
        let input = serde_json::to_vec(&query).unwrap();

        let _ = host_storage_read(&ctx, &input).await;

        let calls = storage.execute_calls.lock().unwrap();
        assert_eq!(calls[0].1, "my-plugin"); // overwritten to the actual caller
    }

    #[tokio::test]
    async fn plugin_id_is_scoped_in_mutation() {
        let storage = Arc::new(MockStorage::new(vec![]));
        let ctx = make_context("my-plugin", &[Capability::StorageWrite], storage.clone());

        // Send a mutation with a different plugin_id — it should be overwritten
        let msg = sample_pipeline_message();
        let mutation = StorageMutation::Insert {
            plugin_id: "malicious-plugin".to_string(),
            collection: "tasks".to_string(),
            data: msg,
        };
        let input = serde_json::to_vec(&mutation).unwrap();

        let _ = host_storage_write(&ctx, &input).await;

        let calls = storage.mutate_calls.lock().unwrap();
        assert_eq!(calls[0].0, "my-plugin"); // overwritten to the actual caller
    }

    #[tokio::test]
    async fn read_results_are_correctly_serialized() {
        let msg1 = sample_pipeline_message();
        let msg2 = sample_pipeline_message();
        let storage = Arc::new(MockStorage::new(vec![msg1, msg2]));
        let ctx = make_context("test-plugin", &[Capability::StorageRead], storage);

        let input = make_query_bytes("tasks");
        let result = host_storage_read(&ctx, &input).await.unwrap();

        let output: Vec<PipelineMessage> = serde_json::from_slice(&result).unwrap();
        assert_eq!(output.len(), 2);
    }

    #[tokio::test]
    async fn invalid_query_input_returns_execution_error() {
        let storage = Arc::new(MockStorage::new(vec![]));
        let ctx = make_context("test-plugin", &[Capability::StorageRead], storage);

        let result = host_storage_read(&ctx, b"not valid json").await;

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ExecutionFailed(_)));
        assert!(err.to_string().contains("deserialize"));
    }

    #[tokio::test]
    async fn invalid_mutation_input_returns_execution_error() {
        let storage = Arc::new(MockStorage::new(vec![]));
        let ctx = make_context("test-plugin", &[Capability::StorageWrite], storage);

        let result = host_storage_write(&ctx, b"not valid json").await;

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ExecutionFailed(_)));
        assert!(err.to_string().contains("deserialize"));
    }
}
