<!--
domain: pipeline-executor
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Design Document — Pipeline Executor

## Introduction

This document describes the technical design of the pipeline executor. It covers the public API, data structures, trigger-to-message conversion, the step execution loop, response construction, async job management, and concurrency control. All types live in `packages/types` and the executor implementation lives in `packages/workflow-engine`.

## Crate Layout

- **`packages/types`** — `TriggerContext`, `StepTrace`, `StepStatus`, `WorkflowResponse`, `JobId`, `JobEntry`, `JobStatus`
- **`packages/workflow-engine`** — `WorkflowExecutor`, `JobRegistry`, workflow YAML loading, step loop

## Public API

```rust
pub struct WorkflowExecutor {
    workflows: HashMap<String, WorkflowDefinition>,
    registry: Arc<JobRegistry>,
    semaphore: Arc<Semaphore>,
    plugin_host: Arc<dyn PluginHost>,
}

impl WorkflowExecutor {
    pub async fn execute(&self, trigger: TriggerContext) -> WorkflowResponse;
    pub fn spawn(&self, trigger: TriggerContext) -> JobId;
}
```

- `execute()` acquires a semaphore permit, resolves the workflow, runs the step loop, and returns the response.
- `spawn()` acquires a permit inside a `tokio::spawn` block, registers the job in the registry, runs the step loop, and updates the registry on completion.

## TriggerContext

```rust
pub enum TriggerContext {
    Endpoint(WorkflowRequest),
    Event { name: String, payload: Option<Value>, source: String },
    Schedule { workflow_id: String },
}
```

Conversion to `PipelineMessage` follows these rules:

- **Endpoint** — `WorkflowRequest.body` maps to `payload`. `params`, `query`, and `identity` are inserted into `metadata`.
- **Event** — `payload` maps to `payload`. `name` and `source` are inserted into `metadata`.
- **Schedule** — `payload` is `Value::Null`. `workflow_id`, `trigger_type: "schedule"`, and `timestamp` (ISO 8601) are inserted into `metadata`.

The conversion is implemented as a `From<TriggerContext> for PipelineMessage` trait impl.

## Workflow Loading

At startup, the executor scans a configured directory for `*.yaml` and `*.yml` files. Each file is deserialized into a `WorkflowDefinition`. The loader:

1. Reads all YAML files from the directory.
2. Deserializes each into `WorkflowDefinition`.
3. Inserts into a `HashMap<String, WorkflowDefinition>` keyed by `workflow_id`.
4. If a duplicate `workflow_id` is found, returns an error and the engine refuses to start.

```rust
pub fn load_workflows(dir: &Path) -> Result<HashMap<String, WorkflowDefinition>, WorkflowLoadError> {
    let mut map = HashMap::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().map_or(false, |ext| ext == "yaml" || ext == "yml") {
            let def: WorkflowDefinition = serde_yaml::from_reader(File::open(&path)?)?;
            if map.contains_key(&def.workflow_id) {
                return Err(WorkflowLoadError::DuplicateId(def.workflow_id));
            }
            map.insert(def.workflow_id.clone(), def);
        }
    }
    Ok(map)
}
```

The returned map is stored as an immutable field on `WorkflowExecutor`.

## Step Execution Loop

The core loop processes steps sequentially:

```rust
async fn run_steps(
    &self,
    workflow: &WorkflowDefinition,
    mut message: PipelineMessage,
) -> WorkflowResponse {
    let start = Instant::now();
    let mut traces: Vec<StepTrace> = Vec::new();
    let mut warnings: Vec<WorkflowError> = Vec::new();

    for step in &workflow.steps {
        let step_start = Instant::now();
        let snapshot = message.clone();

        match self.plugin_host.call_action(&step.plugin_id, &step.action, &message).await {
            Ok(output) => {
                message = output;
                traces.push(StepTrace {
                    plugin_id: step.plugin_id.clone(),
                    action: step.action.clone(),
                    duration_ms: step_start.elapsed().as_millis() as u64,
                    status: StepStatus::Completed,
                });
            }
            Err(err) => {
                traces.push(StepTrace {
                    plugin_id: step.plugin_id.clone(),
                    action: step.action.clone(),
                    duration_ms: step_start.elapsed().as_millis() as u64,
                    status: StepStatus::Failed,
                });
                match step.on_error {
                    OnError::Skip => {
                        warnings.push(err.into());
                        message = snapshot;
                        // Overwrite last trace status
                        traces.last_mut().unwrap().status = StepStatus::Skipped;
                    }
                    OnError::Abort => {
                        return WorkflowResponse::error(err, traces, start.elapsed());
                    }
                }
            }
        }
    }

    WorkflowResponse::build(message, traces, warnings, start.elapsed())
}
```

