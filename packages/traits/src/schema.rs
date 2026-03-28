//! Centralized schema registry for Life Engine.
//!
//! Loads CDM (Canonical Data Model) JSON Schemas at startup and validates
//! documents on every write operation. Plugins declare schemas in their
//! manifest — either referencing CDM schemas via the `cdm:` prefix or
//! providing custom JSON Schema strings. Validation runs on `create`,
//! `update`, and `partial_update` operations; reads bypass validation.

use std::collections::HashMap;

use serde_json::Value;
use thiserror::Error;

use crate::index_hints::IndexHint;
use crate::{EngineError, Severity};

// ── Embedded CDM schemas (compile-time) ─────────────────────────────

const EVENTS_SCHEMA: &str = include_str!("../../../.odm/doc/schemas/events.schema.json");
const TASKS_SCHEMA: &str = include_str!("../../../.odm/doc/schemas/tasks.schema.json");
const CONTACTS_SCHEMA: &str = include_str!("../../../.odm/doc/schemas/contacts.schema.json");
const NOTES_SCHEMA: &str = include_str!("../../../.odm/doc/schemas/notes.schema.json");
const EMAILS_SCHEMA: &str = include_str!("../../../.odm/doc/schemas/emails.schema.json");
const FILES_SCHEMA: &str = include_str!("../../../.odm/doc/schemas/files.schema.json");
const CREDENTIALS_SCHEMA: &str = include_str!("../../../.odm/doc/schemas/credentials.schema.json");

/// CDM collection names and their embedded schema sources.
const CDM_SCHEMAS: &[(&str, &str)] = &[
    ("events", EVENTS_SCHEMA),
    ("tasks", TASKS_SCHEMA),
    ("contacts", CONTACTS_SCHEMA),
    ("notes", NOTES_SCHEMA),
    ("emails", EMAILS_SCHEMA),
    ("files", FILES_SCHEMA),
    ("credentials", CREDENTIALS_SCHEMA),
];

/// Canonical CDM collection names.
const CANONICAL_COLLECTIONS: &[&str] = &[
    "events",
    "tasks",
    "contacts",
    "notes",
    "emails",
    "files",
    "credentials",
];

// ── Error types ─────────────────────────────────────────────────────

/// Errors produced by the schema registry during registration or validation.
#[derive(Debug, Error)]
pub enum SchemaError {
    /// A schema could not be loaded or compiled.
    #[error("invalid schema for collection '{collection}': {message}")]
    InvalidSchema {
        collection: String,
        message: String,
    },

    /// A document failed validation against its collection schema.
    #[error("validation failed for collection '{collection}': {message}")]
    ValidationFailed {
        collection: String,
        message: String,
    },

    /// An attempt was made to modify an immutable system-managed field.
    #[error("immutable field '{field}' cannot be changed on update")]
    ImmutableField {
        field: String,
    },

    /// A plugin attempted to write to another plugin's extension namespace.
    #[error("namespace violation: plugin '{plugin_id}' cannot write to ext.{namespace}")]
    NamespaceViolation {
        plugin_id: String,
        namespace: String,
    },
}

impl EngineError for SchemaError {
    fn code(&self) -> &str {
        match self {
            SchemaError::InvalidSchema { .. } => "SCHEMA_001",
            SchemaError::ValidationFailed { .. } => "SCHEMA_002",
            SchemaError::ImmutableField { .. } => "SCHEMA_003",
            SchemaError::NamespaceViolation { .. } => "SCHEMA_004",
        }
    }

    fn severity(&self) -> Severity {
        Severity::Fatal
    }

    fn source_module(&self) -> &str {
        "schema-registry"
    }
}

// ── Supporting types ────────────────────────────────────────────────

/// Schema configuration for a single plugin collection.
pub struct CollectionSchema {
    /// Collection name.
    pub collection: String,
    /// Plugin that owns this collection.
    pub plugin_id: String,
    /// Body validator. `None` means schemaless — skip validation.
    pub validator: Option<jsonschema::Validator>,
    /// Extension field validator. `None` means no extension schema declared.
    pub extension_validator: Option<jsonschema::Validator>,
    /// When `true`, reject fields not defined in the schema.
    pub strict: bool,
    /// Advisory index hints for storage adapters.
    pub index_hints: Vec<IndexHint>,
}

