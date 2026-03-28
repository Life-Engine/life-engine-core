//! Integration tests for schema compatibility checker.
//!
//! Covers spec requirements 2-5 and 10 from
//! `.odm/spec/schema-versioning-rules/requirements.md`.

use life_engine_traits::schema_versioning::{
    check_compatibility, ChangeKind, CompatibilityResult, DeprecationTracker,
};
use serde_json::json;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn assert_compatible(old: &serde_json::Value, new: &serde_json::Value) {
    let result = check_compatibility(old, new);
    assert_eq!(result, CompatibilityResult::Compatible, "Expected Compatible");
}

fn assert_breaking(
    old: &serde_json::Value,
    new: &serde_json::Value,
    expected_kind: ChangeKind,
) -> Vec<life_engine_traits::schema_versioning::BreakingChange> {
    match check_compatibility(old, new) {
        CompatibilityResult::Breaking(changes) => {
            assert!(
                changes.iter().any(|c| c.kind == expected_kind),
                "Expected a {:?} change, got: {:#?}",
                expected_kind,
                changes
            );
            changes
        }
        CompatibilityResult::Compatible => {
            panic!("Expected Breaking({expected_kind:?}), got Compatible");
        }
    }
}

// ---------------------------------------------------------------------------
// 1. Identical schemas → Compatible
// ---------------------------------------------------------------------------

#[test]
fn test_identical_schemas_compatible() {
    let schema = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "age": { "type": "integer" }
        },
        "required": ["name"]
    });
    assert_compatible(&schema, &schema);
}

// ---------------------------------------------------------------------------
// 2. Adding optional field → Compatible (Req 2.1)
// ---------------------------------------------------------------------------

#[test]
fn test_adding_optional_field_compatible() {
    let old = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" }
        },
        "required": ["name"]
    });
    let new = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "nickname": { "type": "string" }
        },
        "required": ["name"]
    });
    assert_compatible(&old, &new);
}

// ---------------------------------------------------------------------------
// 3. Adding enum value → Compatible (Req 2.2)
// ---------------------------------------------------------------------------

#[test]
fn test_adding_enum_value_compatible() {
    let old = json!({
        "type": "object",
        "properties": {
            "status": {
                "type": "string",
                "enum": ["active", "inactive"]
            }
        }
    });
    let new = json!({
        "type": "object",
        "properties": {
            "status": {
                "type": "string",
                "enum": ["active", "inactive", "archived"]
            }
        }
    });
    assert_compatible(&old, &new);
}

// ---------------------------------------------------------------------------
// 4. Relaxing constraint (reduce minLength) → Compatible (Req 2.4)
// ---------------------------------------------------------------------------

#[test]
fn test_relaxing_constraint_compatible() {
    let old = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string", "minLength": 5 }
        }
    });
    let new = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string", "minLength": 2 }
        }
    });
    assert_compatible(&old, &new);
}

#[test]
fn test_relaxing_constraint_remove_pattern_compatible() {
    let old = json!({
        "type": "object",
        "properties": {
            "email": { "type": "string", "pattern": "^.+@.+$" }
        }
    });
    let new = json!({
        "type": "object",
        "properties": {
            "email": { "type": "string" }
        }
    });
    assert_compatible(&old, &new);
}

#[test]
fn test_relaxing_max_length_compatible() {
    let old = json!({
        "type": "object",
        "properties": {
            "bio": { "type": "string", "maxLength": 100 }
        }
    });
    let new = json!({
        "type": "object",
        "properties": {
            "bio": { "type": "string", "maxLength": 200 }
        }
    });
    assert_compatible(&old, &new);
}

// ---------------------------------------------------------------------------
// 5. Adding $defs → Compatible (Req 2.3 spirit)
// ---------------------------------------------------------------------------

#[test]
fn test_adding_defs_compatible() {
    let old = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" }
        }
    });
    let new = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" }
        },
        "$defs": {
            "Address": {
                "type": "object",
                "properties": {
                    "street": { "type": "string" }
                }
            }
        }
    });
    assert_compatible(&old, &new);
}

