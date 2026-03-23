//! Tests for infrastructure contracts.

use crate::capability::{Capability, CapabilityViolation};
use crate::error::{EngineError, Severity};
use crate::plugin::Action;

#[test]
fn severity_display_formatting() {
    assert_eq!(Severity::Fatal.to_string(), "Fatal");
    assert_eq!(Severity::Retryable.to_string(), "Retryable");
    assert_eq!(Severity::Warning.to_string(), "Warning");
}

#[test]
fn severity_convenience_methods() {
    assert!(Severity::Fatal.is_fatal());
    assert!(!Severity::Fatal.is_retryable());
    assert!(!Severity::Fatal.is_warning());

    assert!(!Severity::Retryable.is_fatal());
    assert!(Severity::Retryable.is_retryable());
    assert!(!Severity::Retryable.is_warning());

    assert!(!Severity::Warning.is_fatal());
    assert!(!Severity::Warning.is_retryable());
    assert!(Severity::Warning.is_warning());
}

#[test]
fn engine_error_is_object_safe() {
    // Verify EngineError can be used as a trait object (Box<dyn EngineError>).
    // This compiles only if the trait is object-safe.
    let violation: Box<dyn EngineError> = Box::new(CapabilityViolation {
        capability: Capability::StorageRead,
        plugin_id: "test".to_string(),
        context: "object safety check".to_string(),
        at_load_time: false,
    });
    assert_eq!(violation.code(), "CAP_002");
    assert_eq!(violation.severity(), Severity::Fatal);
    assert_eq!(violation.source_module(), "capability-enforcement");
}

#[test]
fn capability_fromstr_display_round_trip_all_variants() {
    let all = [
        Capability::StorageRead,
        Capability::StorageWrite,
        Capability::HttpOutbound,
        Capability::EventsEmit,
        Capability::EventsSubscribe,
        Capability::ConfigRead,
    ];

    for cap in &all {
        let s = cap.to_string();
        let parsed: Capability = s.parse().expect("should parse");
        assert_eq!(*cap, parsed);
    }
}

#[test]
fn capability_violation_error_codes() {
    let load_time = CapabilityViolation {
        capability: Capability::StorageWrite,
        plugin_id: "p1".to_string(),
        context: "load".to_string(),
        at_load_time: true,
    };
    assert_eq!(load_time.code(), "CAP_001");

    let runtime = CapabilityViolation {
        capability: Capability::HttpOutbound,
        plugin_id: "p2".to_string(),
        context: "runtime".to_string(),
        at_load_time: false,
    };
    assert_eq!(runtime.code(), "CAP_002");
}

#[test]
fn action_serialization_round_trip() {
    let action = Action {
        name: "sync_contacts".to_string(),
        description: "Synchronize contacts from provider".to_string(),
        input_schema: Some(r#"{"type": "object"}"#.to_string()),
        output_schema: None,
    };

    let json = serde_json::to_string(&action).expect("serialize");
    let restored: Action = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(action.name, restored.name);
    assert_eq!(action.description, restored.description);
    assert_eq!(action.input_schema, restored.input_schema);
    assert_eq!(action.output_schema, restored.output_schema);
}

#[test]
fn action_serialization_skips_none_schemas() {
    let action = Action {
        name: "greet".to_string(),
        description: "Say hello".to_string(),
        input_schema: None,
        output_schema: None,
    };

    let json = serde_json::to_string(&action).expect("serialize");
    assert!(!json.contains("input_schema"));
    assert!(!json.contains("output_schema"));
}

#[test]
fn action_builder_creates_with_defaults() {
    let action = Action::new("greet", "Say hello");

    assert_eq!(action.name, "greet");
    assert_eq!(action.description, "Say hello");
    assert_eq!(action.input_schema, None);
    assert_eq!(action.output_schema, None);
}

#[test]
fn action_builder_with_schemas() {
    let input = r#"{"type": "object"}"#;
    let output = r#"{"type": "string"}"#;
    let action = Action::new("transform", "Transform data")
        .with_input_schema(input)
        .with_output_schema(output);

    assert_eq!(action.name, "transform");
    assert_eq!(action.input_schema, Some(input.to_string()));
    assert_eq!(action.output_schema, Some(output.to_string()));
}

#[test]
fn action_builder_serde_round_trip() {
    let action = Action::new("sync", "Sync data")
        .with_input_schema(r#"{"type": "object"}"#);

    let json = serde_json::to_string(&action).expect("serialize");
    let restored: Action = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(action, restored);
}

#[test]
fn re_exports_are_accessible() {
    // Verify that types re-exported from lib.rs are accessible.
    // This is a compile-time check — if any re-export is missing, this won't compile.
    use crate::{
        Action, Capability, CapabilityViolation, EngineError, Plugin, Severity, StorageBackend,
        StorageMutation, StorageQuery, TlsConfig, Transport, TransportConfig,
    };

    // Use each type to prevent unused import warnings.
    let _ = Severity::Fatal;
    let _ = Capability::StorageRead;
    let _ = std::any::type_name::<Box<dyn EngineError>>();
    let _ = std::any::type_name::<dyn StorageBackend>();
    let _ = std::any::type_name::<dyn Transport>();
    let _ = std::any::type_name::<dyn Plugin>();
    let _ = std::any::type_name::<Action>();
    let _ = std::any::type_name::<CapabilityViolation>();
    let _ = std::any::type_name::<TransportConfig>();
    let _ = std::any::type_name::<TlsConfig>();
    let _ = std::any::type_name::<StorageQuery>();
    let _ = std::any::type_name::<StorageMutation>();
}
