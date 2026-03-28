//! Unit tests for index hint parsing and merging.

use life_engine_traits::index_hints::{
    merge_index_hints, parse_index_hints, CollectionDescriptor, IndexHint,
};
use serde_json::json;

// ---------------------------------------------------------------------------
// parse_index_hints — single index
// ---------------------------------------------------------------------------

#[test]
fn parse_single_index_hint() {
    let manifest = json!({
        "indexes": [
            { "fields": ["email"], "unique": true }
        ]
    });

    let hints = parse_index_hints(&manifest).unwrap();
    assert_eq!(hints.len(), 1);
    assert_eq!(hints[0].fields, vec!["email"]);
    assert!(hints[0].unique);
    assert_eq!(hints[0].name, None);
}

// ---------------------------------------------------------------------------
// parse_index_hints — multiple indexes with optional name
// ---------------------------------------------------------------------------

#[test]
fn parse_multiple_indexes_with_name() {
    let manifest = json!({
        "indexes": [
            { "fields": ["email"], "unique": true, "name": "idx_email" },
            { "fields": ["created_at", "status"], "unique": false, "name": "idx_created_status" }
        ]
    });

    let hints = parse_index_hints(&manifest).unwrap();
    assert_eq!(hints.len(), 2);

    assert_eq!(hints[0].fields, vec!["email"]);
    assert!(hints[0].unique);
    assert_eq!(hints[0].name, Some("idx_email".to_string()));

    assert_eq!(hints[1].fields, vec!["created_at", "status"]);
    assert!(!hints[1].unique);
    assert_eq!(
        hints[1].name,
        Some("idx_created_status".to_string())
    );
}

// ---------------------------------------------------------------------------
// parse_index_hints — empty indexes list
// ---------------------------------------------------------------------------

#[test]
fn parse_empty_indexes_list() {
    let manifest = json!({
        "indexes": []
    });

    let hints = parse_index_hints(&manifest).unwrap();
    assert!(hints.is_empty());
}

// ---------------------------------------------------------------------------
// parse_index_hints — missing indexes key
// ---------------------------------------------------------------------------

#[test]
fn parse_missing_indexes_key() {
    let manifest = json!({
        "schema": "cdm:contacts"
    });

    let hints = parse_index_hints(&manifest).unwrap();
    assert!(hints.is_empty());
}

// ---------------------------------------------------------------------------
// parse_index_hints — null indexes value
// ---------------------------------------------------------------------------

#[test]
fn parse_null_indexes_value() {
    let manifest = json!({
        "indexes": null
    });

    let hints = parse_index_hints(&manifest).unwrap();
    assert!(hints.is_empty());
}

// ---------------------------------------------------------------------------
// parse_index_hints — unique defaults to false when omitted
// ---------------------------------------------------------------------------

#[test]
fn parse_unique_defaults_to_false() {
    let manifest = json!({
        "indexes": [
            { "fields": ["name"] }
        ]
    });

    let hints = parse_index_hints(&manifest).unwrap();
    assert_eq!(hints.len(), 1);
    assert!(!hints[0].unique);
}

// ---------------------------------------------------------------------------
// parse_index_hints — invalid: missing fields
// ---------------------------------------------------------------------------

#[test]
fn parse_invalid_missing_fields() {
    let manifest = json!({
        "indexes": [
            { "unique": true }
        ]
    });

    let err = parse_index_hints(&manifest).unwrap_err();
    assert!(
        err.message.contains("missing required field 'fields'"),
        "expected fields error, got: {}",
        err.message
    );
}

// ---------------------------------------------------------------------------
// parse_index_hints — invalid: fields is not an array
// ---------------------------------------------------------------------------

#[test]
fn parse_invalid_fields_not_array() {
    let manifest = json!({
        "indexes": [
            { "fields": "email", "unique": true }
        ]
    });

    let err = parse_index_hints(&manifest).unwrap_err();
    assert!(
        err.message.contains("'fields' must be an array"),
        "expected array error, got: {}",
        err.message
    );
}

