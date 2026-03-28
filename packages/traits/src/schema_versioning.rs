//! Schema compatibility checker for versioning rules.
//!
//! Compares two JSON Schema versions and classifies changes as breaking or
//! non-breaking, enforcing the additive-only rule within major SDK versions.
//! See `.odm/spec/schema-versioning-rules/requirements.md` for full spec.

use serde_json::Value;

/// Result of comparing two schema versions.
#[derive(Debug, Clone, PartialEq)]
pub enum CompatibilityResult {
    /// All changes are non-breaking (additive-only).
    Compatible,
    /// One or more breaking changes detected.
    Breaking(Vec<BreakingChange>),
}

/// A single breaking change found during schema comparison.
#[derive(Debug, Clone, PartialEq)]
pub struct BreakingChange {
    /// JSON pointer to the changed location (e.g. `/properties/name`).
    pub path: String,
    /// Classification of the breaking change.
    pub kind: ChangeKind,
    /// Human-readable description of what changed.
    pub description: String,
}

/// Classification of a breaking schema change.
#[derive(Debug, Clone, PartialEq)]
pub enum ChangeKind {
    /// A field was removed from the schema (Req 3.1).
    FieldRemoved,
    /// A field was renamed — detected as remove + add (Req 3.2).
    FieldRenamed,
    /// A field's type was changed (Req 3.3).
    TypeChanged,
    /// A new required field was added (Req 3.4).
    RequiredFieldAdded,
    /// An enum value was removed (Req 3.5).
    EnumValueRemoved,
    /// A constraint was tightened: pattern added, maxLength reduced, etc. (Req 3.7).
    ConstraintTightened,
    /// A default value was changed (Req 4.1).
    DefaultChanged,
}

/// A deprecation entry extracted from a schema.
#[derive(Debug, Clone, PartialEq)]
pub struct DeprecationEntry {
    /// JSON pointer to the deprecated field or enum value.
    pub path: String,
    /// The version in which the deprecation was introduced.
    pub deprecated_since: String,
    /// Human-readable deprecation note.
    pub note: String,
}

/// A warning about a removed field that was not properly deprecated first.
#[derive(Debug, Clone, PartialEq)]
pub struct DeprecationWarning {
    /// JSON pointer to the removed field.
    pub path: String,
    /// Human-readable warning message.
    pub message: String,
}

/// Tracks deprecation annotations across schema versions (Req 10).
#[derive(Debug, Clone, Default)]
pub struct DeprecationTracker {
    deprecations: Vec<DeprecationEntry>,
}

impl DeprecationTracker {
    /// Create an empty tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Scan a schema for deprecation annotations (`deprecated: true` or
    /// `x-deprecated` extension keyword) and record them.
    pub fn scan_schema(&mut self, schema: &Value) {
        self.scan_properties(schema, String::new());
    }

    /// Return the current deprecation entries.
    pub fn deprecations(&self) -> &[DeprecationEntry] {
        &self.deprecations
    }

    /// Check whether every field removed between `old_schema` and `new_schema`
    /// was marked as deprecated in `old_schema`. Returns warnings for any
    /// removal that skipped the deprecation step.
    pub fn check_removal_allowed(
        &self,
        old_schema: &Value,
        new_schema: &Value,
    ) -> Vec<DeprecationWarning> {
        let mut warnings = Vec::new();
        self.check_properties_removal(old_schema, new_schema, String::new(), &mut warnings);
        warnings
    }

    fn scan_properties(&mut self, schema: &Value, prefix: String) {
        let props = match schema.get("properties").and_then(Value::as_object) {
            Some(p) => p,
            None => return,
        };

        for (name, field_schema) in props {
            let path = format!("{prefix}/properties/{name}");

            if field_schema.get("deprecated") == Some(&Value::Bool(true)) {
                let since = field_schema
                    .get("x-deprecated-since")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
                let note = field_schema
                    .get("x-deprecated-note")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                self.deprecations.push(DeprecationEntry {
                    path: path.clone(),
                    deprecated_since: since,
                    note,
                });
            }

            // Recurse into nested objects.
            if field_schema.get("type") == Some(&Value::String("object".to_string())) {
                self.scan_properties(field_schema, path);
            }
        }
    }

    fn check_properties_removal(
        &self,
        old_schema: &Value,
        new_schema: &Value,
        prefix: String,
        warnings: &mut Vec<DeprecationWarning>,
    ) {
        let old_props = match old_schema.get("properties").and_then(Value::as_object) {
            Some(p) => p,
            None => return,
        };
        let new_props = new_schema
            .get("properties")
            .and_then(Value::as_object);

        for (name, old_field) in old_props {
            let path = format!("{prefix}/properties/{name}");
            let still_exists = new_props
                .map(|p| p.contains_key(name))
                .unwrap_or(false);

            if !still_exists {
                let was_deprecated = self.deprecations.iter().any(|d| d.path == path);
                if !was_deprecated {
                    warnings.push(DeprecationWarning {
                        path: path.clone(),
                        message: format!(
                            "Field '{name}' was removed without being deprecated first"
                        ),
                    });
                }
            } else if let Some(new_field) = new_props.and_then(|p| p.get(name)) {
                // Recurse into nested objects.
                if old_field.get("type") == Some(&Value::String("object".to_string())) {
                    self.check_properties_removal(old_field, new_field, path, warnings);
                }
            }
        }
    }
}

