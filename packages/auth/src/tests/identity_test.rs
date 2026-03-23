//! Tests for authenticated identity propagation through pipeline messages.
//!
//! Verifies that `AuthIdentity` set as `auth_context` on a `PipelineMessage`
//! survives serialization round-trips (simulating the WASM plugin boundary)
//! and is preserved through plugin execution.

use chrono::Utc;
use life_engine_types::{
    CdmType, MessageMetadata, PipelineMessage, Task, TaskPriority, TaskStatus, TypedPayload,
};
use uuid::Uuid;

use crate::types::AuthIdentity;

/// Build a minimal `PipelineMessage` with optional auth context.
fn make_message(auth_context: Option<serde_json::Value>) -> PipelineMessage {
    PipelineMessage {
        metadata: MessageMetadata {
            correlation_id: Uuid::new_v4(),
            source: "endpoint:POST /tasks".into(),
            timestamp: Utc::now(),
            auth_context,
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

/// Build an `AuthIdentity` for testing.
fn make_identity() -> AuthIdentity {
    AuthIdentity {
        user_id: "user-abc-123".into(),
        provider: "pocket-id".into(),
        scopes: vec!["read".into(), "write".into()],
        authenticated_at: Utc::now(),
    }
}

#[test]
fn auth_identity_survives_pipeline_round_trip() {
    let identity = make_identity();
    let identity_json = serde_json::to_value(&identity).expect("serialize identity");
    let msg = make_message(Some(identity_json));

    // Simulate crossing the WASM boundary: serialize to JSON and back.
    let serialized = serde_json::to_string(&msg).expect("serialize message");
    let restored: PipelineMessage =
        serde_json::from_str(&serialized).expect("deserialize message");

    let ctx = restored
        .metadata
        .auth_context
        .expect("auth_context should be present after round-trip");
    let restored_identity: AuthIdentity =
        serde_json::from_value(ctx).expect("deserialize identity from auth_context");

    assert_eq!(restored_identity.user_id, "user-abc-123");
    assert_eq!(restored_identity.provider, "pocket-id");
    assert_eq!(restored_identity.scopes, vec!["read", "write"]);
}

#[test]
fn auth_context_none_preserved_when_unauthenticated() {
    let msg = make_message(None);

    let serialized = serde_json::to_string(&msg).expect("serialize");
    let restored: PipelineMessage = serde_json::from_str(&serialized).expect("deserialize");

    assert!(
        restored.metadata.auth_context.is_none(),
        "auth_context should remain None for unauthenticated messages"
    );
}

#[test]
fn auth_identity_preserved_through_plugin_output() {
    let identity = make_identity();
    let identity_json = serde_json::to_value(&identity).expect("serialize identity");
    let input = make_message(Some(identity_json));

    // Simulate plugin execution: plugin receives input, produces output
    // with metadata copied from input (as plugins should do).
    let output = PipelineMessage {
        metadata: input.metadata.clone(),
        payload: input.payload.clone(),
    };

    let ctx = output
        .metadata
        .auth_context
        .expect("auth_context should propagate to plugin output");
    let output_identity: AuthIdentity =
        serde_json::from_value(ctx).expect("deserialize identity");

    assert_eq!(output_identity.user_id, identity.user_id);
    assert_eq!(output_identity.provider, identity.provider);
    assert_eq!(output_identity.scopes, identity.scopes);
}

#[test]
fn auth_context_not_present_in_serialized_json_when_none() {
    let msg = make_message(None);
    let serialized = serde_json::to_string(&msg).expect("serialize");

    // The `skip_serializing_if = "Option::is_none"` annotation should omit the field.
    assert!(
        !serialized.contains("auth_context"),
        "auth_context field should be omitted from JSON when None"
    );
}

#[test]
fn api_key_identity_survives_pipeline_round_trip() {
    let identity = AuthIdentity {
        user_id: "service-bot-42".into(),
        provider: "api-key".into(),
        scopes: vec!["automation".into()],
        authenticated_at: Utc::now(),
    };
    let identity_json = serde_json::to_value(&identity).expect("serialize");
    let msg = make_message(Some(identity_json));

    let serialized = serde_json::to_string(&msg).expect("serialize message");
    let restored: PipelineMessage = serde_json::from_str(&serialized).expect("deserialize");

    let ctx = restored.metadata.auth_context.expect("should have auth_context");
    let restored_id: AuthIdentity = serde_json::from_value(ctx).expect("deserialize identity");

    assert_eq!(restored_id.user_id, "service-bot-42");
    assert_eq!(restored_id.provider, "api-key");
    assert_eq!(restored_id.scopes, vec!["automation"]);
}
