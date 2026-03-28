//! Storage context — enforcement layer wrapping `StorageRouter`.
//!
//! `StorageContext` enforces capability-based permissions, collection scoping,
//! schema validation, system field management, extension namespace isolation,
//! and audit event emission before delegating to the underlying `StorageRouter`.

use std::collections::HashSet;
use std::sync::Arc;

use chrono::Utc;
use serde_json::Value;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::blob::{BlobInput, BlobKey, BlobMeta};
use crate::schema::SchemaRegistry;
use crate::storage::{DocumentList, FilterNode, QueryDescriptor, StorageError};
use crate::storage_router::StorageRouter;

/// Fine-grained storage capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageCapability {
    /// Read documents.
    DocRead,
    /// Write (create/update) documents.
    DocWrite,
    /// Delete documents.
    DocDelete,
    /// Read blobs.
    BlobRead,
    /// Write (store/copy) blobs.
    BlobWrite,
    /// Delete blobs.
    BlobDelete,
}

impl StorageCapability {
    /// Parse from the `storage:*` capability string format.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "storage:doc:read" => Some(Self::DocRead),
            "storage:doc:write" => Some(Self::DocWrite),
            "storage:doc:delete" => Some(Self::DocDelete),
            "storage:blob:read" => Some(Self::BlobRead),
            "storage:blob:write" => Some(Self::BlobWrite),
            "storage:blob:delete" => Some(Self::BlobDelete),
            _ => None,
        }
    }

    /// Return the string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DocRead => "storage:doc:read",
            Self::DocWrite => "storage:doc:write",
            Self::DocDelete => "storage:doc:delete",
            Self::BlobRead => "storage:blob:read",
            Self::BlobWrite => "storage:blob:write",
            Self::BlobDelete => "storage:blob:delete",
        }
    }
}

/// Identifies the caller of a storage operation.
#[derive(Debug, Clone)]
pub enum CallerIdentity {
    /// A plugin with an ID and a set of granted capabilities.
    Plugin {
        /// Plugin identifier.
        plugin_id: String,
        /// Granted storage capabilities.
        capabilities: HashSet<StorageCapability>,
        /// Shared collections the plugin has declared access to, with access level.
        shared_collections: Vec<CollectionAccess>,
    },
    /// The system / workflow engine — bypasses capability checks.
    System,
}

/// Declares a plugin's access to a shared collection.
#[derive(Debug, Clone)]
pub struct CollectionAccess {
    /// Collection name.
    pub collection: String,
    /// Access level: "read", "write", or "read-write".
    pub access: String,
}

/// An audit event emitted by StorageContext on write operations.
#[derive(Debug, Clone)]
pub struct AuditEvent {
    /// Event type, e.g. "system.storage.created".
    pub event_type: String,
    /// The originating plugin ID, or "system".
    pub origin: String,
    /// Event payload (collection, id, changed_fields, key).
    pub payload: Value,
}

/// Enforcement layer wrapping `StorageRouter`.
pub struct StorageContext {
    router: Arc<StorageRouter>,
    schema_registry: Arc<SchemaRegistry>,
    audit_tx: mpsc::UnboundedSender<AuditEvent>,
}

impl StorageContext {
    /// Create a new `StorageContext`.
    pub fn new(
        router: Arc<StorageRouter>,
        schema_registry: Arc<SchemaRegistry>,
        audit_tx: mpsc::UnboundedSender<AuditEvent>,
    ) -> Self {
        Self {
            router,
            schema_registry,
            audit_tx,
        }
    }

    /// Check that the caller has the required capability.
    fn check_capability(
        caller: &CallerIdentity,
        required: StorageCapability,
    ) -> Result<(), StorageError> {
        match caller {
            CallerIdentity::System => Ok(()),
            CallerIdentity::Plugin { capabilities, plugin_id, .. } => {
                if capabilities.contains(&required) {
                    Ok(())
                } else {
                    Err(StorageError::PermissionDenied {
                        message: format!(
                            "plugin '{}' lacks capability '{}'",
                            plugin_id,
                            required.as_str()
                        ),
                    })
                }
            }
        }
    }

