//! Integration tests for Phase 2 schema infrastructure.
//!
//! Tests that exercise SchemaRegistry + index hints + versioning checker
//! working together as a system.

use life_engine_traits::index_hints::{
    merge_index_hints, parse_index_hints, CollectionDescriptor, IndexHint,
};
use life_engine_traits::schema::SchemaRegistry;
use life_engine_traits::schema_versioning::{
    check_compatibility, ChangeKind, CompatibilityResult, DeprecationTracker,
};
use serde_json::json;

// ===========================================================================
// Schema versioning integration tests
// ===========================================================================

/// Req 2.1: Adding an optional field is non-breaking (Compatible).
#[test]
fn versioning_add_optional_field_is_compatible() {
    let old = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" }
        },
        "required": ["name"]
    });

    let new = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "nickname": { "type": "string" }
        },
        "required": ["name"]
    });

    assert_eq!(check_compatibility(&old, &new), CompatibilityResult::Compatible);
}

/// Req 3.1: Removing a field is Breaking.
#[test]
fn versioning_remove_field_is_breaking() {
    let old = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "email": { "type": "string" }
        },
        "required": ["name"]
    });

    let new = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" }
        },
        "required": ["name"]
    });

    match check_compatibility(&old, &new) {
        CompatibilityResult::Breaking(changes) => {
            assert_eq!(changes.len(), 1);
            assert_eq!(changes[0].kind, ChangeKind::FieldRemoved);
            assert!(changes[0].path.contains("email"));
        }
        CompatibilityResult::Compatible => panic!("Expected breaking change for field removal"),
    }
}

/// Req 3.3: Changing a field's type is Breaking.
#[test]
fn versioning_change_type_is_breaking() {
    let old = json!({
        "type": "object",
        "properties": {
            "age": { "type": "integer" }
        }
    });

    let new = json!({
        "type": "object",
        "properties": {
            "age": { "type": "string" }
        }
    });

    match check_compatibility(&old, &new) {
        CompatibilityResult::Breaking(changes) => {
            assert!(changes.iter().any(|c| c.kind == ChangeKind::TypeChanged));
        }
        CompatibilityResult::Compatible => panic!("Expected breaking change for type change"),
    }
}

/// Req 3.4: Adding a required field is Breaking.
#[test]
fn versioning_add_required_field_is_breaking() {
    let old = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" }
        },
        "required": ["name"]
    });

    let new = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "email": { "type": "string" }
        },
        "required": ["name", "email"]
    });

    match check_compatibility(&old, &new) {
        CompatibilityResult::Breaking(changes) => {
            assert!(changes.iter().any(|c| c.kind == ChangeKind::RequiredFieldAdded));
        }
        CompatibilityResult::Compatible => {
            panic!("Expected breaking change for adding required field")
        }
    }
}

/// Req 3.5: Removing an enum value is Breaking.
#[test]
fn versioning_remove_enum_value_is_breaking() {
    let old = json!({
        "type": "object",
        "properties": {
            "status": {
                "type": "string",
                "enum": ["active", "inactive", "pending"]
            }
        }
    });

    let new = json!({
        "type": "object",
        "properties": {
            "status": {
                "type": "string",
                "enum": ["active", "inactive"]
            }
        }
    });

    match check_compatibility(&old, &new) {
        CompatibilityResult::Breaking(changes) => {
            assert!(changes.iter().any(|c| c.kind == ChangeKind::EnumValueRemoved));
        }
        CompatibilityResult::Compatible => {
            panic!("Expected breaking change for enum value removal")
        }
    }
}

/// Req 2.2: Adding a new enum value is non-breaking (Compatible).
#[test]
fn versioning_add_enum_value_is_compatible() {
    let old = json!({
        "type": "object",
        "properties": {
            "status": {
                "type": "string",
                "enum": ["active", "inactive"]
            }
        }
    });

    let new = json!({
        "type": "object",
        "properties": {
            "status": {
                "type": "string",
                "enum": ["active", "inactive", "pending"]
            }
        }
    });

    assert_eq!(check_compatibility(&old, &new), CompatibilityResult::Compatible);
}

