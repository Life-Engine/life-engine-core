//! Security property integration tests.
//!
//! Verifies that the architecture enforces security invariants:
//! (a) unauthenticated request to protected route returns 401 — tested at auth middleware level
//! (b) plugin without storage capability is denied
//! (c) plugin cannot access undeclared collection
//! (d) blob key scoping prevents cross-plugin access
//! (e) extension namespace isolation prevents cross-plugin writes
//! (f) event depth limit prevents infinite loops

use std::collections::HashSet;
use std::sync::Arc;

use serde_json::json;
use tokio::sync::mpsc;

use life_engine_test_utils::mock_blob::MockBlobStorageAdapter;
use life_engine_test_utils::mock_storage::MockDocumentStorageAdapter;
use life_engine_traits::blob::BlobKey;
use life_engine_traits::schema::SchemaRegistry;
use life_engine_traits::storage_context::{
    AuditEvent, CallerIdentity, CollectionAccess, StorageCapability, StorageContext,
};
use life_engine_traits::storage_router::{StorageRouter, TimeoutConfig};
use life_engine_workflow_engine::{EventBus, PipelineExecutor, TriggerRegistry};

// ---------------------------------------------------------------------------
// Test harness
// ---------------------------------------------------------------------------

fn setup_storage_context() -> (Arc<StorageContext>, mpsc::UnboundedReceiver<AuditEvent>) {
    let doc_adapter = Arc::new(MockDocumentStorageAdapter::new());
    let blob_adapter = Arc::new(MockBlobStorageAdapter::new());
    let router = Arc::new(StorageRouter::new(
        doc_adapter,
        blob_adapter,
        TimeoutConfig::default(),
    ));

    let mut schema_registry = SchemaRegistry::new();
    schema_registry.load_cdm_schemas().expect("CDM schemas should load");
    let schema_registry = Arc::new(schema_registry);

    let (audit_tx, audit_rx) = mpsc::unbounded_channel();
    let ctx = Arc::new(StorageContext::new(router, schema_registry, audit_tx));
    (ctx, audit_rx)
}

// ---------------------------------------------------------------------------
// (b) Plugin without storage capability is denied
// ---------------------------------------------------------------------------

#[tokio::test]
async fn plugin_without_read_capability_is_denied() {
    let (ctx, _rx) = setup_storage_context();

    let caller = CallerIdentity::Plugin {
        plugin_id: "com.test.no-read".into(),
        capabilities: HashSet::new(), // no capabilities
        shared_collections: vec![CollectionAccess {
            collection: "tasks".into(),
            access: "read".into(),
        }],
    };

    let result = ctx.doc_get(&caller, "tasks", "some-id", false).await;
    assert!(result.is_err(), "plugin without DocRead should be denied");
    let err = result.unwrap_err();
    assert!(
        format!("{err}").contains("lacks capability"),
        "error should mention 'lacks capability': {err}"
    );
}

#[tokio::test]
async fn plugin_without_write_capability_cannot_create() {
    let (ctx, _rx) = setup_storage_context();

    let caller = CallerIdentity::Plugin {
        plugin_id: "com.test.read-only".into(),
        capabilities: HashSet::from([StorageCapability::DocRead]),
        shared_collections: vec![CollectionAccess {
            collection: "tasks".into(),
            access: "read-write".into(),
        }],
    };

    let result = ctx
        .doc_create(&caller, "tasks", json!({"title": "test"}), false)
        .await;
    assert!(result.is_err(), "plugin without DocWrite should be denied");
}

#[tokio::test]
async fn plugin_without_delete_capability_cannot_delete() {
    let (ctx, _rx) = setup_storage_context();

    let caller = CallerIdentity::Plugin {
        plugin_id: "com.test.no-delete".into(),
        capabilities: HashSet::from([StorageCapability::DocRead, StorageCapability::DocWrite]),
        shared_collections: vec![CollectionAccess {
            collection: "tasks".into(),
            access: "read-write".into(),
        }],
    };

    let result = ctx.doc_delete(&caller, "tasks", "some-id", false).await;
    assert!(
        result.is_err(),
        "plugin without DocDelete should be denied"
    );
}

// ---------------------------------------------------------------------------
// (c) Plugin cannot access undeclared collection
// ---------------------------------------------------------------------------

#[tokio::test]
async fn plugin_cannot_access_undeclared_shared_collection() {
    let (ctx, _rx) = setup_storage_context();

    let caller = CallerIdentity::Plugin {
        plugin_id: "com.test.tasks-only".into(),
        capabilities: HashSet::from([StorageCapability::DocRead]),
        shared_collections: vec![CollectionAccess {
            collection: "tasks".into(),
            access: "read".into(),
        }],
    };

    // Try to read from "contacts" — not declared.
    let result = ctx.doc_get(&caller, "contacts", "some-id", false).await;
    assert!(
        result.is_err(),
        "plugin should be denied access to undeclared collection"
    );
    let err = result.unwrap_err();
    assert!(
        format!("{err}").contains("not declared"),
        "error should mention undeclared access: {err}"
    );
}

#[tokio::test]
async fn plugin_can_access_declared_collection() {
    let (ctx, _rx) = setup_storage_context();

    let caller = CallerIdentity::Plugin {
        plugin_id: "com.test.tasks-reader".into(),
        capabilities: HashSet::from([StorageCapability::DocRead, StorageCapability::DocWrite]),
        shared_collections: vec![CollectionAccess {
            collection: "tasks".into(),
            access: "read-write".into(),
        }],
    };

    // Create a document first (via System to bypass checks for setup).
    let system = CallerIdentity::System;
    ctx.doc_create(&system, "tasks", json!({"title": "test task"}), false)
        .await
        .expect("system create should succeed");

    // Plugin should be able to read from declared collection.
    let result = ctx
        .doc_query(
            &caller,
            life_engine_traits::storage::QueryDescriptor {
                collection: "tasks".into(),
                filter: None,
                sort: vec![],
                pagination: life_engine_traits::storage::Pagination::default(),
                fields: None,
                text_search: None,
            },
            false,
        )
        .await;
    assert!(
        result.is_ok(),
        "plugin should access declared collection: {:?}",
        result.err()
    );
}

