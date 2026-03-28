<!--
domain: pipeline-executor
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Requirements Document — Pipeline Executor

## Introduction

The pipeline executor is the central runtime of the workflow engine layer. It accepts a `TriggerContext`, resolves the matching workflow definition, executes each step sequentially by calling plugin actions, and assembles a `WorkflowResponse`. It supports two execution modes: sync (inline await) and async (spawned Tokio task with `JobId` tracking). A `JobRegistry` manages the lifecycle of async jobs, and a configurable concurrency semaphore prevents resource exhaustion.

## Alignment with Product Vision

- **Parse, Don't Validate** — `TriggerContext` is a typed enum; each variant maps unambiguously to a `PipelineMessage`, preventing invalid trigger states from reaching the executor
- **Defence in Depth** — Duplicate workflow IDs are rejected at startup; concurrency limits prevent runaway resource usage; step failures are captured and surfaced rather than silently swallowed
- **Principle of Least Privilege** — The executor delegates all data mutation to plugin actions via the pipeline; it does not access storage directly
- **The Pit of Success** — Transport handlers call `execute()` or `spawn()` based on the workflow's `mode` field; the correct path is explicit and hard to misuse
- **Open/Closed Principle** — New trigger types and step error strategies can be added without modifying the executor's core loop

## Requirements

### Requirement 1 — WorkflowExecutor Public API

**User Story:** As a Core developer, I want a single executor with `execute()` and `spawn()` methods so that transport handlers have a clear, mode-aware entry point.

#### Acceptance Criteria

- 1.1. WHEN the executor is initialised THEN it SHALL expose `pub async fn execute(&self, trigger: TriggerContext) -> WorkflowResponse` for sync workflows.
- 1.2. WHEN the executor is initialised THEN it SHALL expose `pub fn spawn(&self, trigger: TriggerContext) -> JobId` for async workflows.
- 1.3. WHEN `execute()` is called THEN the workflow SHALL run inline on the current Tokio task and return the response directly.
- 1.4. WHEN `spawn()` is called THEN the workflow SHALL run on a new Tokio task created via `tokio::spawn` and the method SHALL return a `JobId` immediately.

### Requirement 2 — TriggerContext Conversion

**User Story:** As a Core developer, I want each trigger type to map to a well-defined initial `PipelineMessage` so that steps receive consistent input regardless of how the workflow was triggered.

#### Acceptance Criteria

- 2.1. WHEN a `TriggerContext::Endpoint(WorkflowRequest)` is received THEN the executor SHALL set `PipelineMessage.payload` to the request body and populate metadata with `params`, `query`, and `identity`.
- 2.2. WHEN a `TriggerContext::Event { name, payload, source }` is received THEN the executor SHALL set `PipelineMessage.payload` to the event payload and populate metadata with `name` and `source`.
- 2.3. WHEN a `TriggerContext::Schedule { workflow_id }` is received THEN the executor SHALL set `PipelineMessage.payload` to empty and populate metadata with `workflow_id`, `trigger_type`, and `timestamp`.

### Requirement 3 — Workflow Lookup

**User Story:** As an operator, I want workflow definitions loaded from YAML so that I can manage workflows without recompiling.

#### Acceptance Criteria

- 3.1. WHEN the engine starts THEN the executor SHALL load all workflow definitions from the configured YAML directory into a `HashMap<String, WorkflowDefinition>`.
- 3.2. WHEN loading is complete THEN the map SHALL be immutable at runtime.
- 3.3. WHEN two workflow files define the same workflow ID THEN the engine SHALL reject startup with an error identifying the duplicate ID.
- 3.4. WHEN `execute()` or `spawn()` is called with a trigger that resolves to an unknown workflow ID THEN the executor SHALL return a `WorkflowResponse` with an appropriate error status.

### Requirement 4 — Sequential Step Execution

**User Story:** As a plugin author, I want each step to receive the previous step's output so that I can build composable data pipelines.

#### Acceptance Criteria

- 4.1. WHEN a workflow is executed THEN the executor SHALL iterate through the workflow's step list in order.
- 4.2. WHEN a step begins THEN the executor SHALL clone the current `PipelineMessage` as a pre-step snapshot.
- 4.3. WHEN a plugin action completes successfully THEN the action's output SHALL replace the current `PipelineMessage`.
- 4.4. WHEN a plugin action fails THEN the executor SHALL apply the step's `on_error` strategy as defined in the workflow definition.

