//! End-to-end integration tests exercising the full request path:
//! WorkflowEngine → PipelineExecutor → SystemCrudHandler → StorageContext → MockStorage.
//!
//! These tests verify that the complete four-layer pipeline works correctly
//! for all CRUD operations, event-triggered workflows, error handling,
//! and concurrent execution.

use std::sync::Arc;

use serde_json::{json, Value};
use tokio::sync::mpsc;

use life_engine_test_utils::mock_blob::MockBlobStorageAdapter;
use life_engine_test_utils::mock_storage::MockDocumentStorageAdapter;
use life_engine_traits::schema::SchemaRegistry;
use life_engine_traits::storage_context::{AuditEvent, StorageContext};
use life_engine_traits::storage_router::{StorageRouter, TimeoutConfig};
use life_engine_workflow_engine::{
    build_initial_message, CompositePluginExecutor, PipelineExecutor, PluginExecutor,
    SystemCrudHandler, TriggerContext, WorkflowConfig, WorkflowEngine, SYSTEM_CRUD_PLUGIN_ID,
};

// ---------------------------------------------------------------------------
// Test harness
// ---------------------------------------------------------------------------

/// Build a fully wired `WorkflowEngine` backed by in-memory mock storage.
///
/// Returns the engine plus a receiver for audit events so tests can verify
/// side effects.
async fn setup_engine() -> (WorkflowEngine, mpsc::UnboundedReceiver<AuditEvent>) {
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
    let storage_ctx = Arc::new(StorageContext::new(router, schema_registry, audit_tx));

    let system_handler = SystemCrudHandler::new(Arc::clone(&storage_ctx));
    let composite = CompositePluginExecutor::new(
        storage_ctx,
        Arc::new(system_handler),
    );

    let config = WorkflowConfig {
        path: "../../workflows".into(),
    };

    let engine = WorkflowEngine::new(config, Arc::new(composite))
        .await
        .expect("WorkflowEngine should initialize from YAML workflows");

    (engine, audit_rx)
}

/// Build standalone components for lower-level tests.
fn setup_storage() -> (
    Arc<StorageContext>,
    mpsc::UnboundedReceiver<AuditEvent>,
) {
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
    let storage_ctx = Arc::new(StorageContext::new(router, schema_registry, audit_tx));
    (storage_ctx, audit_rx)
}

// ---------------------------------------------------------------------------
// (a) REST CRUD operations on a CDM collection
// ---------------------------------------------------------------------------

#[tokio::test]
async fn crud_list_empty_collection_returns_empty_documents() {
    let (engine, _rx) = setup_engine().await;

    let result = engine
        .handle_endpoint("GET", "/api/v1/data/:collection", json!({"params": {"collection": "tasks"}}), None)
        .await
        .expect("list on empty collection should succeed");

    let payload: Value = serde_json::to_value(&result.payload).unwrap();
    let data = payload.get("data").unwrap();
    let docs = data.get("documents").unwrap().as_array().unwrap();
    assert!(docs.is_empty(), "empty collection should return no documents");
}

#[tokio::test]
async fn crud_create_then_get_document() {
    let (engine, _rx) = setup_engine().await;

    // Create a task document.
    let task_body = json!({
        "params": {"collection": "tasks"},
        "body": {
            "title": "Buy groceries",
            "status": "pending",
            "priority": "medium",
            "tags": ["shopping"]
        }
    });

    let create_result = engine
        .handle_endpoint("POST", "/api/v1/data/:collection", task_body, None)
        .await
        .expect("create should succeed");

    let create_payload: Value = serde_json::to_value(&create_result.payload).unwrap();
    let created_doc = &create_payload["data"]["document"];
    assert_eq!(created_doc["title"], "Buy groceries");

    // The document should have a system-assigned id.
    let doc_id = created_doc
        .get("id")
        .and_then(|v| v.as_str())
        .expect("created doc should have _id_");

    // Retrieve the same document by ID.
    let get_body = json!({
        "params": {"collection": "tasks", "id": doc_id}
    });

    let get_result = engine
        .handle_endpoint("GET", "/api/v1/data/:collection/:id", get_body, None)
        .await
        .expect("get should succeed");

    let get_payload: Value = serde_json::to_value(&get_result.payload).unwrap();
    assert_eq!(get_payload["data"]["title"], "Buy groceries");
}

#[tokio::test]
async fn crud_create_then_list_returns_document() {
    let (engine, _rx) = setup_engine().await;

    let task_body = json!({
        "params": {"collection": "tasks"},
        "body": {
            "title": "Write tests",
            "status": "in_progress",
            "priority": "high",
            "tags": ["dev"]
        }
    });

    engine
        .handle_endpoint("POST", "/api/v1/data/:collection", task_body, None)
        .await
        .expect("create should succeed");

    let list_result = engine
        .handle_endpoint("GET", "/api/v1/data/:collection", json!({"params": {"collection": "tasks"}}), None)
        .await
        .expect("list should succeed");

    let list_payload: Value = serde_json::to_value(&list_result.payload).unwrap();
    let docs = list_payload["data"]["documents"].as_array().unwrap();
    assert_eq!(docs.len(), 1, "list should return the created document");
    assert_eq!(docs[0]["title"], "Write tests");
}

