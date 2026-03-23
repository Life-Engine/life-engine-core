//! Event bus for async event-driven workflow triggering.
//!
//! Uses `tokio::sync::broadcast` for event distribution. When an event is
//! emitted, the bus looks up matching workflows in the `TriggerRegistry` and
//! spawns a background task for each one.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::broadcast;
use tracing::{error, info};

use crate::executor::{build_initial_message, PipelineExecutor, WorkflowEventEmitter};
use crate::loader::TriggerRegistry;
use crate::types::TriggerContext;

/// Channel capacity for the broadcast event bus.
const EVENT_CHANNEL_CAPACITY: usize = 256;

/// Thread-safe event bus for distributing events to workflow triggers.
///
/// The event bus owns a broadcast channel for fan-out event delivery and
/// holds references to the trigger registry and pipeline executor so it
/// can spawn workflow executions when matching events arrive.
pub struct EventBus {
    sender: broadcast::Sender<(String, serde_json::Value)>,
    registry: Arc<TriggerRegistry>,
    executor: Arc<PipelineExecutor>,
}

impl EventBus {
    /// Create a new event bus with the given trigger registry and executor.
    pub fn new(registry: Arc<TriggerRegistry>, executor: Arc<PipelineExecutor>) -> Self {
        let (sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            sender,
            registry,
            executor,
        }
    }

    /// Emit an event, triggering any matching workflows.
    ///
    /// For each workflow registered for the given event name, a new tokio task
    /// is spawned to execute the workflow independently. The method returns
    /// immediately without waiting for workflow completion.
    pub async fn emit_event(&self, event_name: String, payload: serde_json::Value) {
        // Send on the broadcast channel for any subscribers (logging, metrics).
        // Ignore send errors — they only occur when there are no receivers.
        let _ = self.sender.send((event_name.clone(), payload.clone()));

        let matching = self.registry.find_event(&event_name);
        let count = matching.len();

        if count == 0 {
            return;
        }

        info!(
            event = %event_name,
            triggered_workflows = count,
            "Event emitted, triggering {} workflow(s)",
            count
        );

        for workflow in matching {
            let workflow = workflow.clone();
            let executor = Arc::clone(&self.executor);
            let event_name = event_name.clone();
            let payload = payload.clone();

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

    /// Subscribe to all events on the bus.
    ///
    /// Returns a broadcast receiver that will receive `(event_name, payload)`
    /// tuples for every event emitted. Useful for logging, metrics, or
    /// external integrations.
    pub fn subscribe(&self) -> broadcast::Receiver<(String, serde_json::Value)> {
        self.sender.subscribe()
    }
}

#[async_trait]
impl WorkflowEventEmitter for EventBus {
    async fn emit(&self, event_name: &str, payload: serde_json::Value) {
        self.emit_event(event_name.to_string(), payload).await;
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
        bus.emit_event(
            "webhook.email.received".to_string(),
            serde_json::json!({"from": "test@example.com"}),
        )
        .await;

        // Give spawned tasks time to execute.
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
        bus.emit_event(
            "nonexistent.event".to_string(),
            serde_json::json!({}),
        )
        .await;

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
        bus.emit_event(
            "webhook.email.received".to_string(),
            serde_json::json!({"subject": "test"}),
        )
        .await;

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        assert_eq!(mock_executor.count(), 2);
    }

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

        bus.emit_event(
            "some.event".to_string(),
            serde_json::json!({"key": "value"}),
        )
        .await;

        let (name, payload) = rx.recv().await.unwrap();
        assert_eq!(name, "some.event");
        assert_eq!(payload, serde_json::json!({"key": "value"}));
    }

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
}
