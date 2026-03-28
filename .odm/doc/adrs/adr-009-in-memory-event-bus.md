---
title: "ADR-009: In-Memory Event Bus"
type: adr
created: 2026-03-28
status: active
---

# ADR-009: In-Memory Event Bus

## Status

Accepted

## Context

Life Engine plugins need to communicate asynchronously — a contacts connector that syncs new contacts should be able to notify other plugins (e.g., a deduplication workflow) without knowing about them directly. The workflow engine also needs to react to system events (plugin loaded, collection migrated, health check failed) to trigger administrative workflows.

The event bus must support two patterns: plugin-emitted events (a plugin calls a host function to emit an event) and system events (Core itself emits events during lifecycle operations). Subscribers are workflows bound to event triggers.

The question is whether to use an external message broker (Redis, NATS, RabbitMQ) or an in-process solution for v1.

## Decision

The v1 event bus is an in-memory pub/sub system within the Core process. There is no external message broker. Events are delivered to subscribers synchronously during the emit call — each matching workflow is spawned as an async task, but the bus itself is a simple in-process dispatcher.

Events follow a namespaced naming convention (`system.*` for system events, `plugin.{plugin_id}.*` for plugin events). Subscribers declare the event names they listen to in their workflow trigger definition.

Loop prevention is built in. The event bus tracks event chains and enforces a maximum depth. If a workflow triggered by an event emits another event that would trigger the same workflow, the chain is broken and a warning is logged.

System events include: `system.plugin.loaded`, `system.plugin.unloaded`, `system.collection.migrated`, `system.health.degraded`, and `system.startup.complete`.

The following are deferred to future versions:

- Persistent event queue (events that survive process restart)
- Wildcard event matching (e.g., `plugin.connector-email.*`)
- Event replay and dead-letter handling
- External broker integration

## Consequences

Positive consequences:

- No external dependencies. Core remains a single-binary deployment with no message broker to install or manage.
- Event delivery is fast and predictable — no network hop, no serialisation to an external system, no broker configuration.
- The in-memory model is simple to implement, test, and debug. Events are traceable within a single process.
- Loop prevention is a core feature, not an afterthought. Recursive event chains are caught before they cause resource exhaustion.

Negative consequences:

- Events are lost if Core crashes or restarts during delivery. There is no persistence or replay mechanism in v1.
- The in-memory bus does not scale to multiple Core instances. If Life Engine ever supports a distributed deployment, the event bus must be replaced or supplemented with an external broker.
- No wildcard matching means subscribers must list every event name explicitly. A plugin that wants to react to all events from a connector must enumerate them.
- Event delivery is fire-and-forget from the emitter's perspective. The emitting plugin cannot know whether subscribers succeeded or failed.
