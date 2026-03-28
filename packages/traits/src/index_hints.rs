//! Index hint types for plugin-declared collection indexes.
//!
//! Plugins declare index hints in their manifest. Adapters that support
//! indexing (`AdapterCapabilities.indexing = true`) create the indexes;
//! adapters without indexing silently ignore them.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

/// Error type for schema and index hint operations.
#[derive(Debug, Clone)]
pub struct SchemaError {
    pub message: String,
}

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SchemaError: {}", self.message)
    }
}

impl std::error::Error for SchemaError {}

/// A single index hint declared by a plugin for a collection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexHint {
    /// Field paths to index (e.g., `["email"]` or `["address.city", "address.state"]`).
    pub fields: Vec<String>,
    /// Whether this index enforces a uniqueness constraint.
    pub unique: bool,
    /// Optional human-readable name for the index.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Describes a collection declared by a plugin, including schema and index hints.
#[derive(Debug, Clone)]
pub struct CollectionDescriptor {
    /// Collection name (e.g., `"contacts"`, `"tasks"`).
    pub name: String,
    /// ID of the plugin that owns this collection.
    pub plugin_id: String,
    /// Schema reference — a file path or `cdm:` prefixed reference, or `None` for schemaless.
    pub schema: Option<String>,
    /// Whether strict mode is enabled (reject unknown fields).
    pub strict: bool,
    /// Index hints for this collection.
    pub indexes: Vec<IndexHint>,
}

/// Parse index hints from a plugin manifest's collection section.
///
/// Expects a JSON value that is either:
/// - A JSON array of index hint objects
/// - `null` or absent (returns empty vec)
///
/// Each index hint object must have a `fields` array of strings and a `unique` boolean.
/// The `name` field is optional.
pub fn parse_index_hints(manifest_collection: &Value) -> Result<Vec<IndexHint>, SchemaError> {
    let indexes_value = match manifest_collection.get("indexes") {
        Some(v) => v,
        None => return Ok(Vec::new()),
    };

    if indexes_value.is_null() {
        return Ok(Vec::new());
    }

    let arr = indexes_value.as_array().ok_or_else(|| SchemaError {
        message: "indexes must be an array".to_string(),
    })?;

    let mut hints = Vec::with_capacity(arr.len());
    for (i, entry) in arr.iter().enumerate() {
        let fields_val = entry.get("fields").ok_or_else(|| SchemaError {
            message: format!("indexes[{i}]: missing required field 'fields'"),
        })?;

        let fields_arr = fields_val.as_array().ok_or_else(|| SchemaError {
            message: format!("indexes[{i}]: 'fields' must be an array"),
        })?;

        if fields_arr.is_empty() {
            return Err(SchemaError {
                message: format!("indexes[{i}]: 'fields' must not be empty"),
            });
        }

        let mut fields = Vec::with_capacity(fields_arr.len());
        for (j, f) in fields_arr.iter().enumerate() {
            let s = f.as_str().ok_or_else(|| SchemaError {
                message: format!("indexes[{i}].fields[{j}]: must be a string"),
            })?;
            fields.push(s.to_string());
        }

        let unique = match entry.get("unique") {
            Some(v) => v.as_bool().ok_or_else(|| SchemaError {
                message: format!("indexes[{i}]: 'unique' must be a boolean"),
            })?,
            None => false,
        };

        let name = entry
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        hints.push(IndexHint {
            fields,
            unique,
            name,
        });
    }

    Ok(hints)
}

/// Merge CDM default indexes with plugin-declared indexes.
///
/// Plugin declarations take precedence on field path conflicts. Two index hints
/// conflict when they have exactly the same sorted field paths. In case of
/// conflict, the plugin hint replaces the CDM hint.
pub fn merge_index_hints(cdm_hints: &[IndexHint], plugin_hints: &[IndexHint]) -> Vec<IndexHint> {
    // Build a set of field-path keys from plugin hints for conflict detection.
    let plugin_keys: std::collections::HashSet<Vec<String>> = plugin_hints
        .iter()
        .map(|h| {
            let mut sorted = h.fields.clone();
            sorted.sort();
            sorted
        })
        .collect();

    let mut merged: Vec<IndexHint> = Vec::new();

    // Add CDM hints that don't conflict with plugin hints.
    for hint in cdm_hints {
        let mut key = hint.fields.clone();
        key.sort();
        if !plugin_keys.contains(&key) {
            merged.push(hint.clone());
        }
    }

    // Add all plugin hints (they take precedence).
    merged.extend(plugin_hints.iter().cloned());

    merged
}
