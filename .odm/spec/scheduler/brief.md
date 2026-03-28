<!--
domain: scheduler
updated: 2026-03-28
-->

# Scheduler Spec

## Overview

This spec defines the scheduler subsystem of the Life Engine workflow engine. The scheduler fires workflows on a time-based schedule using standard five-field cron expressions. It runs as a single Tokio task, producing `TriggerContext::Schedule` values and delegating execution to the pipeline executor.

All cron expressions are evaluated against UTC. Missed ticks (due to Core being offline) are silently skipped with no catch-up behaviour. Before firing a scheduled workflow, the scheduler checks the `JobRegistry` for an in-progress instance and skips the tick if one exists, preventing overlapping executions.

Schedule registration happens once at startup by scanning workflow definitions. The schedule registry is immutable at runtime. Invalid cron expressions cause a startup failure.

## Goals

- Fire workflows on cron-based schedules using standard five-field syntax
- Evaluate all schedules against UTC with no timezone conversion in v1
- Run as a single Tokio task with a sleep-until-next-fire loop
- Skip missed ticks cleanly when Core is offline, with no persistence of last-run timestamps
- Prevent overlapping executions of the same scheduled workflow via JobRegistry checks
- Validate all cron expressions at startup and fail fast on invalid expressions
- Build the correct `PipelineMessage` for schedule-triggered workflows with empty payload

## User Stories

- As a workflow author, I want to attach a cron schedule to my workflow so that it runs automatically at defined intervals.
- As a workflow author, I want invalid cron expressions to be caught at startup so that I can fix configuration errors before the engine runs.
- As a maintainer, I want the scheduler to prevent overlapping runs of the same workflow so that concurrent executions do not cause data conflicts.
- As a maintainer, I want missed scheduled ticks to be silently skipped so that the system does not attempt unpredictable catch-up runs after downtime.
- As a workflow engine developer, I want the scheduler to produce a well-defined `TriggerContext::Schedule` so that the pipeline executor can build the correct initial `PipelineMessage`.

## Functional Requirements Summary

- The system must run a scheduler as a single Tokio task that collects schedule triggers, computes next fire times, sleeps until the earliest fire time, and fires due workflows in a loop.
- The system must support standard five-field cron syntax (`minute hour day-of-month month day-of-week`).
- The system must evaluate all cron expressions against UTC only.
- The system must register all schedules at startup by scanning workflow definitions; the registry must be immutable at runtime.
- The system must reject invalid cron expressions at startup with a clear error, preventing the engine from starting.
- The system must skip missed ticks when Core is offline with no persistence or catch-up behaviour.
- The system must check the `JobRegistry` for an `InProgress` instance before firing a scheduled workflow and silently skip the tick (debug log) if one exists.
- The system must not allow overlapping instances of the same scheduled workflow.
- The system must build the initial `PipelineMessage` for schedule-triggered workflows with an empty payload, `trigger_type` of `"schedule"`, the workflow ID, and the scheduled fire time as the timestamp.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