// ---------------------------------------------------------------------------
// 6. Removing field → Breaking(FieldRemoved) (Req 3.1)
// ---------------------------------------------------------------------------

#[test]
fn test_removing_field_breaking() {
    let old = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "age": { "type": "integer" }
        }
    });
    let new = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" }
        }
    });
    assert_breaking(&old, &new, ChangeKind::FieldRemoved);
}

// ---------------------------------------------------------------------------
// 7. Renaming field → Breaking(FieldRemoved) for old (Req 3.2)
//    Renaming is detected as a removal of the old field.
// ---------------------------------------------------------------------------

#[test]
fn test_renaming_field_breaking() {
    let old = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" }
        }
    });
    let new = json!({
        "type": "object",
        "properties": {
            "full_name": { "type": "string" }
        }
    });
    let changes = assert_breaking(&old, &new, ChangeKind::FieldRemoved);
    assert!(
        changes.iter().any(|c| c.path.contains("name")),
        "Should report the old field name in the path"
    );
}

// ---------------------------------------------------------------------------
// 8. Changing field type → Breaking(TypeChanged) (Req 3.3)
// ---------------------------------------------------------------------------

#[test]
fn test_changing_field_type_breaking() {
    let old = json!({
        "type": "object",
        "properties": {
            "age": { "type": "integer" }
        }
    });
    let new = json!({
        "type": "object",
        "properties": {
            "age": { "type": "string" }
        }
    });
    assert_breaking(&old, &new, ChangeKind::TypeChanged);
}

// ---------------------------------------------------------------------------
// 9. Adding required field → Breaking(RequiredFieldAdded) (Req 3.4)
// ---------------------------------------------------------------------------

#[test]
fn test_adding_required_field_breaking() {
    let old = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "email": { "type": "string" }
        },
        "required": ["name"]
    });
    let new = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "email": { "type": "string" }
        },
        "required": ["name", "email"]
    });
    assert_breaking(&old, &new, ChangeKind::RequiredFieldAdded);
}

#[test]
fn test_adding_new_required_field_breaking() {
    let old = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" }
        },
        "required": ["name"]
    });
    let new = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "email": { "type": "string" }
        },
        "required": ["name", "email"]
    });
    assert_breaking(&old, &new, ChangeKind::RequiredFieldAdded);
}

// ---------------------------------------------------------------------------
// 10. Removing enum value → Breaking(EnumValueRemoved) (Req 3.5)
// ---------------------------------------------------------------------------

#[test]
fn test_removing_enum_value_breaking() {
    let old = json!({
        "type": "object",
        "properties": {
            "status": {
                "type": "string",
                "enum": ["active", "inactive", "archived"]
            }
        }
    });
    let new = json!({
        "type": "object",
        "properties": {
            "status": {
                "type": "string",
                "enum": ["active", "inactive"]
            }
        }
    });
    assert_breaking(&old, &new, ChangeKind::EnumValueRemoved);
}

// ---------------------------------------------------------------------------
// 11. Tightening constraint → Breaking(ConstraintTightened) (Req 3.7)
// ---------------------------------------------------------------------------

#[test]
fn test_tightening_add_pattern_breaking() {
    let old = json!({
        "type": "object",
        "properties": {
            "code": { "type": "string" }
        }
    });
    let new = json!({
        "type": "object",
        "properties": {
            "code": { "type": "string", "pattern": "^[A-Z]{3}$" }
        }
    });
    assert_breaking(&old, &new, ChangeKind::ConstraintTightened);
}

#[test]
fn test_tightening_reduce_max_length_breaking() {
    let old = json!({
        "type": "object",
        "properties": {
            "bio": { "type": "string", "maxLength": 200 }
        }
    });
    let new = json!({
        "type": "object",
        "properties": {
            "bio": { "type": "string", "maxLength": 100 }
        }
    });
    assert_breaking(&old, &new, ChangeKind::ConstraintTightened);
}