/// Compare two JSON Schema versions and classify changes as breaking or
/// non-breaking.
///
/// Non-breaking changes (Req 2): adding optional fields, adding enum values,
/// relaxing constraints (reducing minLength, removing pattern), adding $defs.
///
/// Breaking changes (Req 3): removing fields, renaming fields (detected as
/// remove+add), changing field types, adding required fields, removing enum
/// values, tightening constraints (adding pattern, reducing maxLength, adding
/// minimum).
///
/// Edge cases (Req 4): default value changes are breaking, adding format
/// validation is breaking, ambiguous changes default to breaking.
pub fn check_compatibility(old_schema: &Value, new_schema: &Value) -> CompatibilityResult {
    let mut changes = Vec::new();
    compare_schemas(old_schema, new_schema, String::new(), &mut changes);
    if changes.is_empty() {
        CompatibilityResult::Compatible
    } else {
        CompatibilityResult::Breaking(changes)
    }
}

fn compare_schemas(
    old_schema: &Value,
    new_schema: &Value,
    prefix: String,
    changes: &mut Vec<BreakingChange>,
) {
    compare_properties(old_schema, new_schema, &prefix, changes);
    compare_required(old_schema, new_schema, &prefix, changes);
    compare_enums(old_schema, new_schema, &prefix, changes);
    compare_defs(old_schema, new_schema, changes);
}

fn compare_properties(
    old_schema: &Value,
    new_schema: &Value,
    prefix: &str,
    changes: &mut Vec<BreakingChange>,
) {
    let old_props = match old_schema.get("properties").and_then(Value::as_object) {
        Some(p) => p,
        None => return,
    };
    let new_props = new_schema
        .get("properties")
        .and_then(Value::as_object);

    for (name, old_field) in old_props {
        let path = format!("{prefix}/properties/{name}");

        let new_field = match new_props.and_then(|p| p.get(name)) {
            Some(f) => f,
            None => {
                changes.push(BreakingChange {
                    path,
                    kind: ChangeKind::FieldRemoved,
                    description: format!("Field '{name}' was removed"),
                });
                continue;
            }
        };

        // Check type changes.
        let old_type = old_field.get("type");
        let new_type = new_field.get("type");
        if old_type != new_type {
            changes.push(BreakingChange {
                path: path.clone(),
                kind: ChangeKind::TypeChanged,
                description: format!(
                    "Field '{name}' type changed from {} to {}",
                    type_display(old_type),
                    type_display(new_type)
                ),
            });
        }

        // Check constraint tightening.
        compare_constraints(old_field, new_field, &path, name, changes);

        // Check default value changes.
        let old_default = old_field.get("default");
        let new_default = new_field.get("default");
        if old_default != new_default && old_default.is_some() {
            changes.push(BreakingChange {
                path: path.clone(),
                kind: ChangeKind::DefaultChanged,
                description: format!("Field '{name}' default value changed"),
            });
        }

        // Check enum value removal on field-level enums.
        compare_enums(old_field, new_field, &path, changes);

        // Recurse into nested objects.
        if old_field.get("type") == Some(&Value::String("object".to_string())) {
            compare_schemas(old_field, new_field, path, changes);
        }
    }
}

fn compare_constraints(
    old_field: &Value,
    new_field: &Value,
    path: &str,
    name: &str,
    changes: &mut Vec<BreakingChange>,
) {
    // maxLength: tightening = new < old (or added where none existed).
    check_numeric_tightened(
        old_field, new_field, "maxLength", path, name, changes, true,
    );

    // minLength: tightening = new > old (or added where none existed).
    check_numeric_tightened(
        old_field, new_field, "minLength", path, name, changes, false,
    );

    // maximum: tightening = new < old (or added where none existed).
    check_numeric_tightened(
        old_field, new_field, "maximum", path, name, changes, true,
    );

    // minimum: tightening = new > old (or added where none existed).
    check_numeric_tightened(
        old_field, new_field, "minimum", path, name, changes, false,
    );

    // pattern: adding a pattern where none existed, or changing it, is tightening.
    let old_pattern = old_field.get("pattern");
    let new_pattern = new_field.get("pattern");
    if new_pattern.is_some() && old_pattern != new_pattern {
        if old_pattern.is_none() {
            changes.push(BreakingChange {
                path: path.to_string(),
                kind: ChangeKind::ConstraintTightened,
                description: format!("Field '{name}' had pattern constraint added"),
            });
        } else {
            changes.push(BreakingChange {
                path: path.to_string(),
                kind: ChangeKind::ConstraintTightened,
                description: format!("Field '{name}' pattern constraint changed"),
            });
        }
    }

    // format: adding format to an existing string field is breaking (Req 4.3).
    let old_format = old_field.get("format");
    let new_format = new_field.get("format");
    if new_format.is_some() && old_format.is_none() {
        changes.push(BreakingChange {
            path: path.to_string(),
            kind: ChangeKind::ConstraintTightened,
            description: format!(
                "Field '{name}' had format validation '{}' added",
                new_format.and_then(Value::as_str).unwrap_or("unknown")
            ),
        });
    }
}