// ── SchemaRegistry ──────────────────────────────────────────────────

/// Centralized registry that loads CDM schemas at startup and validates
/// documents on every write operation.
pub struct SchemaRegistry {
    /// Pre-compiled CDM validators keyed by collection name.
    cdm_validators: HashMap<String, jsonschema::Validator>,
    /// Raw CDM schema JSON values for cloning into plugin registrations.
    cdm_values: HashMap<String, Value>,
    /// Plugin collection schemas keyed by (plugin_id, collection_name).
    collections: HashMap<(String, String), CollectionSchema>,
    /// Raw schema JSON values for non-CDM collections (needed for strict mode).
    schema_values: HashMap<(String, String), Value>,
}

impl SchemaRegistry {
    /// Create an empty registry. Call `load_cdm_schemas()` to populate CDM validators.
    pub fn new() -> Self {
        Self {
            cdm_validators: HashMap::new(),
            cdm_values: HashMap::new(),
            collections: HashMap::new(),
            schema_values: HashMap::new(),
        }
    }

    /// Load and compile all 7 CDM schemas. Returns an error if any schema
    /// is invalid JSON or fails to compile.
    pub fn load_cdm_schemas(&mut self) -> Result<(), SchemaError> {
        for (name, raw) in CDM_SCHEMAS {
            let value: Value = serde_json::from_str(raw).map_err(|e| SchemaError::InvalidSchema {
                collection: name.to_string(),
                message: format!("invalid JSON: {e}"),
            })?;

            let validator =
                jsonschema::draft202012::new(&value).map_err(|e| SchemaError::InvalidSchema {
                    collection: name.to_string(),
                    message: format!("failed to compile: {e}"),
                })?;

            self.cdm_validators.insert(name.to_string(), validator);
            self.cdm_values.insert(name.to_string(), value);
        }
        Ok(())
    }

    /// Register a plugin collection with the registry.
    ///
    /// `schema_ref` determines how the schema is resolved:
    /// - `Some("cdm:events")` — resolve to the CDM schema for `events`
    /// - `Some(json_string)` — compile the raw JSON Schema string
    /// - `None` — schemaless, skip validation
    ///
    /// `extension_schema_json` is an optional JSON Schema string for validating
    /// extension fields under `ext.{plugin_id}`.
    pub fn register_plugin_collection(
        &mut self,
        plugin_id: &str,
        collection: &str,
        schema_ref: Option<&str>,
        extension_schema_json: Option<&str>,
        strict: bool,
        index_hints: Vec<IndexHint>,
    ) -> Result<(), SchemaError> {
        let validator = match schema_ref {
            None => None,
            Some(s) if s.starts_with("cdm:") => {
                let cdm_name = &s[4..];
                let value = self.cdm_values.get(cdm_name).ok_or_else(|| {
                    SchemaError::InvalidSchema {
                        collection: collection.to_string(),
                        message: format!("unknown CDM schema '{cdm_name}'"),
                    }
                })?;
                let v = jsonschema::draft202012::new(value).map_err(|e| {
                    SchemaError::InvalidSchema {
                        collection: collection.to_string(),
                        message: format!("failed to compile CDM schema: {e}"),
                    }
                })?;
                Some(v)
            }
            Some(json_str) => {
                let value: Value =
                    serde_json::from_str(json_str).map_err(|e| SchemaError::InvalidSchema {
                        collection: collection.to_string(),
                        message: format!("invalid schema JSON: {e}"),
                    })?;
                let v = jsonschema::draft202012::new(&value).map_err(|e| {
                    SchemaError::InvalidSchema {
                        collection: collection.to_string(),
                        message: format!("failed to compile schema: {e}"),
                    }
                })?;
                // Store the raw value for strict mode property lookup.
                self.schema_values.insert(
                    (plugin_id.to_string(), collection.to_string()),
                    value,
                );
                Some(v)
            }
        };

        let extension_validator = match extension_schema_json {
            None => None,
            Some(json_str) => {
                let value: Value =
                    serde_json::from_str(json_str).map_err(|e| SchemaError::InvalidSchema {
                        collection: collection.to_string(),
                        message: format!("invalid extension schema JSON: {e}"),
                    })?;
                let v = jsonschema::draft202012::new(&value).map_err(|e| {
                    SchemaError::InvalidSchema {
                        collection: collection.to_string(),
                        message: format!("failed to compile extension schema: {e}"),
                    }
                })?;
                Some(v)
            }
        };

        self.collections.insert(
            (plugin_id.to_string(), collection.to_string()),
            CollectionSchema {
                collection: collection.to_string(),
                plugin_id: plugin_id.to_string(),
                validator,
                extension_validator,
                strict,
                index_hints,
            },
        );

        Ok(())
    }

