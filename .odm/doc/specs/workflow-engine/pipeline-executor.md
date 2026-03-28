---
title: Pipeline Executor Specification
type: reference
created: 2026-03-28
status: active
tags:
  - spec
  - workflow
  - pipeline
---

# Pipeline Executor Specification

The pipeline executor runs workflow steps in sequence, manages sync and async execution modes, and builds the final response. It is the central runtime component of the [[workflow|workflow engine layer]].

## Public API

```rust
impl WorkflowExecutor {
    pub async fn execute(&self, trigger: TriggerContext) -> WorkflowResponse;
    pub fn spawn(&self, trigger: TriggerContext) -> JobId;
}
```

The workflow definition's `mode` field determines which path the caller takes:

- **sync** — The handler awaits `execute()`. The task runs inline on the current Tokio task.
- **async** — The handler calls `spawn()`, which returns a `JobId` immediately and runs the workflow on a separate Tokio task.

## TriggerContext

```rust
pub enum TriggerContext {
    Endpoint(WorkflowRequest),
    Event { name: String, payload: Option<Value>, source: String },
    Schedule { workflow_id: String },
}
```

Each variant maps to an initial `PipelineMessage` as follows:

- **Endpoint** — `body` becomes `payload`. `params`, `query`, and `identity` populate metadata fields.
- **Event** — Event `payload` becomes `payload`. Event `name` and `source` populate metadata fields.
- **Schedule** — `payload` is empty. `workflow_id`, `trigger_type`, and `timestamp` populate metadata fields.

## Workflow Lookup

Workflow definitions are stored in a `HashMap<String, WorkflowDefinition>`, loaded from a YAML directory at startup. The map is immutable at runtime.

Duplicate workflow IDs must reject startup with an error.

## Execution Model

Each execution runs as a Tokio task:

- **Sync** — The handler awaits `execute()`. The task runs inline.
- **Async** — `tokio::spawn` creates a new task. The caller receives a `JobId` immediately.
- A configurable concurrency limit applies (default 32). Excess executions queue until a slot is available.

## Step Execution

Each step follows this sequence:

1. Clone the current `PipelineMessage` (pre-step snapshot).
2. Call the plugin action with the current message.
3. On success: the action's output replaces the current message.
4. Append a `StepTrace` to metadata.
5. On failure: apply the step's `on_error` strategy (see [[control-flow]]).

## StepTrace

```rust
pub struct StepTrace {
    pub plugin_id: String,
    pub action: String,
    pub duration_ms: u64,
    pub status: StepStatus, // Completed, Skipped, Failed
}
```

Every step produces a `StepTrace`, regardless of outcome.

## Building WorkflowResponse

The executor builds the final `WorkflowResponse` from:

- **status** — Uses `metadata.status_hint` if set, otherwise defaults to `Ok`.
- **data** — The final `PipelineMessage` payload.
- **errors** — Non-fatal errors from skipped steps, included as warnings.
- **meta** — Request ID echo, total timing, and the accumulated list of `StepTrace` entries.

## Async Job Lifecycle

1. `spawn()` registers the `JobId` in the `JobRegistry` with status `InProgress`.
2. On completion: status updates to `Completed` or `Failed`.
3. Callers poll via `system.job.status` (`GET /api/v1/jobs/:id`).
4. Results are retained for a configurable TTL (default 1 hour), then evicted.

## JobRegistry

```rust
pub struct JobRegistry {
    jobs: RwLock<HashMap<JobId, JobEntry>>,
}

pub struct JobEntry {
    pub status: JobStatus, // InProgress, Completed, Failed
    pub response: Option<WorkflowResponse>,
    pub created_at: Instant,
}
```

The registry is in-memory and lost on restart. Side effects produced by the workflow survive independently of the registry.

## Deferred

The following capabilities are out of scope for v1:

- Parallel step execution (fan-out/fan-in)
- Workflow-level timeouts