/// Req 3.7: Tightening a constraint (adding pattern) is Breaking.
#[test]
fn versioning_tighten_constraint_is_breaking() {
    let old = json!({
        "type": "object",
        "properties": {
            "code": { "type": "string" }
        }
    });

    let new = json!({
        "type": "object",
        "properties": {
            "code": { "type": "string", "pattern": "^[A-Z]{3}$" }
        }
    });

    match check_compatibility(&old, &new) {
        CompatibilityResult::Breaking(changes) => {
            assert!(changes.iter().any(|c| c.kind == ChangeKind::ConstraintTightened));
        }
        CompatibilityResult::Compatible => {
            panic!("Expected breaking change for constraint tightening")
        }
    }
}

/// Req 2.4: Relaxing a constraint (reducing minLength) is non-breaking.
#[test]
fn versioning_relax_constraint_is_compatible() {
    let old = json!({
        "type": "object",
        "properties": {
            "code": { "type": "string", "minLength": 5 }
        }
    });

    let new = json!({
        "type": "object",
        "properties": {
            "code": { "type": "string", "minLength": 3 }
        }
    });

    assert_eq!(check_compatibility(&old, &new), CompatibilityResult::Compatible);
}

/// Req 4.1: Changing a default value is Breaking.
#[test]
fn versioning_change_default_is_breaking() {
    let old = json!({
        "type": "object",
        "properties": {
            "priority": { "type": "integer", "default": 0 }
        }
    });

    let new = json!({
        "type": "object",
        "properties": {
            "priority": { "type": "integer", "default": 5 }
        }
    });

    match check_compatibility(&old, &new) {
        CompatibilityResult::Breaking(changes) => {
            assert!(changes.iter().any(|c| c.kind == ChangeKind::DefaultChanged));
        }
        CompatibilityResult::Compatible => {
            panic!("Expected breaking change for default value change")
        }
    }
}

/// Identical schemas should be Compatible.
#[test]
fn versioning_identical_schemas_compatible() {
    let schema = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "age": { "type": "integer" }
        },
        "required": ["name"]
    });

    assert_eq!(
        check_compatibility(&schema, &schema),
        CompatibilityResult::Compatible
    );
}

// ===========================================================================
// Deprecation tracker integration tests
// ===========================================================================

/// Req 10: Deprecated fields tracked, removal without deprecation produces warning.
#[test]
fn deprecation_tracker_warns_on_undeprecated_removal() {
    let old = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "legacy_id": { "type": "string" }
        }
    });

    let new = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" }
        }
    });

    let tracker = DeprecationTracker::new();
    let warnings = tracker.check_removal_allowed(&old, &new);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].path.contains("legacy_id"));
}

/// Req 10: Removal of a properly deprecated field produces no warning.
#[test]
fn deprecation_tracker_allows_deprecated_removal() {
    let old = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "legacy_id": {
                "type": "string",
                "deprecated": true,
                "x-deprecated-since": "1.3.0",
                "x-deprecated-note": "Use 'id' instead"
            }
        }
    });

    let new = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" }
        }
    });

    let mut tracker = DeprecationTracker::new();
    tracker.scan_schema(&old);

    assert_eq!(tracker.deprecations().len(), 1);
    assert_eq!(tracker.deprecations()[0].deprecated_since, "1.3.0");

    let warnings = tracker.check_removal_allowed(&old, &new);
    assert!(warnings.is_empty(), "Expected no warnings for deprecated field removal");
}

// ===========================================================================
// Index hints + CollectionDescriptor integration tests
// ===========================================================================

