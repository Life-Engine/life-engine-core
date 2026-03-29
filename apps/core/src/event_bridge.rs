//! Bridges between the Core `MessageBus` and the workflow-engine `EventBus`.
//!
//! The system has two event buses that evolved independently:
//!
//! - `MessageBus` (apps/core) — broadcasts `BusEvent` variants for record
//!   changes, plugin lifecycle, and sync completion.
//! - `EventBus` (workflow-engine) — broadcasts structured `Event` records that
//!   trigger workflow execution via the `TriggerRegistry`.
//!
//! This module creates background tasks that forward relevant events between
//! the two buses so that:
//!
//! 1. Storage mutations published through `MessageBus` can trigger workflows.
//! 2. Plugin events emitted through `EventBus` reach Core subscribers (audit,
//!    search indexing).

use std::sync::Arc;

use chrono::Utc;
use life_engine_workflow_engine::event_bus::{Event, EventBus};
use tokio::sync::broadcast;
use tracing::{debug, error, warn};

use crate::message_bus::{BusEvent, MessageBus};

/// Spawns a background task that forwards storage-mutation events from the
/// Core `MessageBus` to the workflow-engine `EventBus`.
///
/// Mapped events:
///
/// - `BusEvent::NewRecords` -> `system.storage.new_records`
/// - `BusEvent::RecordChanged` -> `system.storage.record_changed`
/// - `BusEvent::RecordDeleted` -> `system.storage.record_deleted`
/// - `BusEvent::SyncComplete` -> `system.sync.complete`
/// - `BusEvent::PluginLoaded` -> `system.plugin.loaded`
/// - `BusEvent::PluginError` -> `system.plugin.failed`
pub fn spawn_message_bus_to_event_bus(
    message_bus: &MessageBus,
    event_bus: Arc<EventBus>,
) {
    let mut rx = message_bus.subscribe();

    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(bus_event) => {
                    let event = match bus_event_to_event(bus_event) {
                        Some(e) => e,
                        None => continue,
                    };
                    debug!(
                        event_name = %event.name,
                        "forwarding MessageBus event to EventBus"
                    );
                    event_bus.emit(event).await;
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!(
                        skipped = n,
                        "MessageBus-to-EventBus bridge lagged, {} events dropped", n
                    );
                }
                Err(broadcast::error::RecvError::Closed) => {
                    debug!("MessageBus closed, stopping bridge task");
                    break;
                }
            }
        }
    });
}

/// Spawns a background task that forwards workflow-engine `EventBus` events
/// back to the Core `MessageBus` as `BusEvent` variants.
///
/// Only plugin lifecycle and sync events are forwarded; storage events are
/// not re-published to avoid infinite loops (they originated from Core).
pub fn spawn_event_bus_to_message_bus(
    event_bus: &EventBus,
    message_bus: Arc<MessageBus>,
) {
    let mut rx = event_bus.subscribe();

    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let bus_event = match event_to_bus_event(&event) {
                        Some(e) => e,
                        None => continue,
                    };
                    debug!(
                        event_name = %event.name,
                        "forwarding EventBus event to MessageBus"
                    );
                    message_bus.publish(bus_event);
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!(
                        skipped = n,
                        "EventBus-to-MessageBus bridge lagged, {} events dropped", n
                    );
                }
                Err(broadcast::error::RecvError::Closed) => {
                    debug!("EventBus closed, stopping bridge task");
                    break;
                }
            }
        }
    });
}

/// Convert a Core `BusEvent` into a workflow-engine `Event`.
fn bus_event_to_event(bus_event: BusEvent) -> Option<Event> {
    let (name, payload) = match bus_event {
        BusEvent::NewRecords { collection, count } => (
            "system.storage.new_records".to_string(),
            Some(serde_json::json!({ "collection": collection, "count": count })),
        ),
        BusEvent::RecordChanged { record } => (
            "system.storage.record_changed".to_string(),
            serde_json::to_value(&record).ok(),
        ),
        BusEvent::RecordDeleted {
            record_id,
            collection,
        } => (
            "system.storage.record_deleted".to_string(),
            Some(serde_json::json!({ "record_id": record_id, "collection": collection })),
        ),
        BusEvent::SyncComplete { plugin_id } => (
            "system.sync.complete".to_string(),
            Some(serde_json::json!({ "plugin_id": plugin_id })),
        ),
        BusEvent::PluginLoaded { plugin_id } => (
            "system.plugin.loaded".to_string(),
            Some(serde_json::json!({ "plugin_id": plugin_id })),
        ),
        BusEvent::PluginError { plugin_id, error } => (
            "system.plugin.failed".to_string(),
            Some(serde_json::json!({ "plugin_id": plugin_id, "error": error })),
        ),
        BusEvent::BlobStored {
            blob_key,
            plugin_id,
        } => (
            "system.blob.stored".to_string(),
            Some(serde_json::json!({ "blob_key": blob_key, "plugin_id": plugin_id })),
        ),
        BusEvent::BlobDeleted {
            blob_key,
            plugin_id,
        } => (
            "system.blob.deleted".to_string(),
            Some(serde_json::json!({ "blob_key": blob_key, "plugin_id": plugin_id })),
        ),
        // Security events are internal — not forwarded to workflow engine.
        BusEvent::AuthSuccess { .. }
        | BusEvent::AuthFailure { .. }
        | BusEvent::CredentialEvent { .. }
        | BusEvent::PluginUnloaded { .. }
        | BusEvent::ConnectorEvent { .. } => return None,
    };

    Some(Event {
        name,
        payload,
        source: "system".to_string(),
        timestamp: Utc::now(),
        depth: 0,
    })
}