    /// Resolve the effective collection name, applying plugin-scoped prefixing.
    fn resolve_collection(
        caller: &CallerIdentity,
        collection: &str,
        is_plugin_scoped: bool,
    ) -> Result<String, StorageError> {
        match caller {
            CallerIdentity::System => Ok(collection.to_string()),
            CallerIdentity::Plugin { plugin_id, shared_collections, .. } => {
                if is_plugin_scoped {
                    Ok(format!("{}.{}", plugin_id, collection))
                } else {
                    // Verify the plugin has declared this shared collection.
                    let has_access = shared_collections
                        .iter()
                        .any(|ca| ca.collection == collection);
                    if !has_access {
                        return Err(StorageError::PermissionDenied {
                            message: format!(
                                "plugin '{}' has not declared access to shared collection '{}'",
                                plugin_id, collection
                            ),
                        });
                    }
                    Ok(collection.to_string())
                }
            }
        }
    }

    /// Return the origin identifier for audit events.
    fn origin(caller: &CallerIdentity) -> String {
        match caller {
            CallerIdentity::System => "system".to_string(),
            CallerIdentity::Plugin { plugin_id, .. } => plugin_id.clone(),
        }
    }

    /// Emit an audit event (best-effort, does not fail the operation).
    fn emit_audit(&self, event: AuditEvent) {
        let _ = self.audit_tx.send(event);
    }

    /// Inject/manage system fields on a document for create operations.
    fn inject_system_fields_create(doc: &mut Value) {
        let now = Utc::now().to_rfc3339();
        if let Some(obj) = doc.as_object_mut() {
            // Generate id if missing.
            if !obj.contains_key("id") {
                obj.insert("id".to_string(), Value::String(Uuid::new_v4().to_string()));
            }
            // Always set created_at and updated_at.
            obj.insert("created_at".to_string(), Value::String(now.clone()));
            obj.insert("updated_at".to_string(), Value::String(now));
        }
    }

    /// Inject/manage system fields on a document for update operations.
    fn inject_system_fields_update(doc: &mut Value) {
        let now = Utc::now().to_rfc3339();
        if let Some(obj) = doc.as_object_mut() {
            obj.insert("updated_at".to_string(), Value::String(now));
        }
    }

    /// Check extension namespace isolation: plugin can only write `ext.{own_plugin_id}`.
    fn check_extension_namespace(
        caller: &CallerIdentity,
        doc: &Value,
    ) -> Result<(), StorageError> {
        let plugin_id = match caller {
            CallerIdentity::System => return Ok(()),
            CallerIdentity::Plugin { plugin_id, .. } => plugin_id,
        };

        if let Some(ext) = doc.as_object().and_then(|o| o.get("ext")).and_then(|v| v.as_object()) {
            for ns in ext.keys() {
                if ns != plugin_id {
                    return Err(StorageError::PermissionDenied {
                        message: format!(
                            "plugin '{}' cannot write to extension namespace 'ext.{}'",
                            plugin_id, ns
                        ),
                    });
                }
            }
        }
        Ok(())
    }

    // -- Document read operations ---------------------------------------------

    /// Retrieve a single document by collection and ID.
    pub async fn doc_get(
        &self,
        caller: &CallerIdentity,
        collection: &str,
        id: &str,
        plugin_scoped: bool,
    ) -> Result<Value, StorageError> {
        Self::check_capability(caller, StorageCapability::DocRead)?;
        let resolved = Self::resolve_collection(caller, collection, plugin_scoped)?;
        self.router.doc_get(&resolved, id).await
    }

    /// Execute a query and return matching documents.
    pub async fn doc_query(
        &self,
        caller: &CallerIdentity,
        mut descriptor: QueryDescriptor,
        plugin_scoped: bool,
    ) -> Result<DocumentList, StorageError> {
        Self::check_capability(caller, StorageCapability::DocRead)?;
        descriptor.collection =
            Self::resolve_collection(caller, &descriptor.collection, plugin_scoped)?;
        self.router.doc_query(descriptor).await
    }

    /// Count documents matching optional filters.
    pub async fn doc_count(
        &self,
        caller: &CallerIdentity,
        collection: &str,
        filters: Option<FilterNode>,
        plugin_scoped: bool,
    ) -> Result<u64, StorageError> {
        Self::check_capability(caller, StorageCapability::DocRead)?;
        let resolved = Self::resolve_collection(caller, collection, plugin_scoped)?;
        self.router.doc_count(&resolved, filters).await
    }

    // -- Document write operations --------------------------------------------

