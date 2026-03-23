//! Migration log operations for recording migration run outcomes.
//!
//! Every migration run — successful or failed — is logged to the
//! `migration_log` table for auditability. Per-record failures go to
//! quarantine; this table records the overall run outcome.

use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::StorageError;

/// A single migration log entry recording the outcome of a migration run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationLogEntry {
    /// Unique log entry ID.
    pub id: String,
    /// The plugin whose data was migrated.
    pub plugin_id: String,
    /// The collection that was migrated.
    pub collection: String,
    /// The version records were migrated from.
    pub from_version: String,
    /// The version records were migrated to.
    pub to_version: String,
    /// Count of successfully transformed records.
    pub records_migrated: i64,
    /// Count of records sent to quarantine.
    pub records_quarantined: i64,
    /// Total migration time in milliseconds.
    pub duration_ms: i64,
    /// Path to the pre-migration backup file, if one was created.
    pub backup_path: Option<String>,
    /// Error message if the migration failed entirely (not per-record).
    pub error: Option<String>,
    /// When the migration was logged (ISO 8601).
    pub timestamp: String,
}

/// Record a successful (or partially successful) migration run.
pub fn log_migration(db: &Connection, entry: &MigrationLogEntry) -> Result<(), StorageError> {
    db.execute(
        "INSERT INTO migration_log \
         (id, plugin_id, collection, from_version, to_version, \
          records_migrated, records_quarantined, duration_ms, backup_path, error, timestamp) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            entry.id,
            entry.plugin_id,
            entry.collection,
            entry.from_version,
            entry.to_version,
            entry.records_migrated,
            entry.records_quarantined,
            entry.duration_ms,
            entry.backup_path,
            entry.error,
            entry.timestamp,
        ],
    )?;

    Ok(())
}

/// Record a migration failure that prevented execution entirely.
///
/// This differs from per-record quarantine failures — it covers cases where
/// the migration could not even start (e.g., WASM module failed to load,
/// manifest parse error, etc.).
pub fn log_failure(
    db: &Connection,
    plugin_id: &str,
    collection: &str,
    from_version: &str,
    to_version: &str,
    error: &str,
) -> Result<(), StorageError> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    db.execute(
        "INSERT INTO migration_log \
         (id, plugin_id, collection, from_version, to_version, \
          records_migrated, records_quarantined, duration_ms, backup_path, error, timestamp) \
         VALUES (?1, ?2, ?3, ?4, ?5, 0, 0, 0, NULL, ?6, ?7)",
        rusqlite::params![id, plugin_id, collection, from_version, to_version, error, now],
    )?;

    Ok(())
}

