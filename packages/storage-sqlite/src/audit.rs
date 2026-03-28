//! Audit logging for storage and security operations.
//!
//! Provides an append-only audit log persisted to the `audit_log` SQLite table.
//! Callers can insert events via `log_event` and query them via `query_events`.
//! Retention cleanup deletes entries older than 90 days (configurable via
//! `AUDIT_RETENTION_DAYS` in the schema module).
//!
//! The public API intentionally exposes no update or delete functions beyond
//! `cleanup_old_entries` — the audit log is append-only by design.

use chrono::{DateTime, Utc};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::StorageError;
use crate::schema::AUDIT_RETENTION_DAYS;

// ── Event types ──────────────────────────────────────────────

/// Types of auditable events.
///
/// Covers both storage write events (spec requirements 6.1-6.5) and
/// security events (spec requirements 7.1-7.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventType {
    // Storage write events (requirement 6)
    /// A record was created in a collection.
    StorageCreated,
    /// A record was updated in a collection.
    StorageUpdated,
    /// A record was deleted from a collection.
    StorageDeleted,
    /// A blob was stored.
    BlobStored,
    /// A blob was deleted.
    BlobDeleted,

    // Security events (requirement 7)
    /// Authentication succeeded.
    AuthSuccess,
    /// Authentication failed.
    AuthFailure,
    /// A credential was read.
    CredentialAccess,
    /// A credential was created, rotated, or revoked.
    CredentialModify,
    /// A plugin was loaded/installed.
    PluginInstall,
    /// A plugin was enabled.
    PluginEnable,
    /// A plugin was disabled.
    PluginDisable,
    /// A plugin encountered an error.
    PluginError,
    /// A permission was granted or revoked.
    PermissionChange,
    /// A connector was authorised or its authorisation was revoked.
    ConnectorAuth,
    /// Data was exported.
    DataExport,
}

impl AuditEventType {
    /// Returns the canonical string representation stored in the database.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StorageCreated => "system.storage.created",
            Self::StorageUpdated => "system.storage.updated",
            Self::StorageDeleted => "system.storage.deleted",
            Self::BlobStored => "system.blob.stored",
            Self::BlobDeleted => "system.blob.deleted",
            Self::AuthSuccess => "auth_success",
            Self::AuthFailure => "auth_failure",
            Self::CredentialAccess => "credential_access",
            Self::CredentialModify => "credential_modify",
            Self::PluginInstall => "plugin_install",
            Self::PluginEnable => "plugin_enable",
            Self::PluginDisable => "plugin_disable",
            Self::PluginError => "plugin_error",
            Self::PermissionChange => "permission_change",
            Self::ConnectorAuth => "connector_auth",
            Self::DataExport => "data_export",
        }
    }

    /// Parse from the stored string representation.
    pub fn from_str_repr(s: &str) -> Option<Self> {
        match s {
            "system.storage.created" => Some(Self::StorageCreated),
            "system.storage.updated" => Some(Self::StorageUpdated),
            "system.storage.deleted" => Some(Self::StorageDeleted),
            "system.blob.stored" => Some(Self::BlobStored),
            "system.blob.deleted" => Some(Self::BlobDeleted),
            "auth_success" => Some(Self::AuthSuccess),
            "auth_failure" => Some(Self::AuthFailure),
            "credential_access" => Some(Self::CredentialAccess),
            "credential_modify" => Some(Self::CredentialModify),
            "plugin_install" => Some(Self::PluginInstall),
            "plugin_enable" => Some(Self::PluginEnable),
            "plugin_disable" => Some(Self::PluginDisable),
            "plugin_error" => Some(Self::PluginError),
            "permission_change" => Some(Self::PermissionChange),
            "connector_auth" => Some(Self::ConnectorAuth),
            "data_export" => Some(Self::DataExport),
            _ => None,
        }
    }
}

// ── Audit event ──────────────────────────────────────────────

