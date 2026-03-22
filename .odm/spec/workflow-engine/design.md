<!--
domain: workflow-engine
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Workflow Engine

Reference: [[03 - Projects/Life Engine/Design/Core/Workflow]]

## Purpose

This spec defines how Core chains plugins in sequence to process requests. The workflow engine is the primary way Core processes data — every request that involves plugin logic flows through a workflow. Workflows are API-managed and stored in the database; no Core restart is needed to create, update, or delete them.

## Request Pipeline

Every request to Core that triggers a workflow follows this path:

```text
Client Request
      |
      v
+------------------+
|   API Layer      |  1. Receive request
|   Auth           |  2. Authenticate and authorise
|   Validate       |  3. Validate input data structure
+--------+---------+
         |  validated input
         v
+------------------+
|  Workflow Engine |  4. Load workflow definition
|                  |  5. Execute plugins in sequence
|  Plugin A ----+  |
|  Plugin B <---+  |
|  Plugin C <---+  |
+--------+---------+
         |  final output
         v
+------------------+
|   Response       |  6. Return structured response
+------------------+
```

The API layer handles authentication and input validation. The workflow engine handles orchestration. The response is the output of the final workflow step.

## Workflow Definitions

Workflows are API-managed through the `/api/workflows` endpoint:

- `GET /api/workflows` — List all workflow definitions
- `POST /api/workflows` — Create a new workflow
- `GET /api/workflows/{id}` — Get a workflow definition
- `PUT /api/workflows/{id}` — Update a workflow
- `DELETE /api/workflows/{id}` — Delete a workflow

Definitions are stored in the database. Changes take effect immediately without restarting Core.

## Workflow Structure

A workflow definition consists of an identifier, a name, a trigger route, and an ordered list of steps:

```json
{
  "id": "process-email-sync",
  "name": "Email Sync Pipeline",
  "trigger": "/api/plugins/email-connector/sync",
  "steps": [
    {
      "plugin": "email-connector",
      "action": "fetch",
      "on_error": "halt"
    },
    {
      "plugin": "spam-filter",
      "action": "classify",
      "on_error": "skip"
    },
    {
      "plugin": "email-archiver",
      "action": "store",
      "on_error": "halt"
    }
  ]
}
```

- **id** — Unique identifier for the workflow
- **name** — Human-readable label
- **trigger** — The API route that activates this workflow. When a request hits this route, Core executes the workflow instead of routing directly to a plugin.
- **steps** — Ordered list of plugin actions to execute. Each step specifies the plugin ID, the action to invoke, and an error handling strategy.

## Data Flow Between Steps

Each step receives a typed input and produces a typed output. The workflow engine passes the output of step N as the input to step N+1.

```text
Input (validated request body)
  |
  v
Step 1: email-connector.fetch
  Output: { emails: [...] }
  |
  v
Step 2: spam-filter.classify
  Output: { emails: [...], classifications: [...] }
  |
  v
Step 3: email-archiver.store
  Output: { stored: 42, skipped: 3 }
  |
  v
Response (final step output)
```

The workflow engine validates that each step's output shape is compatible with the next step's expected input at workflow creation time. Incompatible workflows are rejected before they can be saved — the user gets a clear error explaining which step pair is incompatible and why.

## Error Handling

Each step declares an `on_error` strategy that determines what happens when that step fails:

- **halt** (default) — Stop the entire workflow immediately and return an error response to the client. The error includes which step failed and why.
- **skip** — Log the error, skip this step, and pass the previous step's output to the next step. Processing continues as if this step did not exist.
- **retry** — Retry the failed step with exponential backoff up to a configurable maximum number of attempts. If all retries fail, the step halts the workflow (same as `halt`).

Failed workflows are logged with the following details:

- The step that failed (index and plugin ID)
- The error message and code
- The input that was passed to the failed step
- The timestamp

## Relationship to Background Scheduler

Workflows and the background scheduler are independent systems. They serve different purposes and do not invoke each other.

- **Workflows** handle request-triggered processing. A client request activates a workflow, which processes data through a plugin chain and returns a response.
- **Scheduler** handles time-triggered tasks. Connector syncs, token rotation, and cleanup jobs run on a cron-like schedule. The scheduler invokes plugin actions directly — it does not run workflows.

See [[03 - Projects/Life Engine/Planning/specs/core/Background Scheduler]] for the scheduler spec.

## Acceptance Criteria

- Workflows can be created, updated, and deleted via the `/api/workflows` API
- Plugins execute in the defined sequence with data flowing from step to step
- The `halt` error strategy stops the workflow and returns an error response
- The `skip` error strategy logs the error, skips the step, and continues with the previous step's output
- The `retry` error strategy retries with exponential backoff before halting
- Incompatible workflows are rejected at creation time with a clear error message
- Workflow results (success and failure) are logged with full context
- Workflow changes take effect immediately without restarting Core
