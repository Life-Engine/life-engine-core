<!--
domain: background-scheduler
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Requirements Document — Background Scheduler

## Introduction

Core needs a background scheduler to handle periodic, time-triggered work that does not originate from client requests. This includes connector data syncs, OAuth token rotation, audit log cleanup, and custom plugin-scheduled tasks. The scheduler runs as a tokio background loop, executes tasks concurrently, supports configurable retry policies, and emits events on the message bus for observability.

## Alignment with Product Vision

- **Data freshness** — Periodic connector syncs keep local data up to date with external services without user intervention.
- **Security** — Proactive token rotation prevents expired credential errors and maintains continuous service access.
- **Operational simplicity** — Default tasks are registered automatically on startup; operators only configure intervals.
- **Observability** — Events emitted on task completion and failure provide visibility into background operations.

## Requirements

### Requirement 1 — Task CRUD

**User Story:** As a Core operator, I want to create, read, update, and delete scheduled tasks via REST API, so that I can manage background work without restarting Core.

#### Acceptance Criteria

- 1.1. WHEN a client sends `GET /api/scheduler` THEN the system SHALL return a list of all scheduled tasks with their current status, last_run, and next_run timestamps.
- 1.2. WHEN a client sends `GET /api/scheduler/{id}` THEN the system SHALL return the full task configuration or HTTP 404 if the ID does not exist.
- 1.3. WHEN a client sends `POST /api/scheduler` with a valid task payload THEN the system SHALL create the task and return it with a generated ID.
- 1.4. WHEN a client sends `PUT /api/scheduler/{id}` with updated fields THEN the system SHALL update the task and recalculate `next_run` based on the new interval.
- 1.5. WHEN a client sends `DELETE /api/scheduler/{id}` THEN the system SHALL remove the task from the scheduler. If the task is currently running, it SHALL be allowed to finish.

---

### Requirement 2 — Execution Model

**User Story:** As a Core operator, I want scheduled tasks to execute concurrently without blocking each other, so that one slow task does not delay the rest.

#### Acceptance Criteria

- 2.1. WHEN the scheduler loop runs THEN the system SHALL check all enabled tasks and spawn a tokio task for each one whose `next_run` timestamp has passed.
- 2.2. WHEN a task is spawned THEN the system SHALL update its `last_run` to the current timestamp and calculate the next `next_run` based on its interval.
- 2.3. WHEN multiple tasks are due simultaneously THEN the system SHALL spawn all of them concurrently without waiting for any to complete first.
- 2.4. WHEN a task fails THEN the error SHALL NOT affect any other running or pending tasks.
- 2.5. WHEN a task's `enabled` flag is `false` THEN the scheduler loop SHALL skip it entirely.

---

### Requirement 3 — Retry and Backoff

**User Story:** As a connector author, I want failed tasks to retry with backoff, so that transient network errors do not cause permanent sync failures.

#### Acceptance Criteria

- 3.1. WHEN a task fails and `retry_count < max_retries` THEN the system SHALL schedule a retry after the backoff delay.
- 3.2. WHEN the backoff strategy is `exponential` THEN the delay SHALL double with each retry (1s, 2s, 4s, 8s...) up to `max_backoff`.
- 3.3. WHEN the backoff strategy is `fixed` THEN the delay SHALL remain constant between retries.
- 3.4. WHEN all retries are exhausted THEN the system SHALL mark the task as failed, log the error, and resume the task's normal schedule at the next interval.

---

### Requirement 4 — Event Emission

**User Story:** As a plugin author, I want to subscribe to scheduler events, so that my plugin can react when a background task completes or fails.

#### Acceptance Criteria

- 4.1. WHEN a task completes successfully THEN the system SHALL emit a `scheduler.task.complete` event containing the task ID, execution duration, and result summary.
- 4.2. WHEN a task fails after all retries THEN the system SHALL emit a `scheduler.task.error` event containing the task ID, error message, and total retry count.
- 4.3. WHEN events are emitted THEN the system SHALL deliver them to the SSE stream and to any plugin subscribed to scheduler events.

---

### Requirement 5 — Manual Triggers

**User Story:** As an admin, I want to trigger a scheduled task immediately via API, so that I can force a data sync without waiting for the next interval.

#### Acceptance Criteria

- 5.1. WHEN a client sends `POST /api/scheduler/{id}/run` THEN the system SHALL execute the task immediately using the same execution model as a scheduled run.
- 5.2. WHEN a manual trigger completes THEN the task's `next_run` timestamp SHALL remain unchanged from its regular schedule.
- 5.3. WHEN a manual trigger targets a non-existent task ID THEN the system SHALL return HTTP 404.

---

### Requirement 6 — Default Tasks

**User Story:** As a Core operator, I want standard maintenance tasks registered automatically on startup, so that the system is self-maintaining out of the box.

#### Acceptance Criteria

- 6.1. WHEN Core starts THEN the system SHALL register a sync task for each enabled connector with the connector's configured interval (default 5 minutes).
- 6.2. WHEN Core starts THEN the system SHALL register a token rotation task that checks OAuth token expiry every hour.
- 6.3. WHEN Core starts THEN the system SHALL register an audit log rotation task that removes entries older than the retention period (default 90 days) once every 24 hours.
- 6.4. WHEN a default task already exists in the database from a previous startup THEN the system SHALL update its configuration rather than creating a duplicate.

---

### Requirement 7 — Graceful Shutdown

**User Story:** As a Core operator, I want the scheduler to finish running tasks before Core exits, so that data is not corrupted by abrupt termination.

#### Acceptance Criteria

- 7.1. WHEN Core receives a shutdown signal THEN the scheduler SHALL stop spawning new tasks immediately.
- 7.2. WHEN running tasks exist during shutdown THEN the scheduler SHALL wait for them to complete up to the shutdown timeout (5 seconds).
- 7.3. WHEN the shutdown timeout is exceeded THEN the scheduler SHALL log a warning and allow forced shutdown to proceed.