Key design decisions:

- The pre-step snapshot is created via `clone()` so that on `Skip` the message reverts to the pre-step state.
- `StepTrace` is always appended, even for skipped or failed steps.
- The `on_error` strategy is per-step, read from the workflow definition.

## StepTrace

```rust
pub struct StepTrace {
    pub plugin_id: String,
    pub action: String,
    pub duration_ms: u64,
    pub status: StepStatus,
}

pub enum StepStatus {
    Completed,
    Skipped,
    Failed,
}
```

Traces are accumulated in a `Vec<StepTrace>` and included in the response `meta` field.

## WorkflowResponse Construction

```rust
pub struct WorkflowResponse {
    pub status: ResponseStatus,
    pub data: Option<Value>,
    pub errors: Vec<WorkflowError>,
    pub meta: ResponseMeta,
}

pub struct ResponseMeta {
    pub request_id: String,
    pub duration_ms: u64,
    pub traces: Vec<StepTrace>,
}
```

Construction rules:

- **status** — Uses `metadata.status_hint` from the final `PipelineMessage` if present; defaults to `ResponseStatus::Ok`.
- **data** — The final `PipelineMessage.payload`.
- **errors** — Non-fatal errors from skipped steps, surfaced as warnings.
- **meta** — Request ID echoed from the trigger, total `duration_ms`, and all `StepTrace` entries.

## JobRegistry

```rust
pub struct JobRegistry {
    jobs: RwLock<HashMap<JobId, JobEntry>>,
    ttl: Duration,
}

pub struct JobEntry {
    pub status: JobStatus,
    pub response: Option<WorkflowResponse>,
    pub created_at: Instant,
}

pub enum JobStatus {
    InProgress,
    Completed,
    Failed,
}
```

Registry operations:

- **`register()`** — Inserts a new `JobEntry` with `InProgress` status. Returns the `JobId`.
- **`complete(id, response)`** — Updates status to `Completed` and stores the response.
- **`fail(id, response)`** — Updates status to `Failed` and stores the error response.
- **`get(id)`** — Returns a clone of the `JobEntry` if it exists.
- **`evict_expired()`** — Removes entries where `created_at + ttl < now`. Called periodically by a background Tokio task on a configurable interval.

The registry is in-memory only. On restart, all entries are lost. Side effects produced by workflows (database writes, events emitted) survive independently.

## Concurrency Control

The executor uses a `tokio::sync::Semaphore` to limit simultaneous workflow executions:

```rust
let _permit = self.semaphore.acquire().await?;
```

- Default limit: 32
- Both `execute()` and `spawn()` acquire a permit before running the step loop.
- When the limit is reached, callers queue on the semaphore until a permit is released.
- The limit is configurable via engine configuration.

## Async Execution Flow

1. `spawn()` is called with a `TriggerContext`.
2. A `JobId` is generated (UUID v4).
3. The job is registered in the `JobRegistry` with `InProgress` status.
4. The `JobId` is returned to the caller immediately.
5. Inside `tokio::spawn`:
   - A semaphore permit is acquired.
   - The workflow is resolved and steps are executed.
   - On success: `registry.complete(id, response)`.
   - On failure: `registry.fail(id, response)`.
6. The caller polls `GET /api/v1/jobs/:id` to check status and retrieve the result.

## Error Types

```rust
pub enum WorkflowLoadError {
    DuplicateId(String),
    ParseError { path: PathBuf, source: serde_yaml::Error },
    IoError(std::io::Error),
}

pub enum ExecutionError {
    WorkflowNotFound(String),
    ConcurrencyLimitExceeded,
    PluginActionFailed { plugin_id: String, action: String, source: Box<dyn Error> },
}
```

## File Placement

- `packages/types/src/workflow/trigger.rs` — `TriggerContext` enum
- `packages/types/src/workflow/trace.rs` — `StepTrace`, `StepStatus`
- `packages/types/src/workflow/response.rs` — `WorkflowResponse`, `ResponseMeta`, `ResponseStatus`
- `packages/types/src/workflow/job.rs` — `JobId`, `JobEntry`, `JobStatus`
- `packages/workflow-engine/src/executor.rs` — `WorkflowExecutor`
- `packages/workflow-engine/src/loader.rs` — `load_workflows`, `WorkflowLoadError`
- `packages/workflow-engine/src/registry.rs` — `JobRegistry`
