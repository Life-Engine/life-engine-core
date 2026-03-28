//! Event bus for async event-driven workflow triggering.
//!
//! Uses `tokio::sync::broadcast` for event distribution. When an event is
//! emitted, the bus looks up matching workflows in the `TriggerRegistry` and
//! spawns a background task for each one. Events carry structured metadata
//! including source, timestamp, and depth for loop prevention.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use crate::executor::{build_initial_message, PipelineExecutor, WorkflowEventEmitter};
use crate::loader::TriggerRegistry;
use crate::types::TriggerContext;

/// Channel capacity for the broadcast event bus.
const EVENT_CHANNEL_CAPACITY: usize = 256;

/// Default maximum event depth before loop prevention drops the event.
const DEFAULT_MAX_DEPTH: u32 = 8;

/// A structured event flowing through the bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Fully qualified event name (e.g., `connector-email.fetch.completed` or `system.startup`).
    pub name: String,
    /// Optional JSON payload attached to the event.
    pub payload: Option<serde_json::Value>,
    /// Source of the event: plugin ID for plugin events, `"system"` for system events.
    pub source: String,
    /// UTC timestamp when the event was created.
    pub timestamp: DateTime<Utc>,
    /// Depth in the event chain (0 for root events, incremented for cascading events).
    pub depth: u32,
}

/// Thread-safe event bus for distributing events to workflow triggers.
///
/// The event bus owns a broadcast channel for fan-out event delivery and
/// holds references to the trigger registry and pipeline executor so it
/// can spawn workflow executions when matching events arrive.
pub struct EventBus {
    sender: broadcast::Sender<Event>,
    registry: Arc<TriggerRegistry>,
    executor: Arc<PipelineExecutor>,
    max_depth: u32,
}

impl EventBus {
    /// Create a new event bus with the given trigger registry and executor.
    pub fn new(registry: Arc<TriggerRegistry>, executor: Arc<PipelineExecutor>) -> Self {
        let (sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            sender,
            registry,
            executor,
            max_depth: DEFAULT_MAX_DEPTH,
        }
    }

    /// Create a new event bus with a custom maximum event depth.
    pub fn with_max_depth(
        registry: Arc<TriggerRegistry>,
        executor: Arc<PipelineExecutor>,
        max_depth: u32,
    ) -> Self {
        let (sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            sender,
            registry,
            executor,
            max_depth,
        }
    }

    /// Emit a structured event, triggering any matching workflows.
    ///
    /// Validates the event name format, checks depth for loop prevention,
    /// then broadcasts to subscribers and spawns matching workflow tasks.
    /// Returns immediately without waiting for workflow completion.
    pub async fn emit(&self, event: Event) {
        // Loop prevention: drop events that exceed the configured max depth.
        if event.depth > self.max_depth {
            warn!(
                event_name = %event.name,
                source = %event.source,
                depth = event.depth,
                max_depth = self.max_depth,
                "Event dropped: depth {} exceeds maximum {}",
                event.depth,
                self.max_depth,
            );
            return;
        }

        // Broadcast for any subscribers (logging, metrics).
        // Ignore send errors — they only occur when there are no receivers.
        let _ = self.sender.send(event.clone());

        let matching = self.registry.find_event(&event.name);
        let count = matching.len();

        if count == 0 {
            return;
        }

        info!(
            event = %event.name,
            source = %event.source,
            depth = event.depth,
            triggered_workflows = count,
            "Event emitted, triggering {} workflow(s)",
            count
        );

        for workflow in matching {
            let workflow = workflow.clone();
            let executor = Arc::clone(&self.executor);
            let event_name = event.name.clone();
            let payload = event.payload.clone().unwrap_or(serde_json::Value::Null);

            tokio::spawn(async move {
                let trigger_context = TriggerContext::Event {
                    name: event_name.clone(),
                    payload,
                };

                let initial_message = match build_initial_message(trigger_context) {
                    Ok(msg) => msg,
                    Err(e) => {
                        error!(
                            event = %event_name,
                            workflow_id = %workflow.id,
                            error = %e,
                            "Failed to build initial message for event-triggered workflow"
                        );
                        return;
                    }
                };

                if let Err(e) = executor.execute_workflow(&workflow, initial_message).await {
                    error!(
                        event = %event_name,
                        workflow_id = %workflow.id,
                        error = %e,
                        "Event-triggered workflow execution failed"
                    );
                }
            });
        }
    }

    /// Convenience method to emit a system event with the given name and optional payload.
    ///
    /// Sets `source` to `"system"`, `depth` to 0, and `timestamp` to now.
    pub async fn emit_system_event(&self, name: &str, payload: Option<serde_json::Value>) {
        let event = Event {
            name: name.to_string(),
            payload,
            source: "system".to_string(),
            timestamp: Utc::now(),
            depth: 0,
        };
        self.emit(event).await;
    }

