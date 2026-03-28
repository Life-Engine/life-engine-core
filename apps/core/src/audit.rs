//! Audit event subscriber for the Core message bus.
//!
//! Subscribes to `BusEvent` variants on the message bus and persists
//! corresponding audit log entries via `AuditLogger`. This bridges
//! the in-process event bus (requirement 6: storage write audit events)
#![allow(dead_code)]
//! with the append-only audit_log table.
//!
//! Security events (requirement 7) are logged directly by the subsystems
//! that generate them (auth, credentials, plugin loader, connector) using
//! `AuditLogger::log_event_full`.

use tokio::sync::broadcast;

use crate::message_bus::BusEvent;
use crate::sqlite_storage::AuditLogger;

/// Spawn a background task that subscribes to the message bus and
/// persists audit entries for storage and plugin lifecycle events.
///
/// The task runs until the receiver is closed (all senders dropped).
/// It uses `tokio::task::spawn_blocking` for the synchronous SQLite
/// writes to avoid blocking the async runtime.
pub fn spawn_audit_subscriber(
    mut rx: broadcast::Receiver<BusEvent>,
    conn: std::sync::Arc<tokio::sync::Mutex<rusqlite::Connection>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let conn = conn.clone();
                    if let Err(e) = handle_bus_event(&conn, &event).await {
                        tracing::warn!(error = %e, "failed to persist audit event from bus");
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(skipped = n, "audit subscriber lagged behind bus");
                }
                Err(broadcast::error::RecvError::Closed) => {
                    tracing::info!("audit subscriber shutting down — bus closed");
                    break;
                }
            }
        }
    })
}