/// Full flow: parse manifest → merge CDM hints → build CollectionDescriptor.
#[test]
fn index_hints_full_manifest_to_descriptor_flow() {
    // CDM ships default index on created_at.
    let cdm_hints = vec![IndexHint {
        fields: vec!["created_at".to_string()],
        unique: false,
        name: Some("cdm_created_at".to_string()),
    }];

    // Plugin manifest declares indexes, overriding CDM on created_at.
    let manifest = json!({
        "name": "tasks",
        "schema": "cdm:tasks",
        "strict": true,
        "indexes": [
            { "fields": ["email"], "unique": true, "name": "idx_email" },
            { "fields": ["created_at"], "unique": false, "name": "plugin_created_at" }
        ]
    });

    let plugin_hints = parse_index_hints(&manifest).unwrap();
    assert_eq!(plugin_hints.len(), 2);

    let merged = merge_index_hints(&cdm_hints, &plugin_hints);

    // CDM created_at hint replaced by plugin's; email is new.
    assert_eq!(merged.len(), 2);

    let created_at_hint = merged.iter().find(|h| h.fields.contains(&"created_at".to_string())).unwrap();
    assert_eq!(created_at_hint.name, Some("plugin_created_at".to_string()));

    let email_hint = merged.iter().find(|h| h.fields.contains(&"email".to_string())).unwrap();
    assert!(email_hint.unique);

    // Build the descriptor with merged indexes.
    let descriptor = CollectionDescriptor {
        name: "tasks".to_string(),
        plugin_id: "task-manager".to_string(),
        schema: Some("cdm:tasks".to_string()),
        strict: true,
        indexes: merged,
    };

    assert_eq!(descriptor.name, "tasks");
    assert_eq!(descriptor.plugin_id, "task-manager");
    assert_eq!(descriptor.schema, Some("cdm:tasks".to_string()));
    assert!(descriptor.strict);
    assert_eq!(descriptor.indexes.len(), 2);
}

/// Schemaless collection with no indexes: validates the "skip everything" path.
#[test]
fn index_hints_schemaless_collection_no_indexes() {
    let manifest = json!({
        "name": "cache"
    });

    let hints = parse_index_hints(&manifest).unwrap();
    assert!(hints.is_empty());

    let descriptor = CollectionDescriptor {
        name: "cache".to_string(),
        plugin_id: "my-plugin".to_string(),
        schema: None,
        strict: false,
        indexes: hints,
    };

    assert!(descriptor.schema.is_none());
    assert!(descriptor.indexes.is_empty());
}

// ===========================================================================
// Cross-module integration: versioning + index hints
// ===========================================================================

/// Schema evolution that adds an optional field + new index hint:
/// the version change is Compatible AND the new index merges cleanly.
#[test]
fn versioning_plus_index_hints_compatible_evolution() {
    let old_schema = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "email": { "type": "string" }
        },
        "required": ["name"]
    });

    let new_schema = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "email": { "type": "string" },
            "phone": { "type": "string" }
        },
        "required": ["name"]
    });

    // Versioning check: adding optional field is compatible.
    assert_eq!(
        check_compatibility(&old_schema, &new_schema),
        CompatibilityResult::Compatible
    );

    // The plugin adds an index on the new field.
    let manifest = json!({
        "indexes": [
            { "fields": ["phone"], "unique": false }
        ]
    });

    let plugin_hints = parse_index_hints(&manifest).unwrap();
    let cdm_hints = vec![IndexHint {
        fields: vec!["email".to_string()],
        unique: true,
        name: None,
    }];

    let merged = merge_index_hints(&cdm_hints, &plugin_hints);
    assert_eq!(merged.len(), 2);
}

/// Schema evolution that removes a field is Breaking, and index hints
/// referencing the removed field should still parse (adapter decides).
#[test]
fn versioning_plus_index_hints_breaking_evolution() {
    let old_schema = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "legacy_code": { "type": "string" }
        }
    });

    let new_schema = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" }
        }
    });

    // Versioning check: removing field is breaking.
    match check_compatibility(&old_schema, &new_schema) {
        CompatibilityResult::Breaking(changes) => {
            assert!(changes.iter().any(|c| c.kind == ChangeKind::FieldRemoved));
        }
        CompatibilityResult::Compatible => {
            panic!("Expected breaking change for field removal")
        }
    }

    // Index hints can still reference legacy_code — parsing is independent.
    let manifest = json!({
        "indexes": [
            { "fields": ["legacy_code"], "unique": false }
        ]
    });
    let hints = parse_index_hints(&manifest).unwrap();
    assert_eq!(hints.len(), 1);
    assert_eq!(hints[0].fields, vec!["legacy_code"]);
}

// ===========================================================================
// SchemaRegistry integration tests
//
// Full validation pipeline: schema load → document write → validate → accept/reject
// ===========================================================================

