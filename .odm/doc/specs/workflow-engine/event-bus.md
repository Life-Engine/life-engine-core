---
title: Event Bus Specification
type: reference
created: 2026-03-28
status: active
tags:
  - spec
  - event
  - messaging
---

# Event Bus Specification

The event bus provides in-memory, fire-and-forget event delivery between plugins, workflows, and Core internals. It is the primary mechanism for decoupled communication within the engine.

## Event Shape

```rust
pub struct Event {
    pub name: String,
    pub payload: Option<Value>,
    pub source: String,
    pub timestamp: DateTime<Utc>,
    pub depth: u8,
}
```

## Event Naming

- Plugin events are namespaced by plugin ID: `connector-email.fetch.completed`
- System events use the `system.*` prefix: `system.plugin.loaded`, `system.plugin.failed`, `system.startup`
- Plugins must declare emitted events in their manifest. Emitting an undeclared event is rejected at runtime.
- No wildcard matching in v1. Event names must match exactly.

## Delivery Model

- In-memory only, implemented as a Tokio broadcast channel.
- All workflows with a matching `trigger.event` fire independently and concurrently.
- There is no ordering guarantee between subscribers.
- There is no acknowledgement or retry mechanism.
- Events are lost on restart.

## System Events (v1)

Core emits the following system events:

- **system.startup** — Fired after Core initialisation completes.
- **system.plugin.loaded** — A plugin loaded successfully.
- **system.plugin.failed** — A plugin failed to load or crashed at runtime.
- **system.workflow.completed** — An async workflow finished. Payload includes `JobId` and final status.
- **system.workflow.failed** — A workflow terminated with an error. Payload includes `JobId` and error details.

## Loop Prevention

Events carry a `depth` counter to prevent infinite loops:

- New events start at depth 0.
- A child event (emitted during processing of a parent event) has `depth = parent depth + 1`.
- Maximum depth is configurable (default 8).
- Events exceeding the maximum depth are dropped.
- A warning is logged on every dropped event.

## Plugin Event Emission

Plugins emit events via a host function:

```rust
fn emit_event(name: &str, payload: Option<Value>) -> Result<()>
```

The host function:

1. Validates that the event name is declared in the plugin's manifest.
2. Sets `source` to the emitting plugin's ID.
3. Sets `depth` from the current execution context.

## Event-Triggered Workflow Input

When an event triggers a workflow, the [[pipeline-executor]] builds the initial `PipelineMessage` as follows:

- **payload** — The event's `Value` payload.
- **metadata.trigger_type** — `"event"`
- **metadata.event_name** — The event name.
- **metadata.event_source** — The emitting plugin ID, or `"system"` for system events.