    /// Returns `true` if the collection is a canonical CDM collection.
    pub fn is_canonical(&self, collection: &str) -> bool {
        CANONICAL_COLLECTIONS.contains(&collection)
    }

    /// Returns a reference to a registered collection schema, if any.
    pub fn get_collection(
        &self,
        plugin_id: &str,
        collection: &str,
    ) -> Option<&CollectionSchema> {
        self.collections
            .get(&(plugin_id.to_string(), collection.to_string()))
    }

    /// Returns the number of loaded CDM schemas.
    pub fn cdm_schema_count(&self) -> usize {
        self.cdm_validators.len()
    }

    /// Returns the number of registered plugin collections.
    pub fn collection_count(&self) -> usize {
        self.collections.len()
    }

    /// Validate a document for a write operation on a plugin collection.
    ///
    /// This method implements the full write-time validation sequence:
    /// 1. Look up schema for (plugin_id, collection)
    /// 2. If schemaless, return document as-is
    /// 3. Strip system-managed timestamp fields before validation
    /// 4. Handle `id` — generate on create if missing, reject change on update
    /// 5. Reject `created_at` changes on update
    /// 6. Validate document body against schema
    /// 7. If strict, check for unknown fields
    /// 8. Validate extension namespace isolation
    /// 9. Validate extensions against extension_schema if present
    /// 10. Re-inject system fields and return validated document
    ///
    /// `existing_doc` is required for update operations to detect immutable
    /// field changes. Pass `None` for create operations.
    pub fn validate_write(
        &self,
        collection: &str,
        plugin_id: &str,
        document: &Value,
        is_create: bool,
        existing_doc: Option<&Value>,
    ) -> Result<Value, SchemaError> {
        let key = (plugin_id.to_string(), collection.to_string());
        let schema = self.collections.get(&key).ok_or_else(|| {
            SchemaError::ValidationFailed {
                collection: collection.to_string(),
                message: format!(
                    "no schema registered for plugin '{plugin_id}' collection '{collection}'"
                ),
            }
        })?;

        // Schemaless — skip all validation.
        let Some(validator) = &schema.validator else {
            return Ok(document.clone());
        };

        let mut doc = document.clone();
        let obj = doc.as_object_mut().ok_or_else(|| SchemaError::ValidationFailed {
            collection: collection.to_string(),
            message: "document must be a JSON object".to_string(),
        })?;

        // ── Req 4: System-managed base fields ───────────────────────

        // On create, generate id if not provided (Req 4.1/4.2).
        if is_create && !obj.contains_key("id") {
            obj.insert(
                "id".to_string(),
                Value::String(uuid::Uuid::new_v4().to_string()),
            );
        }

        // On update, reject changes to immutable fields (Req 4.5).
        if !is_create
            && let Some(existing) = existing_doc
            && let Some(existing_obj) = existing.as_object()
        {
            if let (Some(new_id), Some(old_id)) = (obj.get("id"), existing_obj.get("id"))
                && new_id != old_id
            {
                return Err(SchemaError::ImmutableField {
                    field: "id".to_string(),
                });
            }
            if let (Some(new_ca), Some(old_ca)) =
                (obj.get("created_at"), existing_obj.get("created_at"))
                && new_ca != old_ca
            {
                return Err(SchemaError::ImmutableField {
                    field: "created_at".to_string(),
                });
            }
        }

        // ── Req 2.1: Strip system timestamps before validation ──────
        // System overwrites these, so caller-provided values are irrelevant
        // for schema validation. We strip them, validate, then re-inject.
        let saved_id = obj.remove("id");
        let saved_created_at = obj.remove("created_at");
        let saved_updated_at = obj.remove("updated_at");

        // ── Req 5: Extension namespace isolation ────────────────────
        if let Some(ext_value) = obj.get("ext")
            && let Some(ext_obj) = ext_value.as_object()
        {
            for ns in ext_obj.keys() {
                if ns != plugin_id {
                    return Err(SchemaError::NamespaceViolation {
                        plugin_id: plugin_id.to_string(),
                        namespace: ns.clone(),
                    });
                }
            }
        }

        // Also check top-level "extensions" field (CDM uses this name).
        if let Some(ext_value) = obj.get("extensions")
            && let Some(ext_obj) = ext_value.as_object()
        {
            for ns in ext_obj.keys() {
                if ns != plugin_id {
                    return Err(SchemaError::NamespaceViolation {
                        plugin_id: plugin_id.to_string(),
                        namespace: ns.clone(),
                    });
                }
            }
        }

        // ── Req 2.2: Validate body against schema ──────────────────
        // Re-inject system fields temporarily for schemas that require them.
        // CDM schemas require id, created_at, updated_at as required fields,
        // so we must provide placeholder values during validation.
        let placeholder_id = saved_id
            .clone()
            .unwrap_or_else(|| Value::String(uuid::Uuid::new_v4().to_string()));
        let placeholder_ts = Value::String(
            chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%SZ")
                .to_string(),
        );

        obj.insert("id".to_string(), placeholder_id.clone());
        obj.insert("created_at".to_string(), placeholder_ts.clone());
        obj.insert("updated_at".to_string(), placeholder_ts);

        let errors: Vec<String> = validator
            .iter_errors(&doc)
            .map(|e| e.to_string())
            .collect();

        if !errors.is_empty() {
            return Err(SchemaError::ValidationFailed {
                collection: collection.to_string(),
                message: errors[0].clone(),
            });
        }

        // ── Req 3: Strict mode ──────────────────────────────────────
        if schema.strict {
            // Re-read obj after validation (doc was mutated above).
            if let Some(obj) = doc.as_object() {
                // Get the schema's declared properties from the CDM value or
                // the compiled schema. We check by looking at the schema JSON
                // value stored in cdm_values, or we use a simpler approach:
                // any field that the schema doesn't define as a property is
                // "unknown". For strict mode, we check against a known set.
                //
                // Since jsonschema::Validator doesn't expose property names,
                // we look up the schema value to extract top-level property keys.
                let schema_properties = self.get_schema_properties(plugin_id, collection);
                if let Some(props) = schema_properties {
                    // System fields and extension fields are always allowed.
                    let system_fields = ["id", "created_at", "updated_at", "ext", "extensions"];
                    for key in obj.keys() {
                        if system_fields.contains(&key.as_str()) {
                            continue;
                        }
                        if !props.contains(&key.as_str()) {
                            return Err(SchemaError::ValidationFailed {
                                collection: collection.to_string(),
                                message: format!(
                                    "strict mode: unknown field '{key}' not defined in schema"
                                ),
                            });
                        }
                    }
                }
            }
        }

        // ── Req 2.3 / 5.4: Validate extensions against extension_schema ─
        if let Some(ext_validator) = &schema.extension_validator {
            // Extract this plugin's extension data.
            let ext_data = doc
                .as_object()
                .and_then(|o| o.get("ext"))
                .and_then(|e| e.as_object())
                .and_then(|e| e.get(plugin_id))
                .cloned()
                .or_else(|| {
                    doc.as_object()
                        .and_then(|o| o.get("extensions"))
                        .and_then(|e| e.as_object())
                        .and_then(|e| e.get(plugin_id))
                        .cloned()
                });

            if let Some(ext_data) = ext_data {
                let ext_errors: Vec<String> = ext_validator
                    .iter_errors(&ext_data)
                    .map(|e| e.to_string())
                    .collect();

                if !ext_errors.is_empty() {
                    return Err(SchemaError::ValidationFailed {
                        collection: collection.to_string(),
                        message: format!("extension validation: {}", ext_errors[0]),
                    });
                }
            }
        }

        // ── Re-inject system fields ─────────────────────────────────
        if let Some(obj) = doc.as_object_mut() {
            // Restore original id or keep the generated one.
            if let Some(id) = saved_id {
                obj.insert("id".to_string(), id);
            }
            // Restore original timestamps (will be overwritten by StorageContext).
            if let Some(ca) = saved_created_at {
                obj.insert("created_at".to_string(), ca);
            }
            if let Some(ua) = saved_updated_at {
                obj.insert("updated_at".to_string(), ua);
            }
        }

        Ok(doc)
    }

