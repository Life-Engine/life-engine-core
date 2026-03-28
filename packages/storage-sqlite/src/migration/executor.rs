//! Migration executor — applies `CollectionDescriptor` changes to the SQLite schema.
//!
//! Because all documents live in the single `plugin_data` table with a JSON
//! `data` column, "migrating" a collection means:
//!
//! 1. Ensuring the base `plugin_data` table exists (handled at init).
//! 2. Creating `json_extract`-based indexes for fields marked `indexed`.
//! 3. Creating composite indexes from the descriptor's `indexes` list.
//! 4. Detecting breaking changes (field removal, type change) and returning
//!    `SchemaConflict` rather than silently dropping data.
//!
//! All DDL uses `IF NOT EXISTS` so re-running is a no-op (idempotent).

use rusqlite::Connection;

use crate::error::StorageError;

/// A field descriptor for migration purposes.
pub struct MigrateField {
    /// Field name (JSON path segment, e.g. `"email"`).
    pub name: String,
    /// Whether this field should have a single-column index.
    pub indexed: bool,
}

/// A collection migration descriptor.
pub struct MigrateDescriptor {
    /// Collection name.
    pub name: String,
    /// Fields with their index flags.
    pub fields: Vec<MigrateField>,
    /// Composite indexes — each entry is a list of field names.
    pub indexes: Vec<Vec<String>>,
}

/// Apply a collection migration descriptor to the database.
///
/// This is idempotent: running it twice with the same descriptor is a no-op.
/// Additive changes (new fields, new indexes) are applied. Breaking changes
/// (removing a field that has an existing index) return `SchemaConflict`.
pub fn execute_migration(conn: &Connection, descriptor: &MigrateDescriptor) -> Result<(), StorageError> {
    // Gather existing indexes for this collection to detect conflicts.
    let existing_indexes = list_collection_indexes(conn, &descriptor.name)?;

    // Detect breaking changes: if an existing index references a field that
    // is no longer present in the descriptor, that is a breaking removal.
    // Include fields from both `fields` and `indexes` in the declared set.
    let mut declared_field_names: std::collections::HashSet<&str> =
        descriptor.fields.iter().map(|f| f.name.as_str()).collect();
    for composite in &descriptor.indexes {
        for field in composite {
            declared_field_names.insert(field.as_str());
        }
    }

    for idx_name in &existing_indexes {
        // Extract field names from the index's CREATE statement by parsing
        // `json_extract(data, '$.field_name')` patterns. This is more robust
        // than splitting the index name on '_', which breaks for field names
        // that contain underscores (e.g. `start_time`).
        let fields_in_index = extract_fields_from_index(conn, idx_name)?;
        for field in &fields_in_index {
            if !declared_field_names.contains(field.as_str()) {
                return Err(StorageError::InvalidConfig(format!(
                    "schema conflict: field '{}' is referenced by existing index '{}' \
                     but is not present in the new descriptor for collection '{}'",
                    field, idx_name, descriptor.name
                )));
            }
        }
    }

    // Create single-field indexes for fields marked as indexed.
    for field in &descriptor.fields {
        if field.indexed {
            let idx_name = format!("idx_col_{}_{}", descriptor.name, field.name);
            let sql = format!(
                "CREATE INDEX IF NOT EXISTS {idx_name} \
                 ON plugin_data(collection, json_extract(data, '$.{field}'))",
                idx_name = idx_name,
                field = field.name,
            );
            conn.execute_batch(&sql).map_err(StorageError::Database)?;
        }
    }

    // Create composite indexes.
    for fields in &descriptor.indexes {
        if fields.is_empty() {
            continue;
        }
        let idx_name = format!("idx_col_{}_{}", descriptor.name, fields.join("_"));
        let columns: Vec<String> = fields
            .iter()
            .map(|f| format!("json_extract(data, '$.{f}')"))
            .collect();
        let sql = format!(
            "CREATE INDEX IF NOT EXISTS {idx_name} \
             ON plugin_data(collection, {cols})",
            idx_name = idx_name,
            cols = columns.join(", "),
        );
        conn.execute_batch(&sql).map_err(StorageError::Database)?;
    }

    Ok(())
}

