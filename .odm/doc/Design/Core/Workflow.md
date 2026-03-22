---
title: "Core — Workflow Engine"
tags: [life-engine, core, workflow, pipeline, plugins]
created: 2026-03-14
updated: 2026-03-23
---

# Workflow Engine

The workflow engine is the central orchestration layer. Every request that involves plugin logic flows through a workflow — a declarative pipeline of plugin steps defined in YAML.

The workflow engine also owns the event bus and cron scheduler. Workflows can be triggered by transport endpoints, events, or schedules.

## Request Flow

```
Transport (REST, GraphQL, etc.)
      |
      v
+------------------+
|  Workflow Engine  |  1. Match trigger to workflow
|                   |  2. Execute plugin steps in sequence
|  Step 1 (Plugin)  |  3. Pass PipelineMessage between steps
|  Step 2 (Plugin)  |  4. Validate output at boundaries
|  Step 3 (Plugin)  |
+--------+---------+
         |
         v
   Response (back through transport)
```

## Workflow Definitions

Workflows are YAML files in a configured directory. Core reads them at startup.

```yaml
# workflows/sync-email.yaml
id: sync-email
name: Email Sync Pipeline
mode: async
validate: edges
trigger:
  schedule: "*/5 * * * *"
  endpoint: "POST /email/sync"
  event: "webhook.email.received"
steps:
  - plugin: connector-email
    action: fetch
    on_error:
      strategy: retry
      max_retries: 3
      fallback: null
  - plugin: search-indexer
    action: index
    on_error:
      strategy: halt
```

### Fields

- **id** — Unique identifier for the workflow
- **name** — Human-readable label
- **mode** — Execution mode (`sync` or `async`)
- **validate** — Schema validation level (`strict`, `edges`, or `none`)
- **trigger** — What activates this workflow (one or more trigger types)
- **steps** — Ordered list of plugin actions to execute

## Triggers

A workflow can have one or more triggers. All trigger types are equivalent — the same pipeline runs regardless of how it was activated.

- **endpoint** — An HTTP path handled by an active transport. The transport routes the request to the workflow engine.
- **event** — An event emitted by a plugin or the system. The event bus (part of the workflow engine) matches events to workflow triggers.
- **schedule** — A cron expression. The built-in scheduler fires the workflow at the specified interval.

```yaml
trigger:
  endpoint: "POST /email/sync"
  event: "webhook.email.received"
  schedule: "*/5 * * * *"
```

A workflow can declare all three, any combination, or just one.

## Execution Modes

- **sync** — All steps must complete before the transport responds. Used for queries where the caller expects data back immediately.
- **async** — Returns a job ID immediately. Steps run in the background. Used for long-running operations like data sync. Results are available via events or polling.

```yaml
# Sync — caller waits for result
get-contacts:
  mode: sync
  trigger:
    endpoint: "GET /contacts"
  steps:
    - plugin: connector-contacts
      action: list

# Async — caller gets job ID, work happens in background
sync-email:
  mode: async
  trigger:
    endpoint: "POST /email/sync"
  steps:
    - plugin: connector-email
      action: fetch
    - plugin: search-indexer
      action: index
```

## Data Flow Between Steps

Each step receives a `PipelineMessage` and returns a `PipelineMessage`. The workflow engine passes the output of step N as the input to step N+1.

```
PipelineMessage (from transport or trigger)
  |
  v
Step 1: connector-email.fetch
  Output: PipelineMessage { payload: Cdm(Emails) }
  |
  v
Step 2: search-indexer.index
  Output: PipelineMessage { payload: Custom(IndexResult) }
  |
  v
Response (final step output)
```

## Control Flow (v1)

Three control flow primitives are supported in v1:

### Sequential

Steps run in order. Output of step N is input to step N+1. This is the default.

### Conditional Branching

Route to different steps based on output content:

```yaml
steps:
  - plugin: classifier
    action: classify
  - condition:
      field: "payload.category"
      equals: "spam"
      then:
        - plugin: spam-handler
          action: quarantine
      else:
        - plugin: email-archiver
          action: store
```

### Error Handling

Each step declares an `on_error` strategy:

- **halt** (default) — Stop the entire workflow and return an error response.
- **retry** — Retry the step up to `max_retries` times (with exponential backoff). If retries are exhausted, run the `fallback` step or halt.
- **skip** — Log the error, skip this step, pass the previous step's output to the next step.

```yaml
steps:
  - plugin: connector-email
    action: fetch
    on_error:
      strategy: retry
      max_retries: 3
      fallback:
        plugin: error-logger
        action: log
```

Failed workflows are logged with the step that failed, the error, and the input that caused it.

## Validation

Schema validation at pipeline boundaries is configurable per workflow:

- **strict** — Validate the output of every step against the declared schema before passing it to the next step. Safe but slower. Useful during development.
- **edges** (default) — Validate when the message enters the workflow and when the final output leaves. Steps in between are trusted.
- **none** — No schema validation. For performance-critical paths where plugins are well-tested.

Validation failures produce an `EngineError` with `Severity::Fatal`.

## Event Bus

The event bus is part of the workflow engine. It provides:

- **Event emission** — Plugins with the `events:emit` capability can emit named events.
- **Event matching** — Workflows with an `event` trigger are activated when a matching event is emitted.
- **System events** — Core emits system events (plugin loaded, plugin failed, storage error) that workflows can react to.

Events are the mechanism for asynchronous, decoupled communication between plugins (via intermediate workflows).

## Scheduler

The built-in cron scheduler is part of the workflow engine. Workflows with a `schedule` trigger are fired at the specified interval. Uses standard cron syntax.

The scheduler, event bus, and endpoint triggers are unified — they all activate the same workflow pipeline. There is no separate "scheduled task" system.