    /// Create a new document.
    pub async fn doc_create(
        &self,
        caller: &CallerIdentity,
        collection: &str,
        mut document: Value,
        plugin_scoped: bool,
    ) -> Result<Value, StorageError> {
        Self::check_capability(caller, StorageCapability::DocWrite)?;
        let resolved = Self::resolve_collection(caller, collection, plugin_scoped)?;
        Self::check_extension_namespace(caller, &document)?;

        // Schema validation (for plugins). Use original collection name for
        // schema lookup since schemas are registered by unqualified name.
        if let CallerIdentity::Plugin { plugin_id, .. } = caller
            && self.schema_registry.get_collection(plugin_id, collection).is_some()
        {
            self.schema_registry
                .validate_write(collection, plugin_id, &document, true, None)
                .map_err(|e| StorageError::ValidationFailed {
                    message: e.to_string(),
                    field: None,
                })?;
        }

        Self::inject_system_fields_create(&mut document);

        let id = document
            .as_object()
            .and_then(|o| o.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let result = self.router.doc_create(&resolved, document).await?;

        self.emit_audit(AuditEvent {
            event_type: "system.storage.created".to_string(),
            origin: Self::origin(caller),
            payload: serde_json::json!({ "collection": resolved, "id": id }),
        });

        Ok(result)
    }

    /// Replace an existing document (full update).
    pub async fn doc_update(
        &self,
        caller: &CallerIdentity,
        collection: &str,
        id: &str,
        mut document: Value,
        plugin_scoped: bool,
    ) -> Result<Value, StorageError> {
        Self::check_capability(caller, StorageCapability::DocWrite)?;
        let resolved = Self::resolve_collection(caller, collection, plugin_scoped)?;
        Self::check_extension_namespace(caller, &document)?;

        // Schema validation (for plugins). Use original collection name for
        // schema lookup since schemas are registered by unqualified name.
        if let CallerIdentity::Plugin { plugin_id, .. } = caller
            && self.schema_registry.get_collection(plugin_id, collection).is_some()
        {
            let existing = self.router.doc_get(&resolved, id).await.ok();
            self.schema_registry
                .validate_write(
                    collection,
                    plugin_id,
                    &document,
                    false,
                    existing.as_ref(),
                )
                .map_err(|e| StorageError::ValidationFailed {
                    message: e.to_string(),
                    field: None,
                })?;
        }

        Self::inject_system_fields_update(&mut document);

        let result = self.router.doc_update(&resolved, id, document).await?;

        self.emit_audit(AuditEvent {
            event_type: "system.storage.updated".to_string(),
            origin: Self::origin(caller),
            payload: serde_json::json!({ "collection": resolved, "id": id }),
        });

        Ok(result)
    }

    /// Delete a document by collection and ID.
    pub async fn doc_delete(
        &self,
        caller: &CallerIdentity,
        collection: &str,
        id: &str,
        plugin_scoped: bool,
    ) -> Result<(), StorageError> {
        Self::check_capability(caller, StorageCapability::DocDelete)?;
        let resolved = Self::resolve_collection(caller, collection, plugin_scoped)?;

        self.router.doc_delete(&resolved, id).await?;

        self.emit_audit(AuditEvent {
            event_type: "system.storage.deleted".to_string(),
            origin: Self::origin(caller),
            payload: serde_json::json!({ "collection": resolved, "id": id }),
        });

        Ok(())
    }

    // -- Blob read operations -------------------------------------------------

    /// Retrieve a blob's data and metadata by key.
    pub async fn blob_retrieve(
        &self,
        caller: &CallerIdentity,
        key: BlobKey,
    ) -> Result<(Vec<u8>, BlobMeta), StorageError> {
        Self::check_capability(caller, StorageCapability::BlobRead)?;
        self.router.blob_retrieve(key).await
    }

    /// Check whether a blob exists at the given key.
    pub async fn blob_exists(
        &self,
        caller: &CallerIdentity,
        key: BlobKey,
    ) -> Result<bool, StorageError> {
        Self::check_capability(caller, StorageCapability::BlobRead)?;
        self.router.blob_exists(key).await
    }

    /// List blobs whose keys start with the given prefix.
    pub async fn blob_list(
        &self,
        caller: &CallerIdentity,
        prefix: &str,
    ) -> Result<Vec<BlobMeta>, StorageError> {
        Self::check_capability(caller, StorageCapability::BlobRead)?;
        self.router.blob_list(prefix).await
    }

    /// Retrieve metadata for a blob without downloading the data.
    pub async fn blob_metadata(
        &self,
        caller: &CallerIdentity,
        key: BlobKey,
    ) -> Result<BlobMeta, StorageError> {
        Self::check_capability(caller, StorageCapability::BlobRead)?;
        self.router.blob_metadata(key).await
    }

    // -- Blob write operations ------------------------------------------------

    /// Store a blob at the given key, returning its metadata.
    pub async fn blob_store(
        &self,
        caller: &CallerIdentity,
        key: BlobKey,
        input: BlobInput,
    ) -> Result<BlobMeta, StorageError> {
        Self::check_capability(caller, StorageCapability::BlobWrite)?;

        let result = self.router.blob_store(key.clone(), input).await?;

        self.emit_audit(AuditEvent {
            event_type: "system.blob.stored".to_string(),
            origin: Self::origin(caller),
            payload: serde_json::json!({ "key": key.as_str() }),
        });

        Ok(result)
    }

    /// Delete a blob by key.
    pub async fn blob_delete(
        &self,
        caller: &CallerIdentity,
        key: BlobKey,
    ) -> Result<(), StorageError> {
        Self::check_capability(caller, StorageCapability::BlobDelete)?;

        self.router.blob_delete(key.clone()).await?;

        self.emit_audit(AuditEvent {
            event_type: "system.blob.deleted".to_string(),
            origin: Self::origin(caller),
            payload: serde_json::json!({ "key": key.as_str() }),
        });

        Ok(())
    }

    // -- Watch ----------------------------------------------------------------

    /// Subscribe to changes on a collection, bridging adapter-level change
    /// events into the audit event bus.
    ///
    /// Returns a receiver of `ChangeEvent`s for the caller to consume. Each
    /// change is also re-emitted as an `AuditEvent` on the audit channel so
    /// that the unified event model stays in sync.
    pub async fn doc_watch(
        &self,
        caller: &CallerIdentity,
        collection: &str,
        plugin_scoped: bool,
    ) -> Result<mpsc::Receiver<crate::storage::ChangeEvent>, StorageError> {
        Self::check_capability(caller, StorageCapability::DocRead)?;
        let resolved = Self::resolve_collection(caller, collection, plugin_scoped)?;

        let mut adapter_rx = self.router.doc_watch(&resolved).await?;

        // Bridge: forward change events to both a new consumer channel and the
        // audit bus.
        let (bridge_tx, bridge_rx) = mpsc::channel(64);
        let audit_tx = self.audit_tx.clone();
        let origin = Self::origin(caller);

        tokio::spawn(async move {
            while let Some(event) = adapter_rx.recv().await {
                let event_type = match event.change_type {
                    crate::storage::ChangeType::Created => "system.storage.created",
                    crate::storage::ChangeType::Updated => "system.storage.updated",
                    crate::storage::ChangeType::Deleted => "system.storage.deleted",
                };
                let _ = audit_tx.send(AuditEvent {
                    event_type: event_type.to_string(),
                    origin: origin.clone(),
                    payload: serde_json::json!({
                        "collection": event.collection,
                        "id": event.document_id,
                        "source": "watch",
                    }),
                });
                if bridge_tx.send(event).await.is_err() {
                    break;
                }
            }
        });

        Ok(bridge_rx)
    }

    // -- Health ---------------------------------------------------------------

    /// Delegate health check to the router.
    pub async fn health(
        &self,
    ) -> Result<crate::storage::HealthReport, StorageError> {
        self.router.health().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;

    use async_trait::async_trait;
    use serde_json::json;

    use crate::blob::{BlobAdapterCapabilities, BlobStorageAdapter};
    use crate::storage::{
        AdapterCapabilities, ChangeEvent, CollectionDescriptor, DocumentStorageAdapter,
        HealthCheck, HealthReport, HealthStatus,
    };
    use crate::storage_router::TimeoutConfig;

    // -- Inline mock adapters for self-contained tests -------------------------

    struct InlineDocAdapter {
        data: tokio::sync::RwLock<HashMap<String, HashMap<String, Value>>>,
    }

    impl InlineDocAdapter {
        fn new() -> Self {
            Self {
                data: tokio::sync::RwLock::new(HashMap::new()),
            }
        }
    }

    #[async_trait]
    impl DocumentStorageAdapter for InlineDocAdapter {
        async fn get(&self, collection: &str, id: &str) -> Result<Value, StorageError> {
            let data = self.data.read().await;
            data.get(collection)
                .and_then(|c| c.get(id))
                .cloned()
                .ok_or_else(|| StorageError::NotFound {
                    collection: collection.into(),
                    id: id.into(),
                })
        }
        async fn create(&self, collection: &str, doc: Value) -> Result<Value, StorageError> {
            let mut data = self.data.write().await;
            let coll = data.entry(collection.to_string()).or_default();
            let id = doc
                .as_object()
                .and_then(|o| o.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            coll.insert(id, doc.clone());
            Ok(doc)
        }
        async fn update(&self, collection: &str, id: &str, doc: Value) -> Result<Value, StorageError> {
            let mut data = self.data.write().await;
            let coll = data.entry(collection.to_string()).or_default();
            coll.insert(id.to_string(), doc.clone());
            Ok(doc)
        }
        async fn partial_update(&self, _c: &str, _id: &str, f: Value) -> Result<Value, StorageError> {
            Ok(f)
        }
        async fn delete(&self, collection: &str, id: &str) -> Result<(), StorageError> {
            let mut data = self.data.write().await;
            let coll = data.get_mut(collection).ok_or_else(|| StorageError::NotFound {
                collection: collection.into(),
                id: id.into(),
            })?;
            coll.remove(id).ok_or_else(|| StorageError::NotFound {
                collection: collection.into(),
                id: id.into(),
            })?;
            Ok(())
        }
        async fn query(&self, d: QueryDescriptor) -> Result<DocumentList, StorageError> {
            let data = self.data.read().await;
            let docs: Vec<Value> = data
                .get(&d.collection)
                .map(|c| c.values().cloned().collect())
                .unwrap_or_default();
            Ok(DocumentList {
                total_count: docs.len() as u64,
                documents: docs,
                next_cursor: None,
            })
        }
        async fn count(&self, collection: &str, _f: Option<FilterNode>) -> Result<u64, StorageError> {
            let data = self.data.read().await;
            Ok(data.get(collection).map(|c| c.len() as u64).unwrap_or(0))
        }
        async fn batch_create(&self, _c: &str, d: Vec<Value>) -> Result<Vec<Value>, StorageError> {
            Ok(d)
        }
        async fn batch_update(&self, _c: &str, _u: Vec<(String, Value)>) -> Result<Vec<Value>, StorageError> {
            Ok(vec![])
        }
        async fn batch_delete(&self, _c: &str, _ids: Vec<String>) -> Result<(), StorageError> {
            Ok(())
        }
        async fn watch(&self, _c: &str) -> Result<mpsc::Receiver<ChangeEvent>, StorageError> {
            let (_tx, rx) = mpsc::channel(1);
            Ok(rx)
        }
        async fn migrate(&self, _d: CollectionDescriptor) -> Result<(), StorageError> {
            Ok(())
        }
        async fn health(&self) -> Result<HealthReport, StorageError> {
            Ok(HealthReport {
                status: HealthStatus::Healthy,
                message: None,
                checks: vec![HealthCheck {
                    name: "inline".into(),
                    status: HealthStatus::Healthy,
                    message: None,
                }],
            })
        }
        fn capabilities(&self) -> AdapterCapabilities {
            AdapterCapabilities {
                indexing: false,
                transactions: false,
                full_text_search: false,
                watch: false,
                batch_operations: false,
                encryption: false,
            }
        }
    }

    struct InlineBlobAdapter;

    #[async_trait]
    impl BlobStorageAdapter for InlineBlobAdapter {
        async fn store(&self, key: BlobKey, input: BlobInput) -> Result<BlobMeta, StorageError> {
            Ok(BlobMeta {
                key: key.as_str().to_string(),
                size_bytes: input.data.len() as u64,
                content_type: input.content_type.unwrap_or_else(|| "application/octet-stream".into()),
                checksum: "abc".into(),
                created_at: Utc::now(),
                metadata: input.metadata,
            })
        }
        async fn retrieve(&self, _k: BlobKey) -> Result<(Vec<u8>, BlobMeta), StorageError> {
            Err(StorageError::NotFound {
                collection: "blob".into(),
                id: "n/a".into(),
            })
        }
        async fn delete(&self, _k: BlobKey) -> Result<(), StorageError> {
            Ok(())
        }
        async fn exists(&self, _k: BlobKey) -> Result<bool, StorageError> {
            Ok(false)
        }
        async fn copy(&self, _s: BlobKey, _d: BlobKey) -> Result<BlobMeta, StorageError> {
            unreachable!()
        }
        async fn list(&self, _p: &str) -> Result<Vec<BlobMeta>, StorageError> {
            Ok(vec![])
        }
        async fn metadata(&self, _k: BlobKey) -> Result<BlobMeta, StorageError> {
            unreachable!()
        }
        async fn health(&self) -> Result<HealthReport, StorageError> {
            Ok(HealthReport {
                status: HealthStatus::Healthy,
                message: None,
                checks: vec![],
            })
        }
        fn capabilities(&self) -> BlobAdapterCapabilities {
            BlobAdapterCapabilities::default()
        }
    }

    // -- Test helpers ---------------------------------------------------------

    fn make_context() -> (StorageContext, mpsc::UnboundedReceiver<AuditEvent>) {
        let router = Arc::new(StorageRouter::new(
            Arc::new(InlineDocAdapter::new()),
            Arc::new(InlineBlobAdapter),
            TimeoutConfig::default(),
        ));
        let registry = Arc::new(SchemaRegistry::new());
        let (tx, rx) = mpsc::unbounded_channel();
        (StorageContext::new(router, registry, tx), rx)
    }

    fn plugin_caller(caps: &[StorageCapability]) -> CallerIdentity {
        CallerIdentity::Plugin {
            plugin_id: "com.test.plugin".to_string(),
            capabilities: caps.iter().copied().collect(),
            shared_collections: vec![
                CollectionAccess {
                    collection: "tasks".to_string(),
                    access: "read-write".to_string(),
                },
            ],
        }
    }

    fn system_caller() -> CallerIdentity {
        CallerIdentity::System
    }

    // =========================================================================
    // Test 4: Capability check passes
    // =========================================================================

    #[tokio::test]
    async fn capability_check_passes_with_correct_capability() {
        let (ctx, _rx) = make_context();
        let caller = plugin_caller(&[StorageCapability::DocRead, StorageCapability::DocWrite]);

        // doc_get should succeed because the caller has DocRead.
        // (NotFound is fine — we just want to verify no PermissionDenied.)
        let result = ctx.doc_get(&caller, "tasks", "1", false).await;
        assert!(
            matches!(result, Err(StorageError::NotFound { .. })) || result.is_ok(),
            "expected NotFound or Ok, got: {result:?}"
        );
    }

    // =========================================================================
    // Test 5: Capability denied -> PermissionDenied
    // =========================================================================

    #[tokio::test]
    async fn capability_denied_returns_permission_denied() {
        let (ctx, _rx) = make_context();
        // Plugin with NO capabilities.
        let caller = plugin_caller(&[]);

        let result = ctx.doc_get(&caller, "tasks", "1", false).await;
        assert!(matches!(result, Err(StorageError::PermissionDenied { .. })));
    }

    #[tokio::test]
    async fn capability_denied_for_blob_write_without_grant() {
        let (ctx, _rx) = make_context();
        let caller = plugin_caller(&[StorageCapability::BlobRead]);

        let key = BlobKey::new("com.test.plugin/docs/file.txt").unwrap();
        let input = BlobInput {
            data: vec![1, 2, 3],
            content_type: None,
            metadata: HashMap::new(),
        };
        let result = ctx.blob_store(&caller, key, input).await;
        assert!(matches!(result, Err(StorageError::PermissionDenied { .. })));
    }

    #[tokio::test]
    async fn system_caller_bypasses_capability_checks() {
        let (ctx, _rx) = make_context();
        let caller = system_caller();

        // System caller has no explicit capabilities but should bypass checks.
        let result = ctx.doc_get(&caller, "tasks", "1", false).await;
        // Should not be PermissionDenied.
        assert!(!matches!(result, Err(StorageError::PermissionDenied { .. })));
    }

    // =========================================================================
    // Test 6: Collection scoping with plugin prefix
    // =========================================================================

    #[tokio::test]
    async fn plugin_scoped_collection_gets_prefix() {
        let (ctx, _rx) = make_context();
        let caller = plugin_caller(&[StorageCapability::DocWrite]);

        // Create in plugin-scoped collection — should namespace as "com.test.plugin.items".
        let doc = json!({"name": "test"});
        let result = ctx.doc_create(&caller, "items", doc, true).await;
        assert!(result.is_ok());

        // Now query the prefixed collection directly through the router.
        // We use the system caller to bypass capability checks.
        let system = system_caller();
        let count = ctx.doc_count(&system, "com.test.plugin.items", None, false).await;
        assert_eq!(count.unwrap(), 1);
    }

    #[tokio::test]
    async fn shared_collection_requires_declaration() {
        let (ctx, _rx) = make_context();
        let caller = plugin_caller(&[StorageCapability::DocRead]);

        // "tasks" is declared, so this should work (or return NotFound).
        let result = ctx.doc_get(&caller, "tasks", "1", false).await;
        assert!(!matches!(result, Err(StorageError::PermissionDenied { .. })));

        // "undeclared_collection" is NOT declared.
        let result = ctx.doc_get(&caller, "undeclared_collection", "1", false).await;
        assert!(matches!(result, Err(StorageError::PermissionDenied { .. })));
    }

    // =========================================================================
    // Test 7: Schema validation on write
    // =========================================================================

    #[tokio::test]
    async fn schema_validation_on_write() {
        let router = Arc::new(StorageRouter::new(
            Arc::new(InlineDocAdapter::new()),
            Arc::new(InlineBlobAdapter),
            TimeoutConfig::default(),
        ));

        let mut registry = SchemaRegistry::new();
        let schema = r#"{
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": { "type": "string" }
            }
        }"#;
        registry
            .register_plugin_collection("com.test.plugin", "items", Some(schema), None, false, vec![])
            .unwrap();

        let (tx, _rx) = mpsc::unbounded_channel();
        let ctx = StorageContext::new(router, Arc::new(registry), tx);

        let caller = CallerIdentity::Plugin {
            plugin_id: "com.test.plugin".to_string(),
            capabilities: [StorageCapability::DocWrite].into_iter().collect(),
            shared_collections: vec![],
        };

        // Valid document (plugin-scoped, so no shared collection check needed).
        let valid = json!({"name": "Alice"});
        let result = ctx.doc_create(&caller, "items", valid, true).await;
        assert!(result.is_ok());

        // Invalid document (missing required field "name").
        let invalid = json!({"age": 30});
        let result = ctx.doc_create(&caller, "items", invalid, true).await;
        assert!(matches!(result, Err(StorageError::ValidationFailed { .. })));
    }

    // =========================================================================
    // Test 8: Extension namespace isolation
    // =========================================================================

    #[tokio::test]
    async fn extension_namespace_isolation_allows_own_namespace() {
        let (ctx, _rx) = make_context();
        let caller = plugin_caller(&[StorageCapability::DocWrite]);

        let doc = json!({
            "name": "test",
            "ext": {
                "com.test.plugin": { "custom_field": 42 }
            }
        });
        // Plugin-scoped to bypass shared collection check.
        let result = ctx.doc_create(&caller, "data", doc, true).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn extension_namespace_isolation_blocks_other_namespace() {
        let (ctx, _rx) = make_context();
        let caller = plugin_caller(&[StorageCapability::DocWrite]);

        let doc = json!({
            "name": "test",
            "ext": {
                "com.other.plugin": { "sneaky": true }
            }
        });
        let result = ctx.doc_create(&caller, "data", doc, true).await;
        assert!(matches!(result, Err(StorageError::PermissionDenied { .. })));
    }

    // =========================================================================
    // Test 9: Audit event emission
    // =========================================================================

    #[tokio::test]
    async fn audit_events_emitted_on_create() {
        let (ctx, mut rx) = make_context();
        let caller = plugin_caller(&[StorageCapability::DocWrite]);

        let doc = json!({"name": "test"});
        ctx.doc_create(&caller, "items", doc, true).await.unwrap();

        let event = rx.try_recv().unwrap();
        assert_eq!(event.event_type, "system.storage.created");
        assert_eq!(event.origin, "com.test.plugin");
        assert!(event.payload.get("collection").is_some());
    }

    #[tokio::test]
    async fn audit_events_emitted_on_delete() {
        let (ctx, mut rx) = make_context();
        let caller = plugin_caller(&[StorageCapability::DocWrite, StorageCapability::DocDelete]);

        // Create a doc first.
        let doc = json!({"id": "del-1", "name": "test"});
        ctx.doc_create(&caller, "items", doc, true).await.unwrap();
        // Drain the create event.
        let _ = rx.try_recv();

        // Delete it.
        ctx.doc_delete(&caller, "items", "del-1", true).await.unwrap();
        let event = rx.try_recv().unwrap();
        assert_eq!(event.event_type, "system.storage.deleted");
    }

    #[tokio::test]
    async fn audit_events_emitted_on_blob_store() {
        let (ctx, mut rx) = make_context();
        let caller = plugin_caller(&[StorageCapability::BlobWrite]);

        let key = BlobKey::new("com.test.plugin/docs/file.txt").unwrap();
        let input = BlobInput {
            data: vec![1, 2, 3],
            content_type: None,
            metadata: HashMap::new(),
        };
        ctx.blob_store(&caller, key, input).await.unwrap();

        let event = rx.try_recv().unwrap();
        assert_eq!(event.event_type, "system.blob.stored");
        assert_eq!(event.origin, "com.test.plugin");
        assert!(event.payload.get("key").is_some());
    }

    #[tokio::test]
    async fn audit_events_emitted_on_blob_delete() {
        let (ctx, mut rx) = make_context();
        let caller = plugin_caller(&[StorageCapability::BlobDelete]);

        let key = BlobKey::new("com.test.plugin/docs/file.txt").unwrap();
        ctx.blob_delete(&caller, key).await.unwrap();

        let event = rx.try_recv().unwrap();
        assert_eq!(event.event_type, "system.blob.deleted");
    }

    #[tokio::test]
    async fn no_audit_event_on_read() {
        let (ctx, mut rx) = make_context();
        let caller = plugin_caller(&[StorageCapability::DocRead]);

        let _ = ctx.doc_get(&caller, "tasks", "1", false).await;
        // No audit event should be emitted for reads.
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn audit_event_includes_system_origin() {
        let (ctx, mut rx) = make_context();
        let caller = system_caller();

        let doc = json!({"name": "system-doc"});
        ctx.doc_create(&caller, "tasks", doc, false).await.unwrap();

        let event = rx.try_recv().unwrap();
        assert_eq!(event.origin, "system");
    }

    // =========================================================================
    // Test: System fields injection
    // =========================================================================

    #[tokio::test]
    async fn system_fields_injected_on_create() {
        let (ctx, _rx) = make_context();
        let caller = plugin_caller(&[StorageCapability::DocWrite, StorageCapability::DocRead]);

        let doc = json!({"name": "test"});
        let created = ctx.doc_create(&caller, "items", doc, true).await.unwrap();

        let obj = created.as_object().unwrap();
        assert!(obj.contains_key("id"), "id should be generated");
        assert!(obj.contains_key("created_at"), "created_at should be set");
        assert!(obj.contains_key("updated_at"), "updated_at should be set");
    }

    #[tokio::test]
    async fn system_fields_caller_provided_id_preserved() {
        let (ctx, _rx) = make_context();
        let caller = plugin_caller(&[StorageCapability::DocWrite]);

        let doc = json!({"id": "my-custom-id", "name": "test"});
        let created = ctx.doc_create(&caller, "items", doc, true).await.unwrap();
        assert_eq!(created["id"], "my-custom-id");
    }

    // =========================================================================
    // Test: Watch-to-event-bus bridge
    // =========================================================================

    #[tokio::test]
    async fn watch_requires_doc_read_capability() {
        let (ctx, _rx) = make_context();
        let caller = plugin_caller(&[]);

        let result = ctx.doc_watch(&caller, "items", true).await;
        assert!(matches!(result, Err(StorageError::PermissionDenied { .. })));
    }

    #[tokio::test]
    async fn watch_resolves_collection_scoping() {
        let (ctx, _rx) = make_context();
        let caller = plugin_caller(&[StorageCapability::DocRead]);

        // Plugin-scoped watch should not produce PermissionDenied.
        let result = ctx.doc_watch(&caller, "items", true).await;
        assert!(!matches!(result, Err(StorageError::PermissionDenied { .. })));
    }

    #[tokio::test]
    async fn watch_shared_collection_requires_declaration() {
        let (ctx, _rx) = make_context();
        let caller = plugin_caller(&[StorageCapability::DocRead]);

        // "undeclared" is not in the plugin's shared_collections.
        let result = ctx.doc_watch(&caller, "undeclared", false).await;
        assert!(matches!(result, Err(StorageError::PermissionDenied { .. })));

        // "tasks" is declared.
        let result = ctx.doc_watch(&caller, "tasks", false).await;
        assert!(!matches!(result, Err(StorageError::PermissionDenied { .. })));
    }
}