/// Retrieve the migration history for a given plugin and collection.
///
/// Results are ordered by timestamp ascending (oldest first) for admin review.
pub fn get_migration_history(
    db: &Connection,
    plugin_id: &str,
    collection: &str,
) -> Result<Vec<MigrationLogEntry>, StorageError> {
    let mut stmt = db.prepare(
        "SELECT id, plugin_id, collection, from_version, to_version, \
         records_migrated, records_quarantined, duration_ms, backup_path, error, timestamp \
         FROM migration_log WHERE plugin_id = ?1 AND collection = ?2 ORDER BY timestamp ASC",
    )?;

    let rows = stmt.query_map(rusqlite::params![plugin_id, collection], |row| {
        Ok(MigrationLogEntry {
            id: row.get(0)?,
            plugin_id: row.get(1)?,
            collection: row.get(2)?,
            from_version: row.get(3)?,
            to_version: row.get(4)?,
            records_migrated: row.get(5)?,
            records_quarantined: row.get(6)?,
            duration_ms: row.get(7)?,
            backup_path: row.get(8)?,
            error: row.get(9)?,
            timestamp: row.get(10)?,
        })
    })?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row?);
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::MIGRATION_LOG_DDL;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(MIGRATION_LOG_DDL)
            .expect("create migration_log table");
        conn
    }

    fn sample_entry() -> MigrationLogEntry {
        MigrationLogEntry {
            id: Uuid::new_v4().to_string(),
            plugin_id: "com.example.plugin".to_string(),
            collection: "events".to_string(),
            from_version: "1.0.0".to_string(),
            to_version: "2.0.0".to_string(),
            records_migrated: 42,
            records_quarantined: 3,
            duration_ms: 150,
            backup_path: Some("/backups/pre-migration-2026-03-23-120000.db".to_string()),
            error: None,
            timestamp: Utc::now().to_rfc3339(),
        }
    }

    #[test]
    fn log_migration_inserts_entry() {
        let conn = setup_db();
        let entry = sample_entry();

        log_migration(&conn, &entry).expect("insert should succeed");

        let count: i64 = conn
            .query_row("SELECT count(*) FROM migration_log", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn log_migration_stores_all_fields() {
        let conn = setup_db();
        let entry = sample_entry();

        log_migration(&conn, &entry).unwrap();

        let results = get_migration_history(&conn, "com.example.plugin", "events").unwrap();
        assert_eq!(results.len(), 1);

        let stored = &results[0];
        assert_eq!(stored.id, entry.id);
        assert_eq!(stored.plugin_id, "com.example.plugin");
        assert_eq!(stored.collection, "events");
        assert_eq!(stored.from_version, "1.0.0");
        assert_eq!(stored.to_version, "2.0.0");
        assert_eq!(stored.records_migrated, 42);
        assert_eq!(stored.records_quarantined, 3);
        assert_eq!(stored.duration_ms, 150);
        assert_eq!(
            stored.backup_path.as_deref(),
            Some("/backups/pre-migration-2026-03-23-120000.db")
        );
        assert!(stored.error.is_none());
        assert!(!stored.timestamp.is_empty());
    }

    #[test]
    fn log_failure_records_error_with_zero_counts() {
        let conn = setup_db();

        log_failure(
            &conn,
            "com.example.plugin",
            "events",
            "1.0.0",
            "2.0.0",
            "WASM module failed to load",
        )
        .expect("log_failure should succeed");

        let results = get_migration_history(&conn, "com.example.plugin", "events").unwrap();
        assert_eq!(results.len(), 1);

        let stored = &results[0];
        assert_eq!(stored.records_migrated, 0);
        assert_eq!(stored.records_quarantined, 0);
        assert_eq!(stored.duration_ms, 0);
        assert!(stored.backup_path.is_none());
        assert_eq!(stored.error.as_deref(), Some("WASM module failed to load"));
    }

    #[test]
    fn get_migration_history_filters_by_plugin_and_collection() {
        let conn = setup_db();

        let mut entry_a = sample_entry();
        entry_a.plugin_id = "plugin-a".to_string();
        entry_a.collection = "events".to_string();
        log_migration(&conn, &entry_a).unwrap();

        let mut entry_b = sample_entry();
        entry_b.plugin_id = "plugin-b".to_string();
        entry_b.collection = "events".to_string();
        log_migration(&conn, &entry_b).unwrap();

        let mut entry_c = sample_entry();
        entry_c.plugin_id = "plugin-a".to_string();
        entry_c.collection = "tasks".to_string();
        log_migration(&conn, &entry_c).unwrap();

        let results = get_migration_history(&conn, "plugin-a", "events").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].plugin_id, "plugin-a");
        assert_eq!(results[0].collection, "events");
    }

    #[test]
    fn get_migration_history_returns_empty_for_no_matches() {
        let conn = setup_db();

        let results = get_migration_history(&conn, "nonexistent", "events").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn log_migration_allows_null_backup_path() {
        let conn = setup_db();
        let mut entry = sample_entry();
        entry.backup_path = None;

        log_migration(&conn, &entry).unwrap();

        let results = get_migration_history(&conn, "com.example.plugin", "events").unwrap();
        assert!(results[0].backup_path.is_none());
    }

    #[test]
    fn log_migration_allows_error_field() {
        let conn = setup_db();
        let mut entry = sample_entry();
        entry.error = Some("partial failure".to_string());

        log_migration(&conn, &entry).unwrap();

        let results = get_migration_history(&conn, "com.example.plugin", "events").unwrap();
        assert_eq!(results[0].error.as_deref(), Some("partial failure"));
    }

    #[test]
    fn multiple_migrations_ordered_by_timestamp() {
        let conn = setup_db();

        // Insert with explicit timestamps to verify ordering.
        let mut e1 = sample_entry();
        e1.timestamp = "2026-01-01T00:00:00Z".to_string();
        e1.from_version = "1.0.0".to_string();
        e1.to_version = "2.0.0".to_string();
        log_migration(&conn, &e1).unwrap();

        let mut e2 = sample_entry();
        e2.timestamp = "2026-01-02T00:00:00Z".to_string();
        e2.from_version = "2.0.0".to_string();
        e2.to_version = "3.0.0".to_string();
        log_migration(&conn, &e2).unwrap();

        let results = get_migration_history(&conn, "com.example.plugin", "events").unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].from_version, "1.0.0");
        assert_eq!(results[1].from_version, "2.0.0");
    }
}