/// An audit event to be recorded in the audit log.
#[derive(Debug, Clone)]
pub struct AuditEvent {
    /// The type of event.
    pub event_type: AuditEventType,
    /// The collection involved, if applicable (storage events).
    pub collection: Option<String>,
    /// The document/record id involved, if applicable.
    pub document_id: Option<String>,
    /// The identity subject (user/principal) that triggered the event.
    pub identity_subject: Option<String>,
    /// The plugin that triggered the event, if applicable.
    pub plugin_id: Option<String>,
    /// Additional structured details about the event.
    pub details: serde_json::Value,
}

/// A persisted audit log entry as returned by queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    /// Unique entry id.
    pub id: String,
    /// When the event occurred (RFC 3339).
    pub timestamp: String,
    /// The event type string.
    pub event_type: String,
    /// The collection involved, if any.
    pub collection: Option<String>,
    /// The document id involved, if any.
    pub document_id: Option<String>,
    /// The identity subject, if any.
    pub identity_subject: Option<String>,
    /// The plugin id, if any.
    pub plugin_id: Option<String>,
    /// Structured details (JSON).
    pub details: serde_json::Value,
    /// When the row was created (RFC 3339).
    pub created_at: String,
}

// ── Write (append-only) ──────────────────────────────────────

/// Insert an audit event into the audit_log table.
///
/// This is the only write path for the audit log. There are no public
/// update or delete-by-id functions — the log is append-only.
pub fn log_event(db: &Connection, event: AuditEvent) -> Result<(), StorageError> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let details_str = serde_json::to_string(&event.details)?;

    db.execute(
        "INSERT INTO audit_log \
         (id, timestamp, event_type, collection, document_id, identity_subject, plugin_id, details, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            id,
            now,
            event.event_type.as_str(),
            event.collection,
            event.document_id,
            event.identity_subject,
            event.plugin_id,
            details_str,
            now,
        ],
    )?;

    Ok(())
}

// ── Query ────────────────────────────────────────────────────

/// Query audit log entries by event type and/or time range.
///
/// All filter parameters are optional — pass `None` to skip a filter.
pub fn query_events(
    db: &Connection,
    event_type: Option<&str>,
    from: Option<&DateTime<Utc>>,
    to: Option<&DateTime<Utc>>,
) -> Result<Vec<AuditLogEntry>, StorageError> {
    let mut clauses = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(et) = event_type {
        clauses.push(format!("event_type = ?{}", params.len() + 1));
        params.push(Box::new(et.to_string()));
    }
    if let Some(f) = from {
        clauses.push(format!("timestamp >= ?{}", params.len() + 1));
        params.push(Box::new(f.to_rfc3339()));
    }
    if let Some(t) = to {
        clauses.push(format!("timestamp <= ?{}", params.len() + 1));
        params.push(Box::new(t.to_rfc3339()));
    }

    let where_clause = if clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", clauses.join(" AND "))
    };

    let sql = format!(
        "SELECT id, timestamp, event_type, collection, document_id, \
         identity_subject, plugin_id, details, created_at \
         FROM audit_log {where_clause} ORDER BY timestamp ASC"
    );

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = db.prepare(&sql)?;
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        let details_str: String = row.get(7)?;
        let details: serde_json::Value =
            serde_json::from_str(&details_str).unwrap_or(serde_json::Value::Null);
        Ok(AuditLogEntry {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            event_type: row.get(2)?,
            collection: row.get(3)?,
            document_id: row.get(4)?,
            identity_subject: row.get(5)?,
            plugin_id: row.get(6)?,
            details,
            created_at: row.get(8)?,
        })
    })?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row?);
    }
    Ok(entries)
}

// ── Retention cleanup ────────────────────────────────────────

