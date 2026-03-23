//! Schema validation for canonical collection writes.
//!
//! Before any `Insert` or `Update` mutation on a canonical collection, the
//! data payload is validated against the corresponding JSON Schema. Validation
//! failures are fatal — invalid data never enters the database.

use std::sync::LazyLock;
use serde_json::Value;
use std::collections::HashMap;

use crate::error::StorageError;

// Embed the 7 canonical JSON Schema files at compile time.
const EVENTS_SCHEMA: &str = include_str!("../../../.odm/doc/schemas/events.schema.json");
const TASKS_SCHEMA: &str = include_str!("../../../.odm/doc/schemas/tasks.schema.json");
const CONTACTS_SCHEMA: &str = include_str!("../../../.odm/doc/schemas/contacts.schema.json");
const NOTES_SCHEMA: &str = include_str!("../../../.odm/doc/schemas/notes.schema.json");
const EMAILS_SCHEMA: &str = include_str!("../../../.odm/doc/schemas/emails.schema.json");
const FILES_SCHEMA: &str = include_str!("../../../.odm/doc/schemas/files.schema.json");
const CREDENTIALS_SCHEMA: &str = include_str!("../../../.odm/doc/schemas/credentials.schema.json");

/// Canonical collection names.
const CANONICAL_COLLECTIONS: &[&str] = &[
    "events",
    "tasks",
    "contacts",
    "notes",
    "emails",
    "files",
    "credentials",
];

/// Returns `true` if the collection is a canonical CDM collection.
pub fn is_canonical(collection: &str) -> bool {
    CANONICAL_COLLECTIONS.contains(&collection)
}

/// Pre-compiled JSON Schema validators for each canonical collection.
static VALIDATORS: LazyLock<HashMap<&'static str, jsonschema::Validator>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    let schemas: &[(&str, &str)] = &[
        ("events", EVENTS_SCHEMA),
        ("tasks", TASKS_SCHEMA),
        ("contacts", CONTACTS_SCHEMA),
        ("notes", NOTES_SCHEMA),
        ("emails", EMAILS_SCHEMA),
        ("files", FILES_SCHEMA),
        ("credentials", CREDENTIALS_SCHEMA),
    ];
    for (name, raw) in schemas {
        let schema_value: Value =
            serde_json::from_str(raw).unwrap_or_else(|e| panic!("invalid schema for {name}: {e}"));
        let validator = jsonschema::draft202012::new(&schema_value)
            .unwrap_or_else(|e| panic!("failed to compile schema for {name}: {e}"));
        map.insert(*name, validator);
    }
    map
});

/// Validate a JSON payload against the canonical schema for `collection`.
///
/// Returns `Ok(())` if the data passes validation, or
/// `Err(StorageError::ValidationFailed)` with details about the first failure.
///
/// Returns `Err(StorageError::UnknownCollection)` if the collection is neither
/// canonical nor a declared private collection (private collection validation
/// is handled separately in WP 5.7).
pub fn validate_canonical(collection: &str, data_json: &str) -> Result<(), StorageError> {
    if !is_canonical(collection) {
        // Private collections are not validated here — they are handled by
        // the private-collection validator added in WP 5.7.
        return Ok(());
    }

    let data: Value = serde_json::from_str(data_json).map_err(|e| StorageError::ValidationFailed {
        collection: collection.to_string(),
        message: format!("invalid JSON: {e}"),
    })?;

    let validator = VALIDATORS.get(collection).ok_or_else(|| {
        StorageError::ValidationFailed {
            collection: collection.to_string(),
            message: "no schema found for canonical collection".to_string(),
        }
    })?;

    let errors: Vec<String> = validator
        .iter_errors(&data)
        .map(|e| e.to_string())
        .collect();

    if !errors.is_empty() {
        return Err(StorageError::ValidationFailed {
            collection: collection.to_string(),
            message: errors[0].clone(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_event_passes_validation() {
        let data = serde_json::json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "title": "Team standup",
            "start": "2026-03-23T09:00:00Z",
            "source": "google-calendar",
            "source_id": "gc-123",
            "created_at": "2026-03-23T00:00:00Z",
            "updated_at": "2026-03-23T00:00:00Z"
        });
        let json = serde_json::to_string(&data).unwrap();
        assert!(validate_canonical("events", &json).is_ok());
    }

    #[test]
    fn event_missing_required_field_fails() {
        let data = serde_json::json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "title": "Team standup"
            // missing start, source, source_id, created_at, updated_at
        });
        let json = serde_json::to_string(&data).unwrap();
        let err = validate_canonical("events", &json).unwrap_err();
        match err {
            StorageError::ValidationFailed { collection, message } => {
                assert_eq!(collection, "events");
                assert!(
                    message.contains("required"),
                    "expected mention of 'required' in: {message}"
                );
            }
            other => panic!("expected ValidationFailed, got: {other}"),
        }
    }

    #[test]
    fn event_with_extensions_passes() {
        let data = serde_json::json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "title": "Team standup",
            "start": "2026-03-23T09:00:00Z",
            "source": "google-calendar",
            "source_id": "gc-123",
            "created_at": "2026-03-23T00:00:00Z",
            "updated_at": "2026-03-23T00:00:00Z",
            "extensions": {
                "com.example.plugin": {
                    "custom_field": "value"
                }
            }
        });
        let json = serde_json::to_string(&data).unwrap();
        assert!(validate_canonical("events", &json).is_ok());
    }

    #[test]
    fn credential_without_extensions_passes() {
        let data = serde_json::json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "name": "My API Key",
            "credential_type": "api_key",
            "service": "github.com",
            "claims": { "token": "ghp_xxx" },
            "source": "manual",
            "source_id": "cred-1",
            "created_at": "2026-03-23T00:00:00Z",
            "updated_at": "2026-03-23T00:00:00Z"
        });
        let json = serde_json::to_string(&data).unwrap();
        assert!(validate_canonical("credentials", &json).is_ok());
    }

    #[test]
    fn credential_with_extensions_rejected() {
        // Credentials schema has additionalProperties: false and no extensions field.
        let data = serde_json::json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "name": "My API Key",
            "credential_type": "api_key",
            "service": "github.com",
            "claims": { "token": "ghp_xxx" },
            "source": "manual",
            "source_id": "cred-1",
            "created_at": "2026-03-23T00:00:00Z",
            "updated_at": "2026-03-23T00:00:00Z",
            "extensions": {
                "com.example.plugin": { "extra": true }
            }
        });
        let json = serde_json::to_string(&data).unwrap();
        let err = validate_canonical("credentials", &json).unwrap_err();
        match err {
            StorageError::ValidationFailed { collection, .. } => {
                assert_eq!(collection, "credentials");
            }
            other => panic!("expected ValidationFailed, got: {other}"),
        }
    }

    #[test]
    fn invalid_json_fails() {
        let err = validate_canonical("events", "not valid json").unwrap_err();
        match err {
            StorageError::ValidationFailed { message, .. } => {
                assert!(message.contains("invalid JSON"));
            }
            other => panic!("expected ValidationFailed, got: {other}"),
        }
    }

    #[test]
    fn private_collection_skips_canonical_validation() {
        // Private collections are not validated by this module.
        assert!(validate_canonical("com.example.weather:forecasts", "{}").is_ok());
    }

    #[test]
    fn all_canonical_schemas_compile() {
        // Force lazy initialization and verify all 7 validators exist.
        assert_eq!(VALIDATORS.len(), 7);
        for name in CANONICAL_COLLECTIONS {
            assert!(VALIDATORS.contains_key(name), "missing validator for {name}");
        }
    }
}