#[test]
fn test_tightening_add_minimum_breaking() {
    let old = json!({
        "type": "object",
        "properties": {
            "score": { "type": "integer" }
        }
    });
    let new = json!({
        "type": "object",
        "properties": {
            "score": { "type": "integer", "minimum": 0 }
        }
    });
    assert_breaking(&old, &new, ChangeKind::ConstraintTightened);
}

#[test]
fn test_tightening_increase_min_length_breaking() {
    let old = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string", "minLength": 1 }
        }
    });
    let new = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string", "minLength": 5 }
        }
    });
    assert_breaking(&old, &new, ChangeKind::ConstraintTightened);
}

// ---------------------------------------------------------------------------
// 12. Default value change → Breaking(DefaultChanged) (Req 4.1)
// ---------------------------------------------------------------------------

#[test]
fn test_default_value_change_breaking() {
    let old = json!({
        "type": "object",
        "properties": {
            "role": { "type": "string", "default": "viewer" }
        }
    });
    let new = json!({
        "type": "object",
        "properties": {
            "role": { "type": "string", "default": "editor" }
        }
    });
    assert_breaking(&old, &new, ChangeKind::DefaultChanged);
}

// ---------------------------------------------------------------------------
// 13. Adding format validation → Breaking(ConstraintTightened) (Req 4.3)
// ---------------------------------------------------------------------------

#[test]
fn test_adding_format_validation_breaking() {
    let old = json!({
        "type": "object",
        "properties": {
            "email": { "type": "string" }
        }
    });
    let new = json!({
        "type": "object",
        "properties": {
            "email": { "type": "string", "format": "email" }
        }
    });
    assert_breaking(&old, &new, ChangeKind::ConstraintTightened);
}

// ---------------------------------------------------------------------------
// 14. Multiple breaking changes → all reported
// ---------------------------------------------------------------------------

#[test]
fn test_multiple_breaking_changes() {
    let old = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "age": { "type": "integer" },
            "status": {
                "type": "string",
                "enum": ["active", "inactive"]
            }
        },
        "required": ["name"]
    });
    let new = json!({
        "type": "object",
        "properties": {
            "name": { "type": "integer" },
            "status": {
                "type": "string",
                "enum": ["active"]
            }
        },
        "required": ["name", "status"]
    });

    match check_compatibility(&old, &new) {
        CompatibilityResult::Breaking(changes) => {
            let kinds: Vec<&ChangeKind> = changes.iter().map(|c| &c.kind).collect();
            assert!(
                kinds.contains(&&ChangeKind::TypeChanged),
                "Should detect name type change"
            );
            assert!(
                kinds.contains(&&ChangeKind::FieldRemoved),
                "Should detect age removal"
            );
            assert!(
                kinds.contains(&&ChangeKind::EnumValueRemoved),
                "Should detect enum value removal"
            );
            assert!(
                kinds.contains(&&ChangeKind::RequiredFieldAdded),
                "Should detect new required field"
            );
            assert!(
                changes.len() >= 4,
                "Should report at least 4 breaking changes, got {}",
                changes.len()
            );
        }
        CompatibilityResult::Compatible => {
            panic!("Expected Breaking with multiple changes, got Compatible");
        }
    }
}

// ---------------------------------------------------------------------------
// 15. Nested object recursion
// ---------------------------------------------------------------------------

#[test]
fn test_nested_object_recursion() {
    let old = json!({
        "type": "object",
        "properties": {
            "address": {
                "type": "object",
                "properties": {
                    "street": { "type": "string" },
                    "city": { "type": "string" }
                }
            }
        }
    });
    let new = json!({
        "type": "object",
        "properties": {
            "address": {
                "type": "object",
                "properties": {
                    "street": { "type": "string" }
                }
            }
        }
    });
    let changes = assert_breaking(&old, &new, ChangeKind::FieldRemoved);
    assert!(
        changes
            .iter()
            .any(|c| c.path.contains("address") && c.path.contains("city")),
        "Should report nested field path: {:#?}",
        changes
    );
}