/// Delete audit log entries older than the retention period (90 days by default).
///
/// Returns the number of deleted rows. This function is intended to be
/// called daily by the scheduler. It is the only deletion path for the
/// audit log — callers cannot delete individual entries.
pub fn cleanup_old_entries(db: &Connection) -> Result<u64, StorageError> {
    let cutoff = Utc::now()
        .checked_sub_signed(chrono::Duration::days(i64::from(AUDIT_RETENTION_DAYS)))
        .unwrap_or_else(Utc::now)
        .to_rfc3339();

    let deleted = db.execute(
        "DELETE FROM audit_log WHERE timestamp < ?1",
        rusqlite::params![cutoff],
    )?;

    Ok(deleted as u64)
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::AUDIT_LOG_DDL;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(AUDIT_LOG_DDL).expect("create audit_log table");
        conn
    }

    fn make_event(event_type: AuditEventType) -> AuditEvent {
        AuditEvent {
            event_type,
            collection: None,
            document_id: None,
            identity_subject: None,
            plugin_id: None,
            details: serde_json::json!({}),
        }
    }

    // ── 1. Audit event struct has all required fields ────────

    #[test]
    fn audit_event_has_all_required_fields() {
        let event = AuditEvent {
            event_type: AuditEventType::StorageCreated,
            collection: Some("tasks".into()),
            document_id: Some("doc-1".into()),
            identity_subject: Some("user:alice".into()),
            plugin_id: Some("com.example.plugin".into()),
            details: serde_json::json!({"extra": true}),
        };

        assert_eq!(event.event_type, AuditEventType::StorageCreated);
        assert_eq!(event.collection.as_deref(), Some("tasks"));
        assert_eq!(event.document_id.as_deref(), Some("doc-1"));
        assert_eq!(event.identity_subject.as_deref(), Some("user:alice"));
        assert_eq!(event.plugin_id.as_deref(), Some("com.example.plugin"));
    }

    // ── 2. Audit event persists to SQLite table ──────────────

    #[test]
    fn log_event_inserts_row_with_all_columns() {
        let conn = setup_db();

        let event = AuditEvent {
            event_type: AuditEventType::StorageCreated,
            collection: Some("tasks".into()),
            document_id: Some("rec-1".into()),
            identity_subject: Some("user:bob".into()),
            plugin_id: Some("com.example.plugin".into()),
            details: serde_json::json!({"action": "create"}),
        };

        log_event(&conn, event).expect("log_event should succeed");

        let (event_type, collection, document_id, identity_subject, plugin_id, details): (
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            String,
        ) = conn
            .query_row(
                "SELECT event_type, collection, document_id, identity_subject, plugin_id, details \
                 FROM audit_log LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
            )
            .unwrap();

        assert_eq!(event_type, "system.storage.created");
        assert_eq!(collection.as_deref(), Some("tasks"));
        assert_eq!(document_id.as_deref(), Some("rec-1"));
        assert_eq!(identity_subject.as_deref(), Some("user:bob"));
        assert_eq!(plugin_id.as_deref(), Some("com.example.plugin"));
        let parsed: serde_json::Value = serde_json::from_str(&details).unwrap();
        assert_eq!(parsed, serde_json::json!({"action": "create"}));
    }

    #[test]
    fn log_event_generates_unique_ids() {
        let conn = setup_db();

        for _ in 0..3 {
            log_event(&conn, make_event(AuditEventType::AuthSuccess)).expect("log_event should succeed");
        }

        let count: i64 = conn
            .query_row("SELECT count(DISTINCT id) FROM audit_log", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 3);
    }

    // ── 3. Query by type and time range ──────────────────────

    #[test]
    fn query_events_by_type() {
        let conn = setup_db();

        log_event(&conn, make_event(AuditEventType::AuthSuccess)).unwrap();
        log_event(&conn, make_event(AuditEventType::AuthFailure)).unwrap();
        log_event(&conn, make_event(AuditEventType::AuthSuccess)).unwrap();

        let results = query_events(&conn, Some("auth_success"), None, None).unwrap();
        assert_eq!(results.len(), 2);
        for entry in &results {
            assert_eq!(entry.event_type, "auth_success");
        }
    }

    #[test]
    fn query_events_by_time_range() {
        let conn = setup_db();

        // Insert an entry with a timestamp 10 days ago via raw SQL.
        let old_ts = Utc::now()
            .checked_sub_signed(chrono::Duration::days(10))
            .unwrap()
            .to_rfc3339();
        conn.execute(
            "INSERT INTO audit_log (id, timestamp, event_type, collection, document_id, \
             identity_subject, plugin_id, details, created_at) \
             VALUES ('old-1', ?1, 'auth_success', NULL, NULL, NULL, NULL, '{}', ?1)",
            rusqlite::params![old_ts],
        )
        .unwrap();

        // Insert a current entry.
        log_event(&conn, make_event(AuditEventType::AuthSuccess)).unwrap();

        // Query for entries from the last 5 days.
        let from = Utc::now()
            .checked_sub_signed(chrono::Duration::days(5))
            .unwrap();
        let results = query_events(&conn, None, Some(&from), None).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn query_events_no_filters_returns_all() {
        let conn = setup_db();

        log_event(&conn, make_event(AuditEventType::AuthSuccess)).unwrap();
        log_event(&conn, make_event(AuditEventType::PluginInstall)).unwrap();

        let results = query_events(&conn, None, None, None).unwrap();
        assert_eq!(results.len(), 2);
    }

    // ── 4. Daily rotation / retention creates cleanup ────────

    #[test]
    fn cleanup_old_entries_removes_expired() {
        let conn = setup_db();

        let old_timestamp = Utc::now()
            .checked_sub_signed(chrono::Duration::days(100))
            .unwrap()
            .to_rfc3339();

        conn.execute(
            "INSERT INTO audit_log (id, timestamp, event_type, collection, document_id, \
             identity_subject, plugin_id, details, created_at) \
             VALUES ('old-1', ?1, 'auth_success', NULL, NULL, NULL, NULL, '{}', ?1)",
            rusqlite::params![old_timestamp],
        )
        .unwrap();

        log_event(&conn, make_event(AuditEventType::AuthSuccess)).unwrap();

        let deleted = cleanup_old_entries(&conn).expect("cleanup should succeed");
        assert_eq!(deleted, 1);

        let remaining: i64 = conn
            .query_row("SELECT count(*) FROM audit_log", [], |row| row.get(0))
            .unwrap();
        assert_eq!(remaining, 1);
    }

    // ── 5. 90-day retention keeps recent ─────────────────────

    #[test]
    fn cleanup_old_entries_keeps_recent() {
        let conn = setup_db();

        for _ in 0..3 {
            log_event(&conn, make_event(AuditEventType::PluginInstall)).unwrap();
        }

        let deleted = cleanup_old_entries(&conn).expect("cleanup should succeed");
        assert_eq!(deleted, 0);

        let remaining: i64 = conn
            .query_row("SELECT count(*) FROM audit_log", [], |row| row.get(0))
            .unwrap();
        assert_eq!(remaining, 3);
    }

    // ── 6. Security event types ──────────────────────────────

    #[test]
    fn all_event_types_serialize_correctly() {
        let conn = setup_db();

        let types = [
            (AuditEventType::StorageCreated, "system.storage.created"),
            (AuditEventType::StorageUpdated, "system.storage.updated"),
            (AuditEventType::StorageDeleted, "system.storage.deleted"),
            (AuditEventType::BlobStored, "system.blob.stored"),
            (AuditEventType::BlobDeleted, "system.blob.deleted"),
            (AuditEventType::AuthSuccess, "auth_success"),
            (AuditEventType::AuthFailure, "auth_failure"),
            (AuditEventType::CredentialAccess, "credential_access"),
            (AuditEventType::CredentialModify, "credential_modify"),
            (AuditEventType::PluginInstall, "plugin_install"),
            (AuditEventType::PluginEnable, "plugin_enable"),
            (AuditEventType::PluginDisable, "plugin_disable"),
            (AuditEventType::PluginError, "plugin_error"),
            (AuditEventType::PermissionChange, "permission_change"),
            (AuditEventType::ConnectorAuth, "connector_auth"),
            (AuditEventType::DataExport, "data_export"),
        ];

        for (event_type, expected_str) in types {
            log_event(&conn, make_event(event_type)).expect("log_event should succeed");

            let stored: String = conn
                .query_row(
                    "SELECT event_type FROM audit_log ORDER BY rowid DESC LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(stored, expected_str, "event type {expected_str} mismatch");
        }
    }

    #[test]
    fn event_type_roundtrips_through_str() {
        let types = [
            AuditEventType::StorageCreated,
            AuditEventType::StorageUpdated,
            AuditEventType::StorageDeleted,
            AuditEventType::BlobStored,
            AuditEventType::BlobDeleted,
            AuditEventType::AuthSuccess,
            AuditEventType::AuthFailure,
            AuditEventType::CredentialAccess,
            AuditEventType::CredentialModify,
            AuditEventType::PluginInstall,
            AuditEventType::PluginEnable,
            AuditEventType::PluginDisable,
            AuditEventType::PluginError,
            AuditEventType::PermissionChange,
            AuditEventType::ConnectorAuth,
            AuditEventType::DataExport,
        ];

        for t in types {
            let s = t.as_str();
            let parsed = AuditEventType::from_str_repr(s)
                .unwrap_or_else(|| panic!("failed to parse {s}"));
            assert_eq!(parsed, t);
        }
    }

    // ── 7. Append-only (no updates/deletes by callers) ───────

    #[test]
    fn audit_log_is_append_only_no_public_update_or_delete() {
        let conn = setup_db();

        log_event(&conn, make_event(AuditEventType::AuthSuccess)).unwrap();
        log_event(&conn, make_event(AuditEventType::AuthFailure)).unwrap();

        // The only deletion path is cleanup_old_entries, which only removes
        // entries older than 90 days. Recent entries must survive cleanup.
        let deleted = cleanup_old_entries(&conn).unwrap();
        assert_eq!(deleted, 0);

        let count: i64 = conn
            .query_row("SELECT count(*) FROM audit_log", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2, "all entries should survive — no ad-hoc delete API exists");
    }

    // ── Storage write events with collection/document ────────

    #[test]
    fn storage_event_persists_collection_and_document() {
        let conn = setup_db();

        let event = AuditEvent {
            event_type: AuditEventType::StorageCreated,
            collection: Some("tasks".into()),
            document_id: Some("task-42".into()),
            identity_subject: None,
            plugin_id: Some("com.tasks.plugin".into()),
            details: serde_json::json!({}),
        };

        log_event(&conn, event).unwrap();

        let entries = query_events(&conn, Some("system.storage.created"), None, None).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].collection.as_deref(), Some("tasks"));
        assert_eq!(entries[0].document_id.as_deref(), Some("task-42"));
        assert_eq!(entries[0].plugin_id.as_deref(), Some("com.tasks.plugin"));
    }

    #[test]
    fn blob_event_persists_correctly() {
        let conn = setup_db();

        let event = AuditEvent {
            event_type: AuditEventType::BlobStored,
            collection: None,
            document_id: Some("blob-key-abc".into()),
            identity_subject: None,
            plugin_id: Some("com.photos.plugin".into()),
            details: serde_json::json!({}),
        };

        log_event(&conn, event).unwrap();

        let entries = query_events(&conn, Some("system.blob.stored"), None, None).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].document_id.as_deref(), Some("blob-key-abc"));
    }

    // ── Query returns entries in chronological order ─────────

    #[test]
    fn query_events_returns_chronological_order() {
        let conn = setup_db();

        // Insert entries in known timestamp order.
        for i in 0..3 {
            let ts = Utc::now()
                .checked_sub_signed(chrono::Duration::seconds(10 - i))
                .unwrap()
                .to_rfc3339();
            conn.execute(
                "INSERT INTO audit_log (id, timestamp, event_type, collection, document_id, \
                 identity_subject, plugin_id, details, created_at) \
                 VALUES (?1, ?2, 'auth_success', NULL, NULL, NULL, NULL, '{}', ?2)",
                rusqlite::params![format!("id-{i}"), ts],
            )
            .unwrap();
        }

        let entries = query_events(&conn, None, None, None).unwrap();
        assert_eq!(entries.len(), 3);
        // Verify ascending order.
        assert!(entries[0].timestamp <= entries[1].timestamp);
        assert!(entries[1].timestamp <= entries[2].timestamp);
    }
}
