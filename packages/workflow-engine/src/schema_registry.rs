//! Private collection schema registry for plugin-owned data.
//!
//! During plugin loading, each plugin's manifest declares private collections
//! with JSON Schema definitions. These schemas are registered here, keyed by
//! `(plugin_id, collection_name)`, and used by the storage validation layer to
//! enforce data integrity on writes. Private collections are fully isolated by
//! plugin ID — no cross-plugin access is permitted.

use std::collections::HashMap;
use std::sync::RwLock;

use serde_json::Value;

/// Canonical CDM collection names that plugins cannot use for private collections.
const CANONICAL_COLLECTIONS: &[&str] = &[
    "events",
    "tasks",
    "contacts",
    "notes",
    "emails",
    "files",
    "credentials",
];

/// A thread-safe registry of JSON Schema definitions for plugin private collections.
///
/// Schemas are stored as raw `serde_json::Value` objects keyed by
/// `(plugin_id, collection_name)`. The storage backend reads schemas from this
/// registry to validate writes to private collections.
pub struct SchemaRegistry {
    schemas: RwLock<HashMap<(String, String), Value>>,
}

impl SchemaRegistry {
    /// Create an empty schema registry.
    pub fn new() -> Self {
        Self {
            schemas: RwLock::new(HashMap::new()),
        }
    }

    /// Register a JSON Schema for a private collection owned by `plugin_id`.
    ///
    /// The schema is stored as-is. Returns an error if `collection` collides
    /// with a canonical CDM collection name.
    pub fn register(
        &self,
        plugin_id: &str,
        collection: &str,
        schema: Value,
    ) -> Result<(), SchemaRegistryError> {
        if CANONICAL_COLLECTIONS.contains(&collection) {
            return Err(SchemaRegistryError::ReservedCollection {
                plugin_id: plugin_id.to_string(),
                collection: collection.to_string(),
            });
        }

        let mut schemas = self.schemas.write().map_err(|_| SchemaRegistryError::LockPoisoned)?;
        schemas.insert(
            (plugin_id.to_string(), collection.to_string()),
            schema,
        );

        tracing::info!(
            plugin_id = %plugin_id,
            collection = %collection,
            "private collection schema registered"
        );

        Ok(())
    }

    /// Retrieve the raw JSON Schema for a private collection.
    ///
    /// Returns `None` if no schema is registered for the given
    /// `(plugin_id, collection)` pair. This enforces namespace isolation:
    /// a plugin can only look up its own schemas.
    pub fn get_schema(&self, plugin_id: &str, collection: &str) -> Option<Value> {
        let schemas = self.schemas.read().ok()?;
        schemas
            .get(&(plugin_id.to_string(), collection.to_string()))
            .cloned()
    }

    /// Returns `true` if a schema is registered for this plugin/collection pair.
    pub fn is_registered(&self, plugin_id: &str, collection: &str) -> bool {
        self.schemas
            .read()
            .map(|s| s.contains_key(&(plugin_id.to_string(), collection.to_string())))
            .unwrap_or(false)
    }

    /// Return the number of registered schemas.
    #[cfg(test)]
    fn len(&self) -> usize {
        self.schemas.read().map(|s| s.len()).unwrap_or(0)
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur during schema registration.
#[derive(Debug, thiserror::Error)]
pub enum SchemaRegistryError {
    /// The collection name is reserved by the canonical data model.
    #[error("plugin '{plugin_id}' cannot register collection '{collection}': name is reserved by canonical CDM")]
    ReservedCollection {
        plugin_id: String,
        collection: String,
    },

    /// The internal RwLock was poisoned.
    #[error("schema registry lock poisoned")]
    LockPoisoned,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn weather_schema() -> Value {
        json!({
            "type": "object",
            "required": ["location", "temperature"],
            "properties": {
                "location": { "type": "string" },
                "temperature": { "type": "number" },
                "conditions": { "type": "string" }
            },
            "additionalProperties": false
        })
    }

    #[test]
    fn registration_succeeds() {
        let registry = SchemaRegistry::new();
        let result = registry.register("com.example.weather", "forecasts", weather_schema());
        assert!(result.is_ok());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn schema_lookup_works() {
        let registry = SchemaRegistry::new();
        registry
            .register("com.example.weather", "forecasts", weather_schema())
            .unwrap();

        let schema = registry.get_schema("com.example.weather", "forecasts");
        assert!(schema.is_some());

        let schema = schema.unwrap();
        assert_eq!(schema["type"], "object");
        assert!(schema["required"].as_array().unwrap().contains(&json!("location")));
    }

    #[test]
    fn unregistered_collection_returns_none() {
        let registry = SchemaRegistry::new();
        assert!(registry.get_schema("com.example.weather", "forecasts").is_none());
        assert!(!registry.is_registered("com.example.weather", "forecasts"));
    }

    #[test]
    fn cross_plugin_access_denied() {
        let registry = SchemaRegistry::new();
        registry
            .register("com.example.weather", "forecasts", weather_schema())
            .unwrap();

        // A different plugin cannot see weather's schemas.
        assert!(registry.get_schema("com.example.maps", "forecasts").is_none());
        assert!(!registry.is_registered("com.example.maps", "forecasts"));
    }

    #[test]
    fn canonical_collection_name_rejected() {
        let registry = SchemaRegistry::new();
        for name in CANONICAL_COLLECTIONS {
            let result = registry.register("com.example.plugin", name, weather_schema());
            assert!(result.is_err(), "should reject canonical name '{name}'");
            assert!(
                matches!(result.unwrap_err(), SchemaRegistryError::ReservedCollection { .. }),
                "expected ReservedCollection error for '{name}'"
            );
        }
    }

    #[test]
    fn is_registered_reflects_state() {
        let registry = SchemaRegistry::new();
        assert!(!registry.is_registered("plug", "coll"));

        registry.register("plug", "coll", json!({"type": "object"})).unwrap();
        assert!(registry.is_registered("plug", "coll"));
        assert!(!registry.is_registered("plug", "other"));
        assert!(!registry.is_registered("other", "coll"));
    }

    #[test]
    fn multiple_plugins_multiple_collections() {
        let registry = SchemaRegistry::new();
        registry
            .register("plugin.a", "data", json!({"type": "object"}))
            .unwrap();
        registry
            .register("plugin.a", "config", json!({"type": "array"}))
            .unwrap();
        registry
            .register("plugin.b", "data", json!({"type": "string"}))
            .unwrap();

        assert_eq!(registry.len(), 3);

        // Same collection name, different plugins, different schemas.
        let a_data = registry.get_schema("plugin.a", "data").unwrap();
        let b_data = registry.get_schema("plugin.b", "data").unwrap();
        assert_eq!(a_data["type"], "object");
        assert_eq!(b_data["type"], "string");
    }

    #[test]
    fn overwrite_replaces_schema() {
        let registry = SchemaRegistry::new();
        registry
            .register("plug", "coll", json!({"type": "object"}))
            .unwrap();
        registry
            .register("plug", "coll", json!({"type": "array"}))
            .unwrap();

        let schema = registry.get_schema("plug", "coll").unwrap();
        assert_eq!(schema["type"], "array");
        assert_eq!(registry.len(), 1);
    }
}
