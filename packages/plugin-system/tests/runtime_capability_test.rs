//! Runtime capability enforcement tests (WP 8.16).
//!
//! Validates that every host function performs a synchronous capability check
//! at the start of execution, returning a Fatal `EngineError` with code
//! "CAP_002" when the calling plugin lacks the required capability. These
//! checks are the second layer of enforcement (defence-in-depth), operating
//! independently of the injection gating layer.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use life_engine_plugin_system::capability::ApprovedCapabilities;
use life_engine_plugin_system::error::PluginError;
use life_engine_plugin_system::host_functions::config::ConfigHostContext;
use life_engine_plugin_system::host_functions::events::{
    EmitRequest, EventsHostContext, SubscribeRequest,
};
use life_engine_plugin_system::host_functions::http::HttpHostContext;
use life_engine_plugin_system::host_functions::storage::StorageHostContext;
use chrono::Utc;
use life_engine_traits::{Capability, EngineError, Severity, StorageBackend};
use life_engine_types::{
    CdmType, MessageMetadata, PipelineMessage, StorageMutation, StorageQuery, Task, TaskPriority,
    TaskStatus, TypedPayload,
};
use life_engine_workflow_engine::WorkflowEventEmitter;
use uuid::Uuid;

// --- Mock backends ---

struct MockStorage;

#[async_trait]
impl StorageBackend for MockStorage {
    async fn execute(
        &self,
        _query: StorageQuery,
    ) -> Result<Vec<PipelineMessage>, Box<dyn EngineError>> {
        Ok(vec![])
    }

    async fn mutate(&self, _op: StorageMutation) -> Result<(), Box<dyn EngineError>> {
        Ok(())
    }

    async fn init(
        _config: toml::Value,
        _key: [u8; 32],
    ) -> Result<Self, Box<dyn EngineError>> {
        Ok(MockStorage)
    }
}

struct MockEventBus {
    emit_calls: Mutex<Vec<(String, serde_json::Value)>>,
}

impl MockEventBus {
    fn new() -> Self {
        Self {
            emit_calls: Mutex::new(vec![]),
        }
    }
}

#[async_trait]
impl WorkflowEventEmitter for MockEventBus {
    async fn emit(&self, event_name: &str, payload: serde_json::Value) {
        self.emit_calls
            .lock()
            .unwrap()
            .push((event_name.to_string(), payload));
    }
}

// --- Helpers ---