/// List all indexes on `plugin_data` whose name starts with `idx_col_{collection}_`.
fn list_collection_indexes(
    conn: &Connection,
    collection: &str,
) -> Result<Vec<String>, StorageError> {
    let prefix = format!("idx_col_{}_", collection);
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='plugin_data'")
        .map_err(StorageError::Database)?;

    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(StorageError::Database)?;

    let mut names = Vec::new();
    for row in rows {
        let name = row.map_err(StorageError::Database)?;
        if name.starts_with(&prefix) {
            names.push(name);
        }
    }
    Ok(names)
}

/// Extract field names from an index's SQL definition by parsing
/// `json_extract(data, '$.field_name')` patterns.
fn extract_fields_from_index(
    conn: &Connection,
    index_name: &str,
) -> Result<Vec<String>, StorageError> {
    let sql: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='index' AND name=?1",
            rusqlite::params![index_name],
            |row| row.get(0),
        )
        .map_err(StorageError::Database)?;

    let mut fields = Vec::new();
    if let Some(sql) = sql {
        // Match all occurrences of json_extract(data, '$.field_name')
        let pattern = "json_extract(data, '$.";
        let mut remaining = sql.as_str();
        while let Some(start) = remaining.find(pattern) {
            let after_prefix = &remaining[start + pattern.len()..];
            if let Some(end) = after_prefix.find("')") {
                fields.push(after_prefix[..end].to_string());
                remaining = &after_prefix[end + 2..];
            } else {
                break;
            }
        }
    }
    Ok(fields)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::PLUGIN_DATA_DDL;

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(PLUGIN_DATA_DDL).unwrap();
        conn
    }

    fn index_exists(conn: &Connection, name: &str) -> bool {
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='index' AND name=?1",
                rusqlite::params![name],
                |row| row.get(0),
            )
            .unwrap();
        count > 0
    }

    // -----------------------------------------------------------------------
    // Requirement 7.1: create collection structure from descriptor
    // -----------------------------------------------------------------------

    #[test]
    fn migrate_creates_single_field_index() {
        let conn = setup_conn();
        let descriptor = MigrateDescriptor {
            name: "events".to_string(),
            fields: vec![MigrateField {
                name: "start_time".to_string(),
                indexed: true,
            }],
            indexes: vec![],
        };

        execute_migration(&conn, &descriptor).unwrap();

        assert!(index_exists(&conn, "idx_col_events_start_time"));
    }

    #[test]
    fn migrate_creates_composite_index() {
        let conn = setup_conn();
        let descriptor = MigrateDescriptor {
            name: "contacts".to_string(),
            fields: vec![],
            indexes: vec![vec!["last_name".to_string(), "first_name".to_string()]],
        };

        execute_migration(&conn, &descriptor).unwrap();

        assert!(index_exists(&conn, "idx_col_contacts_last_name_first_name"));
    }

    #[test]
    fn migrate_skips_non_indexed_fields() {
        let conn = setup_conn();
        let descriptor = MigrateDescriptor {
            name: "notes".to_string(),
            fields: vec![
                MigrateField {
                    name: "title".to_string(),
                    indexed: true,
                },
                MigrateField {
                    name: "body".to_string(),
                    indexed: false,
                },
            ],
            indexes: vec![],
        };

        execute_migration(&conn, &descriptor).unwrap();

        assert!(index_exists(&conn, "idx_col_notes_title"));
        assert!(!index_exists(&conn, "idx_col_notes_body"));
    }

    // -----------------------------------------------------------------------
    // Requirement 7.2: idempotent — running twice is a no-op
    // -----------------------------------------------------------------------

    #[test]
    fn migrate_is_idempotent() {
        let conn = setup_conn();
        let descriptor = MigrateDescriptor {
            name: "events".to_string(),
            fields: vec![MigrateField {
                name: "start_time".to_string(),
                indexed: true,
            }],
            indexes: vec![vec!["source".to_string(), "source_id".to_string()]],
        };

        // Run twice — second run should succeed without error.
        execute_migration(&conn, &descriptor).unwrap();
        execute_migration(&conn, &descriptor).unwrap();

        assert!(index_exists(&conn, "idx_col_events_start_time"));
        assert!(index_exists(&conn, "idx_col_events_source_source_id"));
    }

    // -----------------------------------------------------------------------
    // Requirement 7.3: additive changes applied without disrupting data
    // -----------------------------------------------------------------------

    #[test]
    fn migrate_additive_changes_preserve_existing_indexes() {
        let conn = setup_conn();

        // First migration: one indexed field.
        let desc_v1 = MigrateDescriptor {
            name: "events".to_string(),
            fields: vec![MigrateField {
                name: "start_time".to_string(),
                indexed: true,
            }],
            indexes: vec![],
        };
        execute_migration(&conn, &desc_v1).unwrap();

        // Insert data to verify it's preserved.
        conn.execute(
            "INSERT INTO plugin_data (id, plugin_id, collection, data, version, created_at, updated_at) \
             VALUES ('e1', 'core', 'events', '{\"start_time\":\"2026-01-01\",\"location\":\"home\"}', 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        ).unwrap();

        // Second migration: add new indexed field.
        let desc_v2 = MigrateDescriptor {
            name: "events".to_string(),
            fields: vec![
                MigrateField {
                    name: "start_time".to_string(),
                    indexed: true,
                },
                MigrateField {
                    name: "location".to_string(),
                    indexed: true,
                },
            ],
            indexes: vec![],
        };
        execute_migration(&conn, &desc_v2).unwrap();

        // Both indexes should exist.
        assert!(index_exists(&conn, "idx_col_events_start_time"));
        assert!(index_exists(&conn, "idx_col_events_location"));

        // Data should still be intact.
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM plugin_data WHERE collection = 'events'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    // -----------------------------------------------------------------------
    // Requirement 7.4: breaking changes return SchemaConflict
    // -----------------------------------------------------------------------

    #[test]
    fn migrate_detects_field_removal_with_existing_index() {
        let conn = setup_conn();

        // First: create an index on 'email'.
        let desc_v1 = MigrateDescriptor {
            name: "contacts".to_string(),
            fields: vec![MigrateField {
                name: "email".to_string(),
                indexed: true,
            }],
            indexes: vec![],
        };
        execute_migration(&conn, &desc_v1).unwrap();
        assert!(index_exists(&conn, "idx_col_contacts_email"));

        // Second: descriptor no longer declares 'email'.
        let desc_v2 = MigrateDescriptor {
            name: "contacts".to_string(),
            fields: vec![MigrateField {
                name: "phone".to_string(),
                indexed: true,
            }],
            indexes: vec![],
        };

        let result = execute_migration(&conn, &desc_v2);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("schema conflict"),
            "expected schema conflict, got: {err}"
        );
        assert!(err.contains("email"));
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn migrate_empty_descriptor_is_no_op() {
        let conn = setup_conn();
        let descriptor = MigrateDescriptor {
            name: "empty_collection".to_string(),
            fields: vec![],
            indexes: vec![],
        };

        execute_migration(&conn, &descriptor).unwrap();
        // No indexes created beyond the base ones.
        let col_indexes = list_collection_indexes(&conn, "empty_collection").unwrap();
        assert!(col_indexes.is_empty());
    }

    #[test]
    fn migrate_skips_empty_composite_index() {
        let conn = setup_conn();
        let descriptor = MigrateDescriptor {
            name: "events".to_string(),
            fields: vec![],
            indexes: vec![vec![]], // empty field list
        };

        execute_migration(&conn, &descriptor).unwrap();
        let col_indexes = list_collection_indexes(&conn, "events").unwrap();
        assert!(col_indexes.is_empty());
    }

    #[test]
    fn migrate_multiple_composite_indexes() {
        let conn = setup_conn();
        let descriptor = MigrateDescriptor {
            name: "tasks".to_string(),
            fields: vec![],
            indexes: vec![
                vec!["status".to_string(), "priority".to_string()],
                vec!["assignee".to_string(), "due_date".to_string()],
            ],
        };

        execute_migration(&conn, &descriptor).unwrap();

        assert!(index_exists(&conn, "idx_col_tasks_status_priority"));
        assert!(index_exists(&conn, "idx_col_tasks_assignee_due_date"));
    }
}
