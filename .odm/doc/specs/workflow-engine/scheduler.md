---
title: Scheduler Specification
type: reference
created: 2026-03-28
status: active
tags:
  - spec
  - scheduler
  - cron
---

# Scheduler Specification

The scheduler fires workflows on a time-based schedule using cron expressions. It produces [[pipeline-executor#TriggerContext|TriggerContext::Schedule]] values and delegates execution to the [[pipeline-executor|pipeline executor]].

## Cron Expressions

Standard five-field syntax: `minute hour day-of-month month day-of-week`.

## Timezone

All expressions are evaluated against UTC. No other timezone is supported in v1.

## Implementation

The scheduler runs as a single Tokio task with the following loop:

1. Collect all schedule triggers from workflow definitions.
2. Calculate the next fire time for each trigger.
3. Sleep until the earliest fire time.
4. Fire all workflows whose scheduled time has arrived.
5. Recalculate next fire times and repeat.

## Missed Ticks

- If Core is offline during a scheduled fire time, the tick is skipped.
- There is no persistence of last-run timestamps.
- There is no catch-up behaviour.
- For patterns that must run on every restart, use a `system.startup` event trigger instead (see [[event-bus#System Events (v1)]]).

## Overlap Prevention

Before spawning a scheduled workflow execution:

1. Check the [[pipeline-executor#JobRegistry|JobRegistry]] for an `InProgress` instance of the same workflow.
2. If a running instance exists, silently skip the tick (debug log only).
3. There is no option to allow overlapping instances of the same scheduled workflow.

## Schedule Registration

- All schedules are registered once at startup by scanning workflow definitions.
- The schedule registry is immutable at runtime.
- Invalid cron expressions must prevent startup with an error.

## Schedule-Triggered Input

When the scheduler fires a workflow, the [[pipeline-executor]] builds the initial `PipelineMessage` as follows:

- **payload** — Empty.
- **metadata.trigger_type** — `"schedule"`
- **metadata.workflow_id** — The workflow ID.
- **metadata.timestamp** — The scheduled fire time (UTC).
