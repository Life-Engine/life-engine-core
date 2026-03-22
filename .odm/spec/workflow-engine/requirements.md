<!--
domain: workflow-engine
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Workflow Engine Requirements

## 1. Workflow Loading

Workflows are YAML config files read from a configured directory at startup.

- **1.1** — WHEN Core starts THEN the workflow engine SHALL read all `*.yaml` files from the directory specified by `[workflows] path` in `config.toml`.
- **1.2** — WHEN a YAML file is loaded THEN the engine SHALL parse and validate it against the workflow schema, rejecting files with missing required fields (`id`, `steps`) or invalid values.
- **1.3** — WHEN a workflow definition is loaded THEN it SHALL contain: `id` (unique identifier), `name` (human-readable label), `mode` (`sync` or `async`), `validate` (`strict`, `edges`, or `none`), `trigger` (one or more of: `endpoint`, `event`, `schedule`), and `steps` (ordered list of plugin actions with optional `on_error` config).
- **1.4** — WHEN two workflow files declare the same `id` THEN the engine SHALL fail startup with a clear error identifying the conflicting files.
- **1.5** — WHEN two workflow files declare the same `endpoint` trigger THEN the engine SHALL fail startup with a clear error identifying the conflicting endpoint.

## 2. Triggers

A workflow can be activated by an endpoint, an event, or a schedule. All trigger types execute the same pipeline.

- **2.1** — WHEN a request hits an HTTP path matching a workflow's `endpoint` trigger THEN the workflow engine SHALL execute that workflow's pipeline.
- **2.2** — WHEN an event is emitted matching a workflow's `event` trigger THEN the workflow engine SHALL execute that workflow's pipeline.
- **2.3** — WHEN a cron interval elapses matching a workflow's `schedule` trigger THEN the workflow engine SHALL execute that workflow's pipeline.
- **2.4** — WHEN a workflow declares multiple trigger types THEN each trigger SHALL independently activate the same pipeline.

## 3. Execution Modes

Workflows execute in either sync or async mode.

- **3.1** — WHEN a workflow with `mode: sync` is triggered THEN the engine SHALL execute all steps and return the final output through the transport before responding.
- **3.2** — WHEN a workflow with `mode: async` is triggered THEN the engine SHALL return a job ID immediately and execute steps in the background.
- **3.3** — WHEN an async workflow completes or fails THEN the engine SHALL emit an event containing the job ID and result status.

## 4. Step Execution

Plugins execute in the defined sequence when a workflow is triggered.

- **4.1** — WHEN a workflow is triggered THEN the engine SHALL execute steps in order, passing the PipelineMessage output of step N as input to step N+1.
- **4.2** — WHEN step 1 executes THEN it SHALL receive a PipelineMessage constructed from the trigger context (request body for endpoints, event payload for events, empty payload for schedules).
- **4.3** — WHEN all steps complete successfully THEN the final step's PipelineMessage output SHALL be the workflow result.
- **4.4** — WHEN a step executes THEN the engine SHALL invoke the declared plugin action via Extism, passing and receiving PipelineMessage values.

## 5. Data Flow

Steps communicate through PipelineMessage envelopes containing typed payloads.

- **5.1** — WHEN data flows between steps THEN it SHALL use the PipelineMessage envelope containing MessageMetadata (correlation ID, source, timestamp, auth context) and TypedPayload (CDM type or custom schema-validated value).
- **5.2** — WHEN step N produces output THEN the engine SHALL pass that PipelineMessage as input to step N+1, preserving the metadata correlation ID across the entire pipeline.

## 6. Control Flow

v1 supports sequential execution, conditional branching, and per-step error handling.

- **6.1** — WHEN no control flow directives are present THEN steps SHALL execute sequentially in declaration order.
- **6.2** — WHEN a `condition` step is encountered THEN the engine SHALL evaluate the condition against the current PipelineMessage and route to the `then` or `else` branch accordingly.
- **6.3** — WHEN a condition evaluates a `field` path THEN it SHALL support dot-notation access into the PipelineMessage payload (e.g., `payload.category`).

## 7. Error Handling

Each step declares an error strategy that determines workflow behavior on failure.

- **7.1** — WHEN a step with `on_error.strategy: halt` (or no `on_error` specified) fails THEN the workflow SHALL stop immediately. The engine SHALL construct an EngineError with `Severity::Fatal` including the failed step index, plugin ID, and error details.
- **7.2** — WHEN a step with `on_error.strategy: skip` fails THEN the engine SHALL log a warning, skip the step, and pass the previous step's PipelineMessage to the next step.
- **7.3** — WHEN a step with `on_error.strategy: retry` fails THEN the engine SHALL retry with exponential backoff up to `max_retries` attempts. WHEN all retries are exhausted AND a `fallback` step is declared THEN the engine SHALL execute the fallback step. WHEN all retries are exhausted AND no fallback is declared THEN the engine SHALL halt the workflow.
- **7.4** — WHEN a plugin action returns an EngineError with `Severity::Retryable` THEN the engine SHALL treat it as retryable regardless of the step's declared strategy. WHEN a plugin returns `Severity::Warning` THEN the engine SHALL log the warning and continue.

## 8. Validation

Schema validation at pipeline boundaries is configurable per workflow.

- **8.1** — WHEN a workflow declares `validate: strict` THEN the engine SHALL validate the PipelineMessage output schema at every step boundary before passing it to the next step.
- **8.2** — WHEN a workflow declares `validate: edges` (or omits the validate field) THEN the engine SHALL validate the PipelineMessage at pipeline entry and at final output only.
- **8.3** — WHEN a workflow declares `validate: none` THEN the engine SHALL not perform schema validation during execution.
- **8.4** — WHEN validation fails THEN the engine SHALL produce an EngineError with `Severity::Fatal` and halt the pipeline.

## 9. Event Bus

The event bus is part of the workflow engine, enabling decoupled communication between plugins via intermediate workflows.

- **9.1** — WHEN a plugin with the `events:emit` capability emits a named event THEN the event bus SHALL match it against all workflow `event` triggers and activate matching workflows.
- **9.2** — WHEN a system event occurs (plugin loaded, plugin failed, storage error) THEN the engine SHALL emit a system event that workflows can react to.
- **9.3** — WHEN multiple workflows have the same event trigger THEN all matching workflows SHALL be activated independently.

## 10. Scheduler

The cron scheduler is part of the workflow engine. There is no separate scheduled task system.

- **10.1** — WHEN Core starts THEN the scheduler SHALL register all workflows with `schedule` triggers using their cron expressions.
- **10.2** — WHEN a cron interval elapses THEN the scheduler SHALL trigger the corresponding workflow with an empty PipelineMessage.
- **10.3** — WHEN a scheduled workflow fails THEN the scheduler SHALL emit an error event and continue scheduling future executions.

## 11. Logging

All workflow executions are logged with full context.

- **11.1** — WHEN a workflow executes (success or failure) THEN the engine SHALL log the workflow ID, trigger type, trigger value, timestamp, total duration, and per-step details (step index, plugin ID, action, status, duration).
- **11.2** — WHEN a step fails THEN the log entry SHALL include the error message, error code, severity, the input that was passed to the failed step, and the retry count (if applicable).
- **11.3** — WHEN a step is skipped due to `on_error.strategy: skip` THEN the log SHALL record the skip with the original error.
