<!--
domain: scheduler
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Scheduler — Technical Design

## Purpose

This document describes the technical design for the scheduler subsystem of the Life Engine workflow engine. The scheduler fires workflows on time-based cron schedules, running as a single Tokio task. It lives in `packages/workflow-engine` alongside the pipeline executor and other workflow engine components. Supporting types (`TriggerContext`, `PipelineMessage`) are defined in `packages/types`.

## Crate Layout

The scheduler spans two crates:

- **`packages/types`** — `TriggerContext::Schedule` variant, `ScheduleEntry`, and cron-related types
- **`packages/workflow-engine`** — `Scheduler` struct, schedule registry, and the Tokio task loop

## Core Types

### ScheduleEntry

```rust
use cron::Schedule;

pub struct ScheduleEntry {
    pub workflow_id: String,
    pub cron: Schedule,
}
```

Each `ScheduleEntry` pairs a workflow ID with its parsed cron schedule. The `cron` crate's `Schedule` type handles five-field cron parsing and next-fire-time calculation.

### ScheduleRegistry

```rust
pub struct ScheduleRegistry {
    entries: Vec<ScheduleEntry>,
}

impl ScheduleRegistry {
    pub fn from_workflow_definitions(defs: &[WorkflowDefinition]) -> Result<Self, SchedulerError> {
        let mut entries = Vec::new();
        for def in defs {
            if let Some(cron_expr) = &def.schedule {
                let cron = cron_expr.parse::<Schedule>().map_err(|e| {
                    SchedulerError::InvalidCron {
                        workflow_id: def.id.clone(),
                        expression: cron_expr.clone(),
                        reason: e.to_string(),
                    }
                })?;
                entries.push(ScheduleEntry {
                    workflow_id: def.id.clone(),
                    cron,
                });
            }
        }
        Ok(Self { entries })
    }

    pub fn entries(&self) -> &[ScheduleEntry] {
        &self.entries
    }
}
```

The registry is built once at startup from workflow definitions. If any cron expression is invalid, construction fails with a `SchedulerError::InvalidCron` that identifies the workflow and expression. Once built, the registry is immutable.

### SchedulerError

```rust
#[derive(Debug, thiserror::Error)]
pub enum SchedulerError {
    #[error("invalid cron expression in workflow '{workflow_id}': '{expression}' — {reason}")]
    InvalidCron {
        workflow_id: String,
        expression: String,
        reason: String,
    },
}
```

## TriggerContext::Schedule

The scheduler produces `TriggerContext::Schedule` values for the pipeline executor:

```rust
pub enum TriggerContext {
    Endpoint(WorkflowRequest),
    Event { name: String, payload: Option<Value>, source: String },
    Schedule { workflow_id: String },
}
```

When the pipeline executor receives `TriggerContext::Schedule`, it builds the initial `PipelineMessage` as follows:

- **`payload`** — `serde_json::Value::Null` (empty)
- **`metadata.trigger_type`** — `"schedule"`
- **`metadata.workflow_id`** — The workflow ID from the trigger
- **`metadata.timestamp`** — The scheduled fire time in UTC

## Scheduler Task

The scheduler runs as a single Tokio task with the following structure:

```rust
use chrono::Utc;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

pub struct Scheduler {
    registry: ScheduleRegistry,
    executor: Arc<WorkflowExecutor>,
    job_registry: Arc<JobRegistry>,
}

impl Scheduler {
    pub fn new(
        registry: ScheduleRegistry,
        executor: Arc<WorkflowExecutor>,
        job_registry: Arc<JobRegistry>,
    ) -> Self {
        Self { registry, executor, job_registry }
    }

    pub async fn run(&self) {
        loop {
            let now = Utc::now();
            let mut earliest: Option<chrono::DateTime<Utc>> = None;
            let mut due: Vec<&ScheduleEntry> = Vec::new();

            for entry in self.registry.entries() {
                if let Some(next) = entry.cron.upcoming(Utc).next() {
                    if next <= now {
                        due.push(entry);
                    } else {
                        earliest = Some(match earliest {
                            Some(e) if next < e => next,
                            Some(e) => e,
                            None => next,
                        });
                    }
                }
            }

            for entry in &due {
                self.fire(entry).await;
            }

            if let Some(next_time) = earliest {
                let wait = (next_time - Utc::now())
                    .to_std()
                    .unwrap_or(Duration::from_secs(1));
                sleep(wait).await;
            } else {
                // No schedules registered; sleep briefly and re-check
                sleep(Duration::from_secs(60)).await;
            }
        }
    }

    async fn fire(&self, entry: &ScheduleEntry) {
        if self.job_registry.has_in_progress(&entry.workflow_id) {
            tracing::debug!(
                workflow_id = %entry.workflow_id,
                "skipping scheduled tick — workflow already in progress"
            );
            return;
        }

        let trigger = TriggerContext::Schedule {
            workflow_id: entry.workflow_id.clone(),
        };

        self.executor.spawn(trigger);
    }
}
```

### Loop Behaviour

The scheduler loop follows these steps each iteration:

1. Read the current UTC time.
2. For each registry entry, compute the next fire time. If it is at or before `now`, mark it as due. Otherwise, track the earliest future fire time.
3. Fire all due workflows (with overlap check).
4. Sleep until the earliest future fire time. If no schedules exist, sleep for 60 seconds before re-checking.

### Overlap Check

Before spawning a workflow, the scheduler calls `job_registry.has_in_progress(workflow_id)`. This checks whether the `JobRegistry` contains an entry with status `InProgress` for the given workflow ID. If so, the tick is silently skipped and a debug-level log is emitted. There is no configuration to disable this check.

### Missed Ticks

If Core is offline when a scheduled fire time passes, the cron library's `upcoming()` iterator simply returns the next future time on restart. There is no persistence of last-run timestamps and no catch-up mechanism. The tick is effectively lost.

## Startup Integration

The scheduler integrates into the engine startup sequence as follows:

1. Load workflow definitions from the workflow definition directory.
2. Build the `ScheduleRegistry` from workflow definitions. If this fails (invalid cron), abort startup with the error.
3. Construct the `Scheduler` with the registry, executor, and job registry.
4. Spawn the scheduler's `run` method as a Tokio task.

```rust
// In engine startup
let registry = ScheduleRegistry::from_workflow_definitions(&workflow_defs)?;
let scheduler = Scheduler::new(registry, executor.clone(), job_registry.clone());
tokio::spawn(async move { scheduler.run().await });
```

## Dependencies

The scheduler depends on the following crates:

- **`cron`** — Parsing five-field cron expressions and computing next fire times
- **`chrono`** — UTC timestamps and duration arithmetic
- **`tokio`** — Async sleep and task spawning
- **`tracing`** — Debug-level logging for skipped ticks

## Configuration

The scheduler has no dedicated configuration file in v1. Schedules are defined inline in workflow definition YAML files using a `schedule` field:

```yaml
id: daily-digest
triggers:
  - type: schedule
    cron: "0 6 * * *"
mode: async
steps:
  - action: connector-email/send-digest
```

The `cron` field value is the five-field cron expression evaluated against UTC.
