<!--
domain: workflow-engine
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Workflow Engine Tasks

> spec: ./brief.md

## 1.1 — Scaffold workflow-engine crate
> spec: ./brief.md
> depends: none

Create the `packages/workflow-engine/` crate with the standard internal layout: `lib.rs`, `config.rs`, `error.rs`, `types.rs`, and empty module files for `loader.rs`, `executor.rs`, `event_bus.rs`, `scheduler.rs`. Add `Cargo.toml` with dependencies on `types` and `traits` crates. Add `project.json` for Nx.

- **Files** — `packages/workflow-engine/Cargo.toml`, `packages/workflow-engine/src/lib.rs`, `packages/workflow-engine/src/config.rs`, `packages/workflow-engine/src/error.rs`, `packages/workflow-engine/src/types.rs`, `packages/workflow-engine/src/loader.rs`, `packages/workflow-engine/src/executor.rs`, `packages/workflow-engine/src/event_bus.rs`, `packages/workflow-engine/src/scheduler.rs`, `packages/workflow-engine/project.json`
- **AC** — Crate compiles. `lib.rs` re-exports the public API. Config struct accepts a workflow directory path. Error types implement the EngineError trait with severity levels.

## 1.2 — Define workflow types
> spec: ./brief.md
> depends: 1.1

Define the internal types: `WorkflowDef` (id, name, mode, validate, trigger, steps), `StepDef` (plugin, action, on_error), `TriggerDef` (endpoint, event, schedule — all optional), `ErrorStrategy` (halt, skip, retry with max_retries and fallback), `ExecutionMode` (sync, async), `ValidationLevel` (strict, edges, none), `ConditionDef` (field, equals, then, else).

- **Files** — `packages/workflow-engine/src/types.rs`
- **AC** — All types are defined with serde Deserialize for YAML parsing. `ExecutionMode` defaults to `sync`. `ValidationLevel` defaults to `edges`. `ErrorStrategy` defaults to `halt`.

## 2.1 — Implement YAML workflow loader
> spec: ./brief.md
> depends: 1.2

Implement `loader.rs` to discover and parse all `*.yaml` files from the configured workflow directory. Validate required fields (`id`, `steps`). Detect and reject duplicate `id` values and duplicate `endpoint` triggers across files.

- **Files** — `packages/workflow-engine/src/loader.rs`
- **AC** — All valid YAML files are parsed into `WorkflowDef` values. Invalid files produce clear errors with the filename. Duplicate IDs and duplicate endpoints fail startup with a clear error identifying the conflicting files.

## 2.2 — Build trigger registry
> spec: ./brief.md
> depends: 2.1

After loading all workflows, build an in-memory trigger registry that maps endpoint paths, event names, and cron expressions to their corresponding workflow definitions. The executor, event bus, and scheduler all query this registry.

- **Files** — `packages/workflow-engine/src/loader.rs`, `packages/workflow-engine/src/types.rs`
- **AC** — Endpoint lookup by HTTP method + path works. Event lookup by event name works. Schedule entries are enumerable for the scheduler. Registry is immutable after construction.

## 3.1 — Implement sequential step executor
> spec: ./brief.md
> depends: 1.2

Create the pipeline executor in `executor.rs`. For each step in a workflow, invoke the declared plugin action via Extism with the current PipelineMessage and capture the output PipelineMessage for the next step. Return the final step's output as the workflow result.

- **Files** — `packages/workflow-engine/src/executor.rs`
- **AC** — Steps execute in order. Each step receives the previous step's PipelineMessage output. Final step's output is returned as the workflow result. Plugin actions are called via Extism.

## 3.2 — Implement sync and async execution modes
> spec: ./brief.md
> depends: 3.1

Add execution mode handling. For `sync`, await all steps and return the result. For `async`, spawn the pipeline on a background task, return a job ID immediately, and emit a completion/failure event when done.

- **Files** — `packages/workflow-engine/src/executor.rs`
- **AC** — Sync mode blocks until all steps complete and returns the result. Async mode returns a job ID immediately. Background execution emits an event on completion or failure with the job ID and status.

## 3.3 — Implement initial PipelineMessage construction
> spec: ./brief.md
> depends: 3.1

Construct the initial PipelineMessage from the trigger context: request body for endpoint triggers, event payload for event triggers, empty payload for schedule triggers. Populate MessageMetadata with a new correlation ID, source (trigger type + value), and timestamp.

- **Files** — `packages/workflow-engine/src/executor.rs`
- **AC** — Endpoint triggers produce a PipelineMessage with the request body as payload. Event triggers produce a PipelineMessage with the event payload. Schedule triggers produce a PipelineMessage with an empty payload. All messages have a unique correlation ID.

## 4.1 — Implement halt error strategy
> spec: ./brief.md
> depends: 3.1

When a step with `on_error.strategy: halt` (or no `on_error` specified) fails, stop the workflow immediately. Construct an EngineError with `Severity::Fatal` including the failed step index, plugin ID, action, and underlying error.

- **Files** — `packages/workflow-engine/src/executor.rs`, `packages/workflow-engine/src/error.rs`
- **AC** — Halt stops execution on failure. EngineError includes step index, plugin ID, and error details with Fatal severity. Subsequent steps do not execute.