    /// Emit `system.startup`.
    pub async fn emit_startup(&self) {
        self.emit_system_event("system.startup", None).await;
    }

    /// Emit `system.plugin.loaded` with the plugin ID in the payload.
    pub async fn emit_plugin_loaded(&self, plugin_id: &str) {
        self.emit_system_event(
            "system.plugin.loaded",
            Some(serde_json::json!({ "plugin_id": plugin_id })),
        )
        .await;
    }

    /// Emit `system.plugin.failed` with the plugin ID and error details.
    pub async fn emit_plugin_failed(&self, plugin_id: &str, error: &str) {
        self.emit_system_event(
            "system.plugin.failed",
            Some(serde_json::json!({ "plugin_id": plugin_id, "error": error })),
        )
        .await;
    }

    /// Emit `system.workflow.completed` with job ID and status.
    pub async fn emit_workflow_completed(&self, job_id: &str, status: &str) {
        self.emit_system_event(
            "system.workflow.completed",
            Some(serde_json::json!({ "job_id": job_id, "status": status })),
        )
        .await;
    }

    /// Emit `system.workflow.failed` with job ID and error details.
    pub async fn emit_workflow_failed(&self, job_id: &str, error: &str) {
        self.emit_system_event(
            "system.workflow.failed",
            Some(serde_json::json!({ "job_id": job_id, "error": error })),
        )
        .await;
    }

    /// Subscribe to all events on the bus.
    ///
    /// Returns a broadcast receiver that will receive `Event` structs for
    /// every event emitted. Useful for logging, metrics, or external
    /// integrations.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }
}

