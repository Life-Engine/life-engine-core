//! In-process message bus using `tokio::sync::broadcast` channels.
//!
//! Provides publish/subscribe event delivery to plugins that declare
//! the `EventsSubscribe` capability.

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// Default channel capacity for the event bus.
const DEFAULT_CAPACITY: usize = 256;

/// Events emitted on the Core message bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BusEvent {
    /// New records were created in a collection.
    NewRecords {
        /// The collection name.
        collection: String,
        /// Number of new records.
        count: usize,
    },

    /// A record was created or updated and should be indexed.
    RecordChanged {
        /// The record to index.
        record: crate::storage::Record,
    },

    /// A record was deleted and should be removed from the index.
    RecordDeleted {
        /// The deleted record's ID.
        record_id: String,
    },

    /// A plugin completed a sync operation.
    SyncComplete {
        /// The plugin that finished syncing.
        plugin_id: String,
    },

    /// A plugin was loaded successfully.
    PluginLoaded {
        /// The plugin that was loaded.
        plugin_id: String,
    },

    /// A plugin encountered an error.
    PluginError {
        /// The plugin that errored.
        plugin_id: String,
        /// Description of the error.
        error: String,
    },
}

/// The message bus for broadcasting events across Core subsystems and plugins.
#[derive(Debug)]
pub struct MessageBus {
    sender: broadcast::Sender<BusEvent>,
}

impl MessageBus {
    /// Create a new message bus with the default channel capacity.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    /// Create a new message bus with the specified channel capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Publish an event to all subscribers.
    ///
    /// Returns the number of receivers that received the event, or 0
    /// if there are no active subscribers.
    pub fn publish(&self, event: BusEvent) -> usize {
        // `send` returns Err if there are no receivers. That is not
        // an error for us — events are fire-and-forget.
        self.sender.send(event).unwrap_or(0)
    }

    /// Subscribe to events on this bus.
    ///
    /// Returns a receiver that yields cloned events as they are published.
    pub fn subscribe(&self) -> broadcast::Receiver<BusEvent> {
        self.sender.subscribe()
    }

    /// Returns the number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for MessageBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_bus_has_no_subscribers() {
        let bus = MessageBus::new();
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[test]
    fn publish_with_no_subscribers_returns_zero() {
        let bus = MessageBus::new();
        let count = bus.publish(BusEvent::SyncComplete {
            plugin_id: "test".into(),
        });
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn single_subscriber_receives_event() {
        let bus = MessageBus::new();
        let mut rx = bus.subscribe();

        bus.publish(BusEvent::PluginLoaded {
            plugin_id: "com.test.plugin".into(),
        });

        let event = rx.recv().await.expect("should receive event");
        match event {
            BusEvent::PluginLoaded { plugin_id } => {
                assert_eq!(plugin_id, "com.test.plugin");
            }
            _ => panic!("unexpected event variant"),
        }
    }

    #[tokio::test]
    async fn multiple_subscribers_receive_events() {
        let bus = MessageBus::new();
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        assert_eq!(bus.subscriber_count(), 2);

        let sent = bus.publish(BusEvent::NewRecords {
            collection: "tasks".into(),
            count: 5,
        });
        assert_eq!(sent, 2);

        let e1 = rx1.recv().await.expect("rx1 should receive");
        let e2 = rx2.recv().await.expect("rx2 should receive");

        match (&e1, &e2) {
            (
                BusEvent::NewRecords {
                    collection: c1,
                    count: n1,
                },
                BusEvent::NewRecords {
                    collection: c2,
                    count: n2,
                },
            ) => {
                assert_eq!(c1, "tasks");
                assert_eq!(c2, "tasks");
                assert_eq!(*n1, 5);
                assert_eq!(*n2, 5);
            }
            _ => panic!("unexpected event variants"),
        }
    }

    #[tokio::test]
    async fn plugin_error_event() {
        let bus = MessageBus::new();
        let mut rx = bus.subscribe();

        bus.publish(BusEvent::PluginError {
            plugin_id: "com.test.bad".into(),
            error: "segfault".into(),
        });

        let event = rx.recv().await.expect("should receive event");
        match event {
            BusEvent::PluginError { plugin_id, error } => {
                assert_eq!(plugin_id, "com.test.bad");
                assert_eq!(error, "segfault");
            }
            _ => panic!("unexpected event variant"),
        }
    }

    #[test]
    fn default_impl_works() {
        let bus = MessageBus::default();
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[test]
    fn custom_capacity() {
        let bus = MessageBus::with_capacity(16);
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[tokio::test]
    async fn sync_complete_event() {
        let bus = MessageBus::new();
        let mut rx = bus.subscribe();

        bus.publish(BusEvent::SyncComplete {
            plugin_id: "com.test.sync".into(),
        });

        let event = rx.recv().await.expect("should receive");
        match event {
            BusEvent::SyncComplete { plugin_id } => {
                assert_eq!(plugin_id, "com.test.sync");
            }
            _ => panic!("unexpected event variant"),
        }
    }
}
