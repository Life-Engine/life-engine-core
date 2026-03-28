---
title: "ADR-007: Declarative YAML Workflows as the Sole Execution Path"
type: adr
created: 2026-03-28
status: active
---

# ADR-007: Declarative YAML Workflows as the Sole Execution Path

## Status

Accepted

## Context

Life Engine Core must orchestrate plugin execution for every kind of request: REST API calls, GraphQL queries, internal events, and scheduled jobs. A common pattern in similar systems is to hardcode behaviour for standard operations (CRUD, health checks) and reserve plugin pipelines for custom logic. This creates two execution paths — one for "built-in" operations and one for plugins — which doubles the surface area for bugs and makes the system harder to reason about.

The design also needed to support user customisation. Self-hosters should be able to modify how any operation works — adding validation steps, injecting logging, or replacing the default CRUD implementation entirely — without forking the codebase or writing custom handlers.

## Decision

Every operation in Core is a workflow. Workflows are declarative YAML pipelines of plugin steps, loaded once at startup and immutable at runtime. There are no hardcoded request handlers — even generic CRUD is implemented as system workflows (`collection.list`, `collection.get`, `collection.create`, `collection.update`, `collection.delete`, `graphql.query`, `system.health`).

System workflows are real, editable YAML files shipped with Core's default configuration. Users can modify them, add steps, or replace them entirely. Both REST and GraphQL resolve through the same workflows — the transport handler translates the wire format into a `WorkflowRequest`, and the workflow engine executes the pipeline.

Workflows are activated by three trigger types, all equivalent:

- **Endpoint triggers** — Bound to a REST route or GraphQL operation
- **Event triggers** — Fired when an internal event is emitted on the event bus
- **Schedule triggers** — Activated by a cron expression

The same pipeline runs regardless of how it was triggered. The pipeline executor runs steps in sequence, passing a `PipelineMessage` between each one, with support for conditional branching and per-step error handling (halt, retry, skip).

Sync workflows block until complete and return the result directly. Async workflows return a job ID immediately; the result is retained in a job registry with a configurable TTL (default 1 hour). Concurrency is bounded at 32 concurrent workflow tasks (configurable).

## Consequences

Positive consequences:

- One execution path for everything. A health check and a complex multi-step pipeline follow the same code path through the workflow engine. No special cases to maintain.
- Complete user customisation without code changes. Self-hosters add a validation step to `collection.create` by editing a YAML file, not by writing middleware or forking the application.
- Protocol-agnostic orchestration. The workflow engine never thinks about HTTP or GraphQL. Adding a new transport is purely a handler concern.
- System workflows serve as documentation and reference implementations. New plugin authors can read the default CRUD workflows to understand the pipeline model.

Negative consequences:

- Simple operations carry workflow overhead. A health check traverses the full workflow engine path even though it could be a direct function call.
- Workflow definitions are loaded at startup and immutable at runtime. Changing a workflow requires a restart. Hot-reloading is deferred to a future version.
- The YAML pipeline model limits control flow to sequential execution with conditional branching. Parallel fan-out/fan-in is deferred. Complex orchestration patterns must be decomposed into multiple workflows connected by events.
- Contributors must understand the workflow model before they can add any new behaviour to Core. There is no escape hatch for "just add a handler."
