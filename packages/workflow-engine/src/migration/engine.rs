//! Migration execution engine.
//!
//! Orchestrates the full migration pipeline: backup, per-entry record
//! transformation, quarantine of failures, and logging of outcomes.

use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::Utc;
use rusqlite::Connection;
use tracing::{debug, info, warn};
use uuid::Uuid;

use life_engine_storage_sqlite::migration::backup::create_backup;
use life_engine_storage_sqlite::migration::log::{log_migration, MigrationLogEntry};
use life_engine_storage_sqlite::migration::quarantine::quarantine_record;

use super::runner::run_transform;
use super::MigrationEntry;
use super::MigrationError;

/// Result of a complete migration run across one or more migration entries.
#[derive(Debug, Clone)]
pub struct MigrationResult {
    /// Total records successfully migrated across all entries.
    pub migrated: u64,
    /// Total records sent to quarantine across all entries.
    pub quarantined: u64,
    /// Total elapsed time in milliseconds.
    pub duration_ms: u64,
    /// Path to the pre-migration backup.
    pub backup_path: PathBuf,
    /// Human-readable identifiers of the entries that were applied.
    pub entries_applied: Vec<String>,
}

/// A record fetched from `plugin_data` for migration.
struct PluginRecord {
    id: String,
    data: String,
}

/// Parse the major version number from a migration `from` string.
///
/// Accepts formats like `"1.0.0"`, `"1.x"`, `"1"`, or `"2.0.x"` and returns
/// the leading integer segment as the version to match in `plugin_data.version`.
fn parse_from_version(from: &str) -> Result<i64, MigrationError> {
    let major_str = from.split('.').next().unwrap_or(from);
    major_str.parse::<i64>().map_err(|_| {
        MigrationError::ManifestValidation(format!(
            "cannot parse major version from '{from}'"
        ))
    })
}

