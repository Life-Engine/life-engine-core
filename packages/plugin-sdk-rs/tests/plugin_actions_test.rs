//! TDD tests for plugin action contracts (Phase 6 — Plugin Actions).
//!
//! Covers:
//! 1. Action function signature compiles correctly
//! 2. PluginContext provides typed storage access
//! 3. Lifecycle hooks have default no-op implementations
//! 4. Hard failure returns PluginError
//! 5. Soft warning appends to message warnings
//! 6. PluginError has code/message/detail fields

use async_trait::async_trait;
use chrono::Utc;
use life_engine_plugin_sdk::{
    context::{
        ActionContext, ConfigClient, EventClient, HttpClient, HttpResponse, StorageClient,
    },
    error::PluginError,
    lifecycle::LifecycleHooks,
    types::HttpMethod,
    CdmType, MessageMetadata, PipelineMessage, Task, TaskPriority, TaskStatus, TypedPayload,
};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

fn test_message() -> PipelineMessage {
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
            title: "Test task".into(),
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

// ---------------------------------------------------------------------------
// Shared mock clients
// ---------------------------------------------------------------------------

struct MockStorage;
struct MockEvents;
struct MockConfig;
struct MockHttp;

#[async_trait]
impl StorageClient for MockStorage {
    async fn doc_read(&self, _: &str, _: &str) -> Result<Option<Value>, PluginError> {
        Ok(Some(serde_json::json!({"id": "1"})))
    }
    async fn doc_write(&self, _: &str, _: &str, _: Value) -> Result<(), PluginError> {
        Ok(())
    }
    async fn doc_delete(&self, _: &str, _: &str) -> Result<bool, PluginError> {
        Ok(true)
    }
    async fn doc_query(&self, _: &str, _: Value) -> Result<Vec<Value>, PluginError> {
        Ok(vec![])
    }
}

#[async_trait]
impl EventClient for MockEvents {
    async fn emit(&self, _: &str, _: Value) -> Result<(), PluginError> {
        Ok(())
    }
}

#[async_trait]
impl ConfigClient for MockConfig {
    async fn read(&self, _: &str) -> Result<Option<Value>, PluginError> {
        Ok(Some(serde_json::json!("value")))
    }
}

#[async_trait]
impl HttpClient for MockHttp {
    async fn request(
        &self,
        _: HttpMethod,
        _: &str,
        _: Option<Value>,
        _: Option<String>,
    ) -> Result<HttpResponse, PluginError> {
        Ok(HttpResponse {
            status: 200,
            headers: serde_json::json!({}),
            body: "ok".into(),
        })
    }
}

fn mock_ctx() -> ActionContext {
    ActionContext::new(
        "com.test.actions",
        Arc::new(MockStorage),
        Arc::new(MockEvents),
        Arc::new(MockConfig),
        Arc::new(MockHttp),
    )
}

// ---------------------------------------------------------------------------
// Test 1 — Action function signature compiles correctly
//
// An action is a function:
//   fn(PipelineMessage, &ActionContext) -> Result<PipelineMessage, PluginError>
//
// This test verifies the signature compiles and can be called.
// ---------------------------------------------------------------------------

fn example_action(
    input: PipelineMessage,
    _ctx: &ActionContext,
) -> Result<PipelineMessage, PluginError> {
    Ok(input)
}

#[test]
fn action_function_signature_compiles() {
    let ctx = mock_ctx();
    let msg = test_message();
    let result = example_action(msg, &ctx);
    assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// Test 2 — PluginContext provides typed storage access
// ---------------------------------------------------------------------------

#[tokio::test]
async fn plugin_context_provides_typed_storage_access() {
    let ctx = mock_ctx();

    // doc_read returns typed data
    let read_result = ctx.storage.doc_read("contacts", "1").await;
    assert!(read_result.is_ok());
    let doc = read_result.unwrap();
    assert!(doc.is_some());

    // doc_write accepts typed data
    let write_result = ctx
        .storage
        .doc_write("contacts", "1", serde_json::json!({"name": "test"}))
        .await;
    assert!(write_result.is_ok());

    // events.emit is available
    let emit_result = ctx.events.emit("test.event", serde_json::json!({})).await;
    assert!(emit_result.is_ok());

    // config.read is available
    let config_result = ctx.config.read("api_key").await;
    assert!(config_result.is_ok());

    // http.request is available
    let http_result = ctx
        .http
        .request(HttpMethod::Get, "https://example.com", None, None)
        .await;
    assert!(http_result.is_ok());
}

// ---------------------------------------------------------------------------
// Test 3 — Lifecycle hooks have default no-op implementations
// ---------------------------------------------------------------------------

struct BarePlugin;
impl LifecycleHooks for BarePlugin {}

#[tokio::test]
async fn lifecycle_hooks_have_default_noop_implementations() {
    let plugin = BarePlugin;
    let ctx = mock_ctx();

    let init_result = plugin.init(&ctx).await;
    assert!(init_result.is_ok(), "default init should be a no-op success");

    let shutdown_result = plugin.shutdown(&ctx).await;
    assert!(
        shutdown_result.is_ok(),
        "default shutdown should be a no-op success"
    );
}

// ---------------------------------------------------------------------------
// Test 4 — Hard failure returns PluginError
// ---------------------------------------------------------------------------

fn failing_action(
    _input: PipelineMessage,
    _ctx: &ActionContext,
) -> Result<PipelineMessage, PluginError> {
    Err(PluginError::StorageError {
        message: "database unreachable".into(),
        detail: Some("connection timed out after 30s".into()),
    })
}

#[test]
fn hard_failure_returns_plugin_error() {
    let ctx = mock_ctx();
    let msg = test_message();
    let result = failing_action(msg, &ctx);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code(), "STORAGE_ERROR");
    assert_eq!(err.message(), "database unreachable");
    assert_eq!(err.detail(), Some("connection timed out after 30s"));
}

// ---------------------------------------------------------------------------
// Test 5 — Soft warning appends to message warnings
// ---------------------------------------------------------------------------

fn action_with_warning(
    mut input: PipelineMessage,
    _ctx: &ActionContext,
) -> Result<PipelineMessage, PluginError> {
    // Action succeeds but appends a warning about a deprecated field.
    input
        .metadata
        .warnings
        .push("field 'legacy_id' is deprecated".into());
    Ok(input)
}

#[test]
fn soft_warning_appends_to_message_warnings() {
    let ctx = mock_ctx();
    let msg = test_message();
    assert!(msg.metadata.warnings.is_empty());

    let result = action_with_warning(msg, &ctx);
    assert!(result.is_ok());

    let output = result.unwrap();
    assert_eq!(output.metadata.warnings.len(), 1);
    assert_eq!(
        output.metadata.warnings[0],
        "field 'legacy_id' is deprecated"
    );
}

// ---------------------------------------------------------------------------
// Test 6 — PluginError has code/message/detail fields
// ---------------------------------------------------------------------------

#[test]
fn plugin_error_has_code_message_detail_fields() {
    let variants: Vec<PluginError> = vec![
        PluginError::CapabilityDenied {
            message: "denied".into(),
            detail: Some("storage:read".into()),
        },
        PluginError::NotFound {
            message: "missing".into(),
            detail: None,
        },
        PluginError::ValidationError {
            message: "invalid".into(),
            detail: Some("'email' is required".into()),
        },
        PluginError::StorageError {
            message: "db error".into(),
            detail: None,
        },
        PluginError::NetworkError {
            message: "timeout".into(),
            detail: Some("after 30s".into()),
        },
        PluginError::InternalError {
            message: "panic".into(),
            detail: Some("index out of bounds".into()),
        },
    ];

    let expected_codes = [
        "CAPABILITY_DENIED",
        "NOT_FOUND",
        "VALIDATION_ERROR",
        "STORAGE_ERROR",
        "NETWORK_ERROR",
        "INTERNAL_ERROR",
    ];

    for (err, expected_code) in variants.iter().zip(expected_codes.iter()) {
        // code() is accessible
        assert_eq!(err.code(), *expected_code);
        // message() is accessible
        assert!(!err.message().is_empty());
        // detail() returns Option<&str>
        let _ = err.detail();
        // Display includes code and message
        let display = err.to_string();
        assert!(display.contains(expected_code));
        assert!(display.contains(err.message()));
    }
}

// ---------------------------------------------------------------------------
// Bonus: Connector pattern — read config → fetch → normalise → write → emit
// ---------------------------------------------------------------------------

#[tokio::test]
async fn connector_pattern_read_fetch_normalise_write_emit() {
    let ctx = mock_ctx();

    // 1. Read config
    let config_val = ctx.config.read("sync_url").await.unwrap();
    assert!(config_val.is_some());

    // 2. Fetch via HTTP
    let response = ctx
        .http
        .request(HttpMethod::Get, "https://api.example.com/data", None, None)
        .await
        .unwrap();
    assert_eq!(response.status, 200);

    // 3. Normalise (application logic — just verify we can transform data)
    let normalised = serde_json::json!({"name": "normalised"});

    // 4. Write to storage
    let write_result = ctx
        .storage
        .doc_write("contacts", "1", normalised)
        .await;
    assert!(write_result.is_ok());

    // 5. Emit completion event
    let emit_result = ctx
        .events
        .emit(
            "connector-email.fetch.completed",
            serde_json::json!({"count": 1}),
        )
        .await;
    assert!(emit_result.is_ok());
}