/// Convert a workflow-engine `Event` into a Core `BusEvent`, if applicable.
///
/// Returns `None` for events that should not be forwarded (to avoid loops).
fn event_to_bus_event(event: &Event) -> Option<BusEvent> {
    // Skip storage and blob events — they originated from Core and forwarding
    // them back would create an infinite loop.
    if event.name.starts_with("system.storage.") || event.name.starts_with("system.blob.") {
        return None;
    }

    match event.name.as_str() {
        "system.plugin.loaded" => {
            let plugin_id = event
                .payload
                .as_ref()
                .and_then(|p| p["plugin_id"].as_str())
                .unwrap_or("unknown")
                .to_string();
            Some(BusEvent::PluginLoaded { plugin_id })
        }
        "system.plugin.failed" => {
            let plugin_id = event
                .payload
                .as_ref()
                .and_then(|p| p["plugin_id"].as_str())
                .unwrap_or("unknown")
                .to_string();
            let error = event
                .payload
                .as_ref()
                .and_then(|p| p["error"].as_str())
                .unwrap_or("unknown error")
                .to_string();
            Some(BusEvent::PluginError { plugin_id, error })
        }
        "system.sync.complete" => {
            let plugin_id = event
                .payload
                .as_ref()
                .and_then(|p| p["plugin_id"].as_str())
                .unwrap_or("unknown")
                .to_string();
            Some(BusEvent::SyncComplete { plugin_id })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_records_converts_to_event() {
        let bus_event = BusEvent::NewRecords {
            collection: "tasks".to_string(),
            count: 5,
        };
        let event = bus_event_to_event(bus_event).unwrap();
        assert_eq!(event.name, "system.storage.new_records");
        assert_eq!(event.source, "system");
        assert_eq!(event.depth, 0);
        let payload = event.payload.unwrap();
        assert_eq!(payload["collection"], "tasks");
        assert_eq!(payload["count"], 5);
    }

    #[test]
    fn record_deleted_converts_to_event() {
        let bus_event = BusEvent::RecordDeleted {
            record_id: "r-123".to_string(),
            collection: "notes".to_string(),
        };
        let event = bus_event_to_event(bus_event).unwrap();
        assert_eq!(event.name, "system.storage.record_deleted");
        let payload = event.payload.unwrap();
        assert_eq!(payload["record_id"], "r-123");
        assert_eq!(payload["collection"], "notes");
    }

    #[test]
    fn plugin_loaded_converts_to_event() {
        let bus_event = BusEvent::PluginLoaded {
            plugin_id: "com.test.plugin".to_string(),
        };
        let event = bus_event_to_event(bus_event).unwrap();
        assert_eq!(event.name, "system.plugin.loaded");
        let payload = event.payload.unwrap();
        assert_eq!(payload["plugin_id"], "com.test.plugin");
    }

    #[test]
    fn plugin_error_converts_to_event() {
        let bus_event = BusEvent::PluginError {
            plugin_id: "bad-plugin".to_string(),
            error: "segfault".to_string(),
        };
        let event = bus_event_to_event(bus_event).unwrap();
        assert_eq!(event.name, "system.plugin.failed");
        let payload = event.payload.unwrap();
        assert_eq!(payload["plugin_id"], "bad-plugin");
        assert_eq!(payload["error"], "segfault");
    }

    #[test]
    fn sync_complete_converts_to_event() {
        let bus_event = BusEvent::SyncComplete {
            plugin_id: "connector-email".to_string(),
        };
        let event = bus_event_to_event(bus_event).unwrap();
        assert_eq!(event.name, "system.sync.complete");
    }

    #[test]
    fn storage_events_not_forwarded_to_message_bus() {
        let event = Event {
            name: "system.storage.new_records".to_string(),
            payload: Some(serde_json::json!({"collection": "tasks", "count": 3})),
            source: "system".to_string(),
            timestamp: Utc::now(),
            depth: 0,
        };
        assert!(event_to_bus_event(&event).is_none());
    }

    #[test]
    fn plugin_loaded_event_forwards_to_message_bus() {
        let event = Event {
            name: "system.plugin.loaded".to_string(),
            payload: Some(serde_json::json!({"plugin_id": "connector-email"})),
            source: "system".to_string(),
            timestamp: Utc::now(),
            depth: 0,
        };
        let bus_event = event_to_bus_event(&event).unwrap();
        match bus_event {
            BusEvent::PluginLoaded { plugin_id } => {
                assert_eq!(plugin_id, "connector-email");
            }
            _ => panic!("expected PluginLoaded"),
        }
    }

    #[test]
    fn plugin_failed_event_forwards_to_message_bus() {
        let event = Event {
            name: "system.plugin.failed".to_string(),
            payload: Some(serde_json::json!({"plugin_id": "bad", "error": "crash"})),
            source: "system".to_string(),
            timestamp: Utc::now(),
            depth: 0,
        };
        let bus_event = event_to_bus_event(&event).unwrap();
        match bus_event {
            BusEvent::PluginError { plugin_id, error } => {
                assert_eq!(plugin_id, "bad");
                assert_eq!(error, "crash");
            }
            _ => panic!("expected PluginError"),
        }
    }

    #[test]
    fn unknown_event_not_forwarded_to_message_bus() {
        let event = Event {
            name: "custom.plugin.event".to_string(),
            payload: None,
            source: "some-plugin".to_string(),
            timestamp: Utc::now(),
            depth: 1,
        };
        assert!(event_to_bus_event(&event).is_none());
    }
}