/// Run all migration entries for a plugin against the database.
///
/// Entries are applied in ascending version order. For each entry, all records
/// in the target collection whose `version` matches the entry's `from` major
/// version are transformed via the WASM sandbox. Successfully transformed
/// records are updated in-place; failures are quarantined.
///
/// # Arguments
///
/// - `wasm_path` — Path to the plugin's WASM binary containing transform exports.
/// - `entries` — Migration entries to apply, in any order (sorted internally).
/// - `plugin_id` — The plugin whose data is being migrated.
/// - `db_path` — Path to the SQLite database file.
/// - `data_dir` — Data directory for storing backups.
pub async fn run_migrations(
    wasm_path: &Path,
    entries: &[MigrationEntry],
    plugin_id: &str,
    db_path: &Path,
    data_dir: &Path,
) -> Result<MigrationResult, MigrationError> {
    let start = Instant::now();

    // Step 1: Create a pre-migration backup.
    let backup_path = create_backup(db_path, data_dir).map_err(|e| {
        MigrationError::TransformFailed {
            function: "create_backup".to_string(),
            cause: format!("backup failed: {e}"),
        }
    })?;

    info!(backup = %backup_path.display(), "pre-migration backup created");

    // Sort entries by target version ascending.
    let mut sorted_entries: Vec<&MigrationEntry> = entries.iter().collect();
    sorted_entries.sort_by(|a, b| a.to.cmp(&b.to));

    let mut total_migrated: u64 = 0;
    let mut total_quarantined: u64 = 0;
    let mut entries_applied: Vec<String> = Vec::new();

    let conn = Connection::open(db_path).map_err(|e| MigrationError::TransformFailed {
        function: "open_db".to_string(),
        cause: format!("failed to open database: {e}"),
    })?;

    for entry in &sorted_entries {
        let from_version = parse_from_version(&entry.from)?;
        let to_version = entry.to.major as i64;
        let entry_label = format!("{} -> {}", entry.from, entry.to);

        // Query all records in the target collection matching the from version.
        let records = query_records(&conn, plugin_id, &entry.collection, from_version)?;

        if records.is_empty() {
            debug!(
                entry = %entry_label,
                collection = %entry.collection,
                "no matching records, skipping"
            );
            continue;
        }

        info!(
            entry = %entry_label,
            collection = %entry.collection,
            count = records.len(),
            "migrating records"
        );

        // Begin a transaction for this entry (all-or-nothing per entry).
        conn.execute("BEGIN IMMEDIATE", []).map_err(|e| {
            MigrationError::TransformFailed {
                function: "begin_transaction".to_string(),
                cause: format!("failed to begin transaction: {e}"),
            }
        })?;

        let mut entry_migrated: u64 = 0;
        let mut entry_quarantined: u64 = 0;

        for record in &records {
            let input: serde_json::Value =
                serde_json::from_str(&record.data).map_err(|e| {
                    MigrationError::TransformFailed {
                        function: entry.transform.clone(),
                        cause: format!(
                            "failed to parse record {} data as JSON: {e}",
                            record.id
                        ),
                    }
                })?;

            match run_transform(wasm_path, &entry.transform, input).await {
                Ok(output) => {
                    let output_str = serde_json::to_string(&output).map_err(|e| {
                        MigrationError::TransformFailed {
                            function: entry.transform.clone(),
                            cause: format!("failed to serialize transform output: {e}"),
                        }
                    })?;

                    update_record(&conn, &record.id, &output_str, to_version)?;
                    entry_migrated += 1;
                }
                Err(transform_err) => {
                    warn!(
                        record_id = %record.id,
                        error = %transform_err,
                        "transform failed, quarantining record"
                    );

                    let _ = quarantine_record(
                        &conn,
                        &record.data,
                        plugin_id,
                        &entry.collection,
                        &entry.from,
                        &entry.to.to_string(),
                        &transform_err.to_string(),
                    );

                    entry_quarantined += 1;
                }
            }
        }

        // Commit the transaction.
        conn.execute("COMMIT", []).map_err(|e| {
            MigrationError::TransformFailed {
                function: "commit_transaction".to_string(),
                cause: format!("failed to commit transaction: {e}"),
            }
        })?;

        total_migrated += entry_migrated;
        total_quarantined += entry_quarantined;
        entries_applied.push(entry_label);

        info!(
            migrated = entry_migrated,
            quarantined = entry_quarantined,
            "migration entry complete"
        );
    }

    let duration_ms = start.elapsed().as_millis() as u64;

    // Step 3: Log the overall migration result.
    for entry in &sorted_entries {
        let entry_log = MigrationLogEntry {
            id: Uuid::new_v4().to_string(),
            plugin_id: plugin_id.to_string(),
            collection: entry.collection.clone(),
            from_version: entry.from.clone(),
            to_version: entry.to.to_string(),
            records_migrated: total_migrated as i64,
            records_quarantined: total_quarantined as i64,
            duration_ms: duration_ms as i64,
            backup_path: Some(backup_path.to_string_lossy().to_string()),
            error: None,
            timestamp: Utc::now().to_rfc3339(),
        };

        let _ = log_migration(&conn, &entry_log).map_err(|e| {
            warn!(error = %e, "failed to write migration log entry");
        });
    }

    info!(
        migrated = total_migrated,
        quarantined = total_quarantined,
        duration_ms = duration_ms,
        entries = entries_applied.len(),
        "migration run complete"
    );

    Ok(MigrationResult {
        migrated: total_migrated,
        quarantined: total_quarantined,
        duration_ms,
        backup_path,
        entries_applied,
    })
}

/// Query all records from `plugin_data` matching the given plugin, collection,
/// and schema version.
fn query_records(
    conn: &Connection,
    plugin_id: &str,
    collection: &str,
    version: i64,
) -> Result<Vec<PluginRecord>, MigrationError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, data, version FROM plugin_data \
             WHERE plugin_id = ?1 AND collection = ?2 AND version = ?3",
        )
        .map_err(|e| MigrationError::TransformFailed {
            function: "query_records".to_string(),
            cause: format!("failed to prepare query: {e}"),
        })?;

    let rows = stmt
        .query_map(rusqlite::params![plugin_id, collection, version], |row| {
            Ok(PluginRecord {
                id: row.get(0)?,
                data: row.get(1)?,
            })
        })
        .map_err(|e| MigrationError::TransformFailed {
            function: "query_records".to_string(),
            cause: format!("failed to execute query: {e}"),
        })?;

    let mut records = Vec::new();
    for row in rows {
        records.push(row.map_err(|e| MigrationError::TransformFailed {
            function: "query_records".to_string(),
            cause: format!("failed to read row: {e}"),
        })?);
    }
    Ok(records)
}

