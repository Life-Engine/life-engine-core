<!--
domain: scheduler
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Implementation Tasks — Scheduler

**Progress:** 0 / 8 tasks complete

## 1.1 — Core Types and Error Handling

- [ ] Define `ScheduleEntry` and `SchedulerError` types
  <!-- files: packages/workflow-engine/src/scheduler/types.rs -->
  <!-- purpose: Create the ScheduleEntry struct (workflow_id + parsed cron) and SchedulerError enum with InvalidCron variant -->
  <!-- requirements: 1.1, 6.3 -->

- [ ] Define `ScheduleRegistry` with `from_workflow_definitions` constructor
  <!-- files: packages/workflow-engine/src/scheduler/registry.rs -->
  <!-- purpose: Build an immutable registry by scanning workflow definitions, parsing cron expressions, and failing on invalid expressions -->
  <!-- requirements: 6.1, 6.2, 6.3 -->

## 1.2 — Scheduler Task Loop

- [ ] Implement `Scheduler::new` and `Scheduler::run` loop
  <!-- files: packages/workflow-engine/src/scheduler/mod.rs -->
  <!-- purpose: Create the Scheduler struct and its async run loop that collects triggers, computes next fire times, sleeps until the earliest, and fires due workflows -->
  <!-- requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6 -->

- [ ] Implement `Scheduler::fire` with overlap prevention
  <!-- files: packages/workflow-engine/src/scheduler/mod.rs -->
  <!-- purpose: Check JobRegistry for InProgress instance before spawning, skip with debug log if running -->
  <!-- requirements: 5.1, 5.2, 5.3 -->

## 1.3 — TriggerContext and PipelineMessage Integration

- [ ] Add `TriggerContext::Schedule` variant (if not already present)
  <!-- files: packages/types/src/trigger.rs -->
  <!-- purpose: Ensure the Schedule variant exists on TriggerContext with workflow_id field -->
  <!-- requirements: 7.1, 7.2, 7.3, 7.4 -->

- [ ] Wire schedule trigger into pipeline executor message builder
  <!-- files: packages/workflow-engine/src/executor/message_builder.rs -->
  <!-- purpose: Handle TriggerContext::Schedule by building PipelineMessage with empty payload, trigger_type "schedule", workflow_id, and UTC fire time -->
  <!-- requirements: 7.1, 7.2, 7.3, 7.4, 2.1, 2.2 -->

## 1.4 — Startup Integration

- [ ] Integrate ScheduleRegistry and Scheduler into engine startup
  <!-- files: packages/workflow-engine/src/lib.rs, packages/workflow-engine/src/startup.rs -->
  <!-- purpose: Build the registry from workflow definitions during startup, abort on invalid cron, spawn the scheduler Tokio task -->
  <!-- requirements: 6.1, 6.2, 6.3, 3.1 -->

## 1.5 — Tests

- [ ] Add unit tests for scheduler components
  <!-- files: packages/workflow-engine/src/scheduler/tests.rs -->
  <!-- purpose: Test cron parsing, registry construction with valid/invalid expressions, overlap skip logic, and PipelineMessage shape for schedule triggers -->
  <!-- requirements: 1.1, 1.2, 2.1, 2.2, 4.1, 4.2, 4.3, 5.1, 5.2, 6.3, 7.1, 7.2, 7.3, 7.4 -->