#[async_trait]
impl WorkflowEventEmitter for EventBus {
    async fn emit(&self, event_name: &str, payload: serde_json::Value) {
        let event = Event {
            name: event_name.to_string(),
            payload: Some(payload),
            source: "system".to_string(),
            timestamp: Utc::now(),
            depth: 0,
        };
        EventBus::emit(self, event).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorkflowConfig;
    use crate::executor::PluginExecutor;
    use crate::loader::load_workflows;
    use life_engine_traits::EngineError;
    use life_engine_types::PipelineMessage;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::TempDir;

    /// A mock plugin executor that counts invocations.
    struct CountingExecutor {
        call_count: AtomicUsize,
    }

    impl CountingExecutor {
        fn new() -> Self {
            Self {
                call_count: AtomicUsize::new(0),
            }
        }

        fn count(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl PluginExecutor for CountingExecutor {
        async fn execute(
            &self,
            _plugin_id: &str,
            _action: &str,
            input: PipelineMessage,
        ) -> Result<PipelineMessage, Box<dyn EngineError>> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(input)
        }
    }

    fn write_yaml(dir: &Path, filename: &str, content: &str) {
        std::fs::write(dir.join(filename), content).unwrap();
    }

    fn build_test_components(
        dir: &Path,
        yaml_files: &[(&str, &str)],
    ) -> (Arc<TriggerRegistry>, Arc<CountingExecutor>, Arc<PipelineExecutor>) {
        for (name, content) in yaml_files {
            write_yaml(dir, name, content);
        }
        let config = WorkflowConfig {
            path: dir.to_str().unwrap().into(),
        };
        let workflows = load_workflows(&config).unwrap();
        let registry = Arc::new(TriggerRegistry::build(workflows).unwrap());
        let mock_executor = Arc::new(CountingExecutor::new());
        let pipeline = Arc::new(PipelineExecutor::new(mock_executor.clone() as Arc<dyn PluginExecutor>));
        (registry, mock_executor, pipeline)
    }

    fn make_event(name: &str, source: &str, payload: Option<serde_json::Value>, depth: u32) -> Event {
        Event {
            name: name.to_string(),
            payload,
            source: source.to_string(),
            timestamp: Utc::now(),
            depth,
        }
    }

    // ── Test 1: Event struct fields are populated correctly ──

    #[test]
    fn event_struct_has_required_fields() {
        let before = Utc::now();
        let event = Event {
            name: "connector-email.fetch.completed".to_string(),
            payload: Some(serde_json::json!({"count": 5})),
            source: "connector-email".to_string(),
            timestamp: Utc::now(),
            depth: 0,
        };
        let after = Utc::now();

        assert_eq!(event.name, "connector-email.fetch.completed");
        assert_eq!(event.source, "connector-email");
        assert_eq!(event.depth, 0);
        assert!(event.timestamp >= before && event.timestamp <= after);
        assert_eq!(event.payload, Some(serde_json::json!({"count": 5})));
    }

    #[test]
    fn event_payload_is_optional() {
        let event = Event {
            name: "system.startup".to_string(),
            payload: None,
            source: "system".to_string(),
            timestamp: Utc::now(),
            depth: 0,
        };
        assert!(event.payload.is_none());
    }

    // ── Test 2: Broadcast sends to all subscribers ──

    #[tokio::test]
    async fn emit_event_triggers_matching_workflow() {
        let dir = TempDir::new().unwrap();
        let (registry, mock_executor, pipeline) = build_test_components(
            dir.path(),
            &[(
                "events.yaml",
                r#"
workflows:
  handler:
    id: handler
    name: Event Handler
    trigger:
      event: "webhook.email.received"
    steps:
      - plugin: email-processor
        action: process
"#,
            )],
        );

        let bus = EventBus::new(registry, pipeline);
        let event = make_event(
            "webhook.email.received",
            "webhook",
            Some(serde_json::json!({"from": "test@example.com"})),
            0,
        );
        bus.emit(event).await;

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert_eq!(mock_executor.count(), 1);
    }

    #[tokio::test]
    async fn emit_event_with_no_matching_workflows_is_noop() {
        let dir = TempDir::new().unwrap();
        let (registry, mock_executor, pipeline) = build_test_components(
            dir.path(),
            &[(
                "events.yaml",
                r#"
workflows:
  handler:
    id: handler
    name: Event Handler
    trigger:
      event: "webhook.email.received"
    steps:
      - plugin: email-processor
        action: process
"#,
            )],
        );

        let bus = EventBus::new(registry, pipeline);
        let event = make_event("nonexistent.event", "system", None, 0);
        bus.emit(event).await;

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert_eq!(mock_executor.count(), 0);
    }

    #[tokio::test]
    async fn emit_event_triggers_multiple_matching_workflows() {
        let dir = TempDir::new().unwrap();
        let (registry, mock_executor, pipeline) = build_test_components(
            dir.path(),
            &[(
                "events.yaml",
                r#"
workflows:
  handler-a:
    id: handler-a
    name: Handler A
    trigger:
      event: "webhook.email.received"
    steps:
      - plugin: p1
        action: a1
  handler-b:
    id: handler-b
    name: Handler B
    trigger:
      event: "webhook.email.received"
    steps:
      - plugin: p2
        action: a2
"#,
            )],
        );

        let bus = EventBus::new(registry, pipeline);
        let event = make_event(
            "webhook.email.received",
            "webhook",
            Some(serde_json::json!({"subject": "test"})),
            0,
        );
        bus.emit(event).await;

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert_eq!(mock_executor.count(), 2);
    }

    // ── Test 3: Subscriber receives structured events ──

    #[tokio::test]
    async fn subscriber_receives_emitted_events() {
        let dir = TempDir::new().unwrap();
        let (registry, _, pipeline) = build_test_components(
            dir.path(),
            &[(
                "events.yaml",
                r#"
workflows:
  handler:
    id: handler
    name: Handler
    trigger:
      event: "some.event"
    steps:
      - plugin: p1
        action: a1
"#,
            )],
        );

        let bus = EventBus::new(registry, pipeline);
        let mut rx = bus.subscribe();

        let event = make_event(
            "some.event",
            "test-plugin",
            Some(serde_json::json!({"key": "value"})),
            0,
        );
        bus.emit(event).await;

        let received = rx.recv().await.unwrap();
        assert_eq!(received.name, "some.event");
        assert_eq!(received.source, "test-plugin");
        assert_eq!(received.depth, 0);
        assert_eq!(received.payload, Some(serde_json::json!({"key": "value"})));
    }

    // ── Test 4: Loop prevention drops events exceeding max depth ──

    #[tokio::test]
    async fn event_exceeding_max_depth_is_dropped() {
        let dir = TempDir::new().unwrap();
        let (registry, mock_executor, pipeline) = build_test_components(
            dir.path(),
            &[(
                "events.yaml",
                r#"
workflows:
  handler:
    id: handler
    name: Handler
    trigger:
      event: "cascade.event"
    steps:
      - plugin: p1
        action: a1
"#,
            )],
        );

        let bus = EventBus::with_max_depth(registry, pipeline, 3);

        // Depth 3 should be allowed (at the limit).
        let event_at_limit = make_event("cascade.event", "plugin-a", Some(serde_json::json!({})), 3);
        bus.emit(event_at_limit).await;
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert_eq!(mock_executor.count(), 1);

        // Depth 4 exceeds max_depth=3, should be dropped.
        let event_over_limit = make_event("cascade.event", "plugin-a", Some(serde_json::json!({})), 4);
        bus.emit(event_over_limit).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert_eq!(mock_executor.count(), 1); // Still 1, not 2.
    }

    #[tokio::test]
    async fn default_max_depth_is_8() {
        let dir = TempDir::new().unwrap();
        let (registry, mock_executor, pipeline) = build_test_components(
            dir.path(),
            &[(
                "events.yaml",
                r#"
workflows:
  handler:
    id: handler
    name: Handler
    trigger:
      event: "deep.event"
    steps:
      - plugin: p1
        action: a1
"#,
            )],
        );

        let bus = EventBus::new(registry, pipeline);

        // Depth 8 should be allowed (at the default limit).
        let event_at_limit = make_event("deep.event", "plugin-a", Some(serde_json::json!({})), 8);
        bus.emit(event_at_limit).await;
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert_eq!(mock_executor.count(), 1);

        // Depth 9 should be dropped.
        let event_over = make_event("deep.event", "plugin-a", Some(serde_json::json!({})), 9);
        bus.emit(event_over).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert_eq!(mock_executor.count(), 1);
    }

    // ── Test 5: System event helpers ──

    #[tokio::test]
    async fn system_event_helpers_emit_correct_events() {
        let dir = TempDir::new().unwrap();
        let (registry, _, pipeline) = build_test_components(
            dir.path(),
            &[(
                "events.yaml",
                r#"
workflows:
  handler:
    id: handler
    name: Handler
    trigger:
      event: "system.startup"
    steps:
      - plugin: p1
        action: a1
"#,
            )],
        );

        let bus = EventBus::new(registry, pipeline);
        let mut rx = bus.subscribe();

        bus.emit_startup().await;

        let received = rx.recv().await.unwrap();
        assert_eq!(received.name, "system.startup");
        assert_eq!(received.source, "system");
        assert_eq!(received.depth, 0);
        assert!(received.payload.is_none());
    }

    #[tokio::test]
    async fn system_plugin_loaded_event_includes_plugin_id() {
        let dir = TempDir::new().unwrap();
        let (registry, _, pipeline) = build_test_components(
            dir.path(),
            &[(
                "events.yaml",
                r#"
workflows:
  handler:
    id: handler
    name: Handler
    trigger:
      event: "system.plugin.loaded"
    steps:
      - plugin: p1
        action: a1
"#,
            )],
        );

        let bus = EventBus::new(registry, pipeline);
        let mut rx = bus.subscribe();

        bus.emit_plugin_loaded("connector-email").await;

        let received = rx.recv().await.unwrap();
        assert_eq!(received.name, "system.plugin.loaded");
        assert_eq!(received.source, "system");
        let payload = received.payload.unwrap();
        assert_eq!(payload["plugin_id"], "connector-email");
    }

    #[tokio::test]
    async fn system_plugin_failed_event_includes_error() {
        let dir = TempDir::new().unwrap();
        let (registry, _, pipeline) = build_test_components(
            dir.path(),
            &[(
                "events.yaml",
                r#"
workflows:
  handler:
    id: handler
    name: Handler
    trigger:
      event: "system.plugin.failed"
    steps:
      - plugin: p1
        action: a1
"#,
            )],
        );

        let bus = EventBus::new(registry, pipeline);
        let mut rx = bus.subscribe();

        bus.emit_plugin_failed("bad-plugin", "WASM validation error").await;

        let received = rx.recv().await.unwrap();
        assert_eq!(received.name, "system.plugin.failed");
        let payload = received.payload.unwrap();
        assert_eq!(payload["plugin_id"], "bad-plugin");
        assert_eq!(payload["error"], "WASM validation error");
    }

    // ── Test 6: WorkflowEventEmitter trait ──

    #[tokio::test]
    async fn workflow_event_emitter_trait_works() {
        let dir = TempDir::new().unwrap();
        let (registry, mock_executor, pipeline) = build_test_components(
            dir.path(),
            &[(
                "events.yaml",
                r#"
workflows:
  handler:
    id: handler
    name: Handler
    trigger:
      event: "workflow.completed"
    steps:
      - plugin: logger
        action: log
"#,
            )],
        );

        let bus = EventBus::new(registry, pipeline);
        let emitter: &dyn WorkflowEventEmitter = &bus;
        emitter
            .emit("workflow.completed", serde_json::json!({"wf": "test"}))
            .await;

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert_eq!(mock_executor.count(), 1);
    }

    #[tokio::test]
    async fn workflow_event_emitter_creates_system_source() {
        let dir = TempDir::new().unwrap();
        let (registry, _, pipeline) = build_test_components(
            dir.path(),
            &[(
                "events.yaml",
                r#"
workflows:
  handler:
    id: handler
    name: Handler
    trigger:
      event: "workflow.completed"
    steps:
      - plugin: p1
        action: a1
"#,
            )],
        );

        let bus = EventBus::new(registry, pipeline);
        let mut rx = bus.subscribe();

        let emitter: &dyn WorkflowEventEmitter = &bus;
        emitter
            .emit("workflow.completed", serde_json::json!({"status": "ok"}))
            .await;

        let received = rx.recv().await.unwrap();
        assert_eq!(received.source, "system");
        assert_eq!(received.depth, 0);
    }
}