/// Update a record's data and version in `plugin_data`.
fn update_record(
    conn: &Connection,
    record_id: &str,
    new_data: &str,
    new_version: i64,
) -> Result<(), MigrationError> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE plugin_data SET data = ?1, version = ?2, updated_at = ?3 WHERE id = ?4",
        rusqlite::params![new_data, new_version, now, record_id],
    )
    .map_err(|e| MigrationError::TransformFailed {
        function: "update_record".to_string(),
        cause: format!("failed to update record {record_id}: {e}"),
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use life_engine_storage_sqlite::schema::{MIGRATION_LOG_DDL, PLUGIN_DATA_DDL, QUARANTINE_DDL};
    use semver::Version;
    use tempfile::TempDir;

    fn setup_db(dir: &Path) -> PathBuf {
        let db_path = dir.join("test.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(PLUGIN_DATA_DDL).unwrap();
        conn.execute_batch(QUARANTINE_DDL).unwrap();
        conn.execute_batch(MIGRATION_LOG_DDL).unwrap();
        db_path
    }

    fn insert_record(db_path: &Path, id: &str, plugin_id: &str, collection: &str, data: &str, version: i64) {
        let conn = Connection::open(db_path).unwrap();
        conn.execute(
            "INSERT INTO plugin_data (id, plugin_id, collection, data, version, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![id, plugin_id, collection, data, version, "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z"],
        ).unwrap();
    }

    fn identity_wasm() -> Vec<u8> {
        wat::parse_str(
            r#"
            (module
                (import "extism:host/env" "input_length" (func $input_length (result i64)))
                (import "extism:host/env" "input_load_u8" (func $input_load_u8 (param i64) (result i32)))
                (import "extism:host/env" "output_set" (func $output_set (param i64 i64)))
                (import "extism:host/env" "alloc" (func $alloc (param i64) (result i64)))
                (import "extism:host/env" "store_u8" (func $store_u8 (param i64 i32)))

                (memory (export "memory") 1)

                (func (export "migrate_v1_to_v2") (result i32)
                    (local $len i64)
                    (local $offset i64)
                    (local $i i64)
                    (local $byte i32)

                    (local.set $len (call $input_length))
                    (local.set $offset (call $alloc (local.get $len)))

                    (local.set $i (i64.const 0))
                    (block $break
                        (loop $loop
                            (br_if $break (i64.ge_u (local.get $i) (local.get $len)))
                            (local.set $byte (call $input_load_u8 (local.get $i)))
                            (call $store_u8
                                (i64.add (local.get $offset) (local.get $i))
                                (local.get $byte)
                            )
                            (local.set $i (i64.add (local.get $i) (i64.const 1)))
                            (br $loop)
                        )
                    )

                    (call $output_set (local.get $offset) (local.get $len))
                    (i32.const 0)
                )
            )
            "#,
        )
        .expect("failed to compile WAT to WASM")
    }

    #[test]
    fn parse_from_version_extracts_major() {
        assert_eq!(parse_from_version("1.0.0").unwrap(), 1);
        assert_eq!(parse_from_version("2.x").unwrap(), 2);
        assert_eq!(parse_from_version("3").unwrap(), 3);
        assert_eq!(parse_from_version("10.0.x").unwrap(), 10);
    }

    #[test]
    fn parse_from_version_rejects_invalid() {
        assert!(parse_from_version("x.1.0").is_err());
        assert!(parse_from_version("abc").is_err());
    }

    #[tokio::test]
    async fn run_migrations_with_no_matching_records() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path().join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let db_path = setup_db(tmp.path());

        let wasm_path = tmp.path().join("plugin.wasm");
        std::fs::write(&wasm_path, identity_wasm()).unwrap();

        let entries = vec![MigrationEntry {
            from: "1.0.0".to_string(),
            to: Version::new(2, 0, 0),
            transform: "migrate_v1_to_v2".to_string(),
            description: "test migration".to_string(),
            collection: "events".to_string(),
        }];

        let result = run_migrations(&wasm_path, &entries, "test-plugin", &db_path, &data_dir)
            .await
            .unwrap();

        assert_eq!(result.migrated, 0);
        assert_eq!(result.quarantined, 0);
        assert!(result.entries_applied.is_empty());
        assert!(result.backup_path.exists());
    }

    #[tokio::test]
    async fn run_migrations_transforms_matching_records() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path().join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let db_path = setup_db(tmp.path());

        // Insert records at version 1.
        insert_record(&db_path, "r1", "test-plugin", "events", r#"{"name":"Alice"}"#, 1);
        insert_record(&db_path, "r2", "test-plugin", "events", r#"{"name":"Bob"}"#, 1);
        // A record at version 2 should NOT be picked up.
        insert_record(&db_path, "r3", "test-plugin", "events", r#"{"name":"Charlie"}"#, 2);

        let wasm_path = tmp.path().join("plugin.wasm");
        std::fs::write(&wasm_path, identity_wasm()).unwrap();

        let entries = vec![MigrationEntry {
            from: "1.0.0".to_string(),
            to: Version::new(2, 0, 0),
            transform: "migrate_v1_to_v2".to_string(),
            description: "test migration".to_string(),
            collection: "events".to_string(),
        }];

        let result = run_migrations(&wasm_path, &entries, "test-plugin", &db_path, &data_dir)
            .await
            .unwrap();

        assert_eq!(result.migrated, 2);
        assert_eq!(result.quarantined, 0);
        assert_eq!(result.entries_applied.len(), 1);

        // Verify records were updated to version 2.
        let conn = Connection::open(&db_path).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM plugin_data WHERE plugin_id = 'test-plugin' AND collection = 'events' AND version = 2",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 3); // r1, r2 migrated to v2; r3 was already v2
    }

    #[tokio::test]
    async fn run_migrations_quarantines_failed_transforms() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path().join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let db_path = setup_db(tmp.path());

        insert_record(&db_path, "r1", "test-plugin", "events", r#"{"name":"Alice"}"#, 1);

        // Use a WASM that traps to force a transform failure.
        let bad_wasm = wat::parse_str(
            r#"
            (module
                (memory (export "memory") 1)
                (func (export "bad_migrate") (result i32)
                    unreachable
                )
            )
            "#,
        )
        .unwrap();

        let wasm_path = tmp.path().join("plugin.wasm");
        std::fs::write(&wasm_path, &bad_wasm).unwrap();

        let entries = vec![MigrationEntry {
            from: "1.0.0".to_string(),
            to: Version::new(2, 0, 0),
            transform: "bad_migrate".to_string(),
            description: "bad migration".to_string(),
            collection: "events".to_string(),
        }];

        let result = run_migrations(&wasm_path, &entries, "test-plugin", &db_path, &data_dir)
            .await
            .unwrap();

        assert_eq!(result.migrated, 0);
        assert_eq!(result.quarantined, 1);

        // Verify record is in quarantine.
        let conn = Connection::open(&db_path).unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM quarantine", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn run_migrations_logs_to_migration_log() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path().join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let db_path = setup_db(tmp.path());

        insert_record(&db_path, "r1", "test-plugin", "events", r#"{"name":"Alice"}"#, 1);

        let wasm_path = tmp.path().join("plugin.wasm");
        std::fs::write(&wasm_path, identity_wasm()).unwrap();

        let entries = vec![MigrationEntry {
            from: "1.0.0".to_string(),
            to: Version::new(2, 0, 0),
            transform: "migrate_v1_to_v2".to_string(),
            description: "test migration".to_string(),
            collection: "events".to_string(),
        }];

        run_migrations(&wasm_path, &entries, "test-plugin", &db_path, &data_dir)
            .await
            .unwrap();

        let conn = Connection::open(&db_path).unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM migration_log", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}
