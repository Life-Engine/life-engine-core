//! Schema validation for canonical and private collection writes.
//!
//! Before any `Insert` or `Update` mutation on a canonical collection, the
//! data payload is validated against the corresponding JSON Schema. For private
//! collections, schemas are registered at runtime from plugin manifests and
//! validated on write using a `PrivateSchemaRegistry`.
//!
//! Validation failures are fatal — invalid data never enters the database.

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
/// Non-canonical collections are silently skipped — use `validate_private`
/// for private collection validation.
pub fn validate_canonical(collection: &str, data_json: &str) -> Result<(), StorageError> {
    if !is_canonical(collection) {
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

/// Registry of JSON Schema validators for plugin private collections.
///
/// Each plugin can declare private collections in its manifest with a JSON
/// Schema definition. Schemas are keyed by the composite `(plugin_id,
/// collection_name)` pair to enforce namespace isolation — a plugin can only
/// write to its own private collections.
pub struct PrivateSchemaRegistry {
    validators: HashMap<(String, String), jsonschema::Validator>,
}

impl PrivateSchemaRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            validators: HashMap::new(),
        }
    }

    /// Register a JSON Schema for a private collection owned by `plugin_id`.
    ///
    /// The schema JSON string is compiled into a validator. Returns an error
    /// if the schema is invalid JSON or fails to compile, or if the
    /// `collection_name` collides with a canonical collection.
    pub fn register(
        &mut self,
        plugin_id: &str,
        collection_name: &str,
        schema_json: &str,
    ) -> Result<(), StorageError> {
        if is_canonical(collection_name) {
            return Err(StorageError::ValidationFailed {
                collection: collection_name.to_string(),
                message: format!(
                    "cannot register private schema for canonical collection '{collection_name}'"
                ),
            });
        }

        let schema_value: Value =
            serde_json::from_str(schema_json).map_err(|e| StorageError::ValidationFailed {
                collection: collection_name.to_string(),
                message: format!("invalid schema JSON: {e}"),
            })?;

        let validator = jsonschema::draft202012::new(&schema_value).map_err(|e| {
            StorageError::ValidationFailed {
                collection: collection_name.to_string(),
                message: format!("failed to compile schema: {e}"),
            }
        })?;

        self.validators.insert(
            (plugin_id.to_string(), collection_name.to_string()),
            validator,
        );

        Ok(())
    }

    /// Returns `true` if a schema is registered for this plugin/collection pair.
    pub fn has_schema(&self, plugin_id: &str, collection_name: &str) -> bool {
        self.validators
            .contains_key(&(plugin_id.to_string(), collection_name.to_string()))
    }
}

impl Default for PrivateSchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate a JSON payload against a registered private collection schema.
///
/// Returns `Ok(())` if validation passes.
///
/// Returns `Err(StorageError::UnknownCollection)` if no schema is registered
/// for the `(plugin_id, collection)` pair.
///
/// Returns `Err(StorageError::ValidationFailed)` if the data violates the schema.
pub fn validate_private(
    registry: &PrivateSchemaRegistry,
    plugin_id: &str,
    collection: &str,
    data_json: &str,
) -> Result<(), StorageError> {
    let key = (plugin_id.to_string(), collection.to_string());

    let validator = registry.validators.get(&key).ok_or_else(|| {
        StorageError::UnknownCollection(format!(
            "{plugin_id}:{collection} — no schema registered for this private collection"
        ))
    })?;

    let data: Value = serde_json::from_str(data_json).map_err(|e| StorageError::ValidationFailed {
        collection: collection.to_string(),
        message: format!("invalid JSON: {e}"),
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

    // --- Private collection schema validation tests ---

    fn weather_schema() -> &'static str {
        r#"{
            "type": "object",
            "required": ["location", "temperature"],
            "properties": {
                "location": { "type": "string" },
                "temperature": { "type": "number" },
                "conditions": { "type": "string" }
            },
            "additionalProperties": false
        }"#
    }

    #[test]
    fn private_valid_data_passes() {
        let mut registry = PrivateSchemaRegistry::new();
        registry
            .register("com.example.weather", "forecasts", weather_schema())
            .unwrap();

        let data = serde_json::json!({
            "location": "Sydney",
            "temperature": 22.5
        });
        let json = serde_json::to_string(&data).unwrap();

        assert!(
            validate_private(&registry, "com.example.weather", "forecasts", &json).is_ok()
        );
    }

    #[test]
    fn private_invalid_data_fails() {
        let mut registry = PrivateSchemaRegistry::new();
        registry
            .register("com.example.weather", "forecasts", weather_schema())
            .unwrap();

        // Missing required field "temperature".
        let data = serde_json::json!({ "location": "Sydney" });
        let json = serde_json::to_string(&data).unwrap();

        let err = validate_private(&registry, "com.example.weather", "forecasts", &json)
            .unwrap_err();
        match err {
            StorageError::ValidationFailed { collection, message } => {
                assert_eq!(collection, "forecasts");
                assert!(
                    message.contains("required"),
                    "expected 'required' in: {message}"
                );
            }
            other => panic!("expected ValidationFailed, got: {other}"),
        }
    }

    #[test]
    fn private_unregistered_collection_fails() {
        let registry = PrivateSchemaRegistry::new();

        let err = validate_private(&registry, "com.example.weather", "forecasts", "{}")
            .unwrap_err();
        assert!(
            matches!(err, StorageError::UnknownCollection(_)),
            "expected UnknownCollection, got: {err}"
        );
    }

    #[test]
    fn private_cross_plugin_write_rejected() {
        let mut registry = PrivateSchemaRegistry::new();
        registry
            .register("com.example.weather", "forecasts", weather_schema())
            .unwrap();

        let data = serde_json::json!({ "location": "Sydney", "temperature": 22.5 });
        let json = serde_json::to_string(&data).unwrap();

        // A different plugin tries to write to com.example.weather's collection.
        let err = validate_private(&registry, "com.example.maps", "forecasts", &json)
            .unwrap_err();
        assert!(
            matches!(err, StorageError::UnknownCollection(_)),
            "cross-plugin write should fail with UnknownCollection, got: {err}"
        );
    }

    #[test]
    fn private_register_canonical_name_rejected() {
        let mut registry = PrivateSchemaRegistry::new();
        let result = registry.register("com.example.plugin", "events", weather_schema());
        assert!(result.is_err(), "registering a canonical name should fail");
    }

    #[test]
    fn private_register_invalid_schema_rejected() {
        let mut registry = PrivateSchemaRegistry::new();
        let result = registry.register("com.example.plugin", "data", "not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn private_has_schema_returns_correct_state() {
        let mut registry = PrivateSchemaRegistry::new();
        assert!(!registry.has_schema("plug", "coll"));

        registry
            .register("plug", "coll", r#"{"type": "object"}"#)
            .unwrap();
        assert!(registry.has_schema("plug", "coll"));
        assert!(!registry.has_schema("plug", "other"));
        assert!(!registry.has_schema("other", "coll"));
    }
}
