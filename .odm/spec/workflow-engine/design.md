<!--
domain: workflow-engine
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Workflow Engine

Reference: [Core Workflow Design](../../doc/Design/Core/Workflow.md)

## Purpose

The workflow engine is the central orchestration layer in Core. Every request that involves plugin logic flows through a workflow — a declarative pipeline of plugin steps defined in YAML. The workflow engine also owns the event bus and cron scheduler, unifying endpoint, event, and schedule triggers into a single pipeline system.

Workflows are YAML config files stored in a configured directory and read at startup. Plugins are WASM modules invoked via Extism.

## Crate Location

```
packages/workflow-engine/
```

The crate follows the standard internal layout:

```
src/
  lib.rs          → Public API (init, WorkflowEngine, re-exports)
  config.rs       → Config struct (workflow directory path, defaults)
  error.rs        → Module-specific error types implementing EngineError
  loader.rs       → YAML file discovery and parsing
  executor.rs     → Pipeline executor (step sequencing, data flow, control flow)
  event_bus.rs    → Event emission, matching, and dispatch
  scheduler.rs    → Cron-based workflow triggering
  types.rs        → Module-internal types (WorkflowDef, StepDef, TriggerDef, etc.)
  tests/
    mod.rs
    ...
```

## Request Flow

```text
Transport (REST, GraphQL, CalDAV, etc.)
      |
      v
+------------------+
|  Workflow Engine  |  1. Match trigger to workflow
|                   |  2. Construct initial PipelineMessage
|  Step 1 (Plugin)  |  3. Execute plugin steps in sequence via Extism
|  Step 2 (Plugin)  |  4. Pass PipelineMessage between steps
|  Step 3 (Plugin)  |  5. Validate output at configured boundaries
+--------+---------+
         |
         v
   Response (back through transport, or emitted as event)
```

## Workflow Definitions

Workflows are YAML files in a configured directory (`[workflows] path` in `config.toml`). Core reads them at startup.

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
      fallback:
        plugin: error-logger
        action: log
  - plugin: search-indexer
    action: index
    on_error:
      strategy: halt
```

### Fields

- **id** — Unique identifier for the workflow
- **name** — Human-readable label
- **mode** — Execution mode: `sync` (await all steps) or `async` (return job ID, run in background)
- **validate** — Schema validation level: `strict` (every boundary), `edges` (default, entry/exit only), or `none`
- **trigger** — What activates this workflow (one or more of: `endpoint`, `event`, `schedule`)
- **steps** — Ordered list of plugin actions to execute

## Triggers

A workflow can have one or more triggers. All trigger types are equivalent — the same pipeline runs regardless of how it was activated.

- **endpoint** — An HTTP path handled by an active transport. The transport routes the request to the workflow engine.
- **event** — A named event emitted by a plugin or the system. The event bus matches events to workflow triggers.
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
- **async** — Returns a job ID immediately. Steps run in the background. Used for long-running operations like data sync. Results are available via events.

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

```text
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

The PipelineMessage envelope:

```rust
struct PipelineMessage {
    metadata: MessageMetadata,    // correlation ID, source, timestamp, auth context
    payload: TypedPayload,        // Cdm(CdmType) | Custom(SchemaValidated<Value>)
}
```

Payload types:

- **CDM types** — The 7 canonical collection types: Events, Tasks, Contacts, Emails, Notes, Files, Credentials
- **Custom types** — Plugin-defined types validated against a JSON Schema declared in the plugin manifest

## Control Flow (v1)

Three control flow primitives are supported in v1.

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

- **halt** (default) — Stop the entire workflow and return an error. The engine constructs an EngineError with `Severity::Fatal`.
- **skip** — Log the error as a warning, skip this step, pass the previous step's output to the next step.
- **retry** — Retry the step with exponential backoff up to `max_retries` times. If retries are exhausted, run the `fallback` step (if declared) or halt.

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

Failed workflows are logged with the step that failed, the error (including severity), and the input that caused it.

## Validation

Schema validation at pipeline boundaries is configurable per workflow:

- **strict** — Validate the PipelineMessage output at every step boundary before passing it to the next step. Safe but slower. Useful during development.
- **edges** (default) — Validate at pipeline entry and exit only. Steps in between are trusted.
- **none** — No schema validation. For performance-critical paths where plugins are well-tested.

Validation failures produce an `EngineError` with `Severity::Fatal`.

## Event Bus

The event bus is part of the workflow engine. It provides:

- **Event emission** — Plugins with the `events:emit` capability can emit named events.
- **Event matching** — Workflows with an `event` trigger are activated when a matching event is emitted.
- **System events** — Core emits system events (plugin loaded, plugin failed, storage error) that workflows can react to.

Events are the mechanism for asynchronous, decoupled communication between plugins (via intermediate workflows).

## Scheduler

The built-in cron scheduler is part of the workflow engine. Workflows with a `schedule` trigger are fired at the specified interval using standard cron syntax.

The scheduler, event bus, and endpoint triggers are unified — they all activate the same workflow pipeline. There is no separate "scheduled task" system.

## Error Propagation

The workflow engine uses the `EngineError` trait for all error propagation at module boundaries:

```rust
trait EngineError: std::error::Error {
    fn code(&self) -> &str;         // e.g., "WORKFLOW_001"
    fn severity(&self) -> Severity; // Fatal, Retryable, Warning
    fn source_module(&self) -> &str;
}
```

The workflow engine uses severity to decide behavior:

- **Fatal** — Abort the pipeline, run error handler if configured
- **Retryable** — Retry the step up to the configured limit, then fail
- **Warning** — Log and continue

## Plugin Invocation

Plugins are WASM modules invoked via Extism. The workflow engine does not compile against any plugin. At runtime, for each step:

1. Look up the plugin by ID from the loaded plugin registry
2. Serialize the input PipelineMessage
3. Call the plugin's declared action via Extism
4. Deserialize the output PipelineMessage
5. Apply validation if configured
6. Pass to the next step or return as the final result

## Dependencies

The workflow-engine crate depends on:

- `packages/types` — PipelineMessage, CDM types, envelopes, shared enums
- `packages/traits` — EngineError trait, Plugin trait

It does not depend on storage, auth, crypto, or any transport.

## Acceptance Criteria

- Workflows are loaded from YAML files in the configured directory at startup
- Three trigger types work: endpoint, event, and schedule
- Sync mode completes all steps before responding; async mode returns a job ID immediately
- Steps execute in sequence with PipelineMessage flowing between them
- Conditional branching routes to correct branches based on output content
- The halt error strategy stops the workflow and produces an EngineError with Fatal severity
- The skip error strategy logs a warning, skips the step, and continues with the previous step's output
- The retry error strategy retries with exponential backoff and supports a fallback step
- Validation levels (strict, edges, none) work correctly per workflow
- The event bus matches emitted events to workflow triggers
- The cron scheduler fires workflows on schedule
- All executions (success and failure) are logged with full context