/// Req 1.2 + 2.2: Load CDM schemas → validate a conforming event → accept.
#[test]
fn registry_load_cdm_validate_valid_event_accept() {
    let mut registry = SchemaRegistry::new();
    registry.load_cdm_schemas().unwrap();

    // Register a plugin collection that references the CDM events schema.
    registry
        .register_plugin_collection("cal-plugin", "events", Some("cdm:events"), None, false, vec![])
        .unwrap();

    let valid_event = json!({
        "title": "Team standup",
        "start": "2026-03-28T09:00:00Z",
        "end": "2026-03-28T09:30:00Z",
        "source": "google-calendar",
        "source_id": "evt_123"
    });

    let result = registry.validate_write("events", "cal-plugin", &valid_event, true, None);
    assert!(result.is_ok(), "Expected valid CDM event to be accepted, got: {:?}", result.err());
}

/// Req 2.4: Load CDM schemas → validate an invalid event → reject with details.
#[test]
fn registry_load_cdm_validate_invalid_event_reject() {
    let mut registry = SchemaRegistry::new();
    registry.load_cdm_schemas().unwrap();

    registry
        .register_plugin_collection("cal-plugin", "events", Some("cdm:events"), None, false, vec![])
        .unwrap();

    // Missing required "title" field.
    let invalid_event = json!({
        "start": "2026-03-28T09:00:00Z"
    });

    let result = registry.validate_write("events", "cal-plugin", &invalid_event, true, None);
    assert!(result.is_err(), "Expected invalid CDM event to be rejected");
}

/// Req 1.1 + 2.2: Register a plugin's custom schema → validate → accept.
#[test]
fn registry_register_plugin_schema_validate_accept() {
    let mut registry = SchemaRegistry::new();

    let plugin_schema = json!({
        "type": "object",
        "properties": {
            "task_name": { "type": "string" },
            "priority": { "type": "integer", "minimum": 1, "maximum": 5 }
        },
        "required": ["task_name"]
    });

    registry
        .register_plugin_collection(
            "task-plugin",
            "plugin_tasks",
            Some(&plugin_schema.to_string()),
            None,
            false,
            vec![],
        )
        .unwrap();

    let doc = json!({
        "task_name": "Write tests",
        "priority": 3
    });

    let result = registry.validate_write("plugin_tasks", "task-plugin", &doc, true, None);
    assert!(result.is_ok(), "Expected valid plugin doc to be accepted, got: {:?}", result.err());
}

/// Req 1.3 + 2.5: Schemaless collection → validation is skipped entirely.
#[test]
fn registry_schemaless_collection_skips_validation() {
    let mut registry = SchemaRegistry::new();

    // Register without a schema (schemaless).
    registry
        .register_plugin_collection("my-plugin", "cache", None, None, false, vec![])
        .unwrap();

    let doc = json!({
        "anything": "goes",
        "nested": { "data": [1, 2, 3] }
    });

    let result = registry.validate_write("cache", "my-plugin", &doc, true, None);
    assert!(result.is_ok(), "Expected schemaless collection to skip validation");
}

/// Req 5.1 + 5.2: Extension namespace enforcement end-to-end.
#[test]
fn registry_extension_namespace_isolation() {
    let mut registry = SchemaRegistry::new();
    registry.load_cdm_schemas().unwrap();

    registry
        .register_plugin_collection("plugin-a", "contacts", Some("cdm:contacts"), None, false, vec![])
        .unwrap();

    // Plugin A writes to its own namespace — should succeed.
    let valid_doc = json!({
        "name": { "given": "Alice", "family": "Smith" },
        "source": "google-contacts",
        "source_id": "c_123",
        "ext": {
            "plugin-a": { "custom_field": "value" }
        }
    });

    let result = registry.validate_write("contacts", "plugin-a", &valid_doc, true, None);
    assert!(result.is_ok(), "Expected own namespace write to succeed, got: {:?}", result.err());

    // Plugin A writes to plugin-b's namespace — should be rejected.
    let invalid_doc = json!({
        "name": { "given": "Alice", "family": "Smith" },
        "source": "google-contacts",
        "source_id": "c_123",
        "ext": {
            "plugin-b": { "hijacked": true }
        }
    });

    let result = registry.validate_write("contacts", "plugin-a", &invalid_doc, true, None);
    assert!(result.is_err(), "Expected cross-namespace write to be rejected");
}

