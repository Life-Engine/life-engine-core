<!--
domain: background-scheduler
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Implementation Plan — Background Scheduler

## Task Overview

This plan implements the background scheduler for Core. Work starts with the task data model and storage layer, then builds the scheduler loop with concurrent execution, adds retry/backoff logic, wires up the REST API endpoints, integrates event emission on the message bus, and finally registers default system tasks. The graceful shutdown integration ties into the existing shutdown handler.

**Progress:** 0 / 12 tasks complete

## Steering Document Compliance

- Scheduler is independent of the workflow engine
- Tasks execute as concurrent tokio spawned tasks
- Events are emitted on the Core message bus (SSE + plugin subscriptions)
- Default tasks are registered on startup for connector sync, token rotation, and audit log rotation

## Atomic Task Requirements

- **File Scope:** 1-3 related files maximum
- **Time Boxing:** 15-30 minutes per task
- **Single Purpose:** one testable outcome per task
- **Specific Files:** exact file paths specified
- **Agent-Friendly:** clear input/output, minimal context switching

---

## 1.1 — Task Model and Storage
> spec: ./brief.md

- [ ] Define ScheduledTask struct and retry policy types
  <!-- file: apps/core/src/scheduler/types.rs -->
  <!-- purpose: Define ScheduledTask, RetryPolicy, BackoffStrategy, and TaskStatus types with serde derives -->
  <!-- requirements: 1.3, 3.1 -->
  <!-- leverage: none -->

- [ ] Implement task storage operations
  <!-- file: apps/core/src/scheduler/store.rs -->
  <!-- file: apps/core/src/sqlite_storage.rs -->
  <!-- purpose: CRUD operations for scheduled tasks in SQLite: create table, insert, select, update, delete -->
  <!-- requirements: 1.1, 1.2, 1.3, 1.4, 1.5 -->
  <!-- leverage: existing apps/core/src/sqlite_storage.rs patterns -->

- [ ] Create scheduler module entry point
  <!-- file: apps/core/src/scheduler/mod.rs -->
  <!-- purpose: Re-export types, store, and scheduler loop; register module in main.rs -->
  <!-- requirements: none (structural) -->
  <!-- leverage: none -->

---

## 1.2 — Scheduler Loop
> spec: ./brief.md

- [ ] Implement the main scheduler loop
  <!-- file: apps/core/src/scheduler/loop.rs -->
  <!-- purpose: Background tokio task that checks for due tasks every 10 seconds, spawns concurrent execution, updates timestamps -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.4, 2.5 -->
  <!-- leverage: none -->

- [ ] Add scheduler loop tests
  <!-- file: tests/scheduler/loop_test.rs -->
  <!-- purpose: Test that due tasks are spawned, disabled tasks are skipped, concurrent execution works, and timestamps update correctly -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.5 -->
  <!-- leverage: packages/test-utils -->

---

## 1.3 — Retry and Backoff
> spec: ./brief.md

- [ ] Implement retry logic with configurable backoff
  <!-- file: apps/core/src/scheduler/retry.rs -->
  <!-- purpose: Retry wrapper that handles exponential and fixed backoff, respects max_retries and max_backoff limits -->
  <!-- requirements: 3.1, 3.2, 3.3, 3.4 -->
  <!-- leverage: packages/plugin-sdk-rs/src/retry.rs patterns -->

- [ ] Add retry and backoff tests
  <!-- file: tests/scheduler/retry_test.rs -->
  <!-- purpose: Test exponential doubling, fixed delay, max_backoff cap, and exhaustion behavior -->
  <!-- requirements: 3.1, 3.2, 3.3, 3.4 -->
  <!-- leverage: packages/test-utils -->

---

## 1.4 — REST API Endpoints
> spec: ./brief.md

- [ ] Implement scheduler CRUD routes
  <!-- file: apps/core/src/routes/scheduler.rs -->
  <!-- purpose: GET list, GET by ID, POST create, PUT update, DELETE for scheduled tasks -->
  <!-- requirements: 1.1, 1.2, 1.3, 1.4, 1.5 -->
  <!-- leverage: existing apps/core/src/routes/ patterns -->

- [ ] Implement manual trigger endpoint
  <!-- file: apps/core/src/routes/scheduler.rs -->
  <!-- purpose: POST /api/scheduler/{id}/run endpoint that executes a task immediately without affecting next_run -->
  <!-- requirements: 5.1, 5.2, 5.3 -->
  <!-- leverage: existing apps/core/src/routes/scheduler.rs -->

---

## 1.5 — Event Emission
> spec: ./brief.md

- [ ] Emit scheduler events on the message bus
  <!-- file: apps/core/src/scheduler/loop.rs -->
  <!-- file: apps/core/src/message_bus.rs -->
  <!-- purpose: Emit scheduler.task.complete and scheduler.task.error events with task ID, duration, and error details -->
  <!-- requirements: 4.1, 4.2, 4.3 -->
  <!-- leverage: existing apps/core/src/message_bus.rs -->

---

## 1.6 — Default Task Registration
> spec: ./brief.md

- [ ] Register default tasks on startup
  <!-- file: apps/core/src/scheduler/defaults.rs -->
  <!-- file: apps/core/src/main.rs -->
  <!-- purpose: Register connector sync, token rotation, and audit log rotation tasks during startup step 9; skip duplicates -->
  <!-- requirements: 6.1, 6.2, 6.3, 6.4 -->
  <!-- leverage: existing apps/core/src/main.rs startup sequence -->

---

## 1.7 — Graceful Shutdown
> spec: ./brief.md

- [ ] Integrate scheduler with shutdown handler
  <!-- file: apps/core/src/scheduler/loop.rs -->
  <!-- file: apps/core/src/shutdown.rs -->
  <!-- purpose: On shutdown signal, stop spawning new tasks, wait for running tasks up to 5s timeout, log warning on forced exit -->
  <!-- requirements: 7.1, 7.2, 7.3 -->
  <!-- leverage: existing apps/core/src/shutdown.rs -->