    /// Extract the set of top-level property names from the schema JSON value
    /// for a given plugin collection. Returns `None` if the schema value
    /// cannot be found (e.g. for schemaless collections).
    fn get_schema_properties(&self, plugin_id: &str, collection: &str) -> Option<Vec<&str>> {
        // First check CDM values.
        if let Some(value) = self.cdm_values.get(collection) {
            return Self::extract_property_keys(value);
        }
        // For non-CDM schemas we need to store the schema value. Since we
        // compile validators from JSON strings, we store the raw values too.
        // For now, check our stored collection schemas — if we stored a
        // schema_value alongside the validator we could look it up. As a
        // practical matter, strict mode is most useful with custom schemas
        // where the plugin author has full control. For CDM schemas, strict
        // mode is less common since CDM schemas have `additionalProperties: true`.
        //
        // For non-CDM strict schemas, we store the schema value in the
        // schema_values map.
        let key = (plugin_id.to_string(), collection.to_string());
        if let Some(value) = self.schema_values.get(&key) {
            return Self::extract_property_keys(value);
        }
        None
    }

    fn extract_property_keys(schema_value: &Value) -> Option<Vec<&str>> {
        schema_value
            .as_object()
            .and_then(|o| o.get("properties"))
            .and_then(|p| p.as_object())
            .map(|props| props.keys().map(|k| k.as_str()).collect())
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn setup_registry() -> SchemaRegistry {
        let mut reg = SchemaRegistry::new();
        reg.load_cdm_schemas().unwrap();
        reg
    }

    // ── Test 1: Empty registry ──────────────────────────────────────

    #[test]
    fn new_creates_empty_registry() {
        let reg = SchemaRegistry::new();
        assert_eq!(reg.cdm_schema_count(), 0);
        assert_eq!(reg.collection_count(), 0);
    }

    // ── Test 2: CDM loading ─────────────────────────────────────────

    #[test]
    fn load_cdm_schemas_loads_all_seven() {
        let reg = setup_registry();
        assert_eq!(reg.cdm_schema_count(), 7);
    }

    // ── Test 3: CDM prefix resolution ───────────────────────────────

    #[test]
    fn register_with_cdm_prefix_resolves() {
        let mut reg = setup_registry();
        reg.register_plugin_collection(
            "com.example.cal",
            "events",
            Some("cdm:events"),
            None,
            false,
            vec![],
        )
        .unwrap();
        assert!(reg.get_collection("com.example.cal", "events").is_some());
        assert!(
            reg.get_collection("com.example.cal", "events")
                .unwrap()
                .validator
                .is_some()
        );
    }

    // ── Test 4: Raw JSON schema compilation ─────────────────────────

    #[test]
    fn register_with_schema_json_compiles() {
        let mut reg = setup_registry();
        let schema = r#"{
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": { "type": "string" }
            }
        }"#;
        reg.register_plugin_collection("com.example.plugin", "items", Some(schema), None, false, vec![])
            .unwrap();
        assert!(
            reg.get_collection("com.example.plugin", "items")
                .unwrap()
                .validator
                .is_some()
        );
    }

    // ── Test 5: Schemaless collection ───────────────────────────────

    #[test]
    fn register_without_schema_is_schemaless() {
        let mut reg = setup_registry();
        reg.register_plugin_collection("com.example.plugin", "logs", None, None, false, vec![])
            .unwrap();
        let cs = reg.get_collection("com.example.plugin", "logs").unwrap();
        assert!(cs.validator.is_none());
    }

    // ── Test 6: Valid CDM document passes ───────────────────────────

    #[test]
    fn validate_write_accepts_valid_cdm_doc() {
        let mut reg = setup_registry();
        reg.register_plugin_collection(
            "com.example.cal",
            "events",
            Some("cdm:events"),
            None,
            false,
            vec![],
        )
        .unwrap();

        let doc = json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "title": "Team standup",
            "start": "2026-03-23T09:00:00Z",
            "source": "google-calendar",
            "source_id": "gc-123",
            "created_at": "2026-03-23T00:00:00Z",
            "updated_at": "2026-03-23T00:00:00Z"
        });

        let result = reg.validate_write("events", "com.example.cal", &doc, true, None);
        assert!(result.is_ok());
    }

    // ── Test 7: Invalid CDM document rejected ───────────────────────

    #[test]
    fn validate_write_rejects_invalid_cdm_doc() {
        let mut reg = setup_registry();
        reg.register_plugin_collection(
            "com.example.cal",
            "events",
            Some("cdm:events"),
            None,
            false,
            vec![],
        )
        .unwrap();

        // Missing required field "start".
        let doc = json!({
            "title": "Team standup",
            "source": "google-calendar",
            "source_id": "gc-123"
        });

        let result = reg.validate_write("events", "com.example.cal", &doc, true, None);
        assert!(result.is_err());
        match result.unwrap_err() {
            SchemaError::ValidationFailed { collection, message } => {
                assert_eq!(collection, "events");
                assert!(
                    message.contains("required") || message.contains("start"),
                    "expected mention of missing field in: {message}"
                );
            }
            other => panic!("expected ValidationFailed, got: {other}"),
        }
    }

    // ── Test 8: Schemaless skips validation ──────────────────────────

    #[test]
    fn validate_write_skips_schemaless() {
        let mut reg = setup_registry();
        reg.register_plugin_collection("com.example.plugin", "logs", None, None, false, vec![])
            .unwrap();

        let doc = json!({ "anything": "goes", "no": "schema" });
        let result = reg.validate_write("logs", "com.example.plugin", &doc, true, None);
        assert!(result.is_ok());
    }

    // ── Test 9: System fields stripped and re-injected ──────────────

    #[test]
    fn validate_write_strips_and_reinjects_system_fields() {
        let mut reg = setup_registry();
        reg.register_plugin_collection(
            "com.example.cal",
            "events",
            Some("cdm:events"),
            None,
            false,
            vec![],
        )
        .unwrap();

        let doc = json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "title": "Meeting",
            "start": "2026-03-23T09:00:00Z",
            "source": "google-calendar",
            "source_id": "gc-456",
            "created_at": "caller-provided-timestamp",
            "updated_at": "caller-provided-timestamp"
        });

        let result = reg
            .validate_write("events", "com.example.cal", &doc, true, None)
            .unwrap();
        let obj = result.as_object().unwrap();

        // The caller-provided timestamps should be preserved in the returned
        // document (StorageContext will overwrite them later).
        assert_eq!(obj.get("id").unwrap(), "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(obj.get("created_at").unwrap(), "caller-provided-timestamp");
        assert_eq!(obj.get("updated_at").unwrap(), "caller-provided-timestamp");
    }

    // ── Test 10: Strict mode rejects unknown fields ─────────────────

    #[test]
    fn validate_write_strict_rejects_unknown_fields() {
        let mut reg = setup_registry();
        let schema = r#"{
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": { "type": "string" },
                "age": { "type": "integer" }
            }
        }"#;
        reg.register_plugin_collection(
            "com.example.plugin",
            "people",
            Some(schema),
            None,
            true,
            vec![],
        )
        .unwrap();

        let doc = json!({
            "name": "Alice",
            "age": 30,
            "unknown_field": "should be rejected"
        });

        let result = reg.validate_write("people", "com.example.plugin", &doc, true, None);
        assert!(result.is_err());
        match result.unwrap_err() {
            SchemaError::ValidationFailed { message, .. } => {
                assert!(
                    message.contains("unknown_field"),
                    "expected mention of unknown_field in: {message}"
                );
            }
            other => panic!("expected ValidationFailed, got: {other}"),
        }
    }

    // ── Test 11: Permissive mode accepts unknown fields ─────────────

    #[test]
    fn validate_write_permissive_accepts_unknown_fields() {
        let mut reg = setup_registry();
        let schema = r#"{
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": { "type": "string" }
            }
        }"#;
        reg.register_plugin_collection(
            "com.example.plugin",
            "people",
            Some(schema),
            None,
            false,
            vec![],
        )
        .unwrap();

        let doc = json!({
            "name": "Alice",
            "extra_field": "should be accepted"
        });

        let result = reg.validate_write("people", "com.example.plugin", &doc, true, None);
        assert!(result.is_ok());
    }

    // ── Test 12: Extension namespace isolation ──────────────────────

    #[test]
    fn validate_write_namespace_isolation() {
        let mut reg = setup_registry();
        reg.register_plugin_collection(
            "com.example.cal",
            "events",
            Some("cdm:events"),
            None,
            false,
            vec![],
        )
        .unwrap();

        let doc = json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "title": "Meeting",
            "start": "2026-03-23T09:00:00Z",
            "source": "google-calendar",
            "source_id": "gc-789",
            "created_at": "2026-03-23T00:00:00Z",
            "updated_at": "2026-03-23T00:00:00Z",
            "extensions": {
                "com.other.plugin": { "sneaky": true }
            }
        });

        let result = reg.validate_write("events", "com.example.cal", &doc, true, None);
        assert!(result.is_err());
        match result.unwrap_err() {
            SchemaError::NamespaceViolation {
                plugin_id,
                namespace,
            } => {
                assert_eq!(plugin_id, "com.example.cal");
                assert_eq!(namespace, "com.other.plugin");
            }
            other => panic!("expected NamespaceViolation, got: {other}"),
        }
    }

    // ── Test 13: Extension schema validation ────────────────────────

    #[test]
    fn validate_write_extension_schema_validates() {
        let mut reg = setup_registry();
        let ext_schema = r#"{
            "type": "object",
            "required": ["priority"],
            "properties": {
                "priority": { "type": "integer", "minimum": 1, "maximum": 5 }
            },
            "additionalProperties": false
        }"#;
        reg.register_plugin_collection(
            "com.example.cal",
            "events",
            Some("cdm:events"),
            Some(ext_schema),
            false,
            vec![],
        )
        .unwrap();

        // Valid extension data.
        let valid_doc = json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "title": "Meeting",
            "start": "2026-03-23T09:00:00Z",
            "source": "google-calendar",
            "source_id": "gc-100",
            "created_at": "2026-03-23T00:00:00Z",
            "updated_at": "2026-03-23T00:00:00Z",
            "extensions": {
                "com.example.cal": { "priority": 3 }
            }
        });
        assert!(
            reg.validate_write("events", "com.example.cal", &valid_doc, true, None)
                .is_ok()
        );

        // Invalid extension data (priority out of range).
        let invalid_doc = json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "title": "Meeting",
            "start": "2026-03-23T09:00:00Z",
            "source": "google-calendar",
            "source_id": "gc-101",
            "created_at": "2026-03-23T00:00:00Z",
            "updated_at": "2026-03-23T00:00:00Z",
            "extensions": {
                "com.example.cal": { "priority": 10 }
            }
        });
        let result =
            reg.validate_write("events", "com.example.cal", &invalid_doc, true, None);
        assert!(result.is_err());
        match result.unwrap_err() {
            SchemaError::ValidationFailed { message, .. } => {
                assert!(
                    message.contains("extension"),
                    "expected extension validation error in: {message}"
                );
            }
            other => panic!("expected ValidationFailed, got: {other}"),
        }
    }

    // ── Test 14: Create generates id if missing ─────────────────────

    #[test]
    fn validate_write_create_generates_id() {
        let mut reg = setup_registry();
        reg.register_plugin_collection(
            "com.example.cal",
            "events",
            Some("cdm:events"),
            None,
            false,
            vec![],
        )
        .unwrap();

        let doc = json!({
            "title": "No ID provided",
            "start": "2026-03-23T09:00:00Z",
            "source": "google-calendar",
            "source_id": "gc-200"
        });

        let result = reg
            .validate_write("events", "com.example.cal", &doc, true, None)
            .unwrap();
        let obj = result.as_object().unwrap();
        let id = obj.get("id").unwrap().as_str().unwrap();
        // Should be a valid UUID.
        assert_eq!(id.len(), 36);
        assert!(id.contains('-'));
    }

    // ── Test 15: Update rejects id change ───────────────────────────

    #[test]
    fn validate_write_update_rejects_id_change() {
        let mut reg = setup_registry();
        reg.register_plugin_collection(
            "com.example.cal",
            "events",
            Some("cdm:events"),
            None,
            false,
            vec![],
        )
        .unwrap();

        let existing = json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "title": "Original",
            "start": "2026-03-23T09:00:00Z",
            "source": "google-calendar",
            "source_id": "gc-300",
            "created_at": "2026-03-23T00:00:00Z",
            "updated_at": "2026-03-23T00:00:00Z"
        });

        let update = json!({
            "id": "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
            "title": "Changed ID",
            "start": "2026-03-23T09:00:00Z",
            "source": "google-calendar",
            "source_id": "gc-300",
            "created_at": "2026-03-23T00:00:00Z",
            "updated_at": "2026-03-23T00:00:00Z"
        });

        let result =
            reg.validate_write("events", "com.example.cal", &update, false, Some(&existing));
        assert!(result.is_err());
        match result.unwrap_err() {
            SchemaError::ImmutableField { field } => {
                assert_eq!(field, "id");
            }
            other => panic!("expected ImmutableField, got: {other}"),
        }
    }

    // ── Test 16: Update rejects created_at change ───────────────────

    #[test]
    fn validate_write_update_rejects_created_at_change() {
        let mut reg = setup_registry();
        reg.register_plugin_collection(
            "com.example.cal",
            "events",
            Some("cdm:events"),
            None,
            false,
            vec![],
        )
        .unwrap();

        let existing = json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "title": "Original",
            "start": "2026-03-23T09:00:00Z",
            "source": "google-calendar",
            "source_id": "gc-400",
            "created_at": "2026-03-23T00:00:00Z",
            "updated_at": "2026-03-23T00:00:00Z"
        });

        let update = json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "title": "Same ID",
            "start": "2026-03-23T09:00:00Z",
            "source": "google-calendar",
            "source_id": "gc-400",
            "created_at": "2026-03-24T00:00:00Z",
            "updated_at": "2026-03-23T00:00:00Z"
        });

        let result =
            reg.validate_write("events", "com.example.cal", &update, false, Some(&existing));
        assert!(result.is_err());
        match result.unwrap_err() {
            SchemaError::ImmutableField { field } => {
                assert_eq!(field, "created_at");
            }
            other => panic!("expected ImmutableField, got: {other}"),
        }
    }

    // ── Test 17: is_canonical ───────────────────────────────────────

    #[test]
    fn is_canonical_returns_correct_values() {
        let reg = setup_registry();
        assert!(reg.is_canonical("events"));
        assert!(reg.is_canonical("tasks"));
        assert!(reg.is_canonical("contacts"));
        assert!(reg.is_canonical("notes"));
        assert!(reg.is_canonical("emails"));
        assert!(reg.is_canonical("files"));
        assert!(reg.is_canonical("credentials"));
        assert!(!reg.is_canonical("custom_collection"));
        assert!(!reg.is_canonical("com.example.weather:forecasts"));
    }
}