// ---------------------------------------------------------------------------
// parse_index_hints — invalid: empty fields array
// ---------------------------------------------------------------------------

#[test]
fn parse_invalid_empty_fields_array() {
    let manifest = json!({
        "indexes": [
            { "fields": [], "unique": false }
        ]
    });

    let err = parse_index_hints(&manifest).unwrap_err();
    assert!(
        err.message.contains("'fields' must not be empty"),
        "expected non-empty error, got: {}",
        err.message
    );
}

// ---------------------------------------------------------------------------
// parse_index_hints — invalid: field entry is not a string
// ---------------------------------------------------------------------------

#[test]
fn parse_invalid_field_not_string() {
    let manifest = json!({
        "indexes": [
            { "fields": [123], "unique": false }
        ]
    });

    let err = parse_index_hints(&manifest).unwrap_err();
    assert!(
        err.message.contains("must be a string"),
        "expected string error, got: {}",
        err.message
    );
}

// ---------------------------------------------------------------------------
// parse_index_hints — invalid: indexes is not an array
// ---------------------------------------------------------------------------

#[test]
fn parse_invalid_indexes_not_array() {
    let manifest = json!({
        "indexes": "not_an_array"
    });

    let err = parse_index_hints(&manifest).unwrap_err();
    assert!(
        err.message.contains("indexes must be an array"),
        "expected array error, got: {}",
        err.message
    );
}

// ---------------------------------------------------------------------------
// parse_index_hints — invalid: unique is not a boolean
// ---------------------------------------------------------------------------

#[test]
fn parse_invalid_unique_not_bool() {
    let manifest = json!({
        "indexes": [
            { "fields": ["email"], "unique": "yes" }
        ]
    });

    let err = parse_index_hints(&manifest).unwrap_err();
    assert!(
        err.message.contains("'unique' must be a boolean"),
        "expected boolean error, got: {}",
        err.message
    );
}

// ---------------------------------------------------------------------------
// merge_index_hints — plugin takes precedence on conflict
// ---------------------------------------------------------------------------

#[test]
fn merge_plugin_takes_precedence_on_conflict() {
    let cdm = vec![IndexHint {
        fields: vec!["email".to_string()],
        unique: false,
        name: Some("cdm_email".to_string()),
    }];

    let plugin = vec![IndexHint {
        fields: vec!["email".to_string()],
        unique: true,
        name: Some("plugin_email".to_string()),
    }];

    let merged = merge_index_hints(&cdm, &plugin);

    // Only the plugin version should survive.
    assert_eq!(merged.len(), 1);
    assert!(merged[0].unique);
    assert_eq!(merged[0].name, Some("plugin_email".to_string()));
}

// ---------------------------------------------------------------------------
// merge_index_hints — no conflicts gives union
// ---------------------------------------------------------------------------

#[test]
fn merge_no_conflict_gives_union() {
    let cdm = vec![IndexHint {
        fields: vec!["created_at".to_string()],
        unique: false,
        name: None,
    }];

    let plugin = vec![IndexHint {
        fields: vec!["email".to_string()],
        unique: true,
        name: None,
    }];

    let merged = merge_index_hints(&cdm, &plugin);
    assert_eq!(merged.len(), 2);

    let field_sets: Vec<&Vec<String>> = merged.iter().map(|h| &h.fields).collect();
    assert!(field_sets.contains(&&vec!["created_at".to_string()]));
    assert!(field_sets.contains(&&vec!["email".to_string()]));
}

// ---------------------------------------------------------------------------
// merge_index_hints — empty CDM hints
// ---------------------------------------------------------------------------

#[test]
fn merge_empty_cdm_hints() {
    let plugin = vec![IndexHint {
        fields: vec!["email".to_string()],
        unique: true,
        name: None,
    }];

    let merged = merge_index_hints(&[], &plugin);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].fields, vec!["email"]);
}