## 4.2 — Implement skip error strategy
> spec: ./brief.md
> depends: 3.1

When a step with `on_error.strategy: skip` fails, log a warning, skip the step, and pass the previous step's PipelineMessage (not the failed step's output) to the next step.

- **Files** — `packages/workflow-engine/src/executor.rs`
- **AC** — Skipped step does not block the pipeline. Previous PipelineMessage passes through. Warning is logged with the original error.

## 4.3 — Implement retry error strategy with fallback
> spec: ./brief.md
> depends: 3.1

When a step with `on_error.strategy: retry` fails, retry with exponential backoff (1s, 2s, 4s...) up to `max_retries` attempts. If all retries fail and a `fallback` step is declared, execute the fallback step. If no fallback is declared, halt the workflow.

- **Files** — `packages/workflow-engine/src/executor.rs`
- **AC** — Retries use exponential backoff. Successful retry continues the workflow. Exhausted retries execute the fallback step if declared. Exhausted retries with no fallback halt with an EngineError. Retry count is configurable per step.

## 4.4 — Handle EngineError severity from plugins
> spec: ./brief.md
> depends: 4.1, 4.2, 4.3

When a plugin returns an EngineError with `Severity::Retryable`, treat it as retryable regardless of the step's declared strategy. When a plugin returns `Severity::Warning`, log the warning and continue execution.

- **Files** — `packages/workflow-engine/src/executor.rs`
- **AC** — Retryable errors are retried even if the step declares halt. Warning errors are logged and do not interrupt the pipeline. Fatal errors always halt.

## 5.1 — Implement conditional branching
> spec: ./brief.md
> depends: 3.1

When a `condition` step is encountered, evaluate the condition against the current PipelineMessage. Support dot-notation field access (e.g., `payload.category`) and an `equals` comparator. Route to the `then` or `else` branch and execute those steps.

- **Files** — `packages/workflow-engine/src/executor.rs`
- **AC** — Condition evaluates correctly against PipelineMessage fields. Matching routes to `then` branch. Non-matching routes to `else` branch. Branch steps execute with the same PipelineMessage data flow.

## 6.1 — Implement pipeline validation
> spec: ./brief.md
> depends: 3.1

Implement configurable validation per workflow. For `strict`, validate the PipelineMessage output at every step boundary. For `edges`, validate at pipeline entry and exit only. For `none`, skip validation entirely. Validation failures produce an EngineError with `Severity::Fatal`.

- **Files** — `packages/workflow-engine/src/executor.rs`
- **AC** — Strict validates at every boundary. Edges validates entry and exit only. None skips validation. Validation failures halt the pipeline with a Fatal EngineError.

## 7.1 — Implement event bus
> spec: ./brief.md
> depends: 2.2

Create the event bus in `event_bus.rs`. Accept emitted events (from plugins with `events:emit` capability or from the system), match them against workflow `event` triggers in the registry, and dispatch matching workflows to the executor.

- **Files** — `packages/workflow-engine/src/event_bus.rs`
- **AC** — Emitted events are matched to workflow triggers. Matching workflows are executed. Multiple workflows with the same event trigger all execute independently. System events are emitted for plugin lifecycle and storage errors.

## 7.2 — Implement cron scheduler
> spec: ./brief.md
> depends: 2.2

Create the scheduler in `scheduler.rs`. At startup, register all workflows with `schedule` triggers using their cron expressions. When a cron interval elapses, trigger the corresponding workflow with an empty PipelineMessage. If a scheduled workflow fails, emit an error event and continue scheduling.

- **Files** — `packages/workflow-engine/src/scheduler.rs`
- **AC** — All schedule triggers are registered at startup. Workflows fire at the correct intervals. Failed scheduled workflows emit error events. The scheduler continues after failures.

## 8.1 — Wire endpoint triggers to transports
> spec: ./brief.md
> depends: 2.2, 3.1

Expose a method on the workflow engine that transports call to check if an incoming request path matches a workflow endpoint trigger. If matched, the transport passes the request to the workflow engine instead of handling it directly.

- **Files** — `packages/workflow-engine/src/lib.rs`
- **AC** — Transports can query the workflow engine for endpoint matches. Matched requests are routed to the workflow pipeline. Unmatched requests are handled by the transport normally.

## 9.1 — Implement execution logging
> spec: ./brief.md
> depends: 3.1

After each workflow execution (success or failure), log the workflow ID, trigger type and value, timestamp, total duration, and per-step details (step index, plugin ID, action, status, duration). For failures, include error message, error code, severity, input to the failed step, and retry count.

- **Files** — `packages/workflow-engine/src/executor.rs`
- **AC** — Successful executions are logged with per-step details. Failed executions include error context and severity. Skipped steps are logged with the skip reason and original error.

## 9.2 — Wire logging into executor
> spec: ./brief.md
> depends: 9.1

Instrument the step executor to capture timing and status per step. After the workflow completes (or fails), emit the collected execution data as a structured log entry.

- **Files** — `packages/workflow-engine/src/executor.rs`
- **AC** — Every execution produces a structured log entry. Per-step timing is accurate. Logging does not significantly affect execution performance.