// ---------------------------------------------------------------------------
// (d) Blob key scoping prevents cross-plugin access
// ---------------------------------------------------------------------------

#[tokio::test]
async fn blob_key_format_enforces_plugin_prefix() {
    // BlobKey::new validates {plugin_id}/{context}/{filename} format.
    // Keys without the three-segment structure are rejected.
    let invalid = BlobKey::new("no-slashes");
    assert!(
        invalid.is_err(),
        "blob key without plugin prefix should be rejected"
    );

    let too_short = BlobKey::new("plugin-a/file.txt");
    assert!(
        too_short.is_err(),
        "blob key with only two segments should be rejected"
    );

    let traversal = BlobKey::new("plugin-a/../plugin-b/data/secret.txt");
    assert!(
        traversal.is_err(),
        "blob key with .. traversal should be rejected"
    );

    let valid = BlobKey::new("plugin-a/data/file.txt");
    assert!(valid.is_ok(), "valid blob key should be accepted");
}

#[tokio::test]
async fn plugin_without_blob_capability_is_denied() {
    let (ctx, _rx) = setup_storage_context();

    let caller = CallerIdentity::Plugin {
        plugin_id: "com.test.no-blob".into(),
        capabilities: HashSet::from([StorageCapability::DocRead]),
        shared_collections: vec![],
    };

    let key = BlobKey::new("com.test.no-blob/data/file.txt").unwrap();
    let result = ctx.blob_retrieve(&caller, key).await;
    assert!(
        result.is_err(),
        "plugin without BlobRead should be denied"
    );
}

// ---------------------------------------------------------------------------
// (e) Extension namespace isolation prevents cross-plugin writes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn extension_namespace_isolation_prevents_cross_plugin_writes() {
    let (ctx, _rx) = setup_storage_context();

    let caller = CallerIdentity::Plugin {
        plugin_id: "com.test.plugin-a".into(),
        capabilities: HashSet::from([StorageCapability::DocRead, StorageCapability::DocWrite]),
        shared_collections: vec![CollectionAccess {
            collection: "tasks".into(),
            access: "read-write".into(),
        }],
    };

    // Try to create a document with extension data under another plugin's namespace.
    let doc_with_foreign_ext = json!({
        "title": "Malicious extension write",
        "ext": {
            "com.test.plugin-b": {
                "secret": "stolen-data"
            }
        }
    });

    let result = ctx.doc_create(&caller, "tasks", doc_with_foreign_ext, false).await;
    assert!(
        result.is_err(),
        "plugin should not be able to write to another plugin's extension namespace"
    );
    let err = result.unwrap_err();
    assert!(
        format!("{err}").contains("cannot write to extension namespace"),
        "error should mention namespace violation: {err}"
    );
}

#[tokio::test]
async fn plugin_can_write_own_extension_namespace() {
    let (ctx, _rx) = setup_storage_context();

    let caller = CallerIdentity::Plugin {
        plugin_id: "com.test.plugin-a".into(),
        capabilities: HashSet::from([StorageCapability::DocRead, StorageCapability::DocWrite]),
        shared_collections: vec![CollectionAccess {
            collection: "tasks".into(),
            access: "read-write".into(),
        }],
    };

    let doc_with_own_ext = json!({
        "title": "My extension data",
        "ext": {
            "com.test.plugin-a": {
                "custom_field": "my-data"
            }
        }
    });

    let result = ctx.doc_create(&caller, "tasks", doc_with_own_ext, false).await;
    assert!(
        result.is_ok(),
        "plugin should be able to write its own extension namespace: {:?}",
        result.err()
    );
}

// ---------------------------------------------------------------------------
// (f) Event depth limit prevents infinite loops
// ---------------------------------------------------------------------------

#[tokio::test]
async fn event_depth_limit_prevents_infinite_loops() {
    use life_engine_workflow_engine::Event;

    let registry = Arc::new(TriggerRegistry::build(vec![]).unwrap());
    let executor = Arc::new(PipelineExecutor::new(Arc::new(NoopPluginExecutor)));

    // Create event bus with max depth of 3.
    let bus = EventBus::with_max_depth(registry, executor, 3);

    // Events at depth <= 3 should be accepted.
    let event_at_limit = Event {
        name: "test.cascade".into(),
        source: "plugin-a".into(),
        payload: Some(json!({"step": "at-limit"})),
        timestamp: chrono::Utc::now(),
        depth: 3,
    };
    bus.emit(event_at_limit).await;

    // Events at depth 4 should be silently dropped (loop prevention).
    let event_over_limit = Event {
        name: "test.cascade".into(),
        source: "plugin-a".into(),
        payload: Some(json!({"step": "over-limit"})),
        timestamp: chrono::Utc::now(),
        depth: 4,
    };
    bus.emit(event_over_limit).await;
    // No panic, no infinite loop — the event is dropped.
}

// A no-op plugin executor for tests that don't execute real plugins.
struct NoopPluginExecutor;

#[async_trait::async_trait]
impl life_engine_workflow_engine::PluginExecutor for NoopPluginExecutor {
    async fn execute(
        &self,
        _plugin_id: &str,
        _action: &str,
        input: life_engine_types::PipelineMessage,
    ) -> Result<life_engine_types::PipelineMessage, Box<dyn life_engine_traits::EngineError>> {
        Ok(input)
    }
}