### Requirement 5 — StepTrace Recording

**User Story:** As a workflow author, I want step-level tracing so that I can diagnose which step failed or was slow.

#### Acceptance Criteria

- 5.1. WHEN a step completes (success, skip, or failure) THEN the executor SHALL append a `StepTrace` to the message's metadata.
- 5.2. WHEN a `StepTrace` is created THEN it SHALL contain `plugin_id`, `action`, `duration_ms`, and `status` fields.
- 5.3. WHEN a step's status is recorded THEN it SHALL be one of `Completed`, `Skipped`, or `Failed`.

### Requirement 6 — WorkflowResponse Construction

**User Story:** As a transport handler, I want a structured response that includes status, data, errors, and trace information so that I can return meaningful results to callers.

#### Acceptance Criteria

- 6.1. WHEN the executor finishes all steps THEN it SHALL build a `WorkflowResponse` from the final pipeline state.
- 6.2. WHEN `metadata.status_hint` is set THEN the response status SHALL use that value.
- 6.3. WHEN `metadata.status_hint` is not set THEN the response status SHALL default to `Ok`.
- 6.4. WHEN the response is built THEN `data` SHALL contain the final `PipelineMessage` payload.
- 6.5. WHEN any steps were skipped due to non-fatal errors THEN `errors` SHALL contain those error details as warnings.
- 6.6. WHEN the response is built THEN `meta` SHALL contain the request ID, total execution duration, and the accumulated list of `StepTrace` entries.

### Requirement 7 — Async Job Lifecycle

**User Story:** As a client, I want to poll async job status and retrieve results so that I can consume the output of long-running workflows.

#### Acceptance Criteria

- 7.1. WHEN `spawn()` is called THEN the executor SHALL register a new entry in the `JobRegistry` with status `InProgress`.
- 7.2. WHEN an async workflow completes successfully THEN the job status SHALL update to `Completed` and the `WorkflowResponse` SHALL be stored in the entry.
- 7.3. WHEN an async workflow fails THEN the job status SHALL update to `Failed` and the error details SHALL be stored in the entry.
- 7.4. WHEN a caller queries `GET /api/v1/jobs/:id` THEN the system SHALL return the current `JobEntry` status and response if available.
- 7.5. WHEN a job's `created_at` exceeds the configured TTL (default 1 hour) THEN the registry SHALL evict the entry.

### Requirement 8 — JobRegistry

**User Story:** As a Core developer, I want an in-memory job registry so that async job state is managed without external dependencies.

#### Acceptance Criteria

- 8.1. WHEN the `JobRegistry` is created THEN it SHALL use a `RwLock<HashMap<JobId, JobEntry>>` for concurrent access.
- 8.2. WHEN a `JobEntry` is created THEN it SHALL contain `status` (InProgress, Completed, Failed), `response` (optional `WorkflowResponse`), and `created_at` fields.
- 8.3. WHEN the engine restarts THEN the registry SHALL be empty — it is purely in-memory and does not persist across restarts.
- 8.4. WHEN side effects have been produced by a workflow THEN those effects SHALL survive independently of the registry's lifecycle.

### Requirement 9 — Concurrency Limit

**User Story:** As an operator, I want a concurrency limit on workflow executions so that the engine does not exhaust resources under load.

#### Acceptance Criteria

- 9.1. WHEN the executor is initialised THEN it SHALL enforce a configurable concurrency limit with a default of 32 simultaneous executions.
- 9.2. WHEN the concurrency limit is reached THEN additional executions SHALL queue until a slot becomes available.
- 9.3. WHEN the concurrency limit is configured to a value other than 32 THEN the executor SHALL use the configured value.

### Requirement 10 — Deferred Capabilities

**User Story:** As a Core developer, I want to know what is out of scope for v1 so that I do not implement features prematurely.

#### Acceptance Criteria

- 10.1. WHEN a workflow definition requests parallel step execution (fan-out/fan-in) THEN the executor SHALL reject the definition at load time with an unsupported-feature error.
- 10.2. WHEN a workflow definition specifies a workflow-level timeout THEN the executor SHALL ignore the field in v1 and log a warning.
