---
title: "Engine — Workflow Engine"
tags: [life-engine, engine, workflow, pipeline, plugins]
created: 2026-03-14
---

# Workflow Engine

A workflow is an ordered chain of plugins that processes a request. Core's workflow engine loads each plugin in sequence, passing the output of one step as input to the next. Workflows are the primary way Core processes data — every request that involves plugin logic flows through a workflow.

The workflow engine implements several [[03 - Projects/Life Engine/Design/Principles|Design Principles]]: *Fail-Fast with Defined States* (workflow compatibility is validated at creation time — incompatible step chains are rejected before they can run), *Explicit Over Implicit* (steps, error strategies, and trigger routes are explicitly defined in the workflow JSON — no implicit chaining or auto-discovery), and *Separation of Concerns* (the workflow engine only orchestrates — plugins provide the logic, the scheduler handles time-triggered tasks independently).

## Request Pipeline

Every request to Core follows this path:

```
Client Request
      |
      v
+------------------+
|   API Layer      |  1. Receive request
|   Auth (N/Z)     |  2. Authenticate and authorise
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

## Workflow Definitions

Workflows are API-managed — created, updated, and deleted via the REST API at runtime. Definitions are stored in the database. No Core restart needed.

### Structure

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

- **id** — Unique identifier
- **name** — Human-readable label
- **trigger** — The API route that activates this workflow
- **steps** — Ordered list of plugin actions to execute

### Managing Workflows

Workflows are managed through the `/api/workflows` endpoint:

- `GET /api/workflows` — List all workflow definitions
- `POST /api/workflows` — Create a new workflow
- `GET /api/workflows/{id}` — Get a workflow definition
- `PUT /api/workflows/{id}` — Update a workflow
- `DELETE /api/workflows/{id}` — Delete a workflow

## Data Flow Between Steps

Each step receives a typed input and produces a typed output. The workflow engine passes the output of step N as the input to step N+1.

```
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

The workflow engine validates that each step's output shape is compatible with the next step's expected input. Incompatible workflows are rejected at creation time.

## Error Handling

Each step declares an `on_error` strategy:

- **halt** — Stop the entire workflow and return an error response. The default.
- **skip** — Log the error, skip this step, and pass the previous step's output to the next step.
- **retry** — Retry the step up to a configured maximum (with exponential backoff) before halting.

Failed workflows are logged with the step that failed, the error, and the input that caused it.

## Relationship to Background Scheduler

Workflows and the background scheduler are separate systems:

- **Workflows** handle request-triggered processing. A client request activates a workflow, which processes data through a plugin chain and returns a response.
- **Scheduler** handles time-triggered tasks. Connector syncs, token rotation, and cleanup jobs run on a cron-like schedule. See [[03 - Projects/Life Engine/Design/Core/API#Route Groups|API — Route Groups]].

The scheduler may invoke a plugin's sync action directly — it does not run a workflow. They are independent mechanisms.
