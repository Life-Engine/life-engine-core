<!--
domain: workflow-engine
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Workflow Engine Requirements

## 1. Workflow CRUD

Workflows are API-managed and stored in the database.

- **1.1** — WHEN the API is initialised THEN the system SHALL expose `GET /api/workflows` (list), `POST /api/workflows` (create), `GET /api/workflows/{id}` (get), `PUT /api/workflows/{id}` (update), and `DELETE /api/workflows/{id}` (delete) endpoints.
- **1.2** — WHEN a workflow is created or updated, THEN the change SHALL take effect immediately without restarting Core.
- **1.3** — WHEN a workflow definition is stored THEN it SHALL include an `id`, `name`, `trigger` (API route), and an ordered list of `steps`. Each step SHALL specify `plugin` (ID), `action`, and `on_error` strategy.

## 2. Step Execution

Plugins execute in the defined sequence when a workflow is triggered.

- **2.1** — WHEN a request hits the `trigger` route, THEN the workflow engine SHALL load the workflow definition and execute steps in order.
- **2.2** — WHEN a step executes THEN it SHALL receive the validated request body (for step 1) or the output of the previous step (for subsequent steps) as its input.
- **2.3** — WHEN all steps complete successfully, THEN the response SHALL be the output of the final step.
- **2.4** — WHEN a workflow has no steps, THEN it SHALL pass the input through unchanged as the response.

## 3. Data Flow Between Steps

Step outputs are validated and passed as inputs to the next step.

- **3.1** — WHEN step N produces output, THEN the workflow engine SHALL pass that output as the input to step N+1.
- **3.2** — WHEN a workflow is created or updated, THEN the engine SHALL validate type compatibility between each pair of adjacent steps. Incompatible workflows SHALL be rejected with a clear error specifying which step pair is incompatible and why.
- **3.3** — WHEN type validation passes at creation time, THEN the engine SHALL assume runtime data flow is compatible and not re-validate during execution.

## 4. Error Handling

Each step declares an error strategy that determines workflow behaviour on failure.

- **4.1** — WHEN a step with `on_error: "halt"` fails, THEN the workflow SHALL stop immediately and return an error response including the failed step index, plugin ID, and error details.
- **4.2** — WHEN a step with `on_error: "skip"` fails, THEN the engine SHALL log the error, skip the step, and pass the previous step's output to the next step.
- **4.3** — WHEN a step with `on_error: "retry"` fails, THEN the engine SHALL retry with exponential backoff up to a configurable maximum number of attempts. WHEN all retries fail, THEN the step SHALL halt the workflow.
- **4.4** — WHEN no `on_error` is specified for a step, THEN the default strategy SHALL be `halt`.

## 5. Type Compatibility Validation

Workflows are validated at creation time to prevent runtime data flow errors.

- **5.1** — WHEN a workflow definition is submitted via `POST` or `PUT`, THEN the engine SHALL inspect the declared output type of each step and the declared input type of the following step.
- **5.2** — WHEN the output type of step N is incompatible with the input type of step N+1, THEN the API SHALL return a 422 error with a message naming the incompatible steps and explaining the type mismatch.
- **5.3** — WHEN all adjacent step pairs are compatible, THEN the workflow SHALL be saved successfully.

## 6. Logging

All workflow executions are logged with full context.

- **6.1** — WHEN a workflow executes (success or failure), THEN the engine SHALL log the workflow ID, trigger route, timestamp, total duration, and per-step details (step index, plugin ID, action, status, duration, input summary, output summary).
- **6.2** — WHEN a step fails, THEN the log entry SHALL include the error message, error code, the input that was passed to the failed step, and the retry count (if applicable).
- **6.3** — WHEN a step is skipped due to `on_error: "skip"`, THEN the log SHALL record the skip with the original error.
- **6.4** — Workflow logs SHALL be queryable via a `GET /api/workflows/{id}/logs` endpoint with pagination and time-range filtering.
