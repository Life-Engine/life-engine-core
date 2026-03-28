<!--
domain: event-bus
status: draft
tier: 1
updated: 2026-03-28
-->

# Event Bus Spec

## Overview

The event bus provides in-memory, fire-and-forget event delivery between plugins, workflows, and Core internals. It is the primary mechanism for decoupled communication within the Life Engine. Events are delivered via a Tokio broadcast channel. Subscribers are workflows bound to event triggers. The bus enforces namespaced event naming, manifest-declared emission, and depth-based loop prevention.

Events are not persisted and are lost on restart. There is no ordering guarantee between subscribers, no acknowledgement mechanism, and no wildcard matching in v1.

## Goals

- Decoupled communication — plugins and workflows react to events without direct dependencies on the emitter
- Fire-and-forget delivery — emitters are not blocked by subscriber processing and do not observe subscriber outcomes
- Namespace isolation — plugin events are scoped to the emitting plugin's ID; system events use the `system.*` prefix
- Manifest-enforced emission — plugins can only emit events declared in their manifest, preventing undeclared event sprawl
- Loop prevention — recursive event chains are detected and broken via a configurable depth limit
- Minimal infrastructure — no external message broker; the bus runs entirely in-process

## User Stories

- As a plugin author, I want to emit events when my plugin completes an action so that other plugins and workflows can react without being tightly coupled to my plugin.
- As a workflow author, I want to trigger a workflow on a specific event so that my workflow runs automatically when a condition occurs.
- As a plugin author, I want emitted events to be validated against my manifest so that I receive clear errors if I emit an undeclared event.
- As a system operator, I want system events emitted during Core lifecycle operations so that administrative workflows can respond to plugin failures and startup completion.
- As a system operator, I want loop prevention so that recursive event chains do not cause resource exhaustion.

## Functional Requirements

- The system must deliver events to all workflows with a matching `trigger.event` concurrently and independently.
- The system must enforce event naming conventions: plugin events use `{plugin_id}.{event_name}`, system events use `system.*`.
- The system must reject emission of events not declared in the emitting plugin's manifest.
- The system must set `source` to the emitting plugin's ID (or `"system"` for Core events) and `timestamp` to the current time.
- The system must track event depth and drop events exceeding the configurable maximum depth (default 8), logging a warning on each drop.
- The system must emit a defined set of system events: `system.startup`, `system.plugin.loaded`, `system.plugin.failed`, `system.workflow.completed`, `system.workflow.failed`.
- The system must build an initial `PipelineMessage` from the triggering event, populating payload and metadata fields.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