// ---------------------------------------------------------------------------
// merge_index_hints — empty plugin hints
// ---------------------------------------------------------------------------

#[test]
fn merge_empty_plugin_hints() {
    let cdm = vec![IndexHint {
        fields: vec!["created_at".to_string()],
        unique: false,
        name: None,
    }];

    let merged = merge_index_hints(&cdm, &[]);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].fields, vec!["created_at"]);
}

// ---------------------------------------------------------------------------
// merge_index_hints — both empty
// ---------------------------------------------------------------------------

#[test]
fn merge_both_empty() {
    let merged = merge_index_hints(&[], &[]);
    assert!(merged.is_empty());
}

// ---------------------------------------------------------------------------
// merge_index_hints — multi-field conflict with different order
// ---------------------------------------------------------------------------

#[test]
fn merge_multi_field_conflict_order_independent() {
    let cdm = vec![IndexHint {
        fields: vec!["city".to_string(), "state".to_string()],
        unique: false,
        name: Some("cdm_location".to_string()),
    }];

    // Plugin declares same fields in different order — should still conflict.
    let plugin = vec![IndexHint {
        fields: vec!["state".to_string(), "city".to_string()],
        unique: true,
        name: Some("plugin_location".to_string()),
    }];

    let merged = merge_index_hints(&cdm, &plugin);
    assert_eq!(merged.len(), 1);
    assert!(merged[0].unique);
    assert_eq!(merged[0].name, Some("plugin_location".to_string()));
}

// ---------------------------------------------------------------------------
// CollectionDescriptor — construction with all fields
// ---------------------------------------------------------------------------

#[test]
fn collection_descriptor_construction() {
    let descriptor = CollectionDescriptor {
        name: "contacts".to_string(),
        plugin_id: "google-contacts".to_string(),
        schema: Some("cdm:contacts".to_string()),
        strict: true,
        indexes: vec![
            IndexHint {
                fields: vec!["email".to_string()],
                unique: true,
                name: Some("idx_email".to_string()),
            },
            IndexHint {
                fields: vec!["last_name".to_string(), "first_name".to_string()],
                unique: false,
                name: None,
            },
        ],
    };

    assert_eq!(descriptor.name, "contacts");
    assert_eq!(descriptor.plugin_id, "google-contacts");
    assert_eq!(descriptor.schema, Some("cdm:contacts".to_string()));
    assert!(descriptor.strict);
    assert_eq!(descriptor.indexes.len(), 2);
}

// ---------------------------------------------------------------------------
// CollectionDescriptor — schemaless collection
// ---------------------------------------------------------------------------

#[test]
fn collection_descriptor_schemaless() {
    let descriptor = CollectionDescriptor {
        name: "cache".to_string(),
        plugin_id: "my-plugin".to_string(),
        schema: None,
        strict: false,
        indexes: vec![],
    };

    assert_eq!(descriptor.name, "cache");
    assert!(descriptor.schema.is_none());
    assert!(!descriptor.strict);
    assert!(descriptor.indexes.is_empty());
}

// ---------------------------------------------------------------------------
// IndexHint — serde round-trip
// ---------------------------------------------------------------------------

#[test]
fn index_hint_serde_round_trip() {
    let hint = IndexHint {
        fields: vec!["email".to_string()],
        unique: true,
        name: Some("idx_email".to_string()),
    };

    let json = serde_json::to_string(&hint).unwrap();
    let restored: IndexHint = serde_json::from_str(&json).unwrap();
    assert_eq!(hint, restored);
}

#[test]
fn index_hint_serde_skips_none_name() {
    let hint = IndexHint {
        fields: vec!["email".to_string()],
        unique: false,
        name: None,
    };

    let json = serde_json::to_string(&hint).unwrap();
    assert!(!json.contains("name"));
}
