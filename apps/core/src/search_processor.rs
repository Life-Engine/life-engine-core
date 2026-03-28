//! Search processor — subscribes to the message bus and indexes records.
//!
//! Decouples search indexing from the data route handlers by listening
//! for `RecordChanged` and `RecordDeleted` events on the bus.

use crate::message_bus::{BusEvent, MessageBus};
use crate::search::SearchEngine;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

/// Spawn a background task that subscribes to the message bus and
/// indexes records in the search engine.
///
/// Accepts an optional `shutdown` receiver. When the sender half is
/// dropped or a value is sent, the processor drains remaining bus
/// events, flushes pending index operations, and exits.
///
/// Returns a `JoinHandle` that can be used to await shutdown.
pub fn spawn(
    bus: &Arc<MessageBus>,
    engine: Arc<SearchEngine>,
) -> tokio::task::JoinHandle<()> {
    spawn_with_shutdown(bus, engine, None)
}

/// Like [`spawn`], but accepts a shutdown signal via a `watch` receiver.
///
/// When the watch value becomes `true`, the processor will finish
/// processing any already-received event and then exit cleanly.
pub fn spawn_with_shutdown(
    bus: &Arc<MessageBus>,
    engine: Arc<SearchEngine>,
    shutdown: Option<tokio::sync::watch::Receiver<bool>>,
) -> tokio::task::JoinHandle<()> {
    let mut rx = bus.subscribe();

    tokio::spawn(async move {
        let mut shutdown_rx = shutdown;

        loop {
            // If a shutdown receiver was provided, race it against the
            // next bus event so we can exit promptly.
            let event = if let Some(ref mut srx) = shutdown_rx {
                tokio::select! {
                    biased;
                    result = rx.recv() => result,
                    _ = srx.changed() => {
                        if *srx.borrow() {
                            info!("search processor received shutdown signal, draining");
                            break;
                        }
                        continue;
                    }
                }
            } else {
                rx.recv().await
            };

            match event {
                Ok(BusEvent::RecordChanged { record }) => {
                    // Remove any previous version, then index the new one.
                    if let Err(e) = engine.remove(&record.id).await {
                        debug!(error = %e, record_id = %record.id, "remove before re-index (may be first index)");
                    }
                    if let Err(e) = engine.index_record(&record).await {
                        warn!(error = %e, record_id = %record.id, "search indexing failed");
                    }
                }
                Ok(BusEvent::RecordDeleted { record_id, .. }) => {
                    if let Err(e) = engine.remove(&record_id).await {
                        warn!(error = %e, record_id = %record_id, "search removal failed");
                    }
                }
                Ok(_) => {
                    // Ignore other event types.
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!(
                        skipped = n,
                        "search processor lagged behind message bus — {n} messages were \
                         dropped and those records will not be indexed until their next update"
                    );
                }
                Err(broadcast::error::RecvError::Closed) => {
                    debug!("message bus closed, search processor shutting down");
                    break;
                }
            }
        }

        // Drain any remaining events from the bus before exiting.
        loop {
            match rx.try_recv() {
                Ok(BusEvent::RecordChanged { record }) => {
                    if let Err(e) = engine.remove(&record.id).await {
                        debug!(error = %e, record_id = %record.id, "drain: remove before re-index");
                    }
                    if let Err(e) = engine.index_record(&record).await {
                        warn!(error = %e, record_id = %record.id, "drain: search indexing failed");
                    }
                }
                Ok(BusEvent::RecordDeleted { record_id, .. }) => {
                    if let Err(e) = engine.remove(&record_id).await {
                        warn!(error = %e, record_id = %record_id, "drain: search removal failed");
                    }
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }

        // Flush any buffered documents that haven't reached the commit threshold.
        if let Err(e) = engine.flush().await {
            warn!(error = %e, "failed to flush search index on shutdown");
        }

        info!("search processor shut down cleanly");
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
        let engine = Arc::new(SearchEngine::with_commit_threshold(1).unwrap());

        let _handle = spawn(&bus, Arc::clone(&engine));

        let record = test_record("r1", "tasks");
        bus.publish(BusEvent::RecordChanged {
            record: record.clone(),
        });

        // Retry loop: the background task may need time to process the event,
        // especially on slow CI runners. Retry for up to 2 seconds.
        let mut results = engine.search("Test r1", None, None, None, 10, 0).unwrap();
        for _ in 0..20 {
            if results.total >= 1 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            results = engine.search("Test r1", None, None, None, 10, 0).unwrap();
        }
        assert_eq!(results.total, 1);
        assert_eq!(results.hits[0].id, "r1");
    }

    #[tokio::test]
    async fn removes_record_on_record_deleted_event() {
        let bus = Arc::new(MessageBus::new());
        let engine = Arc::new(SearchEngine::with_commit_threshold(1).unwrap());

        // Index a record first.
        let record = test_record("r2", "tasks");
        engine.index_record(&record).await.unwrap();

        let _handle = spawn(&bus, Arc::clone(&engine));

        bus.publish(BusEvent::RecordDeleted {
            record_id: "r2".into(),
            collection: "test".into(),
        });

        // Retry loop: wait for the background task to process the deletion.
        let mut results = engine.search("Test r2", None, None, None, 10, 0).unwrap();
        for _ in 0..20 {
            if results.total == 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            results = engine.search("Test r2", None, None, None, 10, 0).unwrap();
        }
        assert_eq!(results.total, 0);
    }

    #[tokio::test]
    async fn re_indexes_on_update() {
        let bus = Arc::new(MessageBus::new());
        let engine = Arc::new(SearchEngine::with_commit_threshold(1).unwrap());

        // Index original record.
        let record = test_record("r3", "notes");
        engine.index_record(&record).await.unwrap();

        let _handle = spawn(&bus, Arc::clone(&engine));

        // Publish updated record.
        let mut updated = record;
        updated.data = json!({ "title": "Updated title" });
        updated.version = 2;
        bus.publish(BusEvent::RecordChanged { record: updated });

        // Retry loop: wait for the background task to re-index.
        let mut results = engine.search("Updated title", None, None, None, 10, 0).unwrap();
        for _ in 0..20 {
            if results.total >= 1 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            results = engine.search("Updated title", None, None, None, 10, 0).unwrap();
        }
        assert_eq!(results.total, 1);
        assert_eq!(results.hits[0].id, "r3");

        // Old content should not match.
        let old_results = engine.search("Test r3", None, None, None, 10, 0).unwrap();
        assert_eq!(old_results.total, 0);
    }

    #[tokio::test]
    async fn ignores_unrelated_events() {
        let bus = Arc::new(MessageBus::new());
        let engine = Arc::new(SearchEngine::with_commit_threshold(1).unwrap());

        let _handle = spawn(&bus, Arc::clone(&engine));

        // Publish non-record events.
        bus.publish(BusEvent::NewRecords {
            collection: "tasks".into(),
            count: 5,
        });
        bus.publish(BusEvent::SyncComplete {
            plugin_id: "core".into(),
        });

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Search engine should be empty.
        let results = engine.search("anything", None, None, None, 10, 0);
        assert!(results.is_err() || results.unwrap().total == 0);
    }

    #[tokio::test]
    async fn graceful_shutdown_via_watch() {
        let bus = Arc::new(MessageBus::new());
        let engine = Arc::new(SearchEngine::with_commit_threshold(1).unwrap());

        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let handle = spawn_with_shutdown(&bus, Arc::clone(&engine), Some(shutdown_rx));

        // Index a record to verify the processor is working.
        let record = test_record("s1", "tasks");
        bus.publish(BusEvent::RecordChanged {
            record: record.clone(),
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let results = engine.search("Test s1", None, None, None, 10, 0).unwrap();
        assert_eq!(results.total, 1);

        // Signal shutdown.
        shutdown_tx.send(true).unwrap();

        // The task should complete promptly.
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            handle,
        )
        .await;
        assert!(result.is_ok(), "search processor did not shut down in time");
    }
}