fn make_capabilities(caps: &[Capability]) -> ApprovedCapabilities {
    let set: HashSet<Capability> = caps.iter().copied().collect();
    ApprovedCapabilities::new(set)
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

fn make_insert_bytes(plugin_id: &str, collection: &str) -> Vec<u8> {
    let mutation = StorageMutation::Insert {
        plugin_id: plugin_id.to_string(),
        collection: collection.to_string(),
        data: sample_pipeline_message(),
    };
    serde_json::to_vec(&mutation).unwrap()
}

/// Asserts the error is a RuntimeCapabilityViolation with code CAP_002,
/// Severity::Fatal, and that the message includes the capability name and
/// plugin ID.
fn assert_runtime_violation(err: &PluginError, expected_cap: &str, expected_plugin_id: &str) {
    assert!(
        matches!(err, PluginError::RuntimeCapabilityViolation(_)),
        "expected RuntimeCapabilityViolation, got: {err:?}"
    );
    assert_eq!(
        err.code(),
        "CAP_002",
        "runtime capability violation must use code CAP_002"
    );
    assert_eq!(
        err.severity(),
        Severity::Fatal,
        "runtime capability violation must be Fatal"
    );
    let msg = err.to_string();
    assert!(
        msg.contains(expected_cap),
        "error message should include capability name '{expected_cap}': {msg}"
    );
    assert!(
        msg.contains(expected_plugin_id),
        "error message should include plugin ID '{expected_plugin_id}': {msg}"
    );
}

// ==========================================================================
// Test: approved capability allows host function execution and returns data
// ==========================================================================

#[tokio::test]
async fn storage_read_succeeds_with_approved_capability() {
    let ctx = StorageHostContext {
        plugin_id: "test-plugin".into(),
        capabilities: make_capabilities(&[Capability::StorageRead]),
        storage: Arc::new(MockStorage),
    };

    let query = StorageQuery {
        collection: "tasks".into(),
        plugin_id: "ignored".into(),
        filters: vec![],
        sort: vec![],
        limit: None,
        offset: None,
    };
    let input = serde_json::to_vec(&query).unwrap();

    let result =
        life_engine_plugin_system::host_functions::storage::host_storage_read(&ctx, &input).await;
    assert!(result.is_ok(), "approved capability should allow execution");
}

#[tokio::test]
async fn storage_write_succeeds_with_approved_capability() {
    let ctx = StorageHostContext {
        plugin_id: "test-plugin".into(),
        capabilities: make_capabilities(&[Capability::StorageWrite]),
        storage: Arc::new(MockStorage),
    };

    let input = make_insert_bytes("ignored", "tasks");

    let result =
        life_engine_plugin_system::host_functions::storage::host_storage_write(&ctx, &input).await;
    assert!(result.is_ok(), "approved capability should allow execution");
}

#[test]
fn config_read_succeeds_with_approved_capability() {
    let ctx = ConfigHostContext {
        plugin_id: "test-plugin".into(),
        capabilities: make_capabilities(&[Capability::ConfigRead]),
        plugin_config: Some(serde_json::json!({"key": "value"})),
    };

    let result = life_engine_plugin_system::host_functions::config::host_config_read(&ctx);
    assert!(result.is_ok(), "approved capability should allow execution");
    let output: serde_json::Value = serde_json::from_slice(&result.unwrap()).unwrap();
    assert_eq!(output["key"], "value");
}

#[tokio::test]
async fn events_emit_succeeds_with_approved_capability() {
    let bus = Arc::new(MockEventBus::new());
    let ctx = EventsHostContext {
        plugin_id: "test-plugin".into(),
        capabilities: make_capabilities(&[Capability::EventsEmit]),
        event_bus: bus,
        declared_emit_events: None,
        execution_depth: 0,
    };

    let input = serde_json::to_vec(&EmitRequest {
        event_name: "test.event".into(),
        payload: serde_json::json!({}),
    })
    .unwrap();

    let result =
        life_engine_plugin_system::host_functions::events::host_events_emit(&ctx, &input).await;
    assert!(result.is_ok(), "approved capability should allow execution");
}

#[tokio::test]
async fn events_subscribe_succeeds_with_approved_capability() {
    let bus = Arc::new(MockEventBus::new());
    let ctx = EventsHostContext {
        plugin_id: "test-plugin".into(),
        capabilities: make_capabilities(&[Capability::EventsSubscribe]),
        event_bus: bus,
        declared_emit_events: None,
        execution_depth: 0,
    };

    let input = serde_json::to_vec(&SubscribeRequest {
        event_name: "test.event".into(),
    })
    .unwrap();

    let result =
        life_engine_plugin_system::host_functions::events::host_events_subscribe(&ctx, &input)
            .await;
    assert!(result.is_ok(), "approved capability should allow execution");
}

#[tokio::test]
async fn http_request_returns_cap002_without_http_outbound() {
    let ctx = HttpHostContext {
        plugin_id: "blocked-plugin".into(),
        capabilities: make_capabilities(&[]),
        client: reqwest::Client::new(),
        allowed_domains: None,
    };

    let input = serde_json::to_vec(&serde_json::json!({
        "method": "GET",
        "url": "https://example.com"
    }))
    .unwrap();

    let result =
        life_engine_plugin_system::host_functions::http::host_http_request(&ctx, &input).await;
    let err = result.unwrap_err();
    assert_runtime_violation(&err, "http:outbound", "blocked-plugin");
}

// ==========================================================================
// Test: unapproved capability returns Fatal EngineError with CAP_002 code
// ==========================================================================

#[tokio::test]
async fn storage_read_returns_cap002_without_storage_read() {
    let ctx = StorageHostContext {
        plugin_id: "blocked-plugin".into(),
        capabilities: make_capabilities(&[]),
        storage: Arc::new(MockStorage),
    };

    let query = StorageQuery {
        collection: "tasks".into(),
        plugin_id: "ignored".into(),
        filters: vec![],
        sort: vec![],
        limit: None,
        offset: None,
    };
    let input = serde_json::to_vec(&query).unwrap();

    let result =
        life_engine_plugin_system::host_functions::storage::host_storage_read(&ctx, &input).await;
    let err = result.unwrap_err();
    assert_runtime_violation(&err, "storage:doc:read", "blocked-plugin");
}

#[tokio::test]
async fn storage_write_returns_cap002_without_storage_write() {
    let ctx = StorageHostContext {
        plugin_id: "blocked-plugin".into(),
        capabilities: make_capabilities(&[Capability::StorageRead]),
        storage: Arc::new(MockStorage),
    };

    let input = make_insert_bytes("ignored", "tasks");

    let result =
        life_engine_plugin_system::host_functions::storage::host_storage_write(&ctx, &input).await;
    let err = result.unwrap_err();
    assert_runtime_violation(&err, "storage:doc:write", "blocked-plugin");
}

#[test]
fn config_read_returns_cap002_without_config_read() {
    let ctx = ConfigHostContext {
        plugin_id: "blocked-plugin".into(),
        capabilities: make_capabilities(&[]),
        plugin_config: Some(serde_json::json!({"secret": "data"})),
    };

    let result = life_engine_plugin_system::host_functions::config::host_config_read(&ctx);
    let err = result.unwrap_err();
    assert_runtime_violation(&err, "config:read", "blocked-plugin");
}

#[tokio::test]
async fn events_emit_returns_cap002_without_events_emit() {
    let bus = Arc::new(MockEventBus::new());
    let ctx = EventsHostContext {
        plugin_id: "blocked-plugin".into(),
        capabilities: make_capabilities(&[Capability::EventsSubscribe]),
        event_bus: bus,
        declared_emit_events: None,
        execution_depth: 0,
    };

    let input = serde_json::to_vec(&EmitRequest {
        event_name: "test.event".into(),
        payload: serde_json::json!({}),
    })
    .unwrap();

    let result =
        life_engine_plugin_system::host_functions::events::host_events_emit(&ctx, &input).await;
    let err = result.unwrap_err();
    assert_runtime_violation(&err, "events:emit", "blocked-plugin");
}

#[tokio::test]
async fn events_subscribe_returns_cap002_without_events_subscribe() {
    let bus = Arc::new(MockEventBus::new());
    let ctx = EventsHostContext {
        plugin_id: "blocked-plugin".into(),
        capabilities: make_capabilities(&[Capability::EventsEmit]),
        event_bus: bus,
        declared_emit_events: None,
        execution_depth: 0,
    };

    let input = serde_json::to_vec(&SubscribeRequest {
        event_name: "test.event".into(),
    })
    .unwrap();

    let result =
        life_engine_plugin_system::host_functions::events::host_events_subscribe(&ctx, &input)
            .await;
    let err = result.unwrap_err();
    assert_runtime_violation(&err, "events:subscribe", "blocked-plugin");
}

// ==========================================================================
// Test: the check is synchronous — no async waiting
// ==========================================================================

#[test]
fn config_read_capability_check_is_synchronous() {
    // config::host_config_read is a sync fn — this test proves the check
    // happens without any async runtime. If it required async, this test
    // would fail to compile.
    let ctx = ConfigHostContext {
        plugin_id: "sync-check".into(),
        capabilities: make_capabilities(&[]),
        plugin_config: None,
    };

    let err = life_engine_plugin_system::host_functions::config::host_config_read(&ctx)
        .unwrap_err();
    assert_eq!(err.code(), "CAP_002");
}

// ==========================================================================
// Test: error message includes the capability name and plugin ID
// ==========================================================================

#[tokio::test]
async fn error_message_includes_capability_and_plugin_id() {
    let ctx = StorageHostContext {
        plugin_id: "my-unique-plugin-id".into(),
        capabilities: make_capabilities(&[]),
        storage: Arc::new(MockStorage),
    };

    let query = StorageQuery {
        collection: "x".into(),
        plugin_id: "x".into(),
        filters: vec![],
        sort: vec![],
        limit: None,
        offset: None,
    };
    let input = serde_json::to_vec(&query).unwrap();

    let result =
        life_engine_plugin_system::host_functions::storage::host_storage_read(&ctx, &input).await;
    let err = result.unwrap_err();

    let msg = err.to_string();
    assert!(
        msg.contains("storage:doc:read"),
        "error must name the capability: {msg}"
    );
    assert!(
        msg.contains("my-unique-plugin-id"),
        "error must name the plugin: {msg}"
    );
}