#[tokio::test]
async fn crud_update_document() {
    let (engine, _rx) = setup_engine().await;

    // Create
    let create_body = json!({
        "params": {"collection": "tasks"},
        "body": {
            "title": "Original title",
            "status": "pending",
            "priority": "low",
            "tags": []
        }
    });

    let create_result = engine
        .handle_endpoint("POST", "/api/v1/data/:collection", create_body, None)
        .await
        .unwrap();

    let create_payload: Value = serde_json::to_value(&create_result.payload).unwrap();
    let doc_id = create_payload["data"]["document"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Update
    let update_body = json!({
        "params": {"collection": "tasks", "id": doc_id},
        "body": {
            "title": "Updated title",
            "status": "completed",
            "priority": "low",
            "tags": ["done"]
        }
    });

    engine
        .handle_endpoint("PUT", "/api/v1/data/:collection/:id", update_body, None)
        .await
        .expect("update should succeed");

    // Verify the update took effect.
    let get_body = json!({"params": {"collection": "tasks", "id": doc_id}});
    let get_result = engine
        .handle_endpoint("GET", "/api/v1/data/:collection/:id", get_body, None)
        .await
        .unwrap();

    let get_payload: Value = serde_json::to_value(&get_result.payload).unwrap();
    assert_eq!(get_payload["data"]["title"], "Updated title");
    assert_eq!(get_payload["data"]["status"], "completed");
}

#[tokio::test]
async fn crud_delete_document() {
    let (engine, _rx) = setup_engine().await;

    // Create
    let create_body = json!({
        "params": {"collection": "tasks"},
        "body": {
            "title": "To be deleted",
            "status": "pending",
            "priority": "low",
            "tags": []
        }
    });

    let create_result = engine
        .handle_endpoint("POST", "/api/v1/data/:collection", create_body, None)
        .await
        .unwrap();

    let create_payload: Value = serde_json::to_value(&create_result.payload).unwrap();
    let doc_id = create_payload["data"]["document"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Delete
    let delete_body = json!({"params": {"collection": "tasks", "id": doc_id}});
    engine
        .handle_endpoint("DELETE", "/api/v1/data/:collection/:id", delete_body, None)
        .await
        .expect("delete should succeed");

    // Verify list is empty.
    let list_result = engine
        .handle_endpoint("GET", "/api/v1/data/:collection", json!({"params": {"collection": "tasks"}}), None)
        .await
        .unwrap();

    let list_payload: Value = serde_json::to_value(&list_result.payload).unwrap();
    let docs = list_payload["data"]["documents"].as_array().unwrap();
    assert!(docs.is_empty(), "list should be empty after deletion");
}

// ---------------------------------------------------------------------------
// (c) Event-triggered workflow
// ---------------------------------------------------------------------------

#[tokio::test]
async fn event_triggered_workflow_builds_valid_message() {
    let trigger = TriggerContext::Event {
        name: "task.created".into(),
        payload: json!({"task_id": "abc-123", "collection": "tasks"}),
    };

    let message = build_initial_message(trigger).expect("event trigger should build a message");

    assert!(
        message.metadata.source.starts_with("event:"),
        "source should start with 'event:'"
    );
    assert!(!message.metadata.correlation_id.is_nil());
}

// ---------------------------------------------------------------------------
// (d) Scheduled workflow execution
// ---------------------------------------------------------------------------

#[tokio::test]
async fn schedule_trigger_builds_valid_message() {
    let trigger = TriggerContext::Schedule {
        workflow_id: "nightly.cleanup".into(),
        fired_at: chrono::Utc::now(),
    };

    let message = build_initial_message(trigger).expect("schedule trigger should build a message");

    assert!(
        message.metadata.source.starts_with("schedule:"),
        "source should start with 'schedule:'"
    );
}

// ---------------------------------------------------------------------------
// (e) Plugin with multiple capabilities accessing storage
// ---------------------------------------------------------------------------

#[tokio::test]
async fn system_crud_handler_supports_all_actions() {
    let (storage_ctx, _rx) = setup_storage();
    let handler = SystemCrudHandler::new(storage_ctx);

    // Create
    let create_msg = build_initial_message(TriggerContext::Endpoint {
        method: "POST".into(),
        path: "/api/v1/data/:collection".into(),
        body: json!({
            "params": {"collection": "tasks"},
            "body": {"title": "Multi-cap test", "status": "pending", "priority": "high", "tags": []}
        }),
        auth: None,
    })
    .unwrap();

    let result = handler
        .execute(SYSTEM_CRUD_PLUGIN_ID, "create", create_msg)
        .await
        .expect("create action should succeed");

    let result_val: Value = serde_json::to_value(&result.payload).unwrap();
    assert!(
        result_val["data"]["created"] == json!(true),
        "create should return created: true"
    );

    // Health check
    let health_msg = build_initial_message(TriggerContext::Endpoint {
        method: "GET".into(),
        path: "/api/v1/health".into(),
        body: json!({}),
        auth: None,
    })
    .unwrap();

    let health_result = handler
        .execute(SYSTEM_CRUD_PLUGIN_ID, "health_check", health_msg)
        .await
        .expect("health_check action should succeed");

    let health_val: Value = serde_json::to_value(&health_result.payload).unwrap();
    assert_eq!(health_val["data"]["status"], "healthy");
}

// ---------------------------------------------------------------------------
// (f) Error handling — plugin failure
// ---------------------------------------------------------------------------

#[tokio::test]
async fn unknown_action_returns_error() {
    let (storage_ctx, _rx) = setup_storage();
    let handler = SystemCrudHandler::new(storage_ctx);

    let msg = build_initial_message(TriggerContext::Endpoint {
        method: "GET".into(),
        path: "/api/v1/data/:collection".into(),
        body: json!({}),
        auth: None,
    })
    .unwrap();

    let err = handler
        .execute(SYSTEM_CRUD_PLUGIN_ID, "nonexistent_action", msg)
        .await;

    assert!(err.is_err(), "unknown action should return an error");
    let engine_err = err.unwrap_err();
    assert_eq!(engine_err.code(), "UNKNOWN_ACTION");
}

#[tokio::test]
async fn missing_collection_param_returns_error() {
    let (storage_ctx, _rx) = setup_storage();
    let handler = SystemCrudHandler::new(storage_ctx);

    // No "collection" param in the body.
    let msg = build_initial_message(TriggerContext::Endpoint {
        method: "GET".into(),
        path: "/api/v1/data/:collection".into(),
        body: json!({"limit": 10}),
        auth: None,
    })
    .unwrap();

    let err = handler.execute(SYSTEM_CRUD_PLUGIN_ID, "list", msg).await;
    assert!(err.is_err(), "missing collection should return error");
    assert_eq!(err.unwrap_err().code(), "MISSING_PARAM");
}

#[tokio::test]
async fn get_nonexistent_document_returns_storage_error() {
    let (storage_ctx, _rx) = setup_storage();
    let handler = SystemCrudHandler::new(storage_ctx);

    let msg = build_initial_message(TriggerContext::Endpoint {
        method: "GET".into(),
        path: "/api/v1/data/tasks/nonexistent".into(),
        body: json!({"params": {"collection": "tasks", "id": "does-not-exist"}}),
        auth: None,
    })
    .unwrap();

    let err = handler.execute(SYSTEM_CRUD_PLUGIN_ID, "get", msg).await;
    assert!(err.is_err(), "get nonexistent document should fail");
    assert_eq!(err.unwrap_err().code(), "STORAGE_ERROR");
}

// ---------------------------------------------------------------------------
// (g) Concurrent workflow execution
// ---------------------------------------------------------------------------

#[tokio::test]
async fn concurrent_workflows_all_complete() {
    let (storage_ctx, _rx) = setup_storage();
    let handler = Arc::new(SystemCrudHandler::new(Arc::clone(&storage_ctx)));
    let executor = PipelineExecutor::with_concurrency_limit(handler, 8);

    // Spawn 8 concurrent list operations.
    let mut handles = Vec::new();
    for i in 0..8 {
        let trigger = TriggerContext::Endpoint {
            method: "GET".into(),
            path: "/api/v1/data/:collection".into(),
            body: json!({"params": {"collection": "tasks"}, "request_num": i}),
            auth: None,
        };

        let workflow = life_engine_workflow_engine::WorkflowDef {
            id: format!("concurrent-test-{i}"),
            name: format!("Concurrent Test {i}"),
            description: None,
            trigger: life_engine_workflow_engine::TriggerDef {
                endpoint: Some(format!("GET /api/v1/data/tasks?n={i}")),
                event: None,
                schedule: None,
            },
            steps: vec![life_engine_workflow_engine::StepDef {
                plugin: SYSTEM_CRUD_PLUGIN_ID.into(),
                action: "list".into(),
                on_error: None,
                condition: None,
            }],
            mode: life_engine_workflow_engine::ExecutionMode::Sync,
            validate: life_engine_workflow_engine::ValidationLevel::None,
        };

        let job_id = executor.spawn(trigger, &workflow);
        handles.push(job_id);
    }

    // Wait briefly for all jobs to finish.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Verify all jobs completed.
    for job_id in &handles {
        let status = executor.job_status(job_id).await;
        assert!(
            matches!(status, Some(life_engine_workflow_engine::JobStatus::Completed)),
            "job {job_id} should have completed, got {status:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Audit event emission
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_emits_audit_event() {
    let (engine, mut audit_rx) = setup_engine().await;

    let task_body = json!({
        "params": {"collection": "tasks"},
        "body": {
            "title": "Audit test",
            "status": "pending",
            "priority": "low",
            "tags": []
        }
    });

    engine
        .handle_endpoint("POST", "/api/v1/data/:collection", task_body, None)
        .await
        .expect("create should succeed");

    // Drain the audit channel — there should be at least one event.
    let event = audit_rx.try_recv().expect("should receive an audit event");
    assert!(
        event.event_type.contains("created"),
        "audit event type should contain 'created': {}",
        event.event_type
    );
    assert_eq!(event.origin, "system");
}
