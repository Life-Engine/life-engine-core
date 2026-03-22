<!--
domain: workflow-engine
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Workflow Engine Tasks

> spec: ./brief.md

## 1.1 — Create workflow definitions table
> spec: ./brief.md
> depends: none

Add a database migration for the `workflow_definitions` table with columns: `id` (primary key), `name`, `trigger` (unique route), `steps` (JSON array), `created_at`, `updated_at`.

- **Files** — `apps/core/src/db/migrations/create_workflow_definitions.rs`
- **AC** — Migration runs successfully. Table schema matches the spec. `trigger` column has a unique constraint.

## 1.2 — Implement workflow CRUD repository
> spec: ./brief.md
> depends: 1.1

Create a `WorkflowRepository` with methods: `create(definition)`, `get(id)`, `list()`, `update(id, definition)`, `delete(id)`. All operations write to the database. Changes take effect immediately.

- **Files** — `apps/core/src/workflow/repository.rs`
- **AC** — All CRUD operations work. Creating a workflow with a duplicate trigger returns an error. Deleted workflows are no longer retrievable.

## 1.3 — Expose workflow CRUD API endpoints
> spec: ./brief.md
> depends: 1.2

Register REST endpoints: `GET /api/workflows`, `POST /api/workflows`, `GET /api/workflows/{id}`, `PUT /api/workflows/{id}`, `DELETE /api/workflows/{id}`. Wire to the repository.

- **Files** — `apps/core/src/api/workflows.rs`
- **AC** — All endpoints return correct status codes and payloads. Create validates required fields. 404 on missing workflows.

## 2.1 — Implement step executor
> spec: ./brief.md
> depends: 1.2

Create a `StepExecutor` that receives a workflow definition and an initial input. For each step, load the plugin, invoke the declared action with the current input, and capture the output for the next step.

- **Files** — `apps/core/src/workflow/executor.rs`
- **AC** — Steps execute in order. Each step receives the previous step's output. Final step's output is returned as the workflow result.

## 2.2 — Wire workflow trigger to request routing
> spec: ./brief.md
> depends: 2.1, 1.3

When a request hits a route matching a workflow's `trigger`, intercept it and pass the validated request body to the step executor instead of routing directly to a plugin.

- **Files** — `apps/core/src/api/router.rs`, `apps/core/src/workflow/executor.rs`
- **AC** — Requests to trigger routes execute the workflow. Requests to non-trigger routes route normally. Workflow responses are the final step output.

## 3.1 — Implement inter-step data passing
> spec: ./brief.md
> depends: 2.1

Ensure the output of step N is serialised and passed as the input to step N+1. Handle the case where a workflow has zero steps (pass input through unchanged).

- **Files** — `apps/core/src/workflow/executor.rs`
- **AC** — Data passes correctly between steps. Zero-step workflows return the input unchanged. Serialisation/deserialisation is lossless.

## 4.1 — Implement halt strategy
> spec: ./brief.md
> depends: 2.1

When a step with `on_error: "halt"` (or no `on_error` specified) fails, stop the workflow immediately. Return an error response with the failed step index, plugin ID, action, and error message.

- **Files** — `apps/core/src/workflow/error_handler.rs`
- **AC** — Halt stops execution on failure. Error response includes step index, plugin ID, and error details. Subsequent steps do not execute.

## 4.2 — Implement skip strategy
> spec: ./brief.md
> depends: 2.1

When a step with `on_error: "skip"` fails, log the error, skip the step, and pass the previous step's output (not the failed step's output) to the next step.

- **Files** — `apps/core/src/workflow/error_handler.rs`
- **AC** — Skipped step does not block the pipeline. Previous output passes through. Error is logged.

## 4.3 — Implement retry strategy
> spec: ./brief.md
> depends: 2.1

When a step with `on_error: "retry"` fails, retry with exponential backoff (1s, 2s, 4s...) up to a configurable maximum (default 3 attempts). If all retries fail, halt the workflow.

- **Files** — `apps/core/src/workflow/error_handler.rs`
- **AC** — Retries use exponential backoff. Successful retry continues the workflow. Exhausted retries halt with error. Retry count is configurable per step.

## 5.1 — Implement type compatibility checker
> spec: ./brief.md
> depends: 1.2

When a workflow is created or updated, inspect the declared output type of each step's plugin action and the declared input type of the next step's plugin action. Reject incompatible pairs with a 422 error.

- **Files** — `apps/core/src/workflow/type_validator.rs`
- **AC** — Compatible workflows save successfully. Incompatible workflows return 422 with a message naming the incompatible steps and explaining the mismatch.

## 5.2 — Wire validation into CRUD endpoints
> spec: ./brief.md
> depends: 5.1, 1.3

Run the type compatibility checker before saving workflow definitions in the create and update endpoints. Return the validation error to the caller if it fails.

- **Files** — `apps/core/src/api/workflows.rs`
- **AC** — Create and update reject incompatible workflows. Error message is clear and actionable.

## 6.1 — Create workflow logs table
> spec: ./brief.md
> depends: none

Add a database migration for the `workflow_logs` table with columns: `id`, `workflow_id`, `trigger_route`, `status` (success/failure), `started_at`, `finished_at`, `duration_ms`, `steps` (JSON array of per-step details), `error` (optional).

- **Files** — `apps/core/src/db/migrations/create_workflow_logs.rs`
- **AC** — Migration runs successfully. Table schema captures all required fields.

## 6.2 — Expose logs query endpoint
> spec: ./brief.md
> depends: 6.1

Register `GET /api/workflows/{id}/logs` with pagination (`page`, `per_page`) and time-range filtering (`from`, `to` query parameters).

- **Files** — `apps/core/src/api/workflow_logs.rs`
- **AC** — Endpoint returns paginated logs. Time-range filtering works. Empty results return an empty array.

## 7.1 — Implement execution logger
> spec: ./brief.md
> depends: 2.1, 6.1

After each workflow execution (success or failure), write a log entry to the `workflow_logs` table. Include per-step details: step index, plugin ID, action, status, duration, input summary, output summary. For failures, include error message, error code, input to the failed step, and retry count.

- **Files** — `apps/core/src/workflow/logger.rs`
- **AC** — Successful executions are logged with per-step details. Failed executions include error context. Skipped steps are logged with the skip reason.

## 7.2 — Wire logger into executor
> spec: ./brief.md
> depends: 7.1, 2.1

Instrument the step executor to capture timing and status per step. After the workflow completes (or fails), pass the collected data to the execution logger.

- **Files** — `apps/core/src/workflow/executor.rs`, `apps/core/src/workflow/logger.rs`
- **AC** — Every execution produces a log entry. Per-step timing is accurate. Logger does not affect execution performance significantly.
