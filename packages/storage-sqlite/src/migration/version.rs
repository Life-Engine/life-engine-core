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
}
