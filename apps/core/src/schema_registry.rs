//! Schema registry for loading and compiling JSON Schema files.
//!
//! Loads collection schemas from `docs/schemas/` on startup, compiles them
//! with the `jsonschema` crate, and provides validation against registered
//! schemas. Supports adding plugin-specific schemas at runtime.

use std::collections::HashMap;
use std::path::Path;
use std::sync::RwLock;

use jsonschema::Validator;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::storage::StorageAdapter;

/// Result of validating a value against a schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether the value passed validation.
    pub valid: bool,
    /// Validation error messages, if any.
    pub errors: Vec<String>,
    /// The schema version used for validation.
    pub schema_version: String,
}

/// A compiled schema entry in the registry.
struct SchemaEntry {
    /// The compiled JSON Schema validator.
    validator: Validator,
    /// A version identifier for this schema (derived from the title or filename).
    version: String,
}

/// Core CDM collection names that plugins are not allowed to shadow.
const CORE_CDM_NAMES: &[&str] = &[
    "tasks",
    "contacts",
    "emails",
    "events",
    "files",
    "notes",
    "credentials",
];

/// Registry of compiled JSON Schema validators keyed by collection name.
///
/// The registry is thread-safe: reads (`validate`, `has_schema`, `collections`)
/// acquire a shared read lock while writes (`register`, `register_plugin_schema`)
/// acquire an exclusive write lock. This allows concurrent validation from
/// multiple request handlers without contention.
pub struct SchemaRegistry {
    schemas: RwLock<HashMap<String, SchemaEntry>>,
}

impl SchemaRegistry {
    /// Create an empty schema registry.
    pub fn new() -> Self {
        Self {
            schemas: RwLock::new(HashMap::new()),
        }
    }

    /// Load all `*.schema.json` files from the given directory.
    ///
    /// Each file is expected to be named `{collection}.schema.json`.
    /// The collection name is extracted from the filename (e.g. `tasks.schema.json`
    /// registers as `"tasks"`). Infrastructure schemas are skipped: any schema
    /// whose name starts with `plugin-` (e.g. `plugin-manifest`, `plugin-data`)
    /// or equals `audit-log`.
    pub fn load_from_directory(dir: &Path) -> anyhow::Result<Self> {
        let registry = Self::new();

        let entries = std::fs::read_dir(dir)
            .map_err(|e| anyhow::anyhow!("failed to read schema directory {}: {e}", dir.display()))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            let file_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };

            // Only process *.schema.json files.
            if !file_name.ends_with(".schema.json") {
                continue;
            }

            // Extract collection name: "tasks.schema.json" -> "tasks"
            let collection = file_name.trim_end_matches(".schema.json");

            // Skip infrastructure schemas — they are not data collections.
            // This covers plugin-manifest, plugin-data, audit-log, and any
            // future infrastructure schema whose name starts with "plugin-"
            // or equals "audit-log".
            if collection.starts_with("plugin-") || collection == "audit-log" {
                continue;
            }

            let schema_json = std::fs::read_to_string(&path)
                .map_err(|e| anyhow::anyhow!("failed to read schema file {}: {e}", path.display()))?;

            let schema_value: Value = serde_json::from_str(&schema_json)
                .map_err(|e| anyhow::anyhow!("failed to parse schema {}: {e}", path.display()))?;

            registry.register(collection, &schema_value)?;

