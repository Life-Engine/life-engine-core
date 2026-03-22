//! Configurable JSON payload mapping.
//!
//! Extracts fields from incoming webhook payloads using dot-separated
//! paths and produces a flat output object with the mapped field names.

use crate::models::PayloadMapping;

/// Apply a list of payload mappings to an input JSON value.
///
/// For each mapping, extracts the value at `source_path` (dot-separated)
/// from the input and sets it at `target_field` in the output object.
/// Fields that do not exist in the input are silently skipped.
pub fn apply_mappings(
    input: &serde_json::Value,
    mappings: &[PayloadMapping],
) -> serde_json::Value {
    let mut output = serde_json::Map::new();

    for mapping in mappings {
        if let Some(value) = resolve_path(input, &mapping.source_path) {
            output.insert(mapping.target_field.clone(), value.clone());
        }
    }

    serde_json::Value::Object(output)
}

/// Resolve a dot-separated path in a JSON value.
///
/// For example, `"repository.owner.login"` navigates:
/// `value["repository"]["owner"]["login"]`
fn resolve_path<'a>(value: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let mut current = value;
    for segment in path.split('.') {
        current = current.get(segment)?;
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PayloadMapping;

    #[test]
    fn apply_single_mapping() {
        let input = serde_json::json!({"name": "test"});
        let mappings = vec![PayloadMapping {
            source_path: "name".to_string(),
            target_field: "title".to_string(),
        }];

        let result = apply_mappings(&input, &mappings);
        assert_eq!(result["title"], "test");
    }

    #[test]
    fn apply_nested_mapping() {
        let input = serde_json::json!({
            "repository": {
                "owner": {
                    "login": "octocat"
                }
            }
        });
        let mappings = vec![PayloadMapping {
            source_path: "repository.owner.login".to_string(),
            target_field: "author".to_string(),
        }];

        let result = apply_mappings(&input, &mappings);
        assert_eq!(result["author"], "octocat");
    }

    #[test]
    fn apply_multiple_mappings() {
        let input = serde_json::json!({
            "user": { "email": "test@example.com" },
            "action": "created"
        });
        let mappings = vec![
            PayloadMapping {
                source_path: "user.email".to_string(),
                target_field: "email".to_string(),
            },
            PayloadMapping {
                source_path: "action".to_string(),
                target_field: "event_type".to_string(),
            },
        ];

        let result = apply_mappings(&input, &mappings);
        assert_eq!(result["email"], "test@example.com");
        assert_eq!(result["event_type"], "created");
    }

    #[test]
    fn missing_source_path_is_skipped() {
        let input = serde_json::json!({"exists": true});
        let mappings = vec![
            PayloadMapping {
                source_path: "exists".to_string(),
                target_field: "found".to_string(),
            },
            PayloadMapping {
                source_path: "does.not.exist".to_string(),
                target_field: "missing".to_string(),
            },
        ];

        let result = apply_mappings(&input, &mappings);
        assert_eq!(result["found"], true);
        assert!(result.get("missing").is_none());
    }

    #[test]
    fn empty_mappings_returns_empty_object() {
        let input = serde_json::json!({"data": "value"});
        let result = apply_mappings(&input, &[]);
        assert_eq!(result, serde_json::json!({}));
    }

    #[test]
    fn mapping_preserves_value_types() {
        let input = serde_json::json!({
            "count": 42,
            "active": true,
            "tags": ["a", "b"],
            "meta": { "key": "val" }
        });
        let mappings = vec![
            PayloadMapping {
                source_path: "count".to_string(),
                target_field: "num".to_string(),
            },
            PayloadMapping {
                source_path: "active".to_string(),
                target_field: "is_active".to_string(),
            },
            PayloadMapping {
                source_path: "tags".to_string(),
                target_field: "labels".to_string(),
            },
            PayloadMapping {
                source_path: "meta".to_string(),
                target_field: "metadata".to_string(),
            },
        ];

        let result = apply_mappings(&input, &mappings);
        assert_eq!(result["num"], 42);
        assert_eq!(result["is_active"], true);
        assert!(result["labels"].is_array());
        assert!(result["metadata"].is_object());
    }

    #[test]
    fn resolve_path_returns_none_for_nonexistent() {
        let value = serde_json::json!({"a": {"b": 1}});
        assert!(resolve_path(&value, "a.c").is_none());
        assert!(resolve_path(&value, "x.y.z").is_none());
    }

    #[test]
    fn resolve_path_handles_single_segment() {
        let value = serde_json::json!({"key": "val"});
        let result = resolve_path(&value, "key");
        assert_eq!(result, Some(&serde_json::json!("val")));
    }

    #[test]
    fn resolve_path_handles_deeply_nested() {
        let value = serde_json::json!({"a": {"b": {"c": {"d": 99}}}});
        let result = resolve_path(&value, "a.b.c.d");
        assert_eq!(result, Some(&serde_json::json!(99)));
    }
}
