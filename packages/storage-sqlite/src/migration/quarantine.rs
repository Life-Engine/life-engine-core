//! Quarantine table operations for records that fail migration transforms.
//!
//! When an individual record cannot be migrated (the WASM transform fails or
//! the output fails schema validation), the record is inserted into the
//! quarantine table instead of being lost. An admin can later review, retry,
//! or delete quarantined records.

use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::StorageError;

/// A record that failed migration and was placed in quarantine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarantinedRecord {
    /// Unique quarantine entry ID.
    pub id: String,
    /// The original JSON record data that failed to migrate.
    pub record_data: String,
    /// The plugin that owns this record.
    pub plugin_id: String,
    /// The collection this record belongs to.
    pub collection: String,
    /// The version the record was at before migration.
    pub from_version: String,
    /// The target version the migration was attempting.
    pub to_version: String,
    /// Why the transform failed.
    pub error_message: String,
    /// When the record was quarantined (ISO 8601).
    pub timestamp: String,
}

/// Insert a failed record into the quarantine table.
///
/// Returns the UUID assigned to the quarantine entry.
pub fn quarantine_record(
    db: &Connection,
    record_data: &str,
    plugin_id: &str,
    collection: &str,
    from_version: &str,
    to_version: &str,
    error: &str,
) -> Result<String, StorageError> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    db.execute(
        "INSERT INTO quarantine (id, record_data, plugin_id, collection, from_version, to_version, error_message, timestamp) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![id, record_data, plugin_id, collection, from_version, to_version, error, now],
    )?;

    Ok(id)
}

/// List all quarantined records for a given plugin and collection.
pub fn list_quarantined(
    db: &Connection,
    plugin_id: &str,
    collection: &str,
) -> Result<Vec<QuarantinedRecord>, StorageError> {
    let mut stmt = db.prepare(
        "SELECT id, record_data, plugin_id, collection, from_version, to_version, error_message, timestamp \
         FROM quarantine WHERE plugin_id = ?1 AND collection = ?2 ORDER BY timestamp ASC",
    )?;

    let rows = stmt.query_map(rusqlite::params![plugin_id, collection], |row| {
        Ok(QuarantinedRecord {
            id: row.get(0)?,
            record_data: row.get(1)?,
            plugin_id: row.get(2)?,
            collection: row.get(3)?,
            from_version: row.get(4)?,
            to_version: row.get(5)?,
            error_message: row.get(6)?,
            timestamp: row.get(7)?,
        })
    })?;

    let mut records = Vec::new();
    for row in rows {
        records.push(row?);
    }
    Ok(records)
}

/// Retrieve a single quarantined record by its ID for retry.
///
/// Returns the record's original JSON data so the migration engine can
/// re-attempt the transform.
pub fn retry_quarantined(
    db: &Connection,
    quarantine_id: &str,
) -> Result<serde_json::Value, StorageError> {
    let data: String = db
        .query_row(
            "SELECT record_data FROM quarantine WHERE id = ?1",
            rusqlite::params![quarantine_id],
            |row| row.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                StorageError::NotFound(format!("quarantine entry '{quarantine_id}'"))
            }
            other => StorageError::Database(other),
        })?;

    let value: serde_json::Value = serde_json::from_str(&data)?;
    Ok(value)
}