/// Req 3: Strict mode rejects unknown fields.
#[test]
fn registry_strict_mode_rejects_unknown_fields() {
    let mut registry = SchemaRegistry::new();

    let schema = json!({
        "type": "object",
        "properties": {
            "title": { "type": "string" }
        },
        "required": ["title"]
    });

    registry
        .register_plugin_collection(
            "strict-plugin",
            "strict_col",
            Some(&schema.to_string()),
            None,
            true,
            vec![],
        )
        .unwrap();

    // Doc with only declared fields — should pass.
    let valid = json!({ "title": "Hello" });
    assert!(
        registry.validate_write("strict_col", "strict-plugin", &valid, true, None).is_ok()
    );

    // Doc with unknown field — should be rejected in strict mode.
    let invalid = json!({ "title": "Hello", "rogue_field": 42 });
    assert!(
        registry.validate_write("strict_col", "strict-plugin", &invalid, true, None).is_err(),
        "Expected strict mode to reject unknown fields"
    );
}

/// Full lifecycle: register → validate valid → accept, validate invalid → reject,
/// evolve schema (compatible) → re-register → re-validate.
#[test]
fn registry_full_lifecycle_register_validate_evolve() {
    let mut registry = SchemaRegistry::new();

    let v1_schema_str = json!({
        "type": "object",
        "properties": {
            "title": { "type": "string" },
            "status": { "type": "string", "enum": ["open", "closed"] }
        },
        "required": ["title"]
    });

    registry
        .register_plugin_collection(
            "issue-tracker",
            "issues",
            Some(&v1_schema_str.to_string()),
            None,
            false,
            vec![],
        )
        .unwrap();

    // Valid doc passes.
    let valid = json!({ "title": "Bug report", "status": "open" });
    assert!(
        registry.validate_write("issues", "issue-tracker", &valid, true, None).is_ok()
    );

    // Invalid doc (wrong type for title) fails.
    let invalid = json!({ "title": 42 });
    assert!(
        registry.validate_write("issues", "issue-tracker", &invalid, true, None).is_err()
    );

    // Evolve schema: add optional field (compatible per versioning rules).
    let v2_schema_str = json!({
        "type": "object",
        "properties": {
            "title": { "type": "string" },
            "status": { "type": "string", "enum": ["open", "closed"] },
            "assignee": { "type": "string" }
        },
        "required": ["title"]
    });

    assert_eq!(
        check_compatibility(&v1_schema_str, &v2_schema_str),
        CompatibilityResult::Compatible
    );

    // Re-register with updated schema.
    registry
        .register_plugin_collection(
            "issue-tracker",
            "issues",
            Some(&v2_schema_str.to_string()),
            None,
            false,
            vec![],
        )
        .unwrap();

    // Old doc still valid.
    assert!(
        registry.validate_write("issues", "issue-tracker", &valid, true, None).is_ok()
    );

    // New doc with assignee also valid.
    let with_assignee = json!({ "title": "Feature", "assignee": "bob" });
    assert!(
        registry.validate_write("issues", "issue-tracker", &with_assignee, true, None).is_ok()
    );
}

/// SchemaRegistry + index hints: register collection with index hints,
/// verify they're stored in the CollectionSchema.
#[test]
fn registry_with_index_hints_stored() {
    let mut registry = SchemaRegistry::new();

    let schema = json!({
        "type": "object",
        "properties": {
            "email": { "type": "string" },
            "name": { "type": "string" }
        },
        "required": ["email"]
    });

    let hints = vec![
        IndexHint {
            fields: vec!["email".to_string()],
            unique: true,
            name: Some("idx_email".to_string()),
        },
        IndexHint {
            fields: vec!["name".to_string()],
            unique: false,
            name: None,
        },
    ];

    registry
        .register_plugin_collection(
            "contacts-plugin",
            "my_contacts",
            Some(&schema.to_string()),
            None,
            false,
            hints,
        )
        .unwrap();

    let col = registry.get_collection("contacts-plugin", "my_contacts").unwrap();
    assert_eq!(col.index_hints.len(), 2);
    assert_eq!(col.index_hints[0].fields, vec!["email"]);
    assert!(col.index_hints[0].unique);
    assert_eq!(col.index_hints[1].fields, vec!["name"]);
    assert!(!col.index_hints[1].unique);
}
