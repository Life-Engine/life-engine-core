//! Search processor — subscribes to the message bus and indexes records.
//!
//! Decouples search indexing from the data route handlers by listening
//! for `RecordChanged` and `RecordDeleted` events on the bus.

use crate::message_bus::{BusEvent, MessageBus};
use crate::search::SearchEngine;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, warn};

/// Spawn a background task that subscribes to the message bus and
/// indexes records in the search engine.
///
/// Returns a `JoinHandle` that can be used to await shutdown.
pub fn spawn(
    bus: &Arc<MessageBus>,
    engine: Arc<SearchEngine>,
) -> tokio::task::JoinHandle<()> {
    let mut rx = bus.subscribe();

    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(BusEvent::RecordChanged { record }) => {
                    // Remove any previous version, then index the new one.
                    if let Err(e) = engine.remove(&record.id).await {
                        debug!(error = %e, record_id = %record.id, "remove before re-index (may be first index)");
                    }
                    if let Err(e) = engine.index_record(&record).await {
                        warn!(error = %e, record_id = %record.id, "search indexing failed");
                    }
                }
                Ok(BusEvent::RecordDeleted { record_id }) => {
                    if let Err(e) = engine.remove(&record_id).await {
                        warn!(error = %e, record_id = %record_id, "search removal failed");
                    }
                }
                Ok(_) => {
                    // Ignore other event types.
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!(skipped = n, "search processor lagged behind message bus");
                }
                Err(broadcast::error::RecvError::Closed) => {
                    debug!("message bus closed, search processor shutting down");
                    break;
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Record;
    use chrono::Utc;
    use serde_json::json;

    fn test_record(id: &str, collection: &str) -> Record {
        Record {
            id: id.into(),
            plugin_id: "core".into(),
            collection: collection.into(),
            data: json!({ "title": format!("Test {id}") }),
            version: 1,
            user_id: None,
            household_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn indexes_record_on_record_changed_event() {
        let bus = Arc::new(MessageBus::new());
        let engine = Arc::new(SearchEngine::new().unwrap());

        let _handle = spawn(&bus, Arc::clone(&engine));

        let record = test_record("r1", "tasks");
        bus.publish(BusEvent::RecordChanged {
            record: record.clone(),
        });

        // Give the background task time to process.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let results = engine.search("Test r1", None, 10, 0).unwrap();
        assert_eq!(results.total, 1);
        assert_eq!(results.hits[0].id, "r1");
    }

    #[tokio::test]
    async fn removes_record_on_record_deleted_event() {
        let bus = Arc::new(MessageBus::new());
        let engine = Arc::new(SearchEngine::new().unwrap());

        // Index a record first.
        let record = test_record("r2", "tasks");
        engine.index_record(&record).await.unwrap();

        let _handle = spawn(&bus, Arc::clone(&engine));

        bus.publish(BusEvent::RecordDeleted {
            record_id: "r2".into(),
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let results = engine.search("Test r2", None, 10, 0).unwrap();
        assert_eq!(results.total, 0);
    }

    #[tokio::test]
    async fn re_indexes_on_update() {
        let bus = Arc::new(MessageBus::new());
        let engine = Arc::new(SearchEngine::new().unwrap());

        // Index original record.
        let record = test_record("r3", "notes");
        engine.index_record(&record).await.unwrap();

        let _handle = spawn(&bus, Arc::clone(&engine));

        // Publish updated record.
        let mut updated = record;
        updated.data = json!({ "title": "Updated title" });
        updated.version = 2;
        bus.publish(BusEvent::RecordChanged { record: updated });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let results = engine.search("Updated title", None, 10, 0).unwrap();
        assert_eq!(results.total, 1);
        assert_eq!(results.hits[0].id, "r3");

        // Old content should not match.
        let old_results = engine.search("Test r3", None, 10, 0).unwrap();
        assert_eq!(old_results.total, 0);
    }

    #[tokio::test]
    async fn ignores_unrelated_events() {
        let bus = Arc::new(MessageBus::new());
        let engine = Arc::new(SearchEngine::new().unwrap());

        let _handle = spawn(&bus, Arc::clone(&engine));

        // Publish non-record events.
        bus.publish(BusEvent::NewRecords {
            collection: "tasks".into(),
            count: 5,
        });
        bus.publish(BusEvent::SyncComplete {
            plugin_id: "core".into(),
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Search engine should be empty.
        let results = engine.search("anything", None, 10, 0);
        assert!(results.is_err() || results.unwrap().total == 0);
    }
}
