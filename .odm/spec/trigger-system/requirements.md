<!--
domain: trigger-system
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Requirements Document — Trigger System

## Introduction

The trigger system connects external stimuli to workflow execution in Life Engine. It supports three trigger types — endpoint (HTTP), event (event bus), and schedule (cron) — and resolves each into a `TriggerContext` for the pipeline executor. All triggers are registered once at startup from workflow YAML definitions. The registry is immutable at runtime.

## Alignment with Product Vision

- **Parse, Don't Validate** — All trigger declarations are validated at startup; runtime resolution trusts the registry without re-validation
- **The Pit of Success** — Declaring triggers in workflow YAML is the only way to wire stimuli to workflows, eliminating ad-hoc registration
- **Open/Closed Principle** — New trigger types can be added without modifying existing resolution logic
- **Fail Fast** — Duplicate endpoints, invalid cron expressions, and malformed event names are caught at startup before any traffic is served

## Requirements

### Requirement 1 — Trigger Declaration in Workflow YAML

**User Story:** As a workflow author, I want to declare triggers in my workflow YAML so that the engine knows which stimuli activate my workflow.

#### Acceptance Criteria

- 1.1. WHEN a workflow YAML contains a `trigger` section THEN the system SHALL parse `endpoint`, `event`, and `schedule` fields from it.
- 1.2. WHEN a workflow declares an `endpoint` trigger THEN the value SHALL be a string in the format `METHOD /path` (e.g., `POST /email/sync`).
- 1.3. WHEN a workflow declares an `event` trigger THEN the value SHALL be a dot-separated event name string (e.g., `webhook.email.received`).
- 1.4. WHEN a workflow declares a `schedule` trigger THEN the value SHALL be a valid cron expression string (e.g., `*/5 * * * *`).
- 1.5. WHEN a workflow declares multiple trigger types THEN the system SHALL register all declared triggers for that workflow.
- 1.6. WHEN a workflow declares no `trigger` section THEN the system SHALL reject the workflow definition at startup and log an error identifying the workflow.

### Requirement 2 — Trigger Registration at Startup

**User Story:** As a Core developer, I want all triggers registered at startup by scanning workflow definitions so that the registry is complete before the engine accepts traffic.

#### Acceptance Criteria

- 2.1. WHEN the engine starts THEN the trigger system SHALL scan all loaded `WorkflowDefinition` entries and register their declared triggers.
- 2.2. WHEN endpoint triggers are registered THEN the system SHALL build a map of route pattern to workflow ID.
- 2.3. WHEN event triggers are registered THEN the system SHALL build a map of event name to a list of workflow IDs.
- 2.4. WHEN schedule triggers are registered THEN the system SHALL pass each cron expression and its associated workflow ID to the scheduler.
- 2.5. WHEN registration completes THEN the trigger registry SHALL be immutable for the lifetime of the engine process.

### Requirement 3 — Endpoint Trigger Resolution

**User Story:** As a workflow author, I want an HTTP request to my declared endpoint to activate my workflow so that external clients can invoke workflows via HTTP.

#### Acceptance Criteria

- 3.1. WHEN an HTTP request matches a registered endpoint trigger route THEN the system SHALL resolve it to exactly one workflow ID.
- 3.2. WHEN the route is resolved THEN the system SHALL build a `TriggerContext::Endpoint` containing the `WorkflowRequest` (body, params, query, identity).
- 3.3. WHEN the `TriggerContext::Endpoint` is built THEN the system SHALL pass it to the pipeline executor for the resolved workflow.
- 3.4. WHEN an HTTP request does not match any registered endpoint trigger THEN normal HTTP routing SHALL apply (the trigger system does not intercept it).

### Requirement 4 — Event Trigger Resolution

**User Story:** As a workflow author, I want my workflow to fire whenever a matching event is broadcast so that workflows can react to internal system events and plugin-emitted events.

#### Acceptance Criteria

- 4.1. WHEN the event bus broadcasts an event THEN the trigger system SHALL look up all workflows whose `trigger.event` matches the event name.
- 4.2. WHEN one or more workflows match THEN the system SHALL spawn each matching workflow independently and concurrently.
- 4.3. WHEN multiple workflows match THEN each SHALL receive its own copy of the event payload.
- 4.4. WHEN multiple workflows match THEN there SHALL be no ordering guarantee between their executions.
- 4.5. WHEN a workflow triggered by an event fails THEN it SHALL NOT affect other workflows triggered by the same event.
- 4.6. WHEN the event is resolved THEN the system SHALL build a `TriggerContext::Event` containing the event name, payload, and source for each matching workflow.

### Requirement 5 — Schedule Trigger Resolution

**User Story:** As a workflow author, I want my workflow to run on a cron schedule so that recurring tasks execute automatically without external triggers.

#### Acceptance Criteria

- 5.1. WHEN a cron expression matches the current time THEN the scheduler SHALL fire the associated workflow.
- 5.2. WHEN the scheduler fires a workflow THEN the system SHALL build a `TriggerContext::Schedule` containing only the workflow ID.
- 5.3. WHEN a `TriggerContext::Schedule` is built THEN the system SHALL pass it to the pipeline executor with an empty payload.

### Requirement 6 — Startup Validation of Endpoint Triggers

**User Story:** As an operator, I want the engine to reject duplicate endpoint triggers at startup so that routing ambiguity does not cause silent misbehaviour in production.

#### Acceptance Criteria

- 6.1. WHEN two or more workflows declare the same endpoint trigger THEN the engine SHALL refuse to start and log an error identifying the conflicting workflows and the duplicate route.
- 6.2. WHEN a workflow's `trigger.endpoint` value does not match any route in the listener configuration THEN the engine SHALL log a warning identifying the workflow and the unmatched route.
- 6.3. WHEN a workflow's `trigger.endpoint` value matches a route in the listener configuration THEN validation SHALL pass silently for that trigger.

### Requirement 7 — Startup Validation of Schedule Triggers

**User Story:** As an operator, I want the engine to reject invalid cron expressions at startup so that misconfigured schedules do not cause runtime errors.

#### Acceptance Criteria

- 7.1. WHEN a workflow declares a `trigger.schedule` with a syntactically invalid cron expression THEN the engine SHALL refuse to start and log an error identifying the workflow and the invalid expression.
- 7.2. WHEN a workflow declares a `trigger.schedule` with a syntactically valid cron expression THEN validation SHALL pass for that trigger.

### Requirement 8 — Startup Validation of Event Triggers

**User Story:** As an operator, I want the engine to reject malformed event names at startup so that event triggers follow a consistent naming convention.

#### Acceptance Criteria

- 8.1. WHEN a workflow declares a `trigger.event` that does not follow the dot-separated naming convention THEN the engine SHALL refuse to start and log an error identifying the workflow and the malformed event name.
- 8.2. WHEN a workflow declares a `trigger.event` with a valid dot-separated name THEN validation SHALL pass for that trigger.
- 8.3. WHEN multiple workflows declare the same event trigger THEN validation SHALL pass (event triggers support one-to-many).
