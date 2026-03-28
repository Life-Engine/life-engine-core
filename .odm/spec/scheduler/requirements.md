<!--
domain: scheduler
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Requirements Document — Scheduler

## Introduction

The scheduler fires workflows on a time-based schedule using cron expressions. It runs as a single Tokio task within the workflow engine, producing `TriggerContext::Schedule` values and delegating execution to the pipeline executor. All schedules are registered at startup from workflow definitions and remain immutable at runtime.

The scheduler evaluates cron expressions against UTC only, skips missed ticks without catch-up, and prevents overlapping executions of the same workflow by consulting the `JobRegistry`.

## Alignment with Product Vision

- **Fail Fast** — Invalid cron expressions prevent engine startup, surfacing configuration errors immediately
- **Parse, Don't Validate** — Cron expressions are parsed and validated once at registration; the scheduler loop trusts the validated schedule set
- **Principle of Least Surprise** — Missed ticks are skipped with no hidden catch-up behaviour; the scheduler does exactly what the cron expression says
- **Defence in Depth** — Overlap prevention via `JobRegistry` check protects against concurrent execution of the same workflow

## Requirements

### Requirement 1 — Cron Expression Syntax

**User Story:** As a workflow author, I want to use standard cron syntax so that I can define schedules with familiar, well-documented expressions.

#### Acceptance Criteria

- 1.1. WHEN a workflow definition includes a schedule trigger THEN the scheduler SHALL accept standard five-field cron syntax: `minute hour day-of-month month day-of-week`.
- 1.2. WHEN a cron expression uses non-standard extensions (e.g., seconds field, `@yearly` aliases) THEN the behaviour is undefined in v1 and SHALL NOT be relied upon.

### Requirement 2 — UTC Timezone Evaluation

**User Story:** As a workflow author, I want to know exactly when my schedule fires so that I can reason about timing without timezone ambiguity.

#### Acceptance Criteria

- 2.1. WHEN the scheduler evaluates a cron expression THEN it SHALL use UTC as the reference timezone.
- 2.2. WHEN the scheduler fires a workflow THEN the `metadata.timestamp` in the `PipelineMessage` SHALL be the scheduled fire time in UTC.
- 2.3. WHEN a user requests a non-UTC timezone THEN the scheduler SHALL NOT support it in v1.

### Requirement 3 — Scheduler Task Loop

**User Story:** As a workflow engine developer, I want the scheduler to run as a single efficient task so that it uses minimal system resources while reliably firing workflows on time.

#### Acceptance Criteria

- 3.1. WHEN the engine starts THEN the scheduler SHALL run as a single Tokio task.
- 3.2. WHEN the scheduler loop begins an iteration THEN it SHALL collect all schedule triggers from workflow definitions.
- 3.3. WHEN the scheduler has collected all triggers THEN it SHALL calculate the next fire time for each trigger.
- 3.4. WHEN next fire times have been calculated THEN the scheduler SHALL sleep until the earliest fire time.
- 3.5. WHEN the sleep completes THEN the scheduler SHALL fire all workflows whose scheduled time has arrived.
- 3.6. WHEN all due workflows have been fired THEN the scheduler SHALL recalculate next fire times and repeat the loop.

### Requirement 4 — Missed Tick Handling

**User Story:** As a maintainer, I want missed scheduled ticks to be silently skipped so that the system does not attempt unpredictable catch-up runs after downtime.

#### Acceptance Criteria

- 4.1. WHEN Core is offline during a scheduled fire time THEN the tick SHALL be skipped.
- 4.2. WHEN the scheduler runs THEN it SHALL NOT persist last-run timestamps.
- 4.3. WHEN Core restarts after downtime THEN the scheduler SHALL NOT perform any catch-up execution of missed ticks.
- 4.4. WHEN a pattern requires execution on every restart THEN the workflow author SHALL use a `system.startup` event trigger instead of the scheduler.

### Requirement 5 — Overlap Prevention

**User Story:** As a maintainer, I want the scheduler to prevent overlapping runs of the same workflow so that concurrent executions do not cause data conflicts.

#### Acceptance Criteria

- 5.1. WHEN a scheduled workflow is about to fire THEN the scheduler SHALL check the `JobRegistry` for an `InProgress` instance of the same workflow.
- 5.2. WHEN an `InProgress` instance exists THEN the scheduler SHALL silently skip the tick and emit a debug-level log message.
- 5.3. WHEN the scheduler evaluates overlap THEN there SHALL be no configuration option to allow overlapping instances of the same scheduled workflow.

### Requirement 6 — Schedule Registration

**User Story:** As a workflow author, I want invalid cron expressions to be caught at startup so that I can fix configuration errors before the engine runs.

#### Acceptance Criteria

- 6.1. WHEN the engine starts THEN the scheduler SHALL register all schedules by scanning workflow definitions.
- 6.2. WHEN schedules have been registered THEN the schedule registry SHALL be immutable at runtime.
- 6.3. WHEN a workflow definition contains an invalid cron expression THEN the engine SHALL refuse to start and report a clear error identifying the invalid expression and the workflow.

### Requirement 7 — Schedule-Triggered PipelineMessage

**User Story:** As a workflow engine developer, I want the scheduler to produce a well-defined trigger context so that the pipeline executor can build the correct initial PipelineMessage.

#### Acceptance Criteria

- 7.1. WHEN the scheduler fires a workflow THEN the pipeline executor SHALL build the initial `PipelineMessage` with an empty `payload`.
- 7.2. WHEN the scheduler fires a workflow THEN `metadata.trigger_type` SHALL be `"schedule"`.
- 7.3. WHEN the scheduler fires a workflow THEN `metadata.workflow_id` SHALL be the ID of the scheduled workflow.
- 7.4. WHEN the scheduler fires a workflow THEN `metadata.timestamp` SHALL be the scheduled fire time in UTC.