/// Map a bus event to an audit log entry and persist it.
async fn handle_bus_event(
    conn: &std::sync::Arc<tokio::sync::Mutex<rusqlite::Connection>>,
    event: &BusEvent,
) -> anyhow::Result<()> {
    match event {
        BusEvent::RecordChanged { record } => {
            let conn = conn.lock().await;
            AuditLogger::log_event_full(
                &conn,
                "system.storage.updated",
                Some(&record.collection),
                Some(&record.id),
                None,
                Some(&record.plugin_id),
                None,
            )?;
        }
        BusEvent::RecordDeleted {
            record_id,
            collection,
        } => {
            let conn = conn.lock().await;
            AuditLogger::log_event_full(
                &conn,
                "system.storage.deleted",
                Some(collection),
                Some(record_id),
                None,
                None,
                None,
            )?;
        }
        BusEvent::PluginLoaded { plugin_id } => {
            let conn = conn.lock().await;
            AuditLogger::log_event_full(
                &conn,
                "plugin_install",
                None,
                None,
                None,
                Some(plugin_id),
                None,
            )?;
        }
        BusEvent::PluginError { plugin_id, error } => {
            let conn = conn.lock().await;
            AuditLogger::log_event_full(
                &conn,
                "plugin_error",
                None,
                None,
                None,
                Some(plugin_id),
                Some(&serde_json::json!({"error": error})),
            )?;
        }
        BusEvent::BlobStored {
            blob_key,
            plugin_id,
        } => {
            let conn = conn.lock().await;
            AuditLogger::log_event_full(
                &conn,
                "system.blob.stored",
                None,
                Some(blob_key),
                None,
                Some(plugin_id),
                None,
            )?;
        }
        BusEvent::BlobDeleted {
            blob_key,
            plugin_id,
        } => {
            let conn = conn.lock().await;
            AuditLogger::log_event_full(
                &conn,
                "system.blob.deleted",
                None,
                Some(blob_key),
                None,
                Some(plugin_id),
                None,
            )?;
        }
        BusEvent::AuthSuccess {
            identity_subject,
            method,
        } => {
            let conn = conn.lock().await;
            AuditLogger::log_event_full(
                &conn,
                "auth_success",
                None,
                None,
                Some(identity_subject),
                None,
                Some(&serde_json::json!({"method": method})),
            )?;
        }
        BusEvent::AuthFailure { client_ip, reason } => {
            let conn = conn.lock().await;
            AuditLogger::log_event_full(
                &conn,
                "auth_failure",
                None,
                None,
                None,
                None,
                Some(&serde_json::json!({"client_ip": client_ip, "reason": reason})),
            )?;
        }
        BusEvent::CredentialEvent {
            action,
            plugin_id,
            key,
        } => {
            let event_type = match action.as_str() {
                "access" => "credential_access",
                _ => "credential_modify",
            };
            let conn = conn.lock().await;
            AuditLogger::log_event_full(
                &conn,
                event_type,
                None,
                None,
                None,
                Some(plugin_id),
                Some(&serde_json::json!({"key": key, "action": action})),
            )?;
        }
        BusEvent::PluginUnloaded { plugin_id } => {
            let conn = conn.lock().await;
            AuditLogger::log_event_full(
                &conn,
                "plugin_disable",
                None,
                None,
                None,
                Some(plugin_id),
                None,
            )?;
        }
        BusEvent::ConnectorEvent { action, plugin_id } => {
            let conn = conn.lock().await;
            AuditLogger::log_event_full(
                &conn,
                "connector_auth",
                None,
                None,
                None,
                Some(plugin_id),
                Some(&serde_json::json!({"action": action})),
            )?;
        }
        // NewRecords and SyncComplete are informational — not auditable writes.
        BusEvent::NewRecords { .. } | BusEvent::SyncComplete { .. } => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message_bus::MessageBus;
    use crate::storage::Record;
    use chrono::Utc;
    use rusqlite::Connection;

    fn create_test_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS audit_log (
                id               TEXT PRIMARY KEY,
                timestamp        TEXT NOT NULL,
                event_type       TEXT NOT NULL,
                collection       TEXT,
                document_id      TEXT,
                identity_subject TEXT,
                plugin_id        TEXT,
                details          TEXT,
                created_at       TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_log(timestamp);
            CREATE INDEX IF NOT EXISTS idx_audit_event_type ON audit_log(event_type);",
        )
        .expect("create audit_log table");
        conn
    }

    #[tokio::test]
    async fn subscriber_logs_plugin_loaded() {
        let bus = MessageBus::new();
        let conn = std::sync::Arc::new(tokio::sync::Mutex::new(create_test_db()));
        let rx = bus.subscribe();

        let handle = spawn_audit_subscriber(rx, conn.clone());

        bus.publish(BusEvent::PluginLoaded {
            plugin_id: "com.test.plugin".into(),
        });

        // Give the subscriber time to process.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let db = conn.lock().await;
        let count: i64 = db
            .query_row("SELECT COUNT(*) FROM audit_log WHERE event_type = 'plugin_install'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let plugin_id: Option<String> = db
            .query_row(
                "SELECT plugin_id FROM audit_log WHERE event_type = 'plugin_install' LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(plugin_id.as_deref(), Some("com.test.plugin"));

        drop(db);
        handle.abort();
    }

    #[tokio::test]
    async fn subscriber_logs_plugin_error() {
        let bus = MessageBus::new();
        let conn = std::sync::Arc::new(tokio::sync::Mutex::new(create_test_db()));
        let rx = bus.subscribe();

        let handle = spawn_audit_subscriber(rx, conn.clone());

        bus.publish(BusEvent::PluginError {
            plugin_id: "com.test.bad".into(),
            error: "crash".into(),
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let db = conn.lock().await;
        let count: i64 = db
            .query_row("SELECT COUNT(*) FROM audit_log WHERE event_type = 'plugin_error'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        drop(db);
        handle.abort();
    }

    #[tokio::test]
    async fn subscriber_logs_record_deleted() {
        let bus = MessageBus::new();
        let conn = std::sync::Arc::new(tokio::sync::Mutex::new(create_test_db()));
        let rx = bus.subscribe();

        let handle = spawn_audit_subscriber(rx, conn.clone());

        bus.publish(BusEvent::RecordDeleted {
            record_id: "rec-42".into(),
            collection: "tasks".into(),
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let db = conn.lock().await;
        let (event_type, collection, document_id): (String, Option<String>, Option<String>) = db
            .query_row(
                "SELECT event_type, collection, document_id FROM audit_log LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();

        assert_eq!(event_type, "system.storage.deleted");
        assert_eq!(collection.as_deref(), Some("tasks"));
        assert_eq!(document_id.as_deref(), Some("rec-42"));

        drop(db);
        handle.abort();
    }

    #[tokio::test]
    async fn subscriber_logs_record_changed() {
        let bus = MessageBus::new();
        let conn = std::sync::Arc::new(tokio::sync::Mutex::new(create_test_db()));
        let rx = bus.subscribe();

        let handle = spawn_audit_subscriber(rx, conn.clone());

        bus.publish(BusEvent::RecordChanged {
            record: Record {
                id: "rec-1".into(),
                plugin_id: "com.tasks".into(),
                collection: "tasks".into(),
                data: serde_json::json!({}),
                version: 1,
                user_id: None,
                household_id: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let db = conn.lock().await;
        let (event_type, collection, document_id): (String, Option<String>, Option<String>) = db
            .query_row(
                "SELECT event_type, collection, document_id FROM audit_log LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();

        assert_eq!(event_type, "system.storage.updated");
        assert_eq!(collection.as_deref(), Some("tasks"));
        assert_eq!(document_id.as_deref(), Some("rec-1"));

        drop(db);
        handle.abort();
    }

    #[tokio::test]
    async fn subscriber_ignores_non_auditable_events() {
        let bus = MessageBus::new();
        let conn = std::sync::Arc::new(tokio::sync::Mutex::new(create_test_db()));
        let rx = bus.subscribe();

        let handle = spawn_audit_subscriber(rx, conn.clone());

        bus.publish(BusEvent::SyncComplete {
            plugin_id: "com.sync".into(),
        });
        bus.publish(BusEvent::NewRecords {
            collection: "tasks".into(),
            count: 5,
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let db = conn.lock().await;
        let count: i64 = db
            .query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0, "non-auditable events should not create log entries");

        drop(db);
        handle.abort();
    }

    #[tokio::test]
    async fn subscriber_logs_blob_stored() {
        let bus = MessageBus::new();
        let conn = std::sync::Arc::new(tokio::sync::Mutex::new(create_test_db()));
        let rx = bus.subscribe();

        let handle = spawn_audit_subscriber(rx, conn.clone());

        bus.publish(BusEvent::BlobStored {
            blob_key: "photos/img-001.jpg".into(),
            plugin_id: "com.photos".into(),
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let db = conn.lock().await;
        let (event_type, document_id, plugin_id): (String, Option<String>, Option<String>) = db
            .query_row(
                "SELECT event_type, document_id, plugin_id FROM audit_log LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();

        assert_eq!(event_type, "system.blob.stored");
        assert_eq!(document_id.as_deref(), Some("photos/img-001.jpg"));
        assert_eq!(plugin_id.as_deref(), Some("com.photos"));

        drop(db);
        handle.abort();
    }

    #[tokio::test]
    async fn subscriber_logs_blob_deleted() {
        let bus = MessageBus::new();
        let conn = std::sync::Arc::new(tokio::sync::Mutex::new(create_test_db()));
        let rx = bus.subscribe();

        let handle = spawn_audit_subscriber(rx, conn.clone());

        bus.publish(BusEvent::BlobDeleted {
            blob_key: "photos/img-001.jpg".into(),
            plugin_id: "com.photos".into(),
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let db = conn.lock().await;
        let event_type: String = db
            .query_row(
                "SELECT event_type FROM audit_log LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(event_type, "system.blob.deleted");

        drop(db);
        handle.abort();
    }

    #[tokio::test]
    async fn subscriber_logs_auth_success() {
        let bus = MessageBus::new();
        let conn = std::sync::Arc::new(tokio::sync::Mutex::new(create_test_db()));
        let rx = bus.subscribe();

        let handle = spawn_audit_subscriber(rx, conn.clone());

        bus.publish(BusEvent::AuthSuccess {
            identity_subject: "token-abc".into(),
            method: "bearer_token".into(),
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let db = conn.lock().await;
        let (event_type, identity_subject): (String, Option<String>) = db
            .query_row(
                "SELECT event_type, identity_subject FROM audit_log LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(event_type, "auth_success");
        assert_eq!(identity_subject.as_deref(), Some("token-abc"));

        drop(db);
        handle.abort();
    }

    #[tokio::test]
    async fn subscriber_logs_auth_failure() {
        let bus = MessageBus::new();
        let conn = std::sync::Arc::new(tokio::sync::Mutex::new(create_test_db()));
        let rx = bus.subscribe();

        let handle = spawn_audit_subscriber(rx, conn.clone());

        bus.publish(BusEvent::AuthFailure {
            client_ip: "192.168.1.100".into(),
            reason: "token_expired".into(),
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let db = conn.lock().await;
        let (event_type, details): (String, Option<String>) = db
            .query_row(
                "SELECT event_type, details FROM audit_log LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(event_type, "auth_failure");
        let details: serde_json::Value = serde_json::from_str(&details.unwrap()).unwrap();
        assert_eq!(details["client_ip"], "192.168.1.100");
        assert_eq!(details["reason"], "token_expired");

        drop(db);
        handle.abort();
    }

    #[tokio::test]
    async fn subscriber_logs_credential_access() {
        let bus = MessageBus::new();
        let conn = std::sync::Arc::new(tokio::sync::Mutex::new(create_test_db()));
        let rx = bus.subscribe();

        let handle = spawn_audit_subscriber(rx, conn.clone());

        bus.publish(BusEvent::CredentialEvent {
            action: "access".into(),
            plugin_id: "com.email".into(),
            key: "imap_password".into(),
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let db = conn.lock().await;
        let (event_type, plugin_id, details): (String, Option<String>, Option<String>) = db
            .query_row(
                "SELECT event_type, plugin_id, details FROM audit_log LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();

        assert_eq!(event_type, "credential_access");
        assert_eq!(plugin_id.as_deref(), Some("com.email"));
        let details: serde_json::Value = serde_json::from_str(&details.unwrap()).unwrap();
        assert_eq!(details["key"], "imap_password");

        drop(db);
        handle.abort();
    }

    #[tokio::test]
    async fn subscriber_logs_credential_modify() {
        let bus = MessageBus::new();
        let conn = std::sync::Arc::new(tokio::sync::Mutex::new(create_test_db()));
        let rx = bus.subscribe();

        let handle = spawn_audit_subscriber(rx, conn.clone());

        bus.publish(BusEvent::CredentialEvent {
            action: "modify".into(),
            plugin_id: "com.email".into(),
            key: "smtp_password".into(),
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let db = conn.lock().await;
        let event_type: String = db
            .query_row(
                "SELECT event_type FROM audit_log LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(event_type, "credential_modify");

        drop(db);
        handle.abort();
    }

    #[tokio::test]
    async fn subscriber_logs_plugin_unloaded() {
        let bus = MessageBus::new();
        let conn = std::sync::Arc::new(tokio::sync::Mutex::new(create_test_db()));
        let rx = bus.subscribe();

        let handle = spawn_audit_subscriber(rx, conn.clone());

        bus.publish(BusEvent::PluginUnloaded {
            plugin_id: "com.test.removed".into(),
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let db = conn.lock().await;
        let (event_type, plugin_id): (String, Option<String>) = db
            .query_row(
                "SELECT event_type, plugin_id FROM audit_log LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(event_type, "plugin_disable");
        assert_eq!(plugin_id.as_deref(), Some("com.test.removed"));

        drop(db);
        handle.abort();
    }

    #[tokio::test]
    async fn subscriber_logs_connector_event() {
        let bus = MessageBus::new();
        let conn = std::sync::Arc::new(tokio::sync::Mutex::new(create_test_db()));
        let rx = bus.subscribe();

        let handle = spawn_audit_subscriber(rx, conn.clone());

        bus.publish(BusEvent::ConnectorEvent {
            action: "auth".into(),
            plugin_id: "com.email.imap".into(),
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let db = conn.lock().await;
        let (event_type, plugin_id): (String, Option<String>) = db
            .query_row(
                "SELECT event_type, plugin_id FROM audit_log LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(event_type, "connector_auth");
        assert_eq!(plugin_id.as_deref(), Some("com.email.imap"));

        drop(db);
        handle.abort();
    }
}