            tracing::info!(collection = %collection, path = %path.display(), "schema loaded");
        }

        Ok(registry)
    }

    /// Register a schema for a collection.
    ///
    /// Compiles the schema and stores it. Overwrites any previously
    /// registered schema for the same collection.
    pub fn register(&self, collection: &str, schema: &Value) -> anyhow::Result<()> {
        let version = schema
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or(collection)
            .to_string();

        let validator = jsonschema::draft7::new(schema)
            .map_err(|e| anyhow::anyhow!("failed to compile schema for '{collection}': {e}"))?;

        let mut schemas = self.schemas.write().map_err(|e| {
            anyhow::anyhow!("schema registry write lock poisoned: {e}")
        })?;

        schemas.insert(
            collection.to_string(),
            SchemaEntry { validator, version },
        );

        Ok(())
    }

    /// Register a plugin-declared private collection schema.
    ///
    /// The collection is namespaced as `{plugin_id}/{collection_name}` to
    /// prevent collisions with Core CDM schemas and other plugins. Rejects
    /// any attempt to register a collection whose name matches a Core CDM
    /// name (tasks, contacts, emails, events, files, notes, credentials).
    pub fn register_plugin_schema(
        &self,
        plugin_id: &str,
        collection_name: &str,
        schema: &Value,
    ) -> anyhow::Result<()> {
        // Reject Core CDM names.
        if CORE_CDM_NAMES.contains(&collection_name) {
            return Err(anyhow::anyhow!(
                "plugin '{plugin_id}' cannot register collection '{collection_name}': \
                 name is reserved by Core CDM"
            ));
        }

        let namespaced = format!("{plugin_id}/{collection_name}");
        self.register(&namespaced, schema)?;

        tracing::info!(
            plugin_id = %plugin_id,
            collection = %collection_name,
            namespaced_key = %namespaced,
            "plugin collection schema registered"
        );

        Ok(())
    }

    /// Validate a value against the schema for the given collection.
    ///
    /// Returns `None` if no schema is registered for the collection,
    /// meaning validation is skipped.
    pub fn validate(&self, collection: &str, data: &Value) -> Option<ValidationResult> {
        let schemas = self.schemas.read().ok()?;
        let entry = schemas.get(collection)?;

        let errors: Vec<String> = entry
            .validator
            .iter_errors(data)
            .map(|e| e.to_string())
            .collect();

        Some(ValidationResult {
            valid: errors.is_empty(),
            errors,
            schema_version: entry.version.clone(),
        })
    }

    /// Check whether the registry has a schema for the given collection.
    #[allow(dead_code)]
    pub fn has_schema(&self, collection: &str) -> bool {
        self.schemas
            .read()
            .map(|s| s.contains_key(collection))
            .unwrap_or(false)
    }

    /// Return the list of registered collection names.
    pub fn collections(&self) -> Vec<String> {
        self.schemas
            .read()
            .map(|s| s.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Return the schema version string for a collection, if registered.
    #[allow(dead_code)]
    pub fn schema_version(&self, collection: &str) -> Option<String> {
        self.schemas
            .read()
            .ok()?
            .get(collection)
            .map(|e| e.version.clone())
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────
// Quarantine
// ──────────────────────────────────────────────────────────────

/// The quarantine collection name used in storage.
pub const QUARANTINE_COLLECTION: &str = "_quarantine";

/// A quarantined record that failed schema validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarantineEntry {
    /// The original data that failed validation.
    pub original_data: Value,
    /// The collection the data was destined for.
    pub original_collection: String,
    /// The plugin that submitted the data.
    pub source_plugin_id: String,
    /// Validation error messages.
    pub validation_errors: Vec<String>,
    /// The schema version that was used for validation.
    pub schema_version: String,
    /// When the record was quarantined.
    pub quarantined_at: String,
}

/// Validates record data against the schema registry before storage operations.
///
/// On create/update:
/// - If a schema exists and data passes: proceed normally.
/// - If a schema exists and data fails: store in `_quarantine` collection.
/// - If no schema exists: allow through without validation.
pub struct ValidatedStorage {
    /// The underlying storage backend.
    storage: std::sync::Arc<crate::sqlite_storage::SqliteStorage>,
    /// The schema registry for validation.
    registry: std::sync::Arc<SchemaRegistry>,
}

impl ValidatedStorage {
    /// Create a new validated storage wrapper.
    pub fn new(
        storage: std::sync::Arc<crate::sqlite_storage::SqliteStorage>,
        registry: std::sync::Arc<SchemaRegistry>,
    ) -> Self {
        Self { storage, registry }
    }

    /// Access the underlying storage directly (e.g. for quarantine admin ops).
    pub fn inner(&self) -> &crate::sqlite_storage::SqliteStorage {
        &self.storage
    }

    /// Access the schema registry.
    #[allow(dead_code)]
    pub fn registry(&self) -> &SchemaRegistry {
        &self.registry
    }

    /// Validate data and either create the record or quarantine it.
    ///
    /// Returns `Ok(record)` if validation passes or no schema exists.
    /// Returns `Err` with quarantine details if validation fails.
    #[allow(dead_code)]
    pub async fn validated_create(
        &self,
        plugin_id: &str,
        collection: &str,
        data: Value,
    ) -> Result<crate::storage::Record, QuarantineError> {
        if let Some(result) = self.registry.validate(collection, &data)
            && !result.valid
        {
            // Quarantine the invalid record.
            let entry = QuarantineEntry {
                original_data: data,
                original_collection: collection.to_string(),
                source_plugin_id: plugin_id.to_string(),
                validation_errors: result.errors.clone(),
                schema_version: result.schema_version,
                quarantined_at: chrono::Utc::now().to_rfc3339(),
            };

            let quarantine_data = serde_json::to_value(&entry)
                .map_err(|e| QuarantineError::Internal(e.to_string()))?;

            let quarantine_record = self
                .storage
                .create("core", QUARANTINE_COLLECTION, quarantine_data)
                .await
                .map_err(|e| QuarantineError::Internal(e.to_string()))?;

            tracing::warn!(
                collection = %collection,
                plugin_id = %plugin_id,
                quarantine_id = %quarantine_record.id,
                error_count = result.errors.len(),
                "record quarantined due to validation failure"
            );

            return Err(QuarantineError::ValidationFailed {
                quarantine_id: quarantine_record.id,
                errors: result.errors,
            });
        }

        // Validation passed or no schema — proceed normally.
        self.storage
            .create(plugin_id, collection, data)
            .await
            .map_err(|e| QuarantineError::Internal(e.to_string()))
    }

    /// Validate data and either update the record or quarantine the new version.
    #[allow(dead_code)]
    pub async fn validated_update(
        &self,
        plugin_id: &str,
        collection: &str,
        id: &str,
        data: Value,
        version: i64,
    ) -> Result<crate::storage::Record, QuarantineError> {
        if let Some(result) = self.registry.validate(collection, &data)
            && !result.valid
        {
            let entry = QuarantineEntry {
                original_data: data,
                original_collection: collection.to_string(),
                source_plugin_id: plugin_id.to_string(),
                validation_errors: result.errors.clone(),
                schema_version: result.schema_version,
                quarantined_at: chrono::Utc::now().to_rfc3339(),
            };

            let quarantine_data = serde_json::to_value(&entry)
                .map_err(|e| QuarantineError::Internal(e.to_string()))?;

            let quarantine_record = self
                .storage
                .create("core", QUARANTINE_COLLECTION, quarantine_data)
                .await
                .map_err(|e| QuarantineError::Internal(e.to_string()))?;

            tracing::warn!(
                collection = %collection,
                record_id = %id,
                plugin_id = %plugin_id,
                quarantine_id = %quarantine_record.id,
                error_count = result.errors.len(),
                "update quarantined due to validation failure"
            );

            return Err(QuarantineError::ValidationFailed {
                quarantine_id: quarantine_record.id,
                errors: result.errors,
            });
        }

        self.storage
            .update(plugin_id, collection, id, data, version)
            .await
            .map_err(|e| QuarantineError::Internal(e.to_string()))
    }

    /// Reprocess a quarantined record: re-validate and move back if valid.
    pub async fn reprocess_quarantined(
        &self,
        quarantine_id: &str,
    ) -> Result<crate::storage::Record, QuarantineError> {
        // Fetch the quarantine record.
        let qr = self
            .storage
            .get("core", QUARANTINE_COLLECTION, quarantine_id)
            .await
            .map_err(|e| QuarantineError::Internal(e.to_string()))?
            .ok_or_else(|| {
                QuarantineError::NotFound(quarantine_id.to_string())
            })?;

        // Parse the quarantine entry.
        let entry: QuarantineEntry = serde_json::from_value(qr.data.clone())
            .map_err(|e| QuarantineError::Internal(format!("corrupt quarantine entry: {e}")))?;

        // Re-validate against current schema.
        if let Some(result) = self.registry.validate(&entry.original_collection, &entry.original_data)
            && !result.valid
        {
            return Err(QuarantineError::ValidationFailed {
                quarantine_id: quarantine_id.to_string(),
                errors: result.errors,
            });
        }

        // Validation passes — create in the original collection.
        let record = self
            .storage
            .create(
                &entry.source_plugin_id,
                &entry.original_collection,
                entry.original_data,
            )
            .await
            .map_err(|e| QuarantineError::Internal(e.to_string()))?;

        // Delete the quarantine record.
        self.storage
            .delete("core", QUARANTINE_COLLECTION, quarantine_id)
            .await
            .map_err(|e| QuarantineError::Internal(e.to_string()))?;

        tracing::info!(
            quarantine_id = %quarantine_id,
            new_record_id = %record.id,
            collection = %record.collection,
            "quarantined record reprocessed successfully"
        );

        Ok(record)
    }
}

/// Errors from the quarantine validation layer.
#[derive(Debug)]
pub enum QuarantineError {
    /// Data failed schema validation and was quarantined.
    ValidationFailed {
        /// The ID of the quarantine record.
        quarantine_id: String,
        /// The validation errors.
        errors: Vec<String>,
    },
    /// The quarantine record was not found.
    NotFound(String),
    /// An internal error occurred.
    Internal(String),
}

impl std::fmt::Display for QuarantineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ValidationFailed {
                quarantine_id,
                errors,
            } => write!(
                f,
                "validation failed (quarantine_id={quarantine_id}): {}",
                errors.join("; ")
            ),
            Self::NotFound(id) => write!(f, "quarantine record '{id}' not found"),
            Self::Internal(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

impl std::error::Error for QuarantineError {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use tempfile::TempDir;

    fn minimal_task_schema() -> Value {
        json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "title": "Task",
            "type": "object",
            "required": ["id", "title", "status"],
            "properties": {
                "id": { "type": "string" },
                "title": { "type": "string" },
                "status": {
                    "type": "string",
                    "enum": ["pending", "active", "completed", "cancelled"]
                },
                "description": { "type": "string" }
            },
            "additionalProperties": false
        })
    }

    fn minimal_contacts_schema() -> Value {
        json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "title": "Contact",
            "type": "object",
            "required": ["id", "name"],
            "properties": {
                "id": { "type": "string" },
                "name": { "type": "string" }
            },
            "additionalProperties": false
        })
    }

    #[test]
    fn new_registry_is_empty() {
        let registry = SchemaRegistry::new();
        assert!(registry.collections().is_empty());
    }

    #[test]
    fn register_and_validate_valid_data() {
        let registry = SchemaRegistry::new();
        registry.register("tasks", &minimal_task_schema()).unwrap();

        let valid_task = json!({
            "id": "t1",
            "title": "Test",
            "status": "pending"
        });

        let result = registry.validate("tasks", &valid_task).unwrap();
        assert!(result.valid);
        assert!(result.errors.is_empty());
        assert_eq!(result.schema_version, "Task");
    }

    #[test]
    fn validate_invalid_data_returns_errors() {
        let registry = SchemaRegistry::new();
        registry.register("tasks", &minimal_task_schema()).unwrap();

        // Missing required field "title".
        let invalid_task = json!({
            "id": "t1",
            "status": "pending"
        });

        let result = registry.validate("tasks", &invalid_task).unwrap();
        assert!(!result.valid);
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn validate_invalid_enum_value() {
        let registry = SchemaRegistry::new();
        registry.register("tasks", &minimal_task_schema()).unwrap();

        let bad_status = json!({
            "id": "t1",
            "title": "Test",
            "status": "invalid_status"
        });

        let result = registry.validate("tasks", &bad_status).unwrap();
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("invalid_status") || e.contains("enum")));
    }

    #[test]
    fn validate_additional_properties_rejected() {
        let registry = SchemaRegistry::new();
        registry.register("tasks", &minimal_task_schema()).unwrap();

        let extra_fields = json!({
            "id": "t1",
            "title": "Test",
            "status": "pending",
            "unknown_field": "should not be here"
        });

        let result = registry.validate("tasks", &extra_fields).unwrap();
        assert!(!result.valid);
    }

    #[test]
    fn validate_unknown_collection_returns_none() {
        let registry = SchemaRegistry::new();
        let data = json!({"anything": true});

        let result = registry.validate("unknown_collection", &data);
        assert!(result.is_none());
    }

    #[test]
    fn has_schema_works() {
        let registry = SchemaRegistry::new();
        registry.register("tasks", &minimal_task_schema()).unwrap();

        assert!(registry.has_schema("tasks"));
        assert!(!registry.has_schema("notes"));
    }

    #[test]
    fn schema_version_is_title() {
        let registry = SchemaRegistry::new();
        registry.register("tasks", &minimal_task_schema()).unwrap();

        assert_eq!(registry.schema_version("tasks").as_deref(), Some("Task"));
        assert_eq!(registry.schema_version("missing"), None);
    }

    #[test]
    fn register_overwrites_existing() {
        let registry = SchemaRegistry::new();
        registry.register("tasks", &minimal_task_schema()).unwrap();

        // Re-register with a different schema.
        let new_schema = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "title": "Task v2",
            "type": "object",
            "required": ["id"],
            "properties": {
                "id": { "type": "string" }
            },
            "additionalProperties": false
        });
        registry.register("tasks", &new_schema).unwrap();

        assert_eq!(registry.schema_version("tasks").as_deref(), Some("Task v2"));

        // The old required field "title" is no longer needed.
        let result = registry.validate("tasks", &json!({"id": "t1"})).unwrap();
        assert!(result.valid);
    }

    #[test]
    fn load_from_directory() {
        let dir = TempDir::new().unwrap();

        // Write two collection schemas.
        fs::write(
            dir.path().join("tasks.schema.json"),
            serde_json::to_string(&minimal_task_schema()).unwrap(),
        )
        .unwrap();
        fs::write(
            dir.path().join("contacts.schema.json"),
            serde_json::to_string(&minimal_contacts_schema()).unwrap(),
        )
        .unwrap();

        // Write a plugin-manifest schema that should be skipped.
        fs::write(
            dir.path().join("plugin-manifest.schema.json"),
            r#"{"type": "object"}"#,
        )
        .unwrap();

        // Write a non-schema file that should be ignored.
        fs::write(dir.path().join("readme.txt"), "not a schema").unwrap();

        let registry = SchemaRegistry::load_from_directory(dir.path()).unwrap();

        assert!(registry.has_schema("tasks"));
        assert!(registry.has_schema("contacts"));
        assert!(!registry.has_schema("plugin-manifest"));

        let mut collections = registry.collections();
        collections.sort();
        assert_eq!(collections, vec!["contacts", "tasks"]);
    }

    #[test]
    fn load_from_directory_skips_infrastructure_schemas() {
        let dir = TempDir::new().unwrap();

        // Write a collection schema that should be loaded.
        fs::write(
            dir.path().join("tasks.schema.json"),
            serde_json::to_string(&minimal_task_schema()).unwrap(),
        )
        .unwrap();

        // Write infrastructure schemas that should all be skipped.
        fs::write(
            dir.path().join("plugin-manifest.schema.json"),
            r#"{"type": "object"}"#,
        )
        .unwrap();
        fs::write(
            dir.path().join("plugin-data.schema.json"),
            r#"{"type": "object"}"#,
        )
        .unwrap();
        fs::write(
            dir.path().join("audit-log.schema.json"),
            r#"{"type": "object"}"#,
        )
        .unwrap();

        let registry = SchemaRegistry::load_from_directory(dir.path()).unwrap();

        // Only "tasks" should be loaded.
        assert!(registry.has_schema("tasks"));
        assert!(!registry.has_schema("plugin-manifest"));
        assert!(!registry.has_schema("plugin-data"));
        assert!(!registry.has_schema("audit-log"));

        assert_eq!(registry.collections(), vec!["tasks"]);
    }

    #[test]
    fn load_from_nonexistent_directory_fails() {
        let result = SchemaRegistry::load_from_directory(Path::new("/nonexistent/path"));
        assert!(result.is_err());
    }

    #[test]
    fn load_from_directory_with_invalid_json_fails() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("bad.schema.json"), "not valid json").unwrap();

        let result = SchemaRegistry::load_from_directory(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn multiple_collections_independent() {
        let registry = SchemaRegistry::new();
        registry.register("tasks", &minimal_task_schema()).unwrap();
        registry.register("contacts", &minimal_contacts_schema()).unwrap();

        // Valid task but not a valid contact.
        let task_data = json!({ "id": "t1", "title": "Test", "status": "pending" });
        assert!(registry.validate("tasks", &task_data).unwrap().valid);
        assert!(!registry.validate("contacts", &task_data).unwrap().valid);

        // Valid contact but not a valid task.
        let contact_data = json!({ "id": "c1", "name": "Alice" });
        assert!(registry.validate("contacts", &contact_data).unwrap().valid);
        assert!(!registry.validate("tasks", &contact_data).unwrap().valid);
    }

    #[test]
    fn default_impl() {
        let registry = SchemaRegistry::default();
        assert!(registry.collections().is_empty());
    }

    #[test]
    fn load_real_schemas() {
        // Verify that the actual project schemas in docs/schemas/ can be loaded.
        let schema_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/schemas");

        if schema_dir.exists() {
            let registry = SchemaRegistry::load_from_directory(&schema_dir).unwrap();
            // We expect at least the 7 collection schemas.
            assert!(registry.collections().len() >= 7);
            assert!(registry.has_schema("tasks"));
            assert!(registry.has_schema("contacts"));
            assert!(registry.has_schema("emails"));
            assert!(registry.has_schema("events"));
            assert!(registry.has_schema("files"));
            assert!(registry.has_schema("notes"));
            assert!(registry.has_schema("credentials"));
            // plugin-manifest should NOT be loaded.
            assert!(!registry.has_schema("plugin-manifest"));
        }
    }

    // ── Quarantine & ValidatedStorage tests ──────────────────

    use crate::sqlite_storage::SqliteStorage;
    use crate::storage::StorageAdapter;
    use std::sync::Arc;

    fn setup_validated_storage(schema: Option<(&str, Value)>) -> ValidatedStorage {
        let storage = Arc::new(SqliteStorage::open_in_memory().unwrap());
        let registry = SchemaRegistry::new();
        if let Some((collection, schema_val)) = schema {
            registry.register(collection, &schema_val).unwrap();
        }
        ValidatedStorage::new(storage, Arc::new(registry))
    }

    #[tokio::test]
    async fn validated_create_passes_valid_data() {
        let vs = setup_validated_storage(Some(("tasks", minimal_task_schema())));

        let data = json!({ "id": "t1", "title": "Do thing", "status": "pending" });
        let record = vs.validated_create("plug1", "tasks", data.clone()).await.unwrap();

        assert_eq!(record.collection, "tasks");
        assert_eq!(record.data, data);
    }

    #[tokio::test]
    async fn validated_create_quarantines_invalid_data() {
        let vs = setup_validated_storage(Some(("tasks", minimal_task_schema())));

        // Missing required "title" field.
        let data = json!({ "id": "t1", "status": "pending" });
        let err = vs.validated_create("plug1", "tasks", data).await.unwrap_err();

        match err {
            QuarantineError::ValidationFailed { quarantine_id, errors } => {
                assert!(!quarantine_id.is_empty());
                assert!(!errors.is_empty());

                // Verify the quarantine record was stored.
                let qr = vs.inner()
                    .get("core", QUARANTINE_COLLECTION, &quarantine_id)
                    .await
                    .unwrap()
                    .expect("quarantine record should exist");

                let entry: QuarantineEntry = serde_json::from_value(qr.data).unwrap();
                assert_eq!(entry.original_collection, "tasks");
                assert_eq!(entry.source_plugin_id, "plug1");
                assert!(!entry.validation_errors.is_empty());
                assert_eq!(entry.schema_version, "Task");
            }
            other => panic!("expected ValidationFailed, got: {other}"),
        }
    }

    #[tokio::test]
    async fn validated_create_allows_unknown_collection() {
        let vs = setup_validated_storage(Some(("tasks", minimal_task_schema())));

        // No schema registered for "custom" — should pass through.
        let data = json!({ "any": "data", "is": "fine" });
        let record = vs.validated_create("plug1", "custom", data.clone()).await.unwrap();

        assert_eq!(record.collection, "custom");
        assert_eq!(record.data, data);
    }

    #[tokio::test]
    async fn validated_update_passes_valid_data() {
        let vs = setup_validated_storage(Some(("tasks", minimal_task_schema())));

        // First create a record.
        let data = json!({ "id": "t1", "title": "Original", "status": "pending" });
        let record = vs.validated_create("plug1", "tasks", data).await.unwrap();

        // Update with valid data.
        let new_data = json!({ "id": "t1", "title": "Updated", "status": "active" });
        let updated = vs
            .validated_update("plug1", "tasks", &record.id, new_data.clone(), record.version)
            .await
            .unwrap();

        assert_eq!(updated.data, new_data);
        assert_eq!(updated.version, 2);
    }

    #[tokio::test]
    async fn validated_update_quarantines_invalid_data() {
        let vs = setup_validated_storage(Some(("tasks", minimal_task_schema())));

        let data = json!({ "id": "t1", "title": "Original", "status": "pending" });
        let record = vs.validated_create("plug1", "tasks", data).await.unwrap();

        // Update with invalid data (bad enum value).
        let bad_data = json!({ "id": "t1", "title": "Updated", "status": "invalid" });
        let err = vs
            .validated_update("plug1", "tasks", &record.id, bad_data, record.version)
            .await
            .unwrap_err();

        match err {
            QuarantineError::ValidationFailed { errors, .. } => {
                assert!(!errors.is_empty());
            }
            other => panic!("expected ValidationFailed, got: {other}"),
        }
    }

    #[tokio::test]
    async fn reprocess_quarantined_succeeds_when_valid() {
        let storage = Arc::new(SqliteStorage::open_in_memory().unwrap());
        let registry = Arc::new({
            let r = SchemaRegistry::new();
            // Schema that requires "id" and "title".
            r.register("tasks", &json!({
                "$schema": "http://json-schema.org/draft-07/schema#",
                "title": "Task",
                "type": "object",
                "required": ["id", "title"],
                "properties": {
                    "id": { "type": "string" },
                    "title": { "type": "string" }
                },
                "additionalProperties": false
            })).unwrap();
            r
        });
        let vs = ValidatedStorage::new(storage, registry);

        // Data that satisfies the schema — manually quarantine it for test.
        let entry = QuarantineEntry {
            original_data: json!({ "id": "t1", "title": "Valid now" }),
            original_collection: "tasks".to_string(),
            source_plugin_id: "plug1".to_string(),
            validation_errors: vec!["was invalid before".into()],
            schema_version: "Task".to_string(),
            quarantined_at: chrono::Utc::now().to_rfc3339(),
        };
        let qr_data = serde_json::to_value(&entry).unwrap();
        let qr = vs.inner()
            .create("core", QUARANTINE_COLLECTION, qr_data)
            .await
            .unwrap();

        // Reprocess — should succeed since data now passes.
        let record = vs.reprocess_quarantined(&qr.id).await.unwrap();
        assert_eq!(record.collection, "tasks");
        assert_eq!(record.data["title"], "Valid now");

        // Quarantine record should be deleted.
        let gone = vs.inner()
            .get("core", QUARANTINE_COLLECTION, &qr.id)
            .await
            .unwrap();
        assert!(gone.is_none());
    }

    #[tokio::test]
    async fn reprocess_quarantined_fails_when_still_invalid() {
        let vs = setup_validated_storage(Some(("tasks", minimal_task_schema())));

        // Quarantine with data missing required "title".
        let entry = QuarantineEntry {
            original_data: json!({ "id": "t1", "status": "pending" }),
            original_collection: "tasks".to_string(),
            source_plugin_id: "plug1".to_string(),
            validation_errors: vec!["missing title".into()],
            schema_version: "Task".to_string(),
            quarantined_at: chrono::Utc::now().to_rfc3339(),
        };
        let qr_data = serde_json::to_value(&entry).unwrap();
        let qr = vs.inner()
            .create("core", QUARANTINE_COLLECTION, qr_data)
            .await
            .unwrap();

        let err = vs.reprocess_quarantined(&qr.id).await.unwrap_err();
        match err {
            QuarantineError::ValidationFailed { errors, .. } => {
                assert!(!errors.is_empty());
            }
            other => panic!("expected ValidationFailed, got: {other}"),
        }

        // Quarantine record should still exist.
        let still_there = vs.inner()
            .get("core", QUARANTINE_COLLECTION, &qr.id)
            .await
            .unwrap();
        assert!(still_there.is_some());
    }

    #[tokio::test]
    async fn reprocess_nonexistent_quarantine_record() {
        let vs = setup_validated_storage(None);

        let err = vs.reprocess_quarantined("nonexistent").await.unwrap_err();
        match err {
            QuarantineError::NotFound(id) => assert_eq!(id, "nonexistent"),
            other => panic!("expected NotFound, got: {other}"),
        }
    }

    #[tokio::test]
    async fn quarantine_entry_includes_schema_version() {
        let vs = setup_validated_storage(Some(("tasks", minimal_task_schema())));

        let data = json!({ "id": "t1" }); // Missing title and status.
        let err = vs.validated_create("plug1", "tasks", data).await.unwrap_err();

        match err {
            QuarantineError::ValidationFailed { quarantine_id, .. } => {
                let qr = vs.inner()
                    .get("core", QUARANTINE_COLLECTION, &quarantine_id)
                    .await
                    .unwrap()
                    .unwrap();
                let entry: QuarantineEntry = serde_json::from_value(qr.data).unwrap();
                assert_eq!(entry.schema_version, "Task");
            }
            other => panic!("expected ValidationFailed, got: {other}"),
        }
    }

    #[tokio::test]
    async fn quarantine_records_multiple_errors() {
        let vs = setup_validated_storage(Some(("tasks", minimal_task_schema())));

        // Missing both "title" and "status" — should produce 2 errors.
        let data = json!({ "id": "t1" });
        let err = vs.validated_create("plug1", "tasks", data).await.unwrap_err();

        match err {
            QuarantineError::ValidationFailed { errors, .. } => {
                assert!(errors.len() >= 2, "expected at least 2 errors, got {}", errors.len());
            }
            other => panic!("expected ValidationFailed, got: {other}"),
        }
    }

    #[tokio::test]
    async fn validated_create_empty_object_against_schema() {
        let vs = setup_validated_storage(Some(("tasks", minimal_task_schema())));

        let data = json!({});
        let err = vs.validated_create("plug1", "tasks", data).await.unwrap_err();

        match err {
            QuarantineError::ValidationFailed { errors, .. } => {
                assert!(!errors.is_empty());
            }
            other => panic!("expected ValidationFailed, got: {other}"),
        }
    }

    #[test]
    fn quarantine_error_display() {
        let err = QuarantineError::ValidationFailed {
            quarantine_id: "q1".into(),
            errors: vec!["bad field".into(), "missing prop".into()],
        };
        let msg = err.to_string();
        assert!(msg.contains("q1"));
        assert!(msg.contains("bad field"));

        let err2 = QuarantineError::NotFound("q2".into());
        assert!(err2.to_string().contains("q2"));

        let err3 = QuarantineError::Internal("oops".into());
        assert!(err3.to_string().contains("oops"));
    }

    // ── Plugin schema registration tests ─────────────────────

    fn recipe_schema() -> Value {
        json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "title": "Recipe",
            "type": "object",
            "required": ["id", "name"],
            "properties": {
                "id": { "type": "string" },
                "name": { "type": "string" },
                "servings": { "type": "integer" }
            },
            "additionalProperties": false
        })
    }

    #[test]
    fn register_plugin_schema_namespaces_correctly() {
        let registry = SchemaRegistry::new();
        registry
            .register_plugin_schema("com.example.recipes", "recipes", &recipe_schema())
            .unwrap();

        assert!(registry.has_schema("com.example.recipes/recipes"));
        assert!(!registry.has_schema("recipes"));
    }

    #[test]
    fn validate_against_plugin_schema() {
        let registry = SchemaRegistry::new();
        registry
            .register_plugin_schema("com.example.recipes", "recipes", &recipe_schema())
            .unwrap();

        let valid = json!({ "id": "r1", "name": "Pancakes", "servings": 4 });
        let result = registry
            .validate("com.example.recipes/recipes", &valid)
            .unwrap();
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn invalid_data_caught_by_plugin_schema_validation() {
        let registry = SchemaRegistry::new();
        registry
            .register_plugin_schema("com.example.recipes", "recipes", &recipe_schema())
            .unwrap();

        // Missing required "name" field.
        let invalid = json!({ "id": "r1" });
        let result = registry
            .validate("com.example.recipes/recipes", &invalid)
            .unwrap();
        assert!(!result.valid);
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn plugin_cannot_register_core_cdm_collection_name() {
        let registry = SchemaRegistry::new();

        for name in CORE_CDM_NAMES {
            let result = registry.register_plugin_schema(
                "com.evil.plugin",
                name,
                &json!({"type": "object"}),
            );
            assert!(result.is_err());
            let msg = result.unwrap_err().to_string();
            assert!(msg.contains("reserved by Core CDM"), "expected CDM error for '{name}', got: {msg}");
        }
    }

    #[test]
    fn different_plugins_same_collection_name_no_collision() {
        let registry = SchemaRegistry::new();

        let schema_a = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "title": "Widget A",
            "type": "object",
            "required": ["id", "color"],
            "properties": {
                "id": { "type": "string" },
                "color": { "type": "string" }
            },
            "additionalProperties": false
        });
        let schema_b = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "title": "Widget B",
            "type": "object",
            "required": ["id", "weight"],
            "properties": {
                "id": { "type": "string" },
                "weight": { "type": "number" }
            },
            "additionalProperties": false
        });

        registry
            .register_plugin_schema("com.alpha", "widgets", &schema_a)
            .unwrap();
        registry
            .register_plugin_schema("com.beta", "widgets", &schema_b)
            .unwrap();

        // They are distinct entries.
        assert!(registry.has_schema("com.alpha/widgets"));
        assert!(registry.has_schema("com.beta/widgets"));

        // Validate data against plugin A's schema — requires "color".
        let a_data = json!({ "id": "w1", "color": "red" });
        assert!(registry.validate("com.alpha/widgets", &a_data).unwrap().valid);
        assert!(!registry.validate("com.beta/widgets", &a_data).unwrap().valid);

        // Validate data against plugin B's schema — requires "weight".
        let b_data = json!({ "id": "w2", "weight": 3.5 });
        assert!(registry.validate("com.beta/widgets", &b_data).unwrap().valid);
        assert!(!registry.validate("com.alpha/widgets", &b_data).unwrap().valid);
    }
}
