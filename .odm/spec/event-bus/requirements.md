<!--
domain: event-bus
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Requirements Document — Event Bus

## Introduction

The event bus is an in-memory pub/sub system that delivers events between plugins, workflows, and Core internals. It uses a Tokio broadcast channel for delivery and enforces namespaced event naming, manifest-declared emission, and depth-based loop prevention. Events are fire-and-forget: there is no persistence, no acknowledgement, and no ordering guarantee. Subscribers are workflows bound to event triggers.

## Alignment with Product Vision

- **Decoupled Architecture** — The event bus lets plugins communicate without direct dependencies, enabling a composable plugin ecosystem
- **Single-Binary Deployment** — In-memory delivery avoids external broker dependencies, keeping Core self-contained
- **Defence in Depth** — Manifest-enforced emission and depth-based loop prevention protect the system from misbehaving plugins
- **Principle of Least Privilege** — Plugins can only emit events they have declared; the source field is set by the host, not the caller
- **Parse, Don't Validate** — The `Event` struct enforces a typed shape at the boundary; downstream code trusts the structure

## Requirements

### Requirement 1 — Event Shape and Construction

**User Story:** As a plugin author, I want events to carry a consistent shape so that subscribers can reliably destructure the event payload and metadata.

#### Acceptance Criteria

- 1.1. WHEN an event is created THEN the system SHALL populate `name`, `source`, `timestamp`, and `depth` fields on the `Event` struct.
- 1.2. WHEN an event is created THEN `payload` SHALL be `Option<Value>`, allowing events with or without a JSON payload.
- 1.3. WHEN a plugin emits an event THEN the system SHALL set `source` to the emitting plugin's ID, ignoring any caller-provided value.
- 1.4. WHEN Core emits a system event THEN the system SHALL set `source` to `"system"`.
- 1.5. WHEN an event is created THEN `timestamp` SHALL be set to the current UTC time by the system.

### Requirement 2 — Event Naming and Namespacing

**User Story:** As a plugin author, I want events namespaced by plugin ID so that event names do not collide across plugins.

#### Acceptance Criteria

- 2.1. WHEN a plugin emits an event THEN the event name SHALL be prefixed with the plugin's ID (e.g., `connector-email.fetch.completed`).
- 2.2. WHEN Core emits a system event THEN the event name SHALL use the `system.*` prefix (e.g., `system.plugin.loaded`).
- 2.3. WHEN matching events to workflow triggers THEN the system SHALL use exact string matching; no wildcard or pattern matching is supported in v1.

### Requirement 3 — Manifest-Enforced Emission

**User Story:** As a plugin author, I want the system to validate emitted events against my manifest so that I am alerted immediately if I emit an undeclared event.

#### Acceptance Criteria

- 3.1. WHEN a plugin emits an event whose name is declared in its manifest THEN the system SHALL accept the event and broadcast it.
- 3.2. WHEN a plugin emits an event whose name is NOT declared in its manifest THEN the system SHALL reject the emission with an error and the event SHALL NOT be broadcast.
- 3.3. WHEN a plugin manifest is loaded THEN the system SHALL parse and store the declared event names for runtime validation.

### Requirement 4 — Event Delivery

**User Story:** As a workflow author, I want my workflow to fire automatically when a matching event is emitted so that I do not need to poll for state changes.

#### Acceptance Criteria

- 4.1. WHEN an event is broadcast THEN the system SHALL identify all workflows with a `trigger.event` matching the event name.
- 4.2. WHEN multiple workflows match a single event THEN the system SHALL spawn each workflow independently and concurrently as async tasks.
- 4.3. WHEN an event is broadcast THEN the system SHALL NOT guarantee any ordering between subscriber executions.
- 4.4. WHEN no workflows match a broadcast event THEN the event SHALL be silently discarded with no error.

### Requirement 5 — Loop Prevention

**User Story:** As a system operator, I want recursive event chains detected and broken so that a misbehaving plugin cannot cause runaway resource consumption.

#### Acceptance Criteria

- 5.1. WHEN a new event is emitted outside any event processing context THEN `depth` SHALL be set to 0.
- 5.2. WHEN an event is emitted during processing of a parent event THEN `depth` SHALL be set to `parent_depth + 1`.
- 5.3. WHEN an event's `depth` exceeds the configurable maximum depth (default 8) THEN the system SHALL drop the event and NOT broadcast it.
- 5.4. WHEN an event is dropped due to depth limit THEN the system SHALL log a warning including the event name, source, and current depth.
- 5.5. WHEN the maximum depth is configured to a custom value THEN the system SHALL use that value instead of the default.

### Requirement 6 — System Events

**User Story:** As a system operator, I want Core to emit lifecycle events so that administrative workflows can react to startup, plugin load/failure, and workflow completion.

#### Acceptance Criteria

- 6.1. WHEN Core initialisation completes THEN the system SHALL emit `system.startup`.
- 6.2. WHEN a plugin loads successfully THEN the system SHALL emit `system.plugin.loaded` with the plugin ID in the payload.
- 6.3. WHEN a plugin fails to load or crashes at runtime THEN the system SHALL emit `system.plugin.failed` with the plugin ID and error details in the payload.
- 6.4. WHEN an async workflow completes successfully THEN the system SHALL emit `system.workflow.completed` with `JobId` and final status in the payload.
- 6.5. WHEN a workflow terminates with an error THEN the system SHALL emit `system.workflow.failed` with `JobId` and error details in the payload.

### Requirement 7 — Plugin Event Emission Host Function

**User Story:** As a plugin author, I want a host function to emit events so that my WASM plugin can publish events without direct access to the bus.

#### Acceptance Criteria

- 7.1. WHEN a plugin calls `emit_event(name, payload)` THEN the host function SHALL validate the event name against the plugin's manifest.
- 7.2. WHEN validation passes THEN the host function SHALL construct an `Event` with `source` set to the calling plugin's ID and `depth` derived from the current execution context.
- 7.3. WHEN validation fails THEN the host function SHALL return an error to the plugin without broadcasting any event.

### Requirement 8 — Event-Triggered Workflow Input

**User Story:** As a workflow author, I want the triggering event's data available in the workflow's initial pipeline message so that my workflow steps can act on the event payload.

#### Acceptance Criteria

- 8.1. WHEN an event triggers a workflow THEN the pipeline executor SHALL build an initial `PipelineMessage` with `payload` set to the event's `Value` payload.
- 8.2. WHEN an event triggers a workflow THEN `metadata.trigger_type` SHALL be set to `"event"`.
- 8.3. WHEN an event triggers a workflow THEN `metadata.event_name` SHALL be set to the event name.
- 8.4. WHEN an event triggers a workflow THEN `metadata.event_source` SHALL be set to the emitting plugin ID, or `"system"` for system events.
