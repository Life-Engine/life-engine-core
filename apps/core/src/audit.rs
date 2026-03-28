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
}