#[test]
fn test_nested_type_change() {
    let old = json!({
        "type": "object",
        "properties": {
            "metadata": {
                "type": "object",
                "properties": {
                    "count": { "type": "integer" }
                }
            }
        }
    });
    let new = json!({
        "type": "object",
        "properties": {
            "metadata": {
                "type": "object",
                "properties": {
                    "count": { "type": "string" }
                }
            }
        }
    });
    let changes = assert_breaking(&old, &new, ChangeKind::TypeChanged);
    assert!(
        changes
            .iter()
            .any(|c| c.path.contains("metadata") && c.path.contains("count")),
        "Should report nested path: {:#?}",
        changes
    );
}

// ---------------------------------------------------------------------------
// 16. Deprecation tracking (Req 10)
// ---------------------------------------------------------------------------

#[test]
fn test_deprecation_scan() {
    let schema = json!({
        "type": "object",
        "properties": {
            "old_field": {
                "type": "string",
                "deprecated": true,
                "x-deprecated-since": "1.3.0",
                "x-deprecated-note": "Use new_field instead"
            },
            "active_field": { "type": "string" }
        }
    });

    let mut tracker = DeprecationTracker::new();
    tracker.scan_schema(&schema);

    let deps = tracker.deprecations();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].path, "/properties/old_field");
    assert_eq!(deps[0].deprecated_since, "1.3.0");
    assert_eq!(deps[0].note, "Use new_field instead");
}

#[test]
fn test_deprecation_removal_allowed() {
    let old = json!({
        "type": "object",
        "properties": {
            "old_field": {
                "type": "string",
                "deprecated": true,
                "x-deprecated-since": "1.3.0",
                "x-deprecated-note": "Use new_field instead"
            },
            "active_field": { "type": "string" }
        }
    });
    let new = json!({
        "type": "object",
        "properties": {
            "active_field": { "type": "string" }
        }
    });

    let mut tracker = DeprecationTracker::new();
    tracker.scan_schema(&old);

    let warnings = tracker.check_removal_allowed(&old, &new);
    assert!(
        warnings.is_empty(),
        "Removing a deprecated field should produce no warnings, got: {warnings:#?}"
    );
}

#[test]
fn test_deprecation_removal_not_allowed() {
    let old = json!({
        "type": "object",
        "properties": {
            "not_deprecated": { "type": "string" },
            "active_field": { "type": "string" }
        }
    });
    let new = json!({
        "type": "object",
        "properties": {
            "active_field": { "type": "string" }
        }
    });

    let tracker = DeprecationTracker::new();
    let warnings = tracker.check_removal_allowed(&old, &new);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].path.contains("not_deprecated"));
}

// ---------------------------------------------------------------------------
// Edge case: $defs changes
// ---------------------------------------------------------------------------

#[test]
fn test_removing_def_breaking() {
    let old = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" }
        },
        "$defs": {
            "Address": {
                "type": "object",
                "properties": {
                    "street": { "type": "string" }
                }
            }
        }
    });
    let new = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" }
        }
    });
    assert_breaking(&old, &new, ChangeKind::FieldRemoved);
}

#[test]
fn test_modifying_def_breaking() {
    let old = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" }
        },
        "$defs": {
            "Address": {
                "type": "object",
                "properties": {
                    "street": { "type": "string" },
                    "city": { "type": "string" }
                }
            }
        }
    });
    let new = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" }
        },
        "$defs": {
            "Address": {
                "type": "object",
                "properties": {
                    "street": { "type": "string" }
                }
            }
        }
    });
    assert_breaking(&old, &new, ChangeKind::FieldRemoved);
}

// ---------------------------------------------------------------------------
// Edge case: empty schemas
// ---------------------------------------------------------------------------

#[test]
fn test_empty_schemas_compatible() {
    let schema = json!({});
    assert_compatible(&schema, &schema);
}

#[test]
fn test_adding_properties_to_empty_compatible() {
    let old = json!({ "type": "object" });
    let new = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" }
        }
    });
    assert_compatible(&old, &new);
}
