<!--
domain: background-scheduler
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Background Scheduler

Reference: [[03 - Projects/Life Engine/Design/Core/Workflow]] and [[03 - Projects/Life Engine/Design/Core/API]]

## Purpose

This spec defines the cron-like scheduler for time-triggered background tasks. The scheduler handles periodic work such as connector syncs, OAuth token rotation, and data cleanup.

## Relationship to Workflows

The scheduler and the workflow engine are independent systems. They serve different purposes and do not invoke each other.

- **Workflows** handle request-triggered processing. A client request activates a workflow, which processes data through a plugin chain and returns a response. See [[03 - Projects/Life Engine/Planning/specs/core/Workflow Engine]].
- **Scheduler** handles time-triggered tasks. Tasks run on a cron-like schedule without any client request. The scheduler invokes plugin actions directly — it does not route through the workflow engine.

## API Management

Scheduled tasks are managed through the `/api/scheduler` endpoint:

- `GET /api/scheduler` — List all scheduled tasks with status
- `GET /api/scheduler/{id}` — Get a single scheduled task
- `POST /api/scheduler` — Create a new scheduled task
- `PUT /api/scheduler/{id}` — Update a scheduled task
- `DELETE /api/scheduler/{id}` — Delete a scheduled task
- `POST /api/scheduler/{id}/run` — Trigger a task immediately, regardless of schedule

## Scheduled Task Types

The scheduler supports four categories of tasks:

- **Connector syncs** — Periodic data fetch from external services. Each connector registers its sync task during `on_load`. The interval is configurable per connector.
- **OAuth token rotation** — Check token expiry and rotate before they expire. Prevents service interruptions from expired credentials.
- **Data cleanup** — Remove expired audit log entries, quarantined records, and temporary data. Keeps storage usage predictable.
- **Custom plugin-scheduled tasks** — Plugins can register their own scheduled tasks during `on_load` for any periodic work they need.

## Task Configuration

Each scheduled task has the following properties:

- **id** — Unique identifier
- **name** — Human-readable label
- **plugin_id** — The plugin that owns and executes the task
- **action** — The plugin action to invoke
- **interval** — Cron expression (e.g., `*/5 * * * *`) or simple duration (e.g., `5m`, `1h`, `24h`)
- **enabled** — Boolean flag. Disabled tasks are skipped by the scheduler loop.
- **last_run** — Timestamp of the last execution (ISO 8601)
- **next_run** — Timestamp of the next scheduled execution (ISO 8601)
- **retry_policy** — Configuration for failure handling:
  - `max_retries` — Maximum number of retry attempts (default 3)
  - `backoff` — Backoff strategy: `exponential` (default) or `fixed`
  - `max_backoff` — Maximum delay between retries (default `5m`)

## Execution Model

The scheduler runs as a background loop within the Core process:

- Tasks are executed as `tokio` spawned tasks. Each task runs concurrently and does not block the scheduler loop or other tasks.
- The scheduler loop checks for due tasks at a fixed interval (every 10 seconds by default).
- When a task is due, the scheduler spawns it and updates `last_run` and `next_run` timestamps.
- Errors in one task do not affect other tasks.
- Long-running tasks are allowed to complete — the scheduler does not enforce a timeout, but individual plugins should implement their own timeouts.

## Backoff on Failure

When a task fails, the retry policy determines what happens:

- The task is retried up to `max_retries` times.
- Exponential backoff doubles the delay between each retry: 1s, 2s, 4s, 8s, and so on, capped at `max_backoff`.
- Fixed backoff uses a constant delay between retries.
- After all retries are exhausted, the task is marked as failed and the error is logged. The task remains scheduled and will attempt to run again at its next regular interval.

## Events

The scheduler emits events on the Core message bus:

- `scheduler.task.complete` — Emitted when a task finishes successfully. Includes task ID, duration, and a summary of the result.
- `scheduler.task.error` — Emitted when a task fails (after all retries are exhausted). Includes task ID, error message, and retry count.

These events are delivered to the SSE stream and to any plugin subscribed to scheduler events.

## Default Scheduled Tasks

Core registers these default tasks on startup:

- **Connector sync** — Each enabled connector registers a sync task. Default interval is 5 minutes, configurable per connector.
- **Token rotation checks** — Checks all stored OAuth tokens for approaching expiry and rotates them proactively. Default interval is every hour.
- **Audit log rotation** — Removes audit log entries older than the retention period (default 90 days). Default interval is daily (once every 24 hours).

## Manual Triggers

`POST /api/scheduler/{task-id}/run` triggers a scheduled task immediately, regardless of its next scheduled run time. The task runs with the same execution model as a scheduled run — spawned as a `tokio` task, with the same retry policy and event emission.

Manual triggers do not affect the task's regular schedule. The `next_run` timestamp remains unchanged.

## Acceptance Criteria

- Scheduled tasks execute at their configured intervals within a reasonable tolerance (within one scheduler loop cycle)
- Failed tasks retry with the configured backoff policy before being marked as failed
- `scheduler.task.complete` and `scheduler.task.error` events are emitted and delivered to SSE and subscribed plugins
- Manual trigger via `POST /api/scheduler/{task-id}/run` executes the task immediately
- Disabled tasks are skipped by the scheduler loop
- Stopping Core gracefully waits for currently running tasks to finish before shutting down
- Default tasks (connector sync, token rotation, audit log rotation) are registered on startup
- Task CRUD via the `/api/scheduler` endpoints works correctly
