<!--
domain: event-bus
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Design Document — Event Bus

## Introduction

This document describes the technical design of the Life Engine event bus. The bus is an in-memory pub/sub dispatcher built on a Tokio broadcast channel. It handles event construction, namespace validation, manifest enforcement, depth tracking, delivery to matching workflows, and system event emission.

## Event Struct

The canonical event shape:

```rust
use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct Event {
    pub name: String,
    pub payload: Option<Value>,
    pub source: String,
    pub timestamp: DateTime<Utc>,
    pub depth: u8,
}
```

Field responsibilities:

- **name** — Fully qualified event name. Plugin events use `{plugin_id}.{event_suffix}`. System events use `system.{event_suffix}`.
- **payload** — Optional JSON payload. System events include structured data (plugin ID, job ID, error details). Plugin events carry plugin-defined data.
- **source** — Set by the host, never by the caller. Plugin ID for plugin events, `"system"` for Core events.
- **timestamp** — UTC timestamp set by the host at emission time.
- **depth** — Counter for loop prevention. Starts at 0 for root events, incremented for child events.

## Event Bus Struct

```rust
use tokio::sync::broadcast;

pub struct EventBus {
    sender: broadcast::Sender<Event>,
    max_depth: u8,
}

impl EventBus {
    pub fn new(max_depth: u8, channel_capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(channel_capacity);
        Self { sender, max_depth }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }

    pub fn emit(&self, event: Event) -> Result<(), EventBusError> {
        if event.depth > self.max_depth {
            tracing::warn!(
                event_name = %event.name,
                source = %event.source,
                depth = event.depth,
                "Event dropped: depth limit exceeded"
            );
            return Err(EventBusError::DepthLimitExceeded {
                name: event.name,
                depth: event.depth,
            });
        }
        let _ = self.sender.send(event);
        Ok(())
    }
}
```

Configuration defaults:

- **max_depth** — 8
- **channel_capacity** — 256 (tunable; lagged receivers drop oldest events)

## Event Bus Error Type

```rust
#[derive(Debug, thiserror::Error)]
pub enum EventBusError {
    #[error("Event '{name}' dropped: depth {depth} exceeds limit")]
    DepthLimitExceeded { name: String, depth: u8 },

    #[error("Event '{name}' not declared in manifest for plugin '{plugin_id}'")]
    UndeclaredEvent { name: String, plugin_id: String },
}
```

## Event Naming Convention

Plugin events follow the pattern `{plugin_id}.{suffix}` where `suffix` is one or more dot-separated segments. Examples:

- `connector-email.fetch.completed`
- `connector-email.fetch.failed`
- `notes.created`

System events follow the pattern `system.{suffix}`. The v1 system events are:

- `system.startup`
- `system.plugin.loaded`
- `system.plugin.failed`
- `system.workflow.completed`
- `system.workflow.failed`

## Manifest Declaration

Plugins declare emitted events in `manifest.toml`:

```toml
[events]
emits = [
    "fetch.completed",
    "fetch.failed",
]
```

At runtime, the system prefixes each declared name with the plugin ID to form the fully qualified event name. A plugin with ID `connector-email` declaring `fetch.completed` can emit `connector-email.fetch.completed`.

## Plugin Emission Host Function

The host function exposed to WASM plugins:

```rust
fn emit_event(name: &str, payload: Option<Value>) -> Result<()>
```

Host function implementation steps:

1. Look up the calling plugin's manifest to retrieve declared event names.
2. Verify that `name` matches one of the declared names (after prefixing with plugin ID).
3. If undeclared, return `EventBusError::UndeclaredEvent`.
4. Construct an `Event` with `source` set to the plugin ID, `timestamp` set to `Utc::now()`, and `depth` derived from the current execution context.
5. Call `EventBus::emit()`.

## Depth Tracking

The execution context carries the current event depth. When a workflow is triggered by an event, the runtime sets the context depth to `event.depth + 1`. Any events emitted during that workflow execution inherit the context depth as their `depth` value.

```rust
pub struct ExecutionContext {
    pub plugin_id: String,
    pub event_depth: u8,
    // ... other fields
}
```

When no parent event exists (e.g., an HTTP-triggered workflow emits an event), the context depth is 0.

## Event Delivery and Workflow Dispatch

The workflow engine subscribes to the event bus on startup. A dispatcher task receives events from the broadcast channel and matches them against registered workflow triggers:

```rust
async fn dispatch_events(
    mut receiver: broadcast::Receiver<Event>,
    workflow_registry: Arc<WorkflowRegistry>,
) {
    loop {
        match receiver.recv().await {
            Ok(event) => {
                let workflows = workflow_registry.find_by_event(&event.name);
                for workflow in workflows {
                    let event = event.clone();
                    tokio::spawn(async move {
                        // Build PipelineMessage and execute workflow
                        let message = build_pipeline_message(&event);
                        workflow.execute(message).await;
                    });
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!(missed = n, "Event dispatcher lagged, events dropped");
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}
```

Matching uses exact string comparison on `event.name` against each workflow's `trigger.event` field.

## Event-Triggered PipelineMessage Construction

When an event triggers a workflow, the initial `PipelineMessage` is built as follows:

```rust
fn build_pipeline_message(event: &Event) -> PipelineMessage {
    PipelineMessage {
        payload: event.payload.clone(),
        metadata: PipelineMetadata {
            trigger_type: "event".to_string(),
            event_name: Some(event.name.clone()),
            event_source: Some(event.source.clone()),
            // ... other metadata fields
        },
    }
}
```

## System Event Emission Points

Core emits system events at specific lifecycle points:

- **system.startup** — After all plugins are loaded and the workflow engine is ready. No payload.
- **system.plugin.loaded** — After a plugin loads successfully. Payload: `{ "plugin_id": "..." }`
- **system.plugin.failed** — After a plugin fails to load or crashes. Payload: `{ "plugin_id": "...", "error": "..." }`
- **system.workflow.completed** — After an async workflow finishes. Payload: `{ "job_id": "...", "status": "completed" }`
- **system.workflow.failed** — After a workflow terminates with an error. Payload: `{ "job_id": "...", "error": "..." }`

System events are emitted with `source = "system"` and `depth = 0` (they are always root events).

## Crate Placement

The event bus lives in the `le-event-bus` crate within the workspace. It depends on:

- `tokio` — broadcast channel and async runtime
- `chrono` — timestamp generation
- `serde_json` — `Value` type for payloads
- `thiserror` — error type derivation
- `tracing` — structured logging for dropped events and dispatcher warnings