/// Delete a quarantined record (e.g., after successful retry or admin dismissal).
pub fn delete_quarantined(
    db: &Connection,
    quarantine_id: &str,
) -> Result<(), StorageError> {
    let deleted = db.execute(
        "DELETE FROM quarantine WHERE id = ?1",
        rusqlite::params![quarantine_id],
    )?;

    if deleted == 0 {
        return Err(StorageError::NotFound(format!(
            "quarantine entry '{quarantine_id}'"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::QUARANTINE_DDL;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(QUARANTINE_DDL)
            .expect("create quarantine table");
        conn
    }

    #[test]
    fn quarantine_record_inserts_and_returns_id() {
        let conn = setup_db();

        let id = quarantine_record(
            &conn,
            r#"{"title": "old"}"#,
            "com.example.plugin",
            "events",
            "1.0.0",
            "2.0.0",
            "transform panicked",
        )
        .expect("insert should succeed");

        assert!(!id.is_empty());

        let count: i64 = conn
            .query_row("SELECT count(*) FROM quarantine", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn list_quarantined_returns_matching_records() {
        let conn = setup_db();

        quarantine_record(&conn, r#"{"a":1}"#, "plugin-a", "events", "1.0.0", "2.0.0", "err1")
            .unwrap();
        quarantine_record(&conn, r#"{"b":2}"#, "plugin-a", "events", "1.0.0", "2.0.0", "err2")
            .unwrap();
        quarantine_record(&conn, r#"{"c":3}"#, "plugin-b", "events", "1.0.0", "2.0.0", "err3")
            .unwrap();
        quarantine_record(&conn, r#"{"d":4}"#, "plugin-a", "tasks", "1.0.0", "2.0.0", "err4")
            .unwrap();

        let results = list_quarantined(&conn, "plugin-a", "events").unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].error_message, "err1");
        assert_eq!(results[1].error_message, "err2");
    }

    #[test]
    fn list_quarantined_returns_empty_for_no_matches() {
        let conn = setup_db();

        let results = list_quarantined(&conn, "nonexistent", "events").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn retry_quarantined_returns_record_data() {
        let conn = setup_db();

        let data = r#"{"title":"test","count":42}"#;
        let id = quarantine_record(&conn, data, "plugin-a", "events", "1.0.0", "2.0.0", "err")
            .unwrap();

        let value = retry_quarantined(&conn, &id).unwrap();
        assert_eq!(value["title"], "test");
        assert_eq!(value["count"], 42);
    }

    #[test]
    fn retry_quarantined_errors_for_missing_id() {
        let conn = setup_db();

        let result = retry_quarantined(&conn, "nonexistent-id");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn delete_quarantined_removes_record() {
        let conn = setup_db();

        let id = quarantine_record(&conn, r#"{}"#, "plugin-a", "events", "1.0.0", "2.0.0", "err")
            .unwrap();

        delete_quarantined(&conn, &id).unwrap();

        let count: i64 = conn
            .query_row("SELECT count(*) FROM quarantine", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn delete_quarantined_errors_for_missing_id() {
        let conn = setup_db();

        let result = delete_quarantined(&conn, "nonexistent-id");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn quarantine_record_generates_unique_ids() {
        let conn = setup_db();

        let id1 = quarantine_record(&conn, r#"{}"#, "p", "c", "1.0.0", "2.0.0", "e").unwrap();
        let id2 = quarantine_record(&conn, r#"{}"#, "p", "c", "1.0.0", "2.0.0", "e").unwrap();
        let id3 = quarantine_record(&conn, r#"{}"#, "p", "c", "1.0.0", "2.0.0", "e").unwrap();

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn quarantine_stores_all_fields_correctly() {
        let conn = setup_db();

        let id = quarantine_record(
            &conn,
            r#"{"key":"value"}"#,
            "com.example.myplugin",
            "contacts",
            "1.0.0",
            "3.0.0",
            "field 'name' missing",
        )
        .unwrap();

        let records = list_quarantined(&conn, "com.example.myplugin", "contacts").unwrap();
        assert_eq!(records.len(), 1);

        let rec = &records[0];
        assert_eq!(rec.id, id);
        assert_eq!(rec.record_data, r#"{"key":"value"}"#);
        assert_eq!(rec.plugin_id, "com.example.myplugin");
        assert_eq!(rec.collection, "contacts");
        assert_eq!(rec.from_version, "1.0.0");
        assert_eq!(rec.to_version, "3.0.0");
        assert_eq!(rec.error_message, "field 'name' missing");
        assert!(!rec.timestamp.is_empty());
    }
}
