<!--
domain: background-scheduler
status: draft
tier: 1
updated: 2026-03-22
-->

# Background Scheduler Spec

## Overview

This spec defines the cron-like scheduler for time-triggered background tasks within Core. The scheduler handles periodic work such as connector syncs, OAuth token rotation, and data cleanup. It runs as a background loop in the Core process, executing tasks as concurrent tokio spawned tasks with configurable retry and backoff policies.

## Goals

- Execute scheduled tasks at configured intervals with minimal drift
- Provide retry with exponential or fixed backoff on task failure
- Emit events on the message bus for task completion and failure
- Support manual triggering of any scheduled task via API
- Register default system tasks (connector sync, token rotation, audit log rotation) on startup
- Shut down gracefully by waiting for running tasks to finish

## User Stories

- As a Core operator, I want scheduled tasks to run automatically at configured intervals so that data stays fresh without manual intervention.
- As a connector author, I want my sync task registered automatically during `on_load` so that data is fetched periodically.
- As an admin, I want to manually trigger a scheduled task so that I can force an immediate sync when needed.
- As a developer, I want failed tasks to retry with backoff so that transient errors do not cause permanent data staleness.

## Functional Requirements

- The system must run a background scheduler loop that checks for due tasks at a configurable interval (default 10 seconds).
- The system must support CRUD operations on scheduled tasks via `/api/scheduler` endpoints.
- The system must retry failed tasks according to the configured retry policy with exponential or fixed backoff.
- The system must emit `scheduler.task.complete` and `scheduler.task.error` events on the message bus.
- The system must support manual task triggering via `POST /api/scheduler/{id}/run`.
- The system must register default tasks on startup for connector sync, token rotation, and audit log rotation.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