fn check_numeric_tightened(
    old_field: &Value,
    new_field: &Value,
    keyword: &str,
    path: &str,
    name: &str,
    changes: &mut Vec<BreakingChange>,
    lower_is_tighter: bool,
) {
    let old_val = old_field.get(keyword).and_then(Value::as_f64);
    let new_val = new_field.get(keyword).and_then(Value::as_f64);

    match (old_val, new_val) {
        (Some(old), Some(new)) => {
            let tightened = if lower_is_tighter {
                new < old
            } else {
                new > old
            };
            if tightened {
                changes.push(BreakingChange {
                    path: path.to_string(),
                    kind: ChangeKind::ConstraintTightened,
                    description: format!(
                        "Field '{name}' {keyword} tightened from {old} to {new}"
                    ),
                });
            }
        }
        (None, Some(new)) => {
            changes.push(BreakingChange {
                path: path.to_string(),
                kind: ChangeKind::ConstraintTightened,
                description: format!(
                    "Field '{name}' had {keyword} constraint {new} added"
                ),
            });
        }
        _ => {}
    }
}

fn compare_required(
    old_schema: &Value,
    new_schema: &Value,
    prefix: &str,
    changes: &mut Vec<BreakingChange>,
) {
    let old_required: Vec<&str> = old_schema
        .get("required")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    let new_required: Vec<&str> = new_schema
        .get("required")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    for field in &new_required {
        if !old_required.contains(field) {
            changes.push(BreakingChange {
                path: format!("{prefix}/required/{field}"),
                kind: ChangeKind::RequiredFieldAdded,
                description: format!("Field '{field}' was made required"),
            });
        }
    }
}

fn compare_enums(
    old_schema: &Value,
    new_schema: &Value,
    prefix: &str,
    changes: &mut Vec<BreakingChange>,
) {
    let old_enum = match old_schema.get("enum").and_then(Value::as_array) {
        Some(e) => e,
        None => return,
    };
    let new_enum = match new_schema.get("enum").and_then(Value::as_array) {
        Some(e) => e,
        None => {
            changes.push(BreakingChange {
                path: format!("{prefix}/enum"),
                kind: ChangeKind::EnumValueRemoved,
                description: "Enum constraint was removed entirely".to_string(),
            });
            return;
        }
    };

    for old_val in old_enum {
        if !new_enum.contains(old_val) {
            let display = old_val
                .as_str()
                .map(|s| format!("'{s}'"))
                .unwrap_or_else(|| old_val.to_string());
            changes.push(BreakingChange {
                path: format!("{prefix}/enum"),
                kind: ChangeKind::EnumValueRemoved,
                description: format!("Enum value {display} was removed"),
            });
        }
    }
}

fn compare_defs(
    old_schema: &Value,
    new_schema: &Value,
    changes: &mut Vec<BreakingChange>,
) {
    let old_defs = match old_schema.get("$defs").and_then(Value::as_object) {
        Some(d) => d,
        None => return,
    };
    let new_defs = match new_schema.get("$defs").and_then(Value::as_object) {
        Some(d) => d,
        None => {
            // All $defs were removed — each is a breaking change.
            for name in old_defs.keys() {
                changes.push(BreakingChange {
                    path: format!("/$defs/{name}"),
                    kind: ChangeKind::FieldRemoved,
                    description: format!("Definition '{name}' was removed"),
                });
            }
            return;
        }
    };

    for (name, old_def) in old_defs {
        match new_defs.get(name) {
            Some(new_def) => {
                compare_schemas(old_def, new_def, format!("/$defs/{name}"), changes);
            }
            None => {
                changes.push(BreakingChange {
                    path: format!("/$defs/{name}"),
                    kind: ChangeKind::FieldRemoved,
                    description: format!("Definition '{name}' was removed"),
                });
            }
        }
    }
}

fn type_display(val: Option<&Value>) -> String {
    match val {
        Some(Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
        None => "unspecified".to_string(),
    }
}
