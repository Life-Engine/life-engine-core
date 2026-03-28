//! Smoke test for the Life Engine Plugin SDK.
//!
//! Verifies that a minimal plugin can be implemented using only the
//! `life-engine-plugin-sdk` crate, exercising the core SDK surface:
//! Plugin trait, Action, PipelineMessage, EngineError, and register_plugin!.

use life_engine_plugin_sdk::prelude::*;
use std::fmt;

/// Minimal test plugin implementing the Plugin trait.
#[derive(Default)]
struct TestPlugin;

impl Plugin for TestPlugin {
    fn id(&self) -> &str {
        "test-plugin"
    }

    fn display_name(&self) -> &str {
        "Test Plugin"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn actions(&self) -> Vec<Action> {
        vec![
            Action::new("echo", "Returns the input unchanged"),
            Action::new("transform", "Transforms the input"),
        ]
    }

    fn execute(
        &self,
        action: &str,
        input: PipelineMessage,
    ) -> Result<PipelineMessage, Box<dyn EngineError>> {
        match action {
            "echo" | "transform" => Ok(input),
            other => Err(Box::new(TestPluginError(other.to_string()))),
        }
    }
}

// Verify the register_plugin! macro compiles (only active on wasm32).
life_engine_plugin_sdk::register_plugin!(TestPlugin);

#[derive(Debug)]
struct TestPluginError(String);

impl fmt::Display for TestPluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown action: {}", self.0)
    }
}

impl std::error::Error for TestPluginError {}

impl EngineError for TestPluginError {
    fn code(&self) -> &str {
        "TEST_001"
    }
    fn severity(&self) -> Severity {
        Severity::Fatal
    }
    fn source_module(&self) -> &str {
        "test-plugin"
    }
}

/// Helper to build a minimal PipelineMessage for testing.
fn make_message() -> PipelineMessage {
    use life_engine_plugin_sdk::{
        CdmType, MessageMetadata, Note, NoteFormat, TypedPayload,
    };
    use chrono::Utc;
    use uuid::Uuid;

    PipelineMessage {
        metadata: MessageMetadata {
            correlation_id: Uuid::new_v4(),
            source: "smoke-test".to_string(),
            timestamp: Utc::now(),
            auth_context: None,
            warnings: vec![],
        },
        payload: TypedPayload::Cdm(Box::new(CdmType::Note(Note {
            id: Uuid::new_v4(),
            title: "Smoke Test".to_string(),
            body: "Hello from the SDK smoke test".to_string(),
            format: Some(NoteFormat::Plain),
            pinned: None,
            tags: vec![],
            source: "test".to_string(),
            source_id: "smoke-1".to_string(),
            extensions: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }))),
    }
}

#[test]
fn plugin_metadata_is_correct() {
    let plugin = TestPlugin;
    assert_eq!(plugin.id(), "test-plugin");
    assert_eq!(plugin.display_name(), "Test Plugin");
    assert_eq!(plugin.version(), "0.1.0");
}

#[test]
fn plugin_declares_actions() {
    let plugin = TestPlugin;
    let actions = plugin.actions();
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0].name, "echo");
    assert_eq!(actions[1].name, "transform");
}

#[test]
fn plugin_executes_known_action() {
    let plugin = TestPlugin;
    let msg = make_message();
    let correlation_id = msg.metadata.correlation_id;

    let result = plugin.execute("echo", msg);
    assert!(result.is_ok());
    let output = result.unwrap();
    assert_eq!(output.metadata.correlation_id, correlation_id);
}

#[test]
fn plugin_rejects_unknown_action() {
    let plugin = TestPlugin;
    let msg = make_message();

    let result = plugin.execute("nonexistent", msg);
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert_eq!(err.code(), "TEST_001");
    assert_eq!(err.severity(), Severity::Fatal);
    assert_eq!(err.source_module(), "test-plugin");
}

#[test]
fn pipeline_message_serialization_round_trip() {
    let msg = make_message();
    let json = life_engine_plugin_sdk::serde_json::to_string(&msg).expect("serialize");
    let restored: PipelineMessage =
        life_engine_plugin_sdk::serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.metadata.correlation_id, msg.metadata.correlation_id);
}

#[test]
fn plugin_invocation_envelope_round_trip() {
    use life_engine_plugin_sdk::PluginInvocation;

    let invocation = PluginInvocation {
        action: "echo".to_string(),
        message: make_message(),
    };

    let json = life_engine_plugin_sdk::serde_json::to_string(&invocation).expect("serialize");
    let restored: PluginInvocation =
        life_engine_plugin_sdk::serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.action, "echo");
}

#[test]
fn single_dependency_ergonomics() {
    // Verify that all essential types are accessible from the SDK crate alone.
    // If any of these fail to compile, it means the SDK is missing re-exports.
    let _: Option<Action> = None;
    let _: Option<PipelineMessage> = None;
    let _: Option<Severity> = None;
    // StorageContext<S> requires a StorageBackend type parameter — verify it's accessible
    fn _assert_storage_context_usable<S: life_engine_plugin_sdk::StorageBackend>() {
        let _: Option<StorageContext<S>> = None;
    }
    let _: Option<Capability> = None;

    // CDM types available through the SDK
    let _: Option<life_engine_plugin_sdk::Task> = None;
    let _: Option<life_engine_plugin_sdk::CalendarEvent> = None;
    let _: Option<life_engine_plugin_sdk::Contact> = None;
    let _: Option<life_engine_plugin_sdk::Email> = None;
    let _: Option<life_engine_plugin_sdk::Note> = None;
    let _: Option<life_engine_plugin_sdk::FileMetadata> = None;
    let _: Option<life_engine_plugin_sdk::Credential> = None;
}
