---
title: Scheduler
type: adr
created: 2026-03-28
status: draft
tags:
  - architecture
  - workflow-engine
  - scheduler
  - cron
  - core
---

# Scheduler

## Overview

The scheduler fires workflows on cron expressions. It is part of the workflow engine, unified with the event bus and endpoint triggers — all three activate the same [[pipeline-executor]]. There is no separate "scheduled task" system.

## Cron Expressions

Standard five-field cron syntax:

```
┌───────────── minute (0–59)
│ ┌───────────── hour (0–23)
│ │ ┌───────────── day of month (1–31)
│ │ │ ┌───────────── month (1–12)
│ │ │ │ ┌───────────── day of week (0–6, Sunday = 0)
│ │ │ │ │
* * * * *
```

Examples:

- `*/5 * * * *` — Every 5 minutes
- `0 */6 * * *` — Every 6 hours
- `0 2 * * *` — Daily at 02:00

## Timezone

UTC only for v1. All cron expressions are evaluated against UTC time. This avoids DST edge cases entirely (doubled or skipped hours during transitions). Configurable timezone is a future consideration.

## Implementation

An existing Rust cron crate handles expression parsing and next-fire calculation. The scheduler is a thin Tokio task loop:

1. On startup, collect all schedule triggers from loaded workflow definitions
2. For each trigger, calculate the next fire time from the cron expression
3. Sleep until the earliest next fire time
4. Fire all workflows whose cron expression matches the current time
5. Recalculate next fire times and repeat

The scheduler does not persist state. It evaluates cron expressions from the current time forward on every startup.

## Missed Ticks

If Core is offline when a cron trigger was supposed to fire, the tick is skipped. On restart, the scheduler calculates the next future fire time — it does not look back.

No persistence of last-run timestamps. No catch-up behaviour.

If a user needs guaranteed execution on restart (e.g., "always sync email when Core starts"), they should use a `system.startup` event trigger on a separate workflow rather than relying on the scheduler.

## Overlap Prevention

If a scheduled workflow takes longer than its interval (e.g., email sync every 5 minutes, but a sync takes 7 minutes), the scheduler checks the [[pipeline-executor|JobRegistry]] before spawning:

- If a previous instance of the same workflow is `InProgress`, the tick is silently skipped
- A debug-level log is emitted noting the skip
- The next tick is evaluated normally

This prevents resource pile-up and write conflicts with zero configuration. There is no option to allow overlapping instances — if a user needs concurrent runs of the same logic, they should define separate workflows.

## Schedule Registration

Schedules are registered once at startup from loaded workflow definitions. The scheduler validates each cron expression at registration time — invalid expressions produce a clear error message and prevent startup.

The schedule registry is immutable at runtime. Adding, changing, or removing a schedule requires restarting Core.

## Schedule-Triggered Workflow Input

Scheduled workflows receive an empty `PipelineMessage` with only metadata:

- `payload` — Empty (no input data)
- `metadata.trigger_type` — `"schedule"`
- `metadata.workflow_id` — The workflow ID
- `metadata.timestamp` — The scheduled fire time

Steps that need data fetch it themselves (e.g., a connector plugin polls an external API).
