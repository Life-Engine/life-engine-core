//! Audit logging for storage operations.

use chrono::Utc;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::StorageError;
use crate::schema::AUDIT_RETENTION_DAYS;

/// Types of auditable security events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventType {
    AuthSuccess,
    AuthFailure,
    CredentialAccess,
    CredentialModify,
    PluginLoad,
    PluginError,
    PermissionChange,
    DataExport,
}

impl AuditEventType {
    fn as_str(self) -> &'static str {
        match self {
            Self::AuthSuccess => "auth_success",
            Self::AuthFailure => "auth_failure",
            Self::CredentialAccess => "credential_access",
            Self::CredentialModify => "credential_modify",
            Self::PluginLoad => "plugin_load",
            Self::PluginError => "plugin_error",
            Self::PermissionChange => "permission_change",
            Self::DataExport => "data_export",
        }
    }
}

/// An audit event to be recorded in the audit log.
#[derive(Debug, Clone)]
pub struct AuditEvent {
    /// The type of security event.
    pub event_type: AuditEventType,
    /// The plugin that triggered the event, if applicable.
    pub plugin_id: Option<String>,
    /// Additional details about the event as structured JSON.
    pub details: serde_json::Value,
}

/// Insert an audit event into the audit_log table.
pub fn log_event(db: &Connection, event: AuditEvent) -> Result<(), StorageError> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let details_str = serde_json::to_string(&event.details)?;

    db.execute(
        "INSERT INTO audit_log (id, timestamp, event_type, plugin_id, details, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            id,
            now,
            event.event_type.as_str(),
            event.plugin_id,
            details_str,
            now,
        ],
    )?;

    Ok(())
}

/// Delete audit log entries older than the retention period (90 days).
///
/// Returns the number of deleted rows. This function is intended to be
/// called daily by the scheduler.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::AUDIT_LOG_DDL;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(AUDIT_LOG_DDL).expect("create audit_log table");
        conn
    }

    #[test]
    fn log_event_inserts_row() {
        let conn = setup_db();

        let event = AuditEvent {
            event_type: AuditEventType::AuthSuccess,
            plugin_id: None,
            details: serde_json::json!({"user": "test"}),
        };

        log_event(&conn, event).expect("log_event should succeed");

        let count: i64 = conn
            .query_row("SELECT count(*) FROM audit_log", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn log_event_with_plugin_id() {
        let conn = setup_db();

        let event = AuditEvent {
            event_type: AuditEventType::CredentialAccess,
            plugin_id: Some("com.example.plugin".to_string()),
            details: serde_json::json!({"credential_id": "cred-1"}),
        };

        log_event(&conn, event).expect("log_event should succeed");

        let (event_type, plugin_id): (String, Option<String>) = conn
            .query_row(
                "SELECT event_type, plugin_id FROM audit_log LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(event_type, "credential_access");
        assert_eq!(plugin_id.as_deref(), Some("com.example.plugin"));
    }

    #[test]
    fn log_event_stores_details_as_json() {
        let conn = setup_db();

        let details = serde_json::json!({"action": "read", "count": 5});
        let event = AuditEvent {
            event_type: AuditEventType::DataExport,
            plugin_id: None,
            details: details.clone(),
        };

        log_event(&conn, event).expect("log_event should succeed");

        let stored: String = conn
            .query_row("SELECT details FROM audit_log LIMIT 1", [], |row| row.get(0))
            .unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&stored).unwrap();
        assert_eq!(parsed, details);
    }

    #[test]
    fn log_event_generates_unique_ids() {
        let conn = setup_db();

        for _ in 0..3 {
            let event = AuditEvent {
                event_type: AuditEventType::PluginLoad,
                plugin_id: Some("test-plugin".to_string()),
                details: serde_json::json!({}),
            };
            log_event(&conn, event).expect("log_event should succeed");
        }

        let count: i64 = conn
            .query_row("SELECT count(DISTINCT id) FROM audit_log", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn cleanup_old_entries_removes_expired() {
        let conn = setup_db();

        // Insert an entry with a timestamp 100 days ago (beyond 90-day retention).
        let old_timestamp = Utc::now()
            .checked_sub_signed(chrono::Duration::days(100))
            .unwrap()
            .to_rfc3339();

        conn.execute(
            "INSERT INTO audit_log (id, timestamp, event_type, plugin_id, details, created_at) \
             VALUES ('old-1', ?1, 'auth_success', NULL, '{}', ?1)",
            rusqlite::params![old_timestamp],
        )
        .unwrap();

        // Insert a recent entry.
        let event = AuditEvent {
            event_type: AuditEventType::AuthSuccess,
            plugin_id: None,
            details: serde_json::json!({}),
        };
        log_event(&conn, event).expect("log_event should succeed");

        let deleted = cleanup_old_entries(&conn).expect("cleanup should succeed");
        assert_eq!(deleted, 1);

        // Only the recent entry should remain.
        let remaining: i64 = conn
            .query_row("SELECT count(*) FROM audit_log", [], |row| row.get(0))
            .unwrap();
        assert_eq!(remaining, 1);
    }

    #[test]
    fn cleanup_old_entries_keeps_recent() {
        let conn = setup_db();

        // Insert 3 recent entries.
        for i in 0..3 {
            let event = AuditEvent {
                event_type: AuditEventType::PluginLoad,
                plugin_id: Some(format!("plugin-{i}")),
                details: serde_json::json!({}),
            };
            log_event(&conn, event).expect("log_event should succeed");
        }

        let deleted = cleanup_old_entries(&conn).expect("cleanup should succeed");
        assert_eq!(deleted, 0);

        let remaining: i64 = conn
            .query_row("SELECT count(*) FROM audit_log", [], |row| row.get(0))
            .unwrap();
        assert_eq!(remaining, 3);
    }

    #[test]
    fn all_event_types_serialize_correctly() {
        let conn = setup_db();

        let types = [
            (AuditEventType::AuthSuccess, "auth_success"),
            (AuditEventType::AuthFailure, "auth_failure"),
            (AuditEventType::CredentialAccess, "credential_access"),
            (AuditEventType::CredentialModify, "credential_modify"),
            (AuditEventType::PluginLoad, "plugin_load"),
            (AuditEventType::PluginError, "plugin_error"),
            (AuditEventType::PermissionChange, "permission_change"),
            (AuditEventType::DataExport, "data_export"),
        ];

        for (event_type, expected_str) in types {
            let event = AuditEvent {
                event_type,
                plugin_id: None,
                details: serde_json::json!({}),
            };
            log_event(&conn, event).expect("log_event should succeed");

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
}
