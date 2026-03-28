---
title: Pipeline Executor
type: adr
created: 2026-03-28
status: draft
tags:
  - architecture
  - workflow-engine
  - executor
  - core
---

# Pipeline Executor

## Overview

The pipeline executor is the runtime that loads a workflow definition, runs its steps in sequence, and passes `PipelineMessage` between them. It is the core of the workflow engine — everything else (triggers, events, schedules) ultimately calls the executor.

## Public API

The executor exposes two methods:

```rust
impl WorkflowExecutor {
    /// Run a workflow synchronously. Blocks until all steps complete.
    pub async fn execute(&self, trigger: TriggerContext) -> WorkflowResponse;

    /// Spawn a workflow asynchronously. Returns a job ID immediately.
    pub fn spawn(&self, trigger: TriggerContext) -> JobId;
}
```

The caller does not choose which method to use — the executor checks the workflow definition's `mode` field (`sync` or `async`) and behaves accordingly. Both methods accept a `TriggerContext`.

## TriggerContext

A trigger-agnostic input type that wraps the context from whichever trigger activated the workflow:

```rust
pub enum TriggerContext {
    Endpoint(WorkflowRequest),
    Event { name: String, payload: Option<Value>, source: String },
    Schedule { workflow_id: String },
}
```

The executor builds the initial `PipelineMessage` from this context:

- **Endpoint** — The `WorkflowRequest` body becomes the payload. Params, query, and identity go into metadata.
- **Event** — The event's `Value` payload becomes the `PipelineMessage` payload. Event name and source go into metadata.
- **Schedule** — An empty `PipelineMessage` with only metadata (workflow ID, trigger type, timestamp). Steps that need data fetch it themselves.

## Workflow Lookup

The executor owns all loaded workflow definitions in a `HashMap<String, WorkflowDefinition>`. Workflow YAML files are read from a configured directory at startup. The map is immutable at runtime — adding or changing a workflow requires restarting Core.

If two YAML files declare the same `id`, Core rejects startup with a clear error message.

## Execution Model

Each workflow execution is a Tokio task:

- **Sync workflows** — The transport handler `await`s the executor's `execute` method. The Tokio task runs inline.
- **Async workflows** — The executor calls `tokio::spawn` and returns a `JobId` immediately. The task runs in the background.

A configurable concurrency limit (default 32) caps the number of concurrent workflow tasks. Excess workflows queue until a slot is available. This prevents runaway schedules or event cascades from exhausting resources.

## Step Execution

Steps run sequentially. For each step:

1. Clone the current `PipelineMessage` (the pre-step snapshot)
2. Call the plugin action with the current message
3. If the step succeeds, the output `PipelineMessage` replaces the current message
4. Append a `StepTrace` to the message metadata (plugin ID, action name, duration, status)
5. If the step fails, apply the step's `on_error` strategy (see [[control-flow]])

The pre-step clone is held for error recovery. If a step fails with `on_error: skip`, the pre-step message is passed to the next step. If `on_error: retry`, the pre-step message is replayed as input.

## PipelineMessage Metadata

Metadata accumulates as the message passes through steps. Each step appends a `StepTrace`:

```rust
pub struct StepTrace {
    pub plugin_id: String,
    pub action: String,
    pub duration_ms: u64,
    pub status: StepStatus,  // Completed, Skipped, Failed
}
```

The final `WorkflowResponse` includes the full trace in `ResponseMeta`. The transport handler can strip it before sending to the client if unwanted.

## Building the WorkflowResponse

After the final step completes, the executor translates the `PipelineMessage` into a `WorkflowResponse`:

- **status** — Read from `metadata.status_hint` if set by a plugin, otherwise defaults to `Ok`. System workflows like `collection.create` set `Created` in their default definitions.
- **data** — The final `PipelineMessage` payload.
- **errors** — Any non-fatal errors from skipped steps, included as warnings. The caller knows something was degraded even though the workflow completed.
- **meta** — Request ID echo, timing, and the accumulated `StepTrace` list.

## Async Job Lifecycle

When an async workflow is spawned:

1. The executor creates a `JobId` and registers it in the `JobRegistry` with status `InProgress`
2. The Tokio task runs in the background
3. On completion, the executor updates the registry with the `WorkflowResponse` and status `Completed` (or `Failed`)
4. The caller polls `system.job.status` via `GET /api/v1/jobs/:id` to check progress
5. Completed job results are retained for a configurable TTL (default 1 hour). After TTL, the result is dropped but the job ID returns `Completed` with no data.

## JobRegistry

The `JobRegistry` is a sibling component shared via `Arc` between the executor and transport handlers:

```rust
pub struct JobRegistry {
    jobs: RwLock<HashMap<JobId, JobEntry>>,
}

pub struct JobEntry {
    pub status: JobStatus,           // InProgress, Completed, Failed
    pub response: Option<WorkflowResponse>,
    pub created_at: Instant,
}
```

The executor writes to it; transport handlers read from it. It is an in-memory concurrent map — jobs are lost on restart. This is acceptable because async workflows produce side effects (storage writes) that survive independently of the job record.

## Logging

The executor logs at structured JSON:

- Workflow start (workflow ID, trigger type)
- Each step start and completion (plugin ID, action, duration)
- Step failures (error detail, `on_error` strategy applied)
- Workflow completion (total duration, step count, status)

Logs are separate from the `StepTrace` in metadata. Logs go to the logging infrastructure; `StepTrace` goes in the response.

## Parallel Step Execution

Deferred to post-v1. All steps execute sequentially. The `PipelineMessage` passing model (output of step N is input to step N+1) depends on this. Parallel execution (fan-out/fan-in) would require a different data flow model and is noted as a future consideration.

## Workflow-Level Timeout

Deferred to post-v1. Individual steps can timeout via the Extism host-level plugin execution timeout, but there is no "this entire workflow must complete in N seconds" setting.
