//! Extension namespace validation for canonical data models.
//!
//! Plugins attach custom fields to canonical records via the `extensions` object.
//! Each top-level key must be the writing plugin's reverse-domain ID
//! (e.g., `com.life-engine.github`). This module enforces that constraint on writes
//! while preserving all namespaces on reads.

use serde_json::Value;
use std::fmt;

/// Error returned when extension namespace validation fails.
#[derive(Debug, Clone, PartialEq)]
pub enum ExtensionError {
    /// A plugin attempted to write to another plugin's namespace.
    NamespaceMismatch {
        plugin_id: String,
        foreign_key: String,
    },
}

impl fmt::Display for ExtensionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExtensionError::NamespaceMismatch {
                plugin_id,
                foreign_key,
            } => {
                write!(
                    f,
                    "plugin '{}' cannot write to extension namespace '{}' — \
                     plugins may only write to their own namespace",
                    plugin_id, foreign_key
                )
            }
        }
    }
}

impl std::error::Error for ExtensionError {}

/// Read a single field from a plugin's extension namespace.
///
/// Returns `None` if `extensions` is absent, is not an object, the plugin namespace
/// does not exist, or the field does not exist within the namespace.
pub fn get_ext(extensions: &Option<Value>, plugin_id: &str, field: &str) -> Option<Value> {
    extensions
        .as_ref()?
        .get(plugin_id)?
        .get(field)
        .cloned()
}

/// Set a single field within a plugin's extension namespace, using merge-not-replace
/// semantics. If the extensions object or plugin namespace does not exist yet, they
/// are created. Other plugin namespaces are preserved unchanged.
///
/// Returns the updated extensions value (always `Some`).
pub fn set_ext(
    extensions: &Option<Value>,
    plugin_id: &str,
    field: &str,
    value: Value,
) -> Value {
    let mut root = match extensions {
        Some(Value::Object(map)) => Value::Object(map.clone()),
        _ => Value::Object(serde_json::Map::new()),
    };

    let ns = root
        .as_object_mut()
        .unwrap()
        .entry(plugin_id)
        .or_insert_with(|| Value::Object(serde_json::Map::new()));

    if let Value::Object(ns_map) = ns {
        ns_map.insert(field.to_string(), value);
    }

    root
}

