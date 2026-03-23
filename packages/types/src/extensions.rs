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
