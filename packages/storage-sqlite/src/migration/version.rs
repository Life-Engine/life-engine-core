//! Schema version tracking for canonical and plugin-owned collections.
//!
//! Each collection has a stored version in the `schema_versions` table.
//! During startup, Core compares stored versions against declared versions
//! and runs migration transforms when the stored version is behind.

use chrono::Utc;
use rusqlite::Connection;

use crate::StorageError;

/// Get the current schema version for a plugin/collection pair.
///
/// Returns `None` if no version has been recorded yet (first run).
pub fn get_schema_version(
    conn: &Connection,
    plugin_id: &str,
    collection: &str,
) -> Result<Option<i64>, StorageError> {
    let mut stmt = conn
        .prepare(
            "SELECT version FROM schema_versions \
             WHERE plugin_id = ?1 AND collection = ?2",
        )
        .map_err(StorageError::Database)?;

    match stmt.query_row(rusqlite::params![plugin_id, collection], |row| row.get(0)) {
        Ok(version) => Ok(Some(version)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(StorageError::Database(e)),
    }
}

/// Set the schema version for a plugin/collection pair.
///
/// Uses INSERT OR REPLACE to handle both initial creation and updates.
pub fn set_schema_version(
    conn: &Connection,
    plugin_id: &str,
    collection: &str,
    version: i64,
) -> Result<(), StorageError> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO schema_versions (plugin_id, collection, version, updated_at) \
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![plugin_id, collection, version, now],
    )
    .map_err(StorageError::Database)?;
    Ok(())
}

/// Stamp a record's version after a successful migration transform.
///
/// This must be called within the same transaction as the data update to ensure
/// atomicity. After stamping, the record's version no longer matches the
/// migration entry's `from` range, preventing re-migration on subsequent runs.
pub fn stamp_version(
    conn: &Connection,
    record_id: &str,
    new_version: &str,
) -> Result<(), StorageError> {
    let version: i64 = new_version
        .split('.')
        .next()
        .unwrap_or(new_version)
        .parse()
        .map_err(|_| {
            StorageError::InvalidConfig(format!(
                "cannot parse major version from '{new_version}'"
            ))
        })?;

    let updated = conn
        .execute(
            "UPDATE plugin_data SET version = ?1 WHERE id = ?2",
            rusqlite::params![version, record_id],
        )
        .map_err(StorageError::Database)?;

    if updated == 0 {
        return Err(StorageError::NotFound(format!(
            "plugin_data record '{record_id}'"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::SCHEMA_VERSIONS_DDL;

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA_VERSIONS_DDL).unwrap();
        conn
    }

    #[test]
    fn get_returns_none_for_unknown_collection() {
        let conn = setup_conn();
        let result = get_schema_version(&conn, "core", "events").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn set_then_get_returns_version() {
        let conn = setup_conn();
        set_schema_version(&conn, "core", "events", 3).unwrap();
        let version = get_schema_version(&conn, "core", "events").unwrap();
        assert_eq!(version, Some(3));
    }

    #[test]
    fn set_overwrites_existing_version() {
        let conn = setup_conn();
        set_schema_version(&conn, "core", "events", 1).unwrap();
        set_schema_version(&conn, "core", "events", 2).unwrap();
        let version = get_schema_version(&conn, "core", "events").unwrap();
        assert_eq!(version, Some(2));
    }

    #[test]
    fn versions_are_scoped_by_plugin_and_collection() {
        let conn = setup_conn();
        set_schema_version(&conn, "core", "events", 1).unwrap();
        set_schema_version(&conn, "core", "tasks", 2).unwrap();
        set_schema_version(&conn, "plugin-a", "events", 5).unwrap();

        assert_eq!(get_schema_version(&conn, "core", "events").unwrap(), Some(1));
        assert_eq!(get_schema_version(&conn, "core", "tasks").unwrap(), Some(2));
        assert_eq!(get_schema_version(&conn, "plugin-a", "events").unwrap(), Some(5));
    }

    // --- stamp_version tests ---

    use crate::schema::PLUGIN_DATA_DDL;

    fn setup_conn_with_plugin_data() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(PLUGIN_DATA_DDL).unwrap();
        conn
    }

    fn insert_record(conn: &Connection, id: &str, plugin_id: &str, collection: &str, version: i64) {
        conn.execute(
            "INSERT INTO plugin_data (id, plugin_id, collection, data, version, created_at, updated_at) \
             VALUES (?1, ?2, ?3, '{}', ?4, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            rusqlite::params![id, plugin_id, collection, version],
        )
        .unwrap();
    }

    fn get_record_version(conn: &Connection, id: &str) -> i64 {
        conn.query_row(
            "SELECT version FROM plugin_data WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )
        .unwrap()
    }

    #[test]
    fn stamp_version_updates_record_version() {
        let conn = setup_conn_with_plugin_data();
        insert_record(&conn, "rec-1", "plugin-a", "events", 1);

        stamp_version(&conn, "rec-1", "2.0.0").unwrap();

        assert_eq!(get_record_version(&conn, "rec-1"), 2);
    }

    #[test]
    fn stamp_version_atomic_with_data_update() {
        let conn = setup_conn_with_plugin_data();
        insert_record(&conn, "rec-1", "plugin-a", "events", 1);

        // Start a transaction, update data and stamp version, then rollback.
        conn.execute("BEGIN", []).unwrap();
        conn.execute(
            "UPDATE plugin_data SET data = '{\"migrated\":true}' WHERE id = 'rec-1'",
            [],
        )
        .unwrap();
        stamp_version(&conn, "rec-1", "2.0.0").unwrap();
        conn.execute("ROLLBACK", []).unwrap();

        // Both the data update and version stamp should be rolled back.
        assert_eq!(get_record_version(&conn, "rec-1"), 1);
        let data: String = conn
            .query_row(
                "SELECT data FROM plugin_data WHERE id = 'rec-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(data, "{}");
    }

    #[test]
    fn rerunning_migration_skips_already_migrated_records() {
        let conn = setup_conn_with_plugin_data();
        insert_record(&conn, "rec-1", "plugin-a", "events", 1);
        insert_record(&conn, "rec-2", "plugin-a", "events", 1);

        // Simulate first migration run: stamp rec-1 to version 2.
        stamp_version(&conn, "rec-1", "2.0.0").unwrap();

        // Query for records still at version 1 (the "from" range).
        let mut stmt = conn
            .prepare(
                "SELECT id FROM plugin_data \
                 WHERE plugin_id = 'plugin-a' AND collection = 'events' AND version = 1",
            )
            .unwrap();
        let remaining: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        // Only rec-2 should still need migration.
        assert_eq!(remaining, vec!["rec-2"]);
    }

    #[test]
    fn stamp_version_errors_for_missing_record() {
        let conn = setup_conn_with_plugin_data();

        let result = stamp_version(&conn, "nonexistent", "2.0.0");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn stamp_version_parses_major_from_semver() {
        let conn = setup_conn_with_plugin_data();
        insert_record(&conn, "rec-1", "plugin-a", "events", 1);

        stamp_version(&conn, "rec-1", "3.2.1").unwrap();
        assert_eq!(get_record_version(&conn, "rec-1"), 3);
    }
}