/// Validate that all top-level keys in `extensions` match the writing plugin's ID.
///
/// On write, every key in the extensions object must equal `plugin_id`.
/// Returns `Ok(())` if extensions is null, not an object, or all keys match.
/// Returns `Err(ExtensionError::NamespaceMismatch)` if any key belongs to a
/// different plugin.
pub fn validate_extension_namespace(
    plugin_id: &str,
    extensions: &Value,
) -> Result<(), ExtensionError> {
    if let Value::Object(map) = extensions {
        for key in map.keys() {
            if key != plugin_id {
                return Err(ExtensionError::NamespaceMismatch {
                    plugin_id: plugin_id.to_string(),
                    foreign_key: key.clone(),
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn valid_namespace_passes() {
        let extensions = json!({
            "com.life-engine.github": {
                "repo": "life-engine/core",
                "pr_number": 456
            }
        });
        assert!(validate_extension_namespace("com.life-engine.github", &extensions).is_ok());
    }

    #[test]
    fn cross_namespace_write_rejected() {
        let extensions = json!({
            "com.life-engine.github": {
                "repo": "life-engine/core"
            },
            "com.example.pomodoro": {
                "estimated_pomodoros": 2
            }
        });
        let result = validate_extension_namespace("com.life-engine.github", &extensions);
        assert!(result.is_err());
        match result.unwrap_err() {
            ExtensionError::NamespaceMismatch {
                plugin_id,
                foreign_key,
            } => {
                assert_eq!(plugin_id, "com.life-engine.github");
                assert_eq!(foreign_key, "com.example.pomodoro");
            }
        }
    }

    #[test]
    fn read_returns_all_namespaces() {
        // On read, no validation is performed — the extensions object is
        // returned as-is. This test verifies that multi-namespace data
        // round-trips through serde without loss.
        let extensions = json!({
            "com.life-engine.github": { "repo": "core" },
            "com.example.pomodoro": { "pomodoros": 3 }
        });
        let serialized = serde_json::to_string(&extensions).unwrap();
        let restored: Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(extensions, restored);
    }

    #[test]
    fn empty_extensions_passes() {
        let extensions = json!({});
        assert!(validate_extension_namespace("com.life-engine.github", &extensions).is_ok());
    }

    #[test]
    fn null_extensions_passes() {
        let extensions = Value::Null;
        assert!(validate_extension_namespace("com.life-engine.github", &extensions).is_ok());
    }

    #[test]
    fn get_ext_returns_field_value() {
        let extensions = Some(json!({
            "com.life-engine.github": {
                "repo": "life-engine/core",
                "pr_number": 456
            }
        }));
        let result = super::get_ext(&extensions, "com.life-engine.github", "repo");
        assert_eq!(result, Some(json!("life-engine/core")));
    }

    #[test]
    fn get_ext_returns_none_for_missing_plugin() {
        let extensions = Some(json!({
            "com.life-engine.github": { "repo": "core" }
        }));
        assert_eq!(
            super::get_ext(&extensions, "com.example.other", "repo"),
            None
        );
    }

    #[test]
    fn get_ext_returns_none_for_missing_field() {
        let extensions = Some(json!({
            "com.life-engine.github": { "repo": "core" }
        }));
        assert_eq!(
            super::get_ext(&extensions, "com.life-engine.github", "missing"),
            None
        );
    }

    #[test]
    fn get_ext_returns_none_when_extensions_is_none() {
        assert_eq!(
            super::get_ext(&None, "com.life-engine.github", "repo"),
            None
        );
    }

    #[test]
    fn set_ext_creates_namespace_and_field() {
        let result = super::set_ext(&None, "com.example.plugin", "count", json!(42));
        assert_eq!(result, json!({ "com.example.plugin": { "count": 42 } }));
    }

    #[test]
    fn set_ext_preserves_other_namespaces() {
        let extensions = Some(json!({
            "com.life-engine.github": { "repo": "core" }
        }));
        let result = super::set_ext(&extensions, "com.example.plugin", "count", json!(42));
        assert_eq!(
            result,
            json!({
                "com.life-engine.github": { "repo": "core" },
                "com.example.plugin": { "count": 42 }
            })
        );
    }

    #[test]
    fn set_ext_preserves_other_fields_in_namespace() {
        let extensions = Some(json!({
            "com.example.plugin": { "existing": "value" }
        }));
        let result = super::set_ext(&extensions, "com.example.plugin", "new_field", json!(true));
        assert_eq!(
            result,
            json!({
                "com.example.plugin": { "existing": "value", "new_field": true }
            })
        );
    }

    #[test]
    fn set_ext_overwrites_existing_field() {
        let extensions = Some(json!({
            "com.example.plugin": { "count": 1 }
        }));
        let result = super::set_ext(&extensions, "com.example.plugin", "count", json!(2));
        assert_eq!(result, json!({ "com.example.plugin": { "count": 2 } }));
    }

    #[test]
    fn nested_extension_data_preserved() {
        let extensions = json!({
            "com.life-engine.github": {
                "repo": "life-engine/core",
                "pr": {
                    "number": 456,
                    "labels": ["bug", "urgent"],
                    "reviewers": [
                        { "login": "alice", "approved": true },
                        { "login": "bob", "approved": false }
                    ]
                }
            }
        });
        assert!(validate_extension_namespace("com.life-engine.github", &extensions).is_ok());
        // Verify nested structure is intact after round-trip.
        let serialized = serde_json::to_string(&extensions).unwrap();
        let restored: Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(extensions, restored);
    }
}
