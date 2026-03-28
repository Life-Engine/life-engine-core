---
title: Event Bus
type: adr
created: 2026-03-28
status: draft
tags:
  - architecture
  - workflow-engine
  - events
  - core
---

# Event Bus

## Overview

The event bus is the internal pub/sub mechanism within the workflow engine. Plugins emit events, and workflows subscribe to them via event triggers. The event bus is also how Core communicates system-level occurrences (plugin loaded, storage error) to workflows.

## Event Shape

Events are lightweight structs, distinct from `PipelineMessage`:

```rust
pub struct Event {
    pub name: String,              // e.g. "webhook.email.received"
    pub payload: Option<Value>,    // event data, if any
    pub source: String,            // plugin ID or "system"
    pub timestamp: DateTime<Utc>,
    pub depth: u8,                 // for loop prevention
}
```

The event bus does not use `PipelineMessage` for transport. When an event triggers a workflow, the [[pipeline-executor]] wraps the event payload into a `PipelineMessage` as the initial input.

## Event Naming

Event names use dot-separated segments enforced at registration:

- Plugin events are namespaced by plugin ID: `connector-email.fetch.completed`
- System events use the `system.*` prefix: `system.plugin.loaded`, `system.plugin.failed`, `system.startup`
- Plugins declare the events they emit in their manifest. Emitting an undeclared event is rejected at runtime.

No wildcard matching in v1. Triggers must match the exact event name. Wildcard subscriptions (e.g., `connector-email.*`) are a future consideration.

## Delivery Model

In-memory, fire-and-forget, using a Tokio broadcast channel.

- Events are delivered to all workflows with a matching `trigger.event`
- Each matching workflow fires independently and concurrently as a separate Tokio task
- No ordering guarantee between workflows triggered by the same event
- No acknowledgement or retry — if a triggered workflow fails, the event bus does not re-deliver
- Events in-flight are lost on restart

This model is appropriate for a single-process, single-user system. A persistent event queue (write-ahead log, dequeue after processing) is deferred to post-v1.

## System Events

Core emits system events through the same event bus. There is no separate channel — the model is unified.

System events in v1:

- `system.startup` — Emitted after Core finishes initialisation. Workflows can trigger on this to run tasks on every boot (e.g., sync email on restart).
- `system.plugin.loaded` — A plugin was successfully loaded
- `system.plugin.failed` — A plugin failed to load or crashed at runtime
- `system.workflow.completed` — An async workflow finished. Payload includes the `JobId` and final status. A future WebSocket transport could subscribe to these for real-time notifications.
- `system.workflow.failed` — A workflow terminated with an error

Workflows can trigger on any system event, which enables reactive patterns (e.g., trigger a notification workflow when `system.plugin.failed` fires).

## Loop Prevention

Events can cause cascading workflow activations. If workflow A emits an event that triggers workflow B, which emits an event that triggers workflow A, an infinite loop occurs.

Prevention uses a depth counter:

1. Every event carries a `depth` field (starts at 0 for root events)
2. When a workflow triggered by an event emits a new event, the child event's depth is parent depth + 1
3. Events exceeding a configurable max depth (default 8) are dropped
4. A warning is logged when an event is dropped due to depth limit

This approach is stateless — no graph analysis, no tracking of which workflows have been visited. The depth counter is carried in the event itself and evaluated at emission time.

## Plugin Event Emission

Plugins with the `events:emit` capability can emit events via a host function:

```rust
// Host function exposed to plugins
fn emit_event(name: &str, payload: Option<Value>) -> Result<()>;
```

The host function:

1. Validates the event name is declared in the plugin's manifest
2. Sets the `source` to the plugin's ID
3. Sets the `depth` from the current workflow's event context (or 0 if not event-triggered)
4. Publishes to the broadcast channel

## Event-Triggered Workflow Input

When an event activates a workflow, the executor builds the initial `PipelineMessage`:

- `payload` — The event's `Value` payload (or empty if the event has no payload)
- `metadata.trigger_type` — `"event"`
- `metadata.event_name` — The event name
- `metadata.event_source` — The emitting plugin ID or `"system"`
