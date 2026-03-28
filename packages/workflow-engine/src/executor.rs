//! Workflow step executor.
//!
//! Provides the `PipelineExecutor` which runs workflow steps sequentially,
//! passing each step's output as input to the next step. Supports both
//! sync (blocking) and async (background) execution modes.

use async_trait::async_trait;
use chrono::Utc;
use life_engine_traits::{EngineError, Severity};
use life_engine_types::{MessageMetadata, PipelineMessage, SchemaValidated, TypedPayload};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::error::WorkflowError;
use crate::types::{
    ConditionOperator, ErrorStrategyType, ExecutionMode, StepDef, TriggerContext, ValidationLevel,
    WorkflowDef,
};

/// Default concurrency limit for simultaneous workflow executions.
const DEFAULT_CONCURRENCY_LIMIT: usize = 32;

/// Default TTL for job entries (1 hour).
const DEFAULT_JOB_TTL: std::time::Duration = std::time::Duration::from_secs(3600);

/// Interval between automatic job cleanup runs (5 minutes).
const CLEANUP_INTERVAL: std::time::Duration = std::time::Duration::from_secs(300);

/// Overall execution status of a workflow run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    /// All steps completed successfully.
    Completed,
    /// The workflow failed (halted or retry exhausted).
    Failed,
    /// Some steps failed but execution continued (skip strategy).
    PartiallyFailed,
}

/// Status of an individual step execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    /// Step completed successfully.
    Completed,
    /// Step failed.
    Failed,
    /// Step was skipped due to error strategy.
    Skipped,
    /// Step succeeded after retries.
    Retried,
}

/// Error details for a failed step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepErrorLog {
    /// Error message.
    pub message: String,
    /// Error code.
    pub code: String,
    /// Severity level.
    pub severity: String,
    /// Truncated serialization of the input message for debugging.
    pub input_summary: String,
}

/// Log entry for a single step execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepLog {
    /// Step index in the pipeline.
    pub index: usize,
    /// Plugin ID that was executed.
    pub plugin_id: String,
    /// Action name that was executed.
    pub action: String,
    /// Outcome of the step.
    pub status: StepStatus,
    /// Duration of the step in milliseconds.
    pub duration_ms: u64,
    /// Error details if the step failed or was skipped.
    pub error: Option<StepErrorLog>,
    /// Number of retry attempts (if any).
    pub retry_count: Option<u32>,
}

/// Structured execution log emitted after each workflow run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionLog {
    /// Workflow ID.
    pub workflow_id: String,
    /// Trigger type (e.g., "endpoint", "event", "schedule").
    pub trigger_type: String,
    /// Trigger value (e.g., "POST /email/sync").
    pub trigger_value: String,
    /// When execution started.
    pub started_at: chrono::DateTime<Utc>,
    /// When execution completed.
    pub completed_at: chrono::DateTime<Utc>,
    /// Total duration in milliseconds.
    pub total_duration_ms: u64,
    /// Overall execution status.
    pub status: ExecutionStatus,
    /// Per-step log entries.
    pub steps: Vec<StepLog>,
}

/// Trait for executing a single plugin action.
///
/// The plugin system (Phase 8) will provide a concrete implementation.
/// The workflow engine depends only on this abstraction.
#[async_trait]
pub trait PluginExecutor: Send + Sync {
    /// Execute a plugin action with the given input message.
    async fn execute(
        &self,
        plugin_id: &str,
        action: &str,
        input: PipelineMessage,
    ) -> Result<PipelineMessage, Box<dyn EngineError>>;
}

/// Status of an async workflow job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    /// Job is currently running.
    InProgress,
    /// Job completed successfully.
    Completed,
    /// Job failed with an error message.
    Failed(String),
}

/// An entry in the job registry tracking an async workflow execution.
#[derive(Debug, Clone)]
pub struct JobEntry {
    /// Current status of the job.
    pub status: JobStatus,
    /// The workflow response, available once the job completes.
    pub response: Option<PipelineMessage>,
    /// When this job was created.
    pub created_at: chrono::DateTime<Utc>,
}

/// Trait for emitting workflow events (completion, failure).
///
/// The event bus (WP 7.14) will provide a concrete implementation.
#[async_trait]
pub trait WorkflowEventEmitter: Send + Sync {
    /// Emit a workflow event.
    async fn emit(&self, event_name: &str, payload: serde_json::Value);
}

/// A no-op event emitter used when no event bus is configured.
pub struct NoOpEventEmitter;

#[async_trait]
impl WorkflowEventEmitter for NoOpEventEmitter {
    async fn emit(&self, _event_name: &str, _payload: serde_json::Value) {}
}

/// Build the initial `PipelineMessage` from a trigger context.
///
/// Constructs a new message with a fresh correlation ID, source string
/// derived from the trigger type, current UTC timestamp, and the
/// appropriate payload and auth context.
pub fn build_initial_message(
    trigger_context: TriggerContext,
) -> Result<PipelineMessage, WorkflowError> {
    let correlation_id = Uuid::new_v4();
    let timestamp = Utc::now();
    let schema = serde_json::json!({"type": "object"});

    let (source, payload_value, auth_context) = match trigger_context {
        TriggerContext::Endpoint {
            method,
            path,
            body,
            auth,
        } => {
            let source = format!("endpoint:{method} {path}");
            (source, body, auth)
        }
        TriggerContext::Event { name, payload } => {
            let source = format!("event:{name}");
            (source, payload, None)
        }
        TriggerContext::Schedule {
            workflow_id,
            fired_at: _,
        } => {
            let source = format!("schedule:{workflow_id}");
            (source, serde_json::json!({}), None)
        }
    };

    let validated = SchemaValidated::new(payload_value, &schema).map_err(|e| {
        WorkflowError::PluginExecutionError {
            plugin: "workflow-engine".into(),
            cause: e.to_string(),
        }
    })?;

    Ok(PipelineMessage {
        metadata: MessageMetadata {
            correlation_id,
            source,
            timestamp,
            auth_context,
            warnings: vec![],
        },
        payload: TypedPayload::Custom(validated),
    })
}

/// Validate a `PipelineMessage` payload against its expected schema.
///
/// For `TypedPayload::Cdm`, validates the serialized CDM value against a
/// structural schema (the CDM types are strongly typed so this confirms
/// serialization integrity). For `TypedPayload::Custom`, validates the
/// inner value against the basic object schema.
///
/// Returns `Ok(())` on success or a `WorkflowError::ValidationFailed` on
/// failure.
fn validate_message(
    message: &PipelineMessage,
    step_index: usize,
    context: &str,
) -> Result<(), WorkflowError> {
    let payload_json = serde_json::to_value(&message.payload).map_err(|e| {
        WorkflowError::ValidationFailed {
            step_index,
            details: format!("failed to serialize payload: {e}"),
        }
    })?;

    // Extract the inner data from the TypedPayload envelope
    let data = payload_json
        .get("data")
        .unwrap_or(&payload_json);

    // CDM payloads are strongly typed — validate structural integrity by
    // confirming the serialized form is a valid JSON object.
    // Custom payloads are validated against {"type": "object"}.
    let schema = serde_json::json!({"type": "object"});
    let validator = jsonschema::validator_for(&schema).map_err(|e| {
        WorkflowError::ValidationFailed {
            step_index,
            details: format!("schema compilation error: {e}"),
        }
    })?;

    if let Err(error) = validator.validate(data) {
        return Err(WorkflowError::ValidationFailed {
            step_index,
            details: format!("{context}: {error}"),
        });
    }

    debug!(
        step_index = step_index,
        context = context,
        payload_type = match &message.payload {
            TypedPayload::Cdm(_) => "Cdm",
            TypedPayload::Custom(_) => "Custom",
        },
        "pipeline validation passed"
    );

    Ok(())
}

/// Executes workflow steps sequentially, passing each step's output
/// as input to the next step. Supports sync and async execution modes.
pub struct PipelineExecutor {
    plugin_executor: Arc<dyn PluginExecutor>,
    jobs: Arc<RwLock<HashMap<Uuid, JobEntry>>>,
    event_emitter: Arc<dyn WorkflowEventEmitter>,
    semaphore: Arc<Semaphore>,
    job_ttl: std::time::Duration,
}

impl PipelineExecutor {
    /// Create a new pipeline executor with the given plugin executor.
    pub fn new(plugin_executor: Arc<dyn PluginExecutor>) -> Self {
        Self {
            plugin_executor,
            jobs: Arc::new(RwLock::new(HashMap::new())),
            event_emitter: Arc::new(NoOpEventEmitter),
            semaphore: Arc::new(Semaphore::new(DEFAULT_CONCURRENCY_LIMIT)),
            job_ttl: DEFAULT_JOB_TTL,
        }
    }

    /// Create a new pipeline executor with a plugin executor and event emitter.
    pub fn with_event_emitter(
        plugin_executor: Arc<dyn PluginExecutor>,
        event_emitter: Arc<dyn WorkflowEventEmitter>,
    ) -> Self {
        Self {
            plugin_executor,
            jobs: Arc::new(RwLock::new(HashMap::new())),
            event_emitter,
            semaphore: Arc::new(Semaphore::new(DEFAULT_CONCURRENCY_LIMIT)),
            job_ttl: DEFAULT_JOB_TTL,
        }
    }

    /// Create a new pipeline executor with a custom concurrency limit.
    pub fn with_concurrency_limit(
        plugin_executor: Arc<dyn PluginExecutor>,
        concurrency_limit: usize,
    ) -> Self {
        Self {
            plugin_executor,
            jobs: Arc::new(RwLock::new(HashMap::new())),
            event_emitter: Arc::new(NoOpEventEmitter),
            semaphore: Arc::new(Semaphore::new(concurrency_limit)),
            job_ttl: DEFAULT_JOB_TTL,
        }
    }

    /// Query the status of an async job by its ID.
    pub async fn job_status(&self, job_id: &Uuid) -> Option<JobStatus> {
        self.jobs.read().await.get(job_id).map(|e| e.status.clone())
    }

    /// Query the full job entry for an async job by its ID.
    pub async fn job_entry(&self, job_id: &Uuid) -> Option<JobEntry> {
        self.jobs.read().await.get(job_id).cloned()
    }

    /// Remove job entries whose `created_at` exceeds the configured TTL.
    pub async fn cleanup_expired_jobs(&self) -> usize {
        let now = Utc::now();
        let ttl = chrono::Duration::from_std(self.job_ttl).unwrap_or(chrono::Duration::hours(1));
        let mut jobs = self.jobs.write().await;
        let before = jobs.len();
        jobs.retain(|_, entry| (now - entry.created_at) < ttl);
        before - jobs.len()
    }

    /// Spawn a background task that periodically cleans up expired jobs.
    ///
    /// The task runs every `CLEANUP_INTERVAL` (5 minutes) and removes job
    /// entries whose `created_at` exceeds the configured TTL, preventing
    /// unbounded memory growth from completed/failed jobs.
    ///
    /// Returns a `JoinHandle` that can be used to abort the task on shutdown.
    pub fn spawn_cleanup_task(&self) -> tokio::task::JoinHandle<()> {
        let jobs = Arc::clone(&self.jobs);
        let job_ttl = self.job_ttl;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(CLEANUP_INTERVAL);
            loop {
                interval.tick().await;
                let now = Utc::now();
                let ttl = chrono::Duration::from_std(job_ttl)
                    .unwrap_or(chrono::Duration::hours(1));
                let mut jobs = jobs.write().await;
                let before = jobs.len();
                jobs.retain(|_, entry| (now - entry.created_at) < ttl);
                let removed = before - jobs.len();
                if removed > 0 {
                    info!(removed, remaining = jobs.len(), "cleaned up expired jobs");
                }
            }
        })
    }

    /// Spawn an async workflow execution, returning its `JobId` immediately.
    ///
    /// The workflow runs on a background Tokio task. The caller can poll
    /// `job_status()` or `job_entry()` to check progress.
    pub fn spawn(&self, trigger: TriggerContext, workflow: &WorkflowDef) -> Uuid {
        let job_id = Uuid::new_v4();
        let created_at = Utc::now();
        let workflow = workflow.clone();
        let jobs = Arc::clone(&self.jobs);
        let event_emitter = Arc::clone(&self.event_emitter);
        let plugin_executor = Arc::clone(&self.plugin_executor);
        let semaphore = Arc::clone(&self.semaphore);
        let job_ttl = self.job_ttl;

        // Register job as InProgress via a quick spawned task
        let jobs_insert = Arc::clone(&jobs);
        tokio::spawn(async move {
            jobs_insert.write().await.insert(
                job_id,
                JobEntry {
                    status: JobStatus::InProgress,
                    response: None,
                    created_at,
                },
            );
        });

        // Spawn the actual workflow execution
        tokio::spawn(async move {
            let _permit = match semaphore.acquire().await {
                Ok(p) => p,
                Err(_) => {
                    jobs.write().await.insert(
                        job_id,
                        JobEntry {
                            status: JobStatus::Failed("concurrency semaphore closed".into()),
                            response: None,
                            created_at,
                        },
                    );
                    return;
                }
            };

            let initial_message = match build_initial_message(trigger) {
                Ok(msg) => msg,
                Err(err) => {
                    let err_msg = err.to_string();
                    jobs.write().await.insert(
                        job_id,
                        JobEntry {
                            status: JobStatus::Failed(err_msg.clone()),
                            response: None,
                            created_at,
                        },
                    );
                    event_emitter
                        .emit(
                            "workflow.failed",
                            serde_json::json!({
                                "job_id": job_id.to_string(),
                                "workflow_id": workflow.id,
                                "error": err_msg,
                            }),
                        )
                        .await;
                    return;
                }
            };

            let bg_executor = PipelineExecutor {
                plugin_executor,
                jobs: Arc::clone(&jobs),
                event_emitter: Arc::clone(&event_emitter),
                semaphore: Arc::new(Semaphore::new(1)),
                job_ttl,
            };

            match bg_executor.run_steps(&workflow, initial_message).await {
                Ok(result) => {
                    jobs.write().await.insert(
                        job_id,
                        JobEntry {
                            status: JobStatus::Completed,
                            response: Some(result),
                            created_at,
                        },
                    );
                    event_emitter
                        .emit(
                            "workflow.completed",
                            serde_json::json!({
                                "job_id": job_id.to_string(),
                                "workflow_id": workflow.id,
                            }),
                        )
                        .await;
                    info!(
                        workflow_id = %workflow.id,
                        job_id = %job_id,
                        "async workflow completed via spawn()"
                    );
                }
                Err(err) => {
                    let err_msg = err.to_string();
                    jobs.write().await.insert(
                        job_id,
                        JobEntry {
                            status: JobStatus::Failed(err_msg.clone()),
                            response: None,
                            created_at,
                        },
                    );
                    event_emitter
                        .emit(
                            "workflow.failed",
                            serde_json::json!({
                                "job_id": job_id.to_string(),
                                "workflow_id": workflow.id,
                                "error": err_msg,
                            }),
                        )
                        .await;
                    error!(
                        workflow_id = %workflow.id,
                        job_id = %job_id,
                        error = %err,
                        "async workflow failed via spawn()"
                    );
                }
            }
        });

        job_id
    }

    /// Execute a workflow respecting its execution mode.
    ///
    /// For `Sync` mode: runs all steps sequentially and returns the final result.
    /// For `Async` mode: spawns execution in a background task and returns immediately
    /// with a `PipelineMessage` containing the job ID in metadata.
    pub async fn execute_workflow(
        &self,
        workflow: &WorkflowDef,
        initial_message: PipelineMessage,
    ) -> Result<PipelineMessage, WorkflowError> {
        match workflow.mode {
            ExecutionMode::Sync => self.execute_sync(workflow, initial_message).await,
            ExecutionMode::Async => self.execute_async(workflow, initial_message).await,
        }
    }

    /// Execute a workflow synchronously — all steps run sequentially and the
    /// final result is returned directly. Acquires a concurrency permit.
    async fn execute_sync(
        &self,
        workflow: &WorkflowDef,
        initial_message: PipelineMessage,
    ) -> Result<PipelineMessage, WorkflowError> {
        let _permit = self.semaphore.acquire().await.map_err(|_| {
            WorkflowError::PluginExecutionError {
                plugin: "workflow-engine".into(),
                cause: "concurrency semaphore closed".into(),
            }
        })?;
        self.run_steps(workflow, initial_message).await
    }

    /// Execute a workflow asynchronously — spawn a background task and return
    /// a PipelineMessage containing the job ID immediately.
    async fn execute_async(
        &self,
        workflow: &WorkflowDef,
        initial_message: PipelineMessage,
    ) -> Result<PipelineMessage, WorkflowError> {
        let job_id = Uuid::new_v4();
        let workflow_id = workflow.id.clone();

        info!(
            workflow_id = %workflow_id,
            job_id = %job_id,
            "starting async workflow execution"
        );

        let created_at = Utc::now();

        // Mark the job as in-progress
        self.jobs.write().await.insert(job_id, JobEntry {
            status: JobStatus::InProgress,
            response: None,
            created_at,
        });

        // Clone what the background task needs
        let jobs = Arc::clone(&self.jobs);
        let event_emitter = Arc::clone(&self.event_emitter);
        let plugin_executor = Arc::clone(&self.plugin_executor);
        let semaphore = Arc::clone(&self.semaphore);
        let job_ttl = self.job_ttl;
        let workflow = workflow.clone();
        let correlation_id = initial_message.metadata.correlation_id;
        let auth_context = initial_message.metadata.auth_context.clone();

        tokio::spawn(async move {
            let _permit = match semaphore.acquire().await {
                Ok(p) => p,
                Err(_) => {
                    jobs.write().await.insert(job_id, JobEntry {
                        status: JobStatus::Failed("concurrency semaphore closed".into()),
                        response: None,
                        created_at,
                    });
                    return;
                }
            };

            let bg_executor = PipelineExecutor {
                plugin_executor,
                jobs: Arc::clone(&jobs),
                event_emitter: Arc::clone(&event_emitter),
                semaphore: Arc::new(Semaphore::new(1)),
                job_ttl,
            };

            match bg_executor.run_steps(&workflow, initial_message).await {
                Ok(result) => {
                    jobs.write().await.insert(job_id, JobEntry {
                        status: JobStatus::Completed,
                        response: Some(result),
                        created_at,
                    });
                    event_emitter
                        .emit(
                            "workflow.completed",
                            serde_json::json!({
                                "job_id": job_id.to_string(),
                                "workflow_id": workflow.id,
                            }),
                        )
                        .await;
                    info!(
                        workflow_id = %workflow.id,
                        job_id = %job_id,
                        "async workflow completed"
                    );
                }
                Err(err) => {
                    let err_msg = err.to_string();
                    jobs.write().await.insert(job_id, JobEntry {
                        status: JobStatus::Failed(err_msg.clone()),
                        response: None,
                        created_at,
                    });
                    event_emitter
                        .emit(
                            "workflow.failed",
                            serde_json::json!({
                                "job_id": job_id.to_string(),
                                "workflow_id": workflow.id,
                                "error": err_msg,
                            }),
                        )
                        .await;
                    error!(
                        workflow_id = %workflow.id,
                        job_id = %job_id,
                        error = %err,
                        "async workflow failed"
                    );
                }
            }
        });

        // Return immediately with a message containing the job_id
        let schema = serde_json::json!({"type": "object"});
        let payload = serde_json::json!({
            "job_id": job_id.to_string(),
            "workflow_id": workflow_id,
            "status": "accepted",
        });
        let response = PipelineMessage {
            metadata: MessageMetadata {
                correlation_id,
                source: format!("workflow-engine:async:{workflow_id}"),
                timestamp: Utc::now(),
                auth_context,
                warnings: vec![],
            },
            payload: TypedPayload::Custom(
                SchemaValidated::new(payload, &schema)
                    .map_err(|e| WorkflowError::PluginExecutionError {
                        plugin: "workflow-engine".into(),
                        cause: e.to_string(),
                    })?,
            ),
        };

        Ok(response)
    }

    /// Derive trigger type and value from a workflow definition.
    fn derive_trigger_info(workflow: &WorkflowDef) -> (String, String) {
        if let Some(ref endpoint) = workflow.trigger.endpoint {
            ("endpoint".to_string(), endpoint.clone())
        } else if let Some(ref event) = workflow.trigger.event {
            ("event".to_string(), event.clone())
        } else if let Some(ref schedule) = workflow.trigger.schedule {
            ("schedule".to_string(), schedule.clone())
        } else {
            ("unknown".to_string(), String::new())
        }
    }

    /// Truncate an input message to a short summary for error logging.
    fn summarize_input(message: &PipelineMessage) -> String {
        let json = serde_json::to_string(&message.payload).unwrap_or_default();
        if json.len() > 200 {
            format!("{}...", &json[..200])
        } else {
            json
        }
    }

    /// Run the steps of a workflow sequentially, respecting each step's error strategy.
    /// Emits a structured `ExecutionLog` after completion (success or failure).
    async fn run_steps(
        &self,
        workflow: &WorkflowDef,
        initial_message: PipelineMessage,
    ) -> Result<PipelineMessage, WorkflowError> {
        let started_at = Utc::now();
        let (trigger_type, trigger_value) = Self::derive_trigger_info(workflow);

        info!(
            workflow_id = %workflow.id,
            workflow_name = %workflow.name,
            step_count = workflow.steps.len(),
            validate = ?workflow.validate,
            "starting workflow execution"
        );

        // Edges and Strict both validate the entry message
        if matches!(
            workflow.validate,
            ValidationLevel::Strict | ValidationLevel::Edges
        ) {
            validate_message(&initial_message, 0, "entry validation")?;
        }

        let mut step_logs: Vec<StepLog> = Vec::new();

        let result = self
            .execute_steps(
                &workflow.id,
                &workflow.steps,
                initial_message,
                0,
                &workflow.validate,
                &mut step_logs,
            )
            .await;

        let completed_at = Utc::now();
        let total_duration_ms = (completed_at - started_at).num_milliseconds().max(0) as u64;

        let has_skipped = step_logs.iter().any(|s| s.status == StepStatus::Skipped);

        let status = match &result {
            Ok(_) if has_skipped => ExecutionStatus::PartiallyFailed,
            Ok(_) => ExecutionStatus::Completed,
            Err(_) => ExecutionStatus::Failed,
        };

        let execution_log = ExecutionLog {
            workflow_id: workflow.id.clone(),
            trigger_type,
            trigger_value,
            started_at,
            completed_at,
            total_duration_ms,
            status: status.clone(),
            steps: step_logs,
        };

        match status {
            ExecutionStatus::Failed => {
                error!(execution_log = ?execution_log, "workflow execution failed");
            }
            _ => {
                info!(execution_log = ?execution_log, "workflow execution completed");
            }
        }

        // Validate exit message on success
        if let Ok(ref msg) = result
            && matches!(
                workflow.validate,
                ValidationLevel::Strict | ValidationLevel::Edges
            )
        {
            let last_index = workflow.steps.len().saturating_sub(1);
            validate_message(msg, last_index, "exit validation")?;
        }

        result
    }

    /// Resolve a dot-notation field path against a PipelineMessage payload.
    ///
    /// Supports paths like `"payload.category"` or `"status"`. The `payload.`
    /// prefix is stripped since we index directly into the payload value.
    fn resolve_field(message: &PipelineMessage, field: &str) -> Option<serde_json::Value> {
        let payload_value = serde_json::to_value(&message.payload).unwrap_or_default();

        // TypedPayload serialises with #[serde(tag = "type", content = "data")]
        // so the structure is {"type": "Custom"|"Cdm", "data": <inner>}.
        // Extract the "data" field as the root for field resolution.
        let root = payload_value
            .get("data")
            .cloned()
            .unwrap_or(payload_value);

        // Strip leading "payload." prefix — callers write field paths relative
        // to the message payload, not the internal TypedPayload wrapper.
        let path = field.strip_prefix("payload.").unwrap_or(field);

        let mut current = &root;
        for segment in path.split('.') {
            match current.get(segment) {
                Some(v) => current = v,
                None => return None,
            }
        }
        Some(current.clone())
    }

    /// Evaluate a condition operator against a resolved field value.
    ///
    /// - `Equals`: true if field exists and matches `comparison_value` exactly.
    /// - `NotEquals`: true if field exists and does not match `comparison_value`.
    /// - `Exists`: true if field is present (including null).
    /// - `IsEmpty`: true if field is absent, null, empty string, or empty array.
    ///
    /// Missing fields (resolved == None) take the else branch for Equals/NotEquals/Exists,
    /// and the then branch for IsEmpty.
    fn evaluate_condition(
        operator: &ConditionOperator,
        resolved: &Option<serde_json::Value>,
        comparison_value: &serde_json::Value,
    ) -> bool {
        match operator {
            ConditionOperator::Equals => resolved
                .as_ref()
                .map(|v| v == comparison_value)
                .unwrap_or(false),
            ConditionOperator::NotEquals => resolved
                .as_ref()
                .map(|v| v != comparison_value)
                .unwrap_or(false),
            ConditionOperator::Exists => resolved.is_some(),
            ConditionOperator::IsEmpty => match resolved {
                None => true,
                Some(v) => {
                    v.is_null()
                        || v.as_str().map(|s| s.is_empty()).unwrap_or(false)
                        || v.as_array().map(|a| a.is_empty()).unwrap_or(false)
                }
            },
        }
    }

    /// Execute a list of steps sequentially, returning the final message.
    ///
    /// This is used both for top-level workflow steps and for conditional
    /// branch steps (`then_steps` / `else_steps`). Step logs are appended
    /// to the provided `step_logs` vector for execution logging.
    fn execute_steps<'a>(
        &'a self,
        workflow_id: &'a str,
        steps: &'a [StepDef],
        initial_message: PipelineMessage,
        base_index: usize,
        validate: &'a ValidationLevel,
        step_logs: &'a mut Vec<StepLog>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<PipelineMessage, WorkflowError>> + Send + 'a>> {
        Box::pin(async move {
        let mut current_message = initial_message;

        for (offset, step) in steps.iter().enumerate() {
            let index = base_index + offset;

            // Check for conditional branching
            if let Some(condition) = &step.condition {
                let resolved = Self::resolve_field(&current_message, &condition.field);
                let matches = Self::evaluate_condition(&condition.operator, &resolved, &condition.value);

                info!(
                    workflow_id = %workflow_id,
                    step_index = index,
                    field = %condition.field,
                    operator = ?condition.operator,
                    matches = matches,
                    "evaluating conditional branch"
                );

                let branch = if matches {
                    &condition.then_steps
                } else {
                    &condition.else_steps
                };

                current_message = self
                    .execute_steps(workflow_id, branch, current_message, index * 100, validate, step_logs)
                    .await?;
                continue;
            }

            let step_start = Utc::now();

            info!(
                workflow_id = %workflow_id,
                step_index = index,
                plugin = %step.plugin,
                action = %step.action,
                "executing step"
            );

            match self
                .plugin_executor
                .execute(&step.plugin, &step.action, current_message.clone())
                .await
            {
                Ok(output) => {
                    let duration_ms = (Utc::now() - step_start).num_milliseconds().max(0) as u64;
                    step_logs.push(StepLog {
                        index,
                        plugin_id: step.plugin.clone(),
                        action: step.action.clone(),
                        status: StepStatus::Completed,
                        duration_ms,
                        error: None,
                        retry_count: None,
                    });

                    current_message = output;

                    // Strict: validate after every step
                    if *validate == ValidationLevel::Strict {
                        validate_message(
                            &current_message,
                            index,
                            &format!("strict validation after step {index} ({}.{})", step.plugin, step.action),
                        )?;
                    }
                }
                Err(plugin_error) => {
                    let severity = plugin_error.severity();
                    let step_strategy = step
                        .on_error
                        .as_ref()
                        .map(|s| &s.strategy)
                        .unwrap_or(&ErrorStrategyType::Halt);

                    // Severity overrides: plugin severity > step strategy for Fatal and Warning.
                    // For Retryable severity, the step's declared strategy wins.
                    let effective_strategy = match severity {
                        Severity::Fatal => {
                            // Always halt regardless of declared strategy.
                            &ErrorStrategyType::Halt
                        }
                        Severity::Warning => {
                            // Log warning and continue — no error strategy applied.
                            warn!(
                                workflow_id = %workflow_id,
                                step_index = index,
                                plugin = %step.plugin,
                                action = %step.action,
                                error = %plugin_error,
                                "step returned warning severity — continuing execution"
                            );
                            // current_message is unchanged — pass through input.
                            continue;
                        }
                        Severity::Retryable => {
                            // Step strategy wins for Retryable severity.
                            // If the step doesn't declare a strategy (defaults to Halt),
                            // upgrade to retry with defaults since the error is retryable.
                            if step.on_error.is_none() {
                                &ErrorStrategyType::Retry
                            } else {
                                step_strategy
                            }
                        }
                    };

                    match effective_strategy {
                        ErrorStrategyType::Halt => {
                            let duration_ms = (Utc::now() - step_start).num_milliseconds().max(0) as u64;
                            let input_summary = Self::summarize_input(&current_message);
                            step_logs.push(StepLog {
                                index,
                                plugin_id: step.plugin.clone(),
                                action: step.action.clone(),
                                status: StepStatus::Failed,
                                duration_ms,
                                error: Some(StepErrorLog {
                                    message: plugin_error.to_string(),
                                    code: plugin_error.code().to_string(),
                                    severity: format!("{:?}", severity),
                                    input_summary,
                                }),
                                retry_count: None,
                            });

                            error!(
                                workflow_id = %workflow_id,
                                step_index = index,
                                plugin = %step.plugin,
                                action = %step.action,
                                error = %plugin_error,
                                severity = ?severity,
                                "step failed with halt strategy — stopping workflow"
                            );
                            return Err(WorkflowError::StepHalted {
                                step_index: index,
                                plugin: step.plugin.clone(),
                                action: step.action.clone(),
                                cause: plugin_error.to_string(),
                            });
                        }
                        ErrorStrategyType::Skip => {
                            let duration_ms = (Utc::now() - step_start).num_milliseconds().max(0) as u64;
                            let input_summary = Self::summarize_input(&current_message);
                            step_logs.push(StepLog {
                                index,
                                plugin_id: step.plugin.clone(),
                                action: step.action.clone(),
                                status: StepStatus::Skipped,
                                duration_ms,
                                error: Some(StepErrorLog {
                                    message: plugin_error.to_string(),
                                    code: plugin_error.code().to_string(),
                                    severity: format!("{:?}", severity),
                                    input_summary,
                                }),
                                retry_count: None,
                            });

                            warn!(
                                workflow_id = %workflow_id,
                                step_index = index,
                                plugin = %step.plugin,
                                action = %step.action,
                                error = %plugin_error,
                                "step failed with skip strategy — skipping and continuing"
                            );
                            // Append error to warnings so callers can detect degradation (Req 8.2).
                            current_message.metadata.warnings.push(format!(
                                "step {} ({}.{}) skipped: {}",
                                index, step.plugin, step.action, plugin_error
                            ));
                            // current_message is otherwise unchanged — the failed step's output is discarded.
                        }
                        ErrorStrategyType::Retry => {
                            let max_retries = step
                                .on_error
                                .as_ref()
                                .and_then(|s| s.max_retries)
                                .unwrap_or(3);

                            let mut last_error = plugin_error;
                            let mut succeeded = false;
                            let mut retry_count: u32 = 0;

                            for attempt in 1..=max_retries {
                                retry_count = attempt;
                                let backoff_secs = std::cmp::min(1u64 << (attempt - 1), 30);
                                let backoff = std::time::Duration::from_secs(backoff_secs);
                                warn!(
                                    workflow_id = %workflow_id,
                                    step_index = index,
                                    plugin = %step.plugin,
                                    action = %step.action,
                                    attempt = attempt,
                                    max_retries = max_retries,
                                    backoff_ms = backoff.as_millis() as u64,
                                    "retrying failed step"
                                );
                                tokio::time::sleep(backoff).await;

                                match self
                                    .plugin_executor
                                    .execute(
                                        &step.plugin,
                                        &step.action,
                                        current_message.clone(),
                                    )
                                    .await
                                {
                                    Ok(output) => {
                                        info!(
                                            workflow_id = %workflow_id,
                                            step_index = index,
                                            plugin = %step.plugin,
                                            action = %step.action,
                                            attempt = attempt,
                                            "retry succeeded"
                                        );
                                        current_message = output;
                                        succeeded = true;
                                        break;
                                    }
                                    Err(retry_error) => {
                                        last_error = retry_error;
                                    }
                                }
                            }

                            if succeeded {
                                let duration_ms = (Utc::now() - step_start).num_milliseconds().max(0) as u64;
                                step_logs.push(StepLog {
                                    index,
                                    plugin_id: step.plugin.clone(),
                                    action: step.action.clone(),
                                    status: StepStatus::Retried,
                                    duration_ms,
                                    error: None,
                                    retry_count: Some(retry_count),
                                });
                            } else {
                                // Check for fallback step
                                let fallback = step
                                    .on_error
                                    .as_ref()
                                    .and_then(|s| s.fallback.as_ref());

                                if let Some(fallback_step) = fallback {
                                    warn!(
                                        workflow_id = %workflow_id,
                                        step_index = index,
                                        plugin = %step.plugin,
                                        action = %step.action,
                                        retries = max_retries,
                                        fallback_plugin = %fallback_step.plugin,
                                        fallback_action = %fallback_step.action,
                                        "all retries exhausted — executing fallback step"
                                    );

                                    match self
                                        .plugin_executor
                                        .execute(
                                            &fallback_step.plugin,
                                            &fallback_step.action,
                                            current_message.clone(),
                                        )
                                        .await
                                    {
                                        Ok(output) => {
                                            current_message = output;
                                        }
                                        Err(fallback_error) => {
                                            let duration_ms = (Utc::now() - step_start).num_milliseconds().max(0) as u64;
                                            let input_summary = Self::summarize_input(&current_message);
                                            step_logs.push(StepLog {
                                                index,
                                                plugin_id: step.plugin.clone(),
                                                action: step.action.clone(),
                                                status: StepStatus::Failed,
                                                duration_ms,
                                                error: Some(StepErrorLog {
                                                    message: format!(
                                                        "retries failed: {}; fallback also failed: {}",
                                                        last_error, fallback_error
                                                    ),
                                                    code: last_error.code().to_string(),
                                                    severity: format!("{:?}", last_error.severity()),
                                                    input_summary,
                                                }),
                                                retry_count: Some(max_retries),
                                            });

                                            return Err(WorkflowError::RetryExhausted {
                                                step_index: index,
                                                plugin: step.plugin.clone(),
                                                action: step.action.clone(),
                                                retries: max_retries,
                                                cause: format!(
                                                    "retries failed: {}; fallback also failed: {}",
                                                    last_error, fallback_error
                                                ),
                                            });
                                        }
                                    }
                                } else {
                                    let duration_ms = (Utc::now() - step_start).num_milliseconds().max(0) as u64;
                                    let input_summary = Self::summarize_input(&current_message);
                                    step_logs.push(StepLog {
                                        index,
                                        plugin_id: step.plugin.clone(),
                                        action: step.action.clone(),
                                        status: StepStatus::Failed,
                                        duration_ms,
                                        error: Some(StepErrorLog {
                                            message: last_error.to_string(),
                                            code: last_error.code().to_string(),
                                            severity: format!("{:?}", last_error.severity()),
                                            input_summary,
                                        }),
                                        retry_count: Some(max_retries),
                                    });

                                    return Err(WorkflowError::RetryExhausted {
                                        step_index: index,
                                        plugin: step.plugin.clone(),
                                        action: step.action.clone(),
                                        retries: max_retries,
                                        cause: last_error.to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(current_message)
        }) // Box::pin
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use life_engine_types::{MessageMetadata, SchemaValidated, TypedPayload};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::Notify;

    use crate::types::{StepDef, TriggerDef};

    /// A mock plugin executor that transforms the message payload
    /// by appending the step info to a JSON array.
    struct MockPluginExecutor {
        call_count: AtomicUsize,
    }

    impl MockPluginExecutor {
        fn new() -> Self {
            Self {
                call_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl PluginExecutor for MockPluginExecutor {
        async fn execute(
            &self,
            plugin_id: &str,
            action: &str,
            mut input: PipelineMessage,
        ) -> Result<PipelineMessage, Box<dyn EngineError>> {
            self.call_count.fetch_add(1, Ordering::SeqCst);

            // Transform the payload by wrapping it with step info
            let current = serde_json::to_value(&input.payload).unwrap_or_default();
            let transformed = serde_json::json!({
                "previous": current,
                "executed_by": format!("{plugin_id}.{action}"),
            });
            let schema = serde_json::json!({"type": "object"});
            input.payload =
                TypedPayload::Custom(SchemaValidated::new(transformed, &schema).unwrap());
            Ok(input)
        }
    }

    /// A mock plugin executor that fails on a specific step index.
    struct FailingPluginExecutor {
        fail_on_call: usize,
        call_count: AtomicUsize,
    }

    #[derive(Debug)]
    struct MockPluginError(String);

    impl std::fmt::Display for MockPluginError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl std::error::Error for MockPluginError {}

    impl EngineError for MockPluginError {
        fn code(&self) -> &str {
            "MOCK_001"
        }
        fn severity(&self) -> life_engine_traits::Severity {
            life_engine_traits::Severity::Retryable
        }
        fn source_module(&self) -> &str {
            "mock"
        }
    }

    impl FailingPluginExecutor {
        fn new(fail_on_call: usize) -> Self {
            Self {
                fail_on_call,
                call_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl PluginExecutor for FailingPluginExecutor {
        async fn execute(
            &self,
            plugin_id: &str,
            action: &str,
            mut input: PipelineMessage,
        ) -> Result<PipelineMessage, Box<dyn EngineError>> {
            let call = self.call_count.fetch_add(1, Ordering::SeqCst);
            if call == self.fail_on_call {
                return Err(Box::new(MockPluginError(format!(
                    "step {plugin_id}.{action} failed"
                ))));
            }
            let schema = serde_json::json!({"type": "object"});
            let transformed = serde_json::json!({"executed_by": format!("{plugin_id}.{action}")});
            input.payload =
                TypedPayload::Custom(SchemaValidated::new(transformed, &schema).unwrap());
            Ok(input)
        }
    }

    /// A mock plugin executor that waits on a Notify before completing.
    struct SlowPluginExecutor {
        notify: Arc<Notify>,
        call_count: AtomicUsize,
    }

    impl SlowPluginExecutor {
        fn new(notify: Arc<Notify>) -> Self {
            Self {
                notify,
                call_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl PluginExecutor for SlowPluginExecutor {
        async fn execute(
            &self,
            plugin_id: &str,
            action: &str,
            mut input: PipelineMessage,
        ) -> Result<PipelineMessage, Box<dyn EngineError>> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            // Wait until notified — simulates a slow plugin
            self.notify.notified().await;
            let schema = serde_json::json!({"type": "object"});
            let transformed = serde_json::json!({"executed_by": format!("{plugin_id}.{action}")});
            input.payload =
                TypedPayload::Custom(SchemaValidated::new(transformed, &schema).unwrap());
            Ok(input)
        }
    }

    /// A mock event emitter that records emitted events.
    struct RecordingEventEmitter {
        events: Arc<RwLock<Vec<(String, serde_json::Value)>>>,
    }

    impl RecordingEventEmitter {
        fn new() -> Self {
            Self {
                events: Arc::new(RwLock::new(Vec::new())),
            }
        }

        fn events(&self) -> Arc<RwLock<Vec<(String, serde_json::Value)>>> {
            Arc::clone(&self.events)
        }
    }

    #[async_trait]
    impl WorkflowEventEmitter for RecordingEventEmitter {
        async fn emit(&self, event_name: &str, payload: serde_json::Value) {
            self.events
                .write()
                .await
                .push((event_name.to_string(), payload));
        }
    }

    fn make_test_message() -> PipelineMessage {
        let schema = serde_json::json!({"type": "object"});
        PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: Uuid::new_v4(),
                source: "test:unit".into(),
                timestamp: Utc::now(),
                auth_context: None,
                warnings: vec![],
            },
            payload: TypedPayload::Custom(
                SchemaValidated::new(serde_json::json!({"input": true}), &schema).unwrap(),
            ),
        }
    }

    fn make_step(plugin: &str, action: &str) -> StepDef {
        StepDef {
            plugin: plugin.into(),
            action: action.into(),
            on_error: None,
            condition: None,
        }
    }

    fn make_workflow(steps: Vec<StepDef>) -> WorkflowDef {
        WorkflowDef {
            id: "test-workflow".into(),
            name: "Test Workflow".into(),
            description: None,
            mode: ExecutionMode::Sync,
            validate: crate::types::ValidationLevel::None,
            trigger: TriggerDef {
                endpoint: Some("POST /test".into()),
                event: None,
                schedule: None,
            },
            steps,
        }
    }

    fn make_async_workflow(steps: Vec<StepDef>) -> WorkflowDef {
        WorkflowDef {
            id: "async-test-workflow".into(),
            name: "Async Test Workflow".into(),
            description: None,
            mode: ExecutionMode::Async,
            validate: crate::types::ValidationLevel::None,
            trigger: TriggerDef {
                endpoint: Some("POST /async-test".into()),
                event: None,
                schedule: None,
            },
            steps,
        }
    }

    // --- Sync mode tests (existing) ---

    #[tokio::test]
    async fn executes_single_step() {
        let mock = Arc::new(MockPluginExecutor::new());
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![make_step("email-plugin", "sync")]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        assert!(result.is_ok());
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn executes_multiple_steps_sequentially() {
        let mock = Arc::new(MockPluginExecutor::new());
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![
            make_step("parser", "parse"),
            make_step("transformer", "transform"),
            make_step("storage", "store"),
        ]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await
            .unwrap();

        assert_eq!(mock.call_count.load(Ordering::SeqCst), 3);

        // Verify the final output was produced by the last step
        let payload_json = serde_json::to_value(&result.payload).unwrap();
        let data = &payload_json["data"];
        assert_eq!(data["executed_by"], "storage.store");
    }

    #[tokio::test]
    async fn step_output_becomes_next_step_input() {
        let mock = Arc::new(MockPluginExecutor::new());
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![
            make_step("step-a", "act"),
            make_step("step-b", "act"),
        ]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await
            .unwrap();

        // step-b should have received step-a's output as "previous"
        let payload_json = serde_json::to_value(&result.payload).unwrap();
        let data = &payload_json["data"];
        assert_eq!(data["executed_by"], "step-b.act");
        assert!(data["previous"].is_object());
    }

    #[tokio::test]
    async fn halts_on_step_failure() {
        let mock = Arc::new(FailingPluginExecutor::new(1)); // fail on second call
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![
            make_step("step-a", "act"),
            make_step_with_strategy("step-b", "act", ErrorStrategyType::Halt),
            make_step("step-c", "act"),
        ]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        match &err {
            WorkflowError::StepHalted {
                step_index,
                plugin,
                action,
                ..
            } => {
                assert_eq!(*step_index, 1);
                assert_eq!(plugin, "step-b");
                assert_eq!(action, "act");
            }
            other => panic!("expected StepHalted, got: {other:?}"),
        }

        // step-c should NOT have been called
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn preserves_correlation_id() {
        let mock = Arc::new(MockPluginExecutor::new());
        let executor = PipelineExecutor::new(mock);
        let workflow = make_workflow(vec![make_step("p", "a"), make_step("q", "b")]);
        let msg = make_test_message();
        let expected_id = msg.metadata.correlation_id;

        let result = executor.execute_workflow(&workflow, msg).await.unwrap();
        assert_eq!(result.metadata.correlation_id, expected_id);
    }

    #[tokio::test]
    async fn empty_steps_returns_initial_message() {
        let mock = Arc::new(MockPluginExecutor::new());
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![]);
        let msg = make_test_message();
        let expected_id = msg.metadata.correlation_id;

        let result = executor.execute_workflow(&workflow, msg).await.unwrap();
        assert_eq!(result.metadata.correlation_id, expected_id);
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 0);
    }

    // --- Async mode tests ---

    #[tokio::test]
    async fn async_mode_returns_job_id_immediately() {
        let notify = Arc::new(Notify::new());
        let mock = Arc::new(SlowPluginExecutor::new(Arc::clone(&notify)));
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_async_workflow(vec![make_step("slow-plugin", "process")]);
        let msg = make_test_message();
        let correlation_id = msg.metadata.correlation_id;

        let result = executor
            .execute_workflow(&workflow, msg)
            .await
            .unwrap();

        // Should get back immediately with a job_id
        let payload_json = serde_json::to_value(&result.payload).unwrap();
        let data = &payload_json["data"];
        assert_eq!(data["status"], "accepted");
        assert!(data["job_id"].is_string());
        assert_eq!(data["workflow_id"], "async-test-workflow");
        assert_eq!(result.metadata.correlation_id, correlation_id);

        // The plugin hasn't been released yet — job should be InProgress
        let job_id: Uuid = data["job_id"].as_str().unwrap().parse().unwrap();
        let status = executor.job_status(&job_id).await;
        assert_eq!(status, Some(JobStatus::InProgress));

        // Release the slow plugin
        notify.notify_one();
        // Give the background task time to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let status = executor.job_status(&job_id).await;
        assert_eq!(status, Some(JobStatus::Completed));
    }

    #[tokio::test]
    async fn async_mode_emits_completed_event() {
        let emitter = RecordingEventEmitter::new();
        let events = emitter.events();
        let mock = Arc::new(MockPluginExecutor::new());
        let executor =
            PipelineExecutor::with_event_emitter(mock, Arc::new(emitter));
        let workflow = make_async_workflow(vec![make_step("fast-plugin", "act")]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await
            .unwrap();

        let payload_json = serde_json::to_value(&result.payload).unwrap();
        let job_id = payload_json["data"]["job_id"].as_str().unwrap().to_string();

        // Wait for background task
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let recorded = events.read().await;
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].0, "workflow.completed");
        assert_eq!(recorded[0].1["job_id"], job_id);
        assert_eq!(recorded[0].1["workflow_id"], "async-test-workflow");
    }

    #[tokio::test]
    async fn async_mode_emits_failed_event_on_error() {
        let emitter = RecordingEventEmitter::new();
        let events = emitter.events();
        let mock = Arc::new(FailingPluginExecutor::new(0)); // fail immediately
        let executor =
            PipelineExecutor::with_event_emitter(mock, Arc::new(emitter));
        let workflow = make_async_workflow(vec![make_step_with_strategy(
            "bad-plugin",
            "crash",
            ErrorStrategyType::Halt,
        )]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await
            .unwrap();

        let payload_json = serde_json::to_value(&result.payload).unwrap();
        let job_id_str = payload_json["data"]["job_id"].as_str().unwrap();
        let job_id: Uuid = job_id_str.parse().unwrap();

        // Wait for background task
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Job status should be Failed
        let status = executor.job_status(&job_id).await;
        assert!(matches!(status, Some(JobStatus::Failed(_))));

        // Event should have been emitted
        let recorded = events.read().await;
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].0, "workflow.failed");
        assert_eq!(recorded[0].1["job_id"], job_id_str);
        assert!(recorded[0].1["error"].is_string());
    }

    #[tokio::test]
    async fn sync_mode_returns_result_after_all_steps() {
        let mock = Arc::new(MockPluginExecutor::new());
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![
            make_step("a", "act"),
            make_step("b", "act"),
        ]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await
            .unwrap();

        // All steps should have executed
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 2);

        // Result should be the final step's output, not a job_id
        let payload_json = serde_json::to_value(&result.payload).unwrap();
        let data = &payload_json["data"];
        assert_eq!(data["executed_by"], "b.act");
        assert!(data.get("job_id").is_none());
    }

    #[tokio::test]
    async fn async_preserves_correlation_id() {
        let mock = Arc::new(MockPluginExecutor::new());
        let executor = PipelineExecutor::new(mock);
        let workflow = make_async_workflow(vec![make_step("p", "a")]);
        let msg = make_test_message();
        let expected_id = msg.metadata.correlation_id;

        let result = executor.execute_workflow(&workflow, msg).await.unwrap();
        assert_eq!(result.metadata.correlation_id, expected_id);
    }

    // --- build_initial_message tests ---

    #[test]
    fn build_message_from_endpoint_trigger() {
        let ctx = TriggerContext::Endpoint {
            method: "POST".into(),
            path: "/email/sync".into(),
            body: serde_json::json!({"folder": "inbox"}),
            auth: Some(serde_json::json!({"user_id": "u-123", "provider": "pocket-id"})),
        };

        let msg = build_initial_message(ctx).unwrap();

        assert_eq!(msg.metadata.source, "endpoint:POST /email/sync");
        assert!(msg.metadata.auth_context.is_some());
        assert_eq!(
            msg.metadata.auth_context.unwrap()["user_id"],
            "u-123"
        );

        let payload_json = serde_json::to_value(&msg.payload).unwrap();
        assert_eq!(payload_json["data"]["folder"], "inbox");
    }

    #[test]
    fn build_message_from_endpoint_trigger_no_auth() {
        let ctx = TriggerContext::Endpoint {
            method: "GET".into(),
            path: "/health".into(),
            body: serde_json::json!({}),
            auth: None,
        };

        let msg = build_initial_message(ctx).unwrap();

        assert_eq!(msg.metadata.source, "endpoint:GET /health");
        assert!(msg.metadata.auth_context.is_none());
    }

    #[test]
    fn build_message_from_event_trigger() {
        let ctx = TriggerContext::Event {
            name: "webhook.email.received".into(),
            payload: serde_json::json!({"sender": "alice@example.com", "subject": "Hello"}),
        };

        let msg = build_initial_message(ctx).unwrap();

        assert_eq!(msg.metadata.source, "event:webhook.email.received");
        assert!(msg.metadata.auth_context.is_none());

        let payload_json = serde_json::to_value(&msg.payload).unwrap();
        assert_eq!(payload_json["data"]["sender"], "alice@example.com");
    }

    #[test]
    fn build_message_from_schedule_trigger() {
        let fired_at = Utc::now();
        let ctx = TriggerContext::Schedule {
            workflow_id: "sync-email".into(),
            fired_at,
        };

        let msg = build_initial_message(ctx).unwrap();

        assert_eq!(msg.metadata.source, "schedule:sync-email");
        assert!(msg.metadata.auth_context.is_none());

        // Payload should be an empty object
        let payload_json = serde_json::to_value(&msg.payload).unwrap();
        let data = &payload_json["data"];
        assert!(data.is_object());
        assert_eq!(data.as_object().unwrap().len(), 0);
    }

    #[test]
    fn build_message_generates_unique_correlation_ids() {
        let ctx1 = TriggerContext::Event {
            name: "test".into(),
            payload: serde_json::json!({}),
        };
        let ctx2 = TriggerContext::Event {
            name: "test".into(),
            payload: serde_json::json!({}),
        };

        let msg1 = build_initial_message(ctx1).unwrap();
        let msg2 = build_initial_message(ctx2).unwrap();

        assert_ne!(msg1.metadata.correlation_id, msg2.metadata.correlation_id);
    }

    // --- Halt error strategy tests (WP 7.8) ---

    use crate::types::{ErrorStrategy, ErrorStrategyType};
    use life_engine_traits::Severity;

    fn make_step_with_strategy(plugin: &str, action: &str, strategy: ErrorStrategyType) -> StepDef {
        StepDef {
            plugin: plugin.into(),
            action: action.into(),
            on_error: Some(ErrorStrategy {
                strategy,
                max_retries: None,
                fallback: None,
            }),
            condition: None,
        }
    }

    #[tokio::test]
    async fn halt_strategy_stops_pipeline_on_failure() {
        // Step 2 of 3 fails with halt strategy → step 3 must not execute
        let mock = Arc::new(FailingPluginExecutor::new(1)); // fail on second call
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![
            make_step("step-a", "act"),
            make_step_with_strategy("step-b", "act", ErrorStrategyType::Halt),
            make_step("step-c", "act"),
        ]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        assert!(result.is_err());
        // step-c should NOT have been called (only step-a and step-b)
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn halt_error_includes_step_index_and_plugin_info() {
        let mock = Arc::new(FailingPluginExecutor::new(1));
        let executor = PipelineExecutor::new(mock);
        let workflow = make_workflow(vec![
            make_step("step-a", "act"),
            make_step_with_strategy("failing-plugin", "do-thing", ErrorStrategyType::Halt),
            make_step("step-c", "act"),
        ]);

        let err = executor
            .execute_workflow(&workflow, make_test_message())
            .await
            .unwrap_err();

        match &err {
            WorkflowError::StepHalted {
                step_index,
                plugin,
                action,
                cause,
            } => {
                assert_eq!(*step_index, 1);
                assert_eq!(plugin, "failing-plugin");
                assert_eq!(action, "do-thing");
                assert!(!cause.is_empty());
            }
            other => panic!("expected StepHalted, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn halt_error_has_fatal_severity() {
        let mock = Arc::new(FailingPluginExecutor::new(0));
        let executor = PipelineExecutor::new(mock);
        let workflow = make_workflow(vec![make_step_with_strategy(
            "bad-plugin",
            "crash",
            ErrorStrategyType::Halt,
        )]);

        let err = executor
            .execute_workflow(&workflow, make_test_message())
            .await
            .unwrap_err();

        assert_eq!(err.code(), "WORKFLOW_003");
        assert_eq!(err.severity(), Severity::Fatal);
    }

    #[tokio::test]
    async fn default_strategy_with_retryable_severity_upgrades_to_retry() {
        // Steps with no on_error and Retryable severity upgrade to retry
        let mock = Arc::new(FailingPluginExecutor::new(1));
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![
            make_step("step-a", "act"),
            make_step("step-b", "act"), // no on_error + Retryable severity → retries
            make_step("step-c", "act"),
        ]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        // Retryable severity upgrades default halt to retry; retry succeeds on second attempt
        assert!(result.is_ok());
        // step-a succeeds (call 0), step-b fails (call 1), step-b retry succeeds (call 2), step-c succeeds (call 3)
        // But FailingPluginExecutor only fails on call at index fail_on_call=1, all others succeed
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 4);
    }

    // --- Skip error strategy tests (WP 7.9) ---

    #[tokio::test]
    async fn skip_strategy_continues_pipeline_on_failure() {
        // Step 2 of 3 fails with skip → step 3 receives step 1's output
        let mock = Arc::new(FailingPluginExecutor::new(1)); // fail on second call
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![
            make_step("step-a", "act"),
            make_step_with_strategy("step-b", "act", ErrorStrategyType::Skip),
            make_step("step-c", "act"),
        ]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        assert!(result.is_ok());
        // All 3 steps should have been attempted (step-a, step-b fails, step-c)
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn skip_strategy_passes_previous_output_to_next_step() {
        // Step 2 fails with skip → step 3 receives step 1's output (not step 2's)
        // FailingPluginExecutor: on success sets payload to {"executed_by": "<plugin>.<action>"}
        let mock = Arc::new(FailingPluginExecutor::new(1)); // fail on second call
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![
            make_step("step-a", "act"),
            make_step_with_strategy("step-b", "act", ErrorStrategyType::Skip),
            make_step("step-c", "act"),
        ]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await
            .unwrap();

        // step-c received step-a's output (step-b was skipped).
        // FailingPluginExecutor sets payload to {"executed_by": "<plugin>.<action>"} on success,
        // so step-c's output is {"executed_by": "step-c.act"}.
        let payload_json = serde_json::to_value(&result.payload).unwrap();
        let data = &payload_json["data"];
        assert_eq!(data["executed_by"], "step-c.act");

        // All 3 steps were attempted
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn skip_strategy_first_step_fails_passes_initial_message() {
        // First step fails with skip → second step receives the initial PipelineMessage
        let mock = Arc::new(FailingPluginExecutor::new(0)); // fail immediately
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![
            make_step_with_strategy("step-a", "act", ErrorStrategyType::Skip),
            make_step("step-b", "act"),
        ]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        assert!(result.is_ok());
        // step-a failed (1 call), step-b succeeded (1 call) = 2 total
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 2);

        // step-b received the initial message (not step-a's output)
        let result = result.unwrap();
        let payload_json = serde_json::to_value(&result.payload).unwrap();
        let data = &payload_json["data"];
        assert_eq!(data["executed_by"], "step-b.act");
    }

    #[tokio::test]
    async fn skip_strategy_pipeline_completes_successfully() {
        // A skipped step should not cause the pipeline to report failure
        let mock = Arc::new(FailingPluginExecutor::new(0));
        let executor = PipelineExecutor::new(mock);
        let workflow = make_workflow(vec![
            make_step_with_strategy("failing-plugin", "crash", ErrorStrategyType::Skip),
        ]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        // Pipeline should complete successfully even though the only step failed
        assert!(result.is_ok());
        // The result should be the initial message passed through unchanged
        let msg = result.unwrap();
        let payload_json = serde_json::to_value(&msg.payload).unwrap();
        let data = &payload_json["data"];
        assert_eq!(data["input"], true);
    }

    #[tokio::test]
    async fn halt_on_first_step_skips_all_remaining() {
        let mock = Arc::new(FailingPluginExecutor::new(0)); // fail immediately
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![
            make_step_with_strategy("bad-plugin", "crash", ErrorStrategyType::Halt),
            make_step("step-b", "act"),
            make_step("step-c", "act"),
        ]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        assert!(result.is_err());
        // Only the first step should have been called
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 1);
    }

    // --- Retry error strategy tests (WP 7.10) ---

    /// A mock plugin executor that fails for the first N calls, then succeeds.
    struct RetryablePluginExecutor {
        fail_count: usize,
        call_count: AtomicUsize,
    }

    impl RetryablePluginExecutor {
        fn new(fail_count: usize) -> Self {
            Self {
                fail_count,
                call_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl PluginExecutor for RetryablePluginExecutor {
        async fn execute(
            &self,
            plugin_id: &str,
            action: &str,
            mut input: PipelineMessage,
        ) -> Result<PipelineMessage, Box<dyn EngineError>> {
            let call = self.call_count.fetch_add(1, Ordering::SeqCst);
            if call < self.fail_count {
                return Err(Box::new(MockPluginError(format!(
                    "attempt {call} failed for {plugin_id}.{action}"
                ))));
            }
            let schema = serde_json::json!({"type": "object"});
            let transformed =
                serde_json::json!({"executed_by": format!("{plugin_id}.{action}"), "attempt": call});
            input.payload =
                TypedPayload::Custom(SchemaValidated::new(transformed, &schema).unwrap());
            Ok(input)
        }
    }

    fn make_retry_step(plugin: &str, action: &str, max_retries: u32) -> StepDef {
        StepDef {
            plugin: plugin.into(),
            action: action.into(),
            on_error: Some(ErrorStrategy {
                strategy: ErrorStrategyType::Retry,
                max_retries: Some(max_retries),
                fallback: None,
            }),
            condition: None,
        }
    }

    fn make_retry_step_with_fallback(
        plugin: &str,
        action: &str,
        max_retries: u32,
        fallback_plugin: &str,
        fallback_action: &str,
    ) -> StepDef {
        StepDef {
            plugin: plugin.into(),
            action: action.into(),
            on_error: Some(ErrorStrategy {
                strategy: ErrorStrategyType::Retry,
                max_retries: Some(max_retries),
                fallback: Some(Box::new(StepDef {
                    plugin: fallback_plugin.into(),
                    action: fallback_action.into(),
                    on_error: None,
                    condition: None,
                })),
            }),
            condition: None,
        }
    }

    #[tokio::test]
    async fn retry_succeeds_on_second_attempt() {
        // Fails on first call (call 0), succeeds on retry (call 1)
        let mock = Arc::new(RetryablePluginExecutor::new(1));
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![make_retry_step("flaky-plugin", "act", 3)]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        assert!(result.is_ok());
        // call 0 = initial fail, call 1 = first retry succeeds
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 2);

        let payload_json = serde_json::to_value(&result.unwrap().payload).unwrap();
        assert_eq!(payload_json["data"]["executed_by"], "flaky-plugin.act");
    }

    #[tokio::test]
    async fn retry_all_fail_without_fallback_halts_pipeline() {
        // Fails on all calls (more than max_retries + 1)
        let mock = Arc::new(RetryablePluginExecutor::new(100));
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![make_retry_step("bad-plugin", "crash", 2)]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        match &err {
            WorkflowError::RetryExhausted {
                step_index,
                plugin,
                action,
                retries,
                ..
            } => {
                assert_eq!(*step_index, 0);
                assert_eq!(plugin, "bad-plugin");
                assert_eq!(action, "crash");
                assert_eq!(*retries, 2);
            }
            other => panic!("expected RetryExhausted, got: {other:?}"),
        }
        // 1 initial + 2 retries = 3 total calls
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn retry_all_fail_with_fallback_executes_fallback() {
        // The retryable executor will fail for calls 0..2 (initial + 1 retry),
        // then succeed on call 2 which is the fallback.
        // We use fail_count=2 so calls 0,1 fail and call 2 (fallback) succeeds.
        let mock = Arc::new(RetryablePluginExecutor::new(2));
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![make_retry_step_with_fallback(
            "flaky-plugin",
            "act",
            1, // only 1 retry
            "fallback-plugin",
            "recover",
        )]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        assert!(result.is_ok());
        // call 0: initial fail, call 1: retry fail, call 2: fallback succeeds
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 3);

        let payload_json = serde_json::to_value(&result.unwrap().payload).unwrap();
        assert_eq!(
            payload_json["data"]["executed_by"],
            "fallback-plugin.recover"
        );
    }

    #[tokio::test]
    async fn retry_default_max_retries_is_3() {
        // Use a step with retry strategy but no explicit max_retries
        let mock = Arc::new(RetryablePluginExecutor::new(100));
        let executor = PipelineExecutor::new(mock.clone());
        let step = StepDef {
            plugin: "bad-plugin".into(),
            action: "crash".into(),
            on_error: Some(ErrorStrategy {
                strategy: ErrorStrategyType::Retry,
                max_retries: None, // should default to 3
                fallback: None,
            }),
            condition: None,
        };
        let workflow = make_workflow(vec![step]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            WorkflowError::RetryExhausted { retries, .. } => {
                assert_eq!(retries, 3);
            }
            other => panic!("expected RetryExhausted, got: {other:?}"),
        }
        // 1 initial + 3 retries = 4 total
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn retry_continues_pipeline_after_success() {
        // Step 1 is retryable and fails once, then step 2 runs normally
        // RetryablePluginExecutor: fails on call 0, succeeds on calls 1+
        let mock = Arc::new(RetryablePluginExecutor::new(1));
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![
            make_retry_step("flaky-plugin", "act", 3),
            make_step("next-plugin", "process"),
        ]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        assert!(result.is_ok());
        // call 0: flaky initial fail, call 1: flaky retry success, call 2: next-plugin
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 3);

        let payload_json = serde_json::to_value(&result.unwrap().payload).unwrap();
        assert_eq!(payload_json["data"]["executed_by"], "next-plugin.process");
    }

    // --- Severity override tests (WP 7.11) ---

    /// A mock error with configurable severity.
    #[derive(Debug)]
    struct SeverityError {
        msg: String,
        severity: Severity,
    }

    impl std::fmt::Display for SeverityError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.msg)
        }
    }

    impl std::error::Error for SeverityError {}

    impl EngineError for SeverityError {
        fn code(&self) -> &str {
            "SEVERITY_TEST"
        }
        fn severity(&self) -> Severity {
            self.severity.clone()
        }
        fn source_module(&self) -> &str {
            "test"
        }
    }

    /// A mock plugin executor that always fails with a configurable severity.
    struct SeverityPluginExecutor {
        severity: Severity,
        call_count: AtomicUsize,
    }

    impl SeverityPluginExecutor {
        fn new(severity: Severity) -> Self {
            Self {
                severity,
                call_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl PluginExecutor for SeverityPluginExecutor {
        async fn execute(
            &self,
            plugin_id: &str,
            action: &str,
            _input: PipelineMessage,
        ) -> Result<PipelineMessage, Box<dyn EngineError>> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Err(Box::new(SeverityError {
                msg: format!("{plugin_id}.{action} failed"),
                severity: self.severity.clone(),
            }))
        }
    }

    /// A mock executor that fails with configurable severity for the first N calls,
    /// then succeeds.
    struct SeverityRetryableExecutor {
        severity: Severity,
        fail_count: usize,
        call_count: AtomicUsize,
    }

    impl SeverityRetryableExecutor {
        fn new(severity: Severity, fail_count: usize) -> Self {
            Self {
                severity,
                fail_count,
                call_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl PluginExecutor for SeverityRetryableExecutor {
        async fn execute(
            &self,
            plugin_id: &str,
            action: &str,
            mut input: PipelineMessage,
        ) -> Result<PipelineMessage, Box<dyn EngineError>> {
            let call = self.call_count.fetch_add(1, Ordering::SeqCst);
            if call < self.fail_count {
                return Err(Box::new(SeverityError {
                    msg: format!("attempt {call} failed for {plugin_id}.{action}"),
                    severity: self.severity.clone(),
                }));
            }
            let schema = serde_json::json!({"type": "object"});
            let transformed =
                serde_json::json!({"executed_by": format!("{plugin_id}.{action}"), "attempt": call});
            input.payload =
                TypedPayload::Custom(SchemaValidated::new(transformed, &schema).unwrap());
            Ok(input)
        }
    }

    #[tokio::test]
    async fn fatal_severity_halts_even_with_skip_strategy() {
        let mock = Arc::new(SeverityPluginExecutor::new(Severity::Fatal));
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![make_step_with_strategy(
            "fatal-plugin",
            "crash",
            ErrorStrategyType::Skip,
        )]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        // Despite skip strategy, Fatal severity forces halt.
        assert!(result.is_err());
        match result.unwrap_err() {
            WorkflowError::StepHalted { plugin, .. } => {
                assert_eq!(plugin, "fatal-plugin");
            }
            other => panic!("expected StepHalted, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn fatal_severity_halts_even_with_retry_strategy() {
        let mock = Arc::new(SeverityPluginExecutor::new(Severity::Fatal));
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![make_retry_step("fatal-plugin", "crash", 3)]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        // Despite retry strategy, Fatal severity forces halt immediately.
        assert!(result.is_err());
        match result.unwrap_err() {
            WorkflowError::StepHalted { plugin, .. } => {
                assert_eq!(plugin, "fatal-plugin");
            }
            other => panic!("expected StepHalted, got: {other:?}"),
        }
        // Only one call — no retries attempted.
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn warning_severity_continues_with_halt_strategy() {
        let mock = Arc::new(SeverityPluginExecutor::new(Severity::Warning));
        let executor = PipelineExecutor::new(mock.clone());
        // Step with halt strategy but plugin returns Warning severity.
        let workflow = make_workflow(vec![make_step_with_strategy(
            "warn-plugin",
            "act",
            ErrorStrategyType::Halt,
        )]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        // Warning overrides halt — pipeline continues successfully.
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn warning_severity_passes_through_input() {
        // Two steps: first returns Warning error, second should receive the original input.
        // We need a mixed executor: step 0 warns, step 1 succeeds.
        struct WarnThenSucceedExecutor {
            call_count: AtomicUsize,
        }

        #[async_trait]
        impl PluginExecutor for WarnThenSucceedExecutor {
            async fn execute(
                &self,
                plugin_id: &str,
                action: &str,
                mut input: PipelineMessage,
            ) -> Result<PipelineMessage, Box<dyn EngineError>> {
                let call = self.call_count.fetch_add(1, Ordering::SeqCst);
                if call == 0 {
                    return Err(Box::new(SeverityError {
                        msg: "just a warning".into(),
                        severity: Severity::Warning,
                    }));
                }
                let schema = serde_json::json!({"type": "object"});
                let transformed =
                    serde_json::json!({"executed_by": format!("{plugin_id}.{action}")});
                input.payload =
                    TypedPayload::Custom(SchemaValidated::new(transformed, &schema).unwrap());
                Ok(input)
            }
        }

        let mock = Arc::new(WarnThenSucceedExecutor {
            call_count: AtomicUsize::new(0),
        });
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![
            make_step_with_strategy("warn-plugin", "act", ErrorStrategyType::Halt),
            make_step("next-plugin", "process"),
        ]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        assert!(result.is_ok());
        // Both steps were called.
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 2);
        // The second step received the original input (pass-through from warning).
        let payload_json = serde_json::to_value(&result.unwrap().payload).unwrap();
        assert_eq!(payload_json["data"]["executed_by"], "next-plugin.process");
    }

    #[tokio::test]
    async fn retryable_severity_upgrades_default_strategy_to_retry() {
        // Step has no declared strategy (defaults to Halt), plugin returns Retryable.
        // Since there's no explicit strategy, severity upgrades to retry with defaults.
        let mock = Arc::new(SeverityRetryableExecutor::new(Severity::Retryable, 1));
        let executor = PipelineExecutor::new(mock.clone());
        // make_step has no on_error (defaults to halt)
        let workflow = make_workflow(vec![make_step("flaky-plugin", "act")]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        // Should succeed after retry.
        assert!(result.is_ok());
        // call 0: initial fail, call 1: retry success
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn retryable_severity_respects_explicit_halt_strategy() {
        // Step explicitly declares halt, plugin returns Retryable.
        // Explicit step strategy wins — should halt, not retry.
        let mock = Arc::new(SeverityRetryableExecutor::new(Severity::Retryable, 1));
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![make_step_with_strategy(
            "flaky-plugin",
            "act",
            ErrorStrategyType::Halt,
        )]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            WorkflowError::StepHalted { plugin, .. } => {
                assert_eq!(plugin, "flaky-plugin");
            }
            other => panic!("expected StepHalted, got: {other:?}"),
        }
        // Only 1 call — halted immediately, no retries.
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn retryable_severity_respects_explicit_skip_strategy() {
        // Step explicitly declares skip, plugin returns Retryable.
        // Explicit step strategy wins — should skip, not retry.
        let mock = Arc::new(SeverityRetryableExecutor::new(Severity::Retryable, 100));
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![make_step_with_strategy(
            "flaky-plugin",
            "act",
            ErrorStrategyType::Skip,
        )]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        // Should succeed (skip continues pipeline).
        assert!(result.is_ok());
        // Only 1 call — skipped, no retries.
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn retryable_severity_uses_step_max_retries_when_declared() {
        // Step declares retry with max_retries=2, plugin returns Retryable.
        // Since the step also declares retry, the step's max_retries should be used.
        let mock = Arc::new(SeverityRetryableExecutor::new(Severity::Retryable, 100));
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![make_retry_step("flaky-plugin", "act", 2)]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        assert!(result.is_err());
        // 1 initial + 2 retries = 3 total calls.
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn retryable_severity_with_no_strategy_uses_default_retries() {
        // Step has no declared strategy, severity is Retryable.
        // Should use default max_retries=3.
        let mock = Arc::new(SeverityRetryableExecutor::new(Severity::Retryable, 100));
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![make_step("flaky-plugin", "act")]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        assert!(result.is_err());
        // 1 initial + 3 default retries = 4 total calls.
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn warning_severity_continues_with_skip_strategy() {
        let mock = Arc::new(SeverityPluginExecutor::new(Severity::Warning));
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![make_step_with_strategy(
            "warn-plugin",
            "act",
            ErrorStrategyType::Skip,
        )]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        // Warning always continues regardless of strategy.
        assert!(result.is_ok());
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn warning_severity_continues_with_retry_strategy() {
        let mock = Arc::new(SeverityPluginExecutor::new(Severity::Warning));
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![make_retry_step("warn-plugin", "act", 3)]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        // Warning always continues — no retries attempted.
        assert!(result.is_ok());
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 1);
    }

    // --- Conditional branching tests ---

    fn make_conditional_step(
        field: &str,
        equals: serde_json::Value,
        then_steps: Vec<StepDef>,
        else_steps: Vec<StepDef>,
    ) -> StepDef {
        use crate::types::{ConditionDef, ConditionOperator};
        StepDef {
            plugin: String::new(),
            action: String::new(),
            on_error: None,
            condition: Some(ConditionDef {
                field: field.into(),
                operator: ConditionOperator::Equals,
                value: equals,
                then_steps,
                else_steps,
            }),
        }
    }

    fn make_condition_step_with_operator(
        field: &str,
        operator: crate::types::ConditionOperator,
        value: serde_json::Value,
        then_steps: Vec<StepDef>,
        else_steps: Vec<StepDef>,
    ) -> StepDef {
        use crate::types::ConditionDef;
        StepDef {
            plugin: String::new(),
            action: String::new(),
            on_error: None,
            condition: Some(ConditionDef {
                field: field.into(),
                operator,
                value,
                then_steps,
                else_steps,
            }),
        }
    }

    #[tokio::test]
    async fn conditional_branch_takes_then_path_when_field_matches() {
        let mock = Arc::new(MockPluginExecutor::new());
        let executor = PipelineExecutor::new(mock.clone());

        // Create a message with category = "spam"
        let schema = serde_json::json!({"type": "object"});
        let msg = PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: Uuid::new_v4(),
                source: "test:unit".into(),
                timestamp: Utc::now(),
                auth_context: None,
                warnings: vec![],
            },
            payload: TypedPayload::Custom(
                SchemaValidated::new(serde_json::json!({"category": "spam"}), &schema).unwrap(),
            ),
        };

        let workflow = make_workflow(vec![make_conditional_step(
            "category",
            serde_json::json!("spam"),
            vec![make_step("spam-handler", "quarantine")],
            vec![make_step("inbox", "deliver")],
        )]);

        let result = executor.execute_workflow(&workflow, msg).await.unwrap();

        // Only the then branch should execute
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 1);
        let payload_json = serde_json::to_value(&result.payload).unwrap();
        let executed_by = payload_json
            .pointer("/data/executed_by")
            .and_then(|v| v.as_str());
        assert_eq!(executed_by, Some("spam-handler.quarantine"));
    }

    #[tokio::test]
    async fn conditional_branch_takes_else_path_when_field_does_not_match() {
        let mock = Arc::new(MockPluginExecutor::new());
        let executor = PipelineExecutor::new(mock.clone());

        let schema = serde_json::json!({"type": "object"});
        let msg = PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: Uuid::new_v4(),
                source: "test:unit".into(),
                timestamp: Utc::now(),
                auth_context: None,
                warnings: vec![],
            },
            payload: TypedPayload::Custom(
                SchemaValidated::new(serde_json::json!({"category": "legit"}), &schema).unwrap(),
            ),
        };

        let workflow = make_workflow(vec![make_conditional_step(
            "category",
            serde_json::json!("spam"),
            vec![make_step("spam-handler", "quarantine")],
            vec![make_step("inbox", "deliver")],
        )]);

        let result = executor.execute_workflow(&workflow, msg).await.unwrap();

        assert_eq!(mock.call_count.load(Ordering::SeqCst), 1);
        let payload_json = serde_json::to_value(&result.payload).unwrap();
        let executed_by = payload_json
            .pointer("/data/executed_by")
            .and_then(|v| v.as_str());
        assert_eq!(executed_by, Some("inbox.deliver"));
    }

    #[tokio::test]
    async fn conditional_branch_supports_dot_notation_field_path() {
        let mock = Arc::new(MockPluginExecutor::new());
        let executor = PipelineExecutor::new(mock.clone());

        let schema = serde_json::json!({"type": "object"});
        let msg = PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: Uuid::new_v4(),
                source: "test:unit".into(),
                timestamp: Utc::now(),
                auth_context: None,
                warnings: vec![],
            },
            payload: TypedPayload::Custom(
                SchemaValidated::new(
                    serde_json::json!({"result": {"status": "approved"}}),
                    &schema,
                )
                .unwrap(),
            ),
        };

        let workflow = make_workflow(vec![make_conditional_step(
            "result.status",
            serde_json::json!("approved"),
            vec![make_step("notifier", "send-approval")],
            vec![make_step("notifier", "send-rejection")],
        )]);

        let result = executor.execute_workflow(&workflow, msg).await.unwrap();

        assert_eq!(mock.call_count.load(Ordering::SeqCst), 1);
        let payload_json = serde_json::to_value(&result.payload).unwrap();
        let executed_by = payload_json
            .pointer("/data/executed_by")
            .and_then(|v| v.as_str());
        assert_eq!(executed_by, Some("notifier.send-approval"));
    }

    #[tokio::test]
    async fn conditional_branch_takes_else_when_field_missing() {
        let mock = Arc::new(MockPluginExecutor::new());
        let executor = PipelineExecutor::new(mock.clone());

        // Message with no "category" field
        let result = executor
            .execute_workflow(
                &make_workflow(vec![make_conditional_step(
                    "category",
                    serde_json::json!("spam"),
                    vec![make_step("spam-handler", "quarantine")],
                    vec![make_step("inbox", "deliver")],
                )]),
                make_test_message(),
            )
            .await
            .unwrap();

        assert_eq!(mock.call_count.load(Ordering::SeqCst), 1);
        let payload_json = serde_json::to_value(&result.payload).unwrap();
        let executed_by = payload_json
            .pointer("/data/executed_by")
            .and_then(|v| v.as_str());
        assert_eq!(executed_by, Some("inbox.deliver"));
    }

    // --- Pipeline validation tests ---

    fn make_workflow_with_validation(
        steps: Vec<StepDef>,
        validate: crate::types::ValidationLevel,
    ) -> WorkflowDef {
        WorkflowDef {
            id: "validation-test".into(),
            name: "Validation Test Workflow".into(),
            description: None,
            mode: ExecutionMode::Sync,
            validate,
            trigger: TriggerDef {
                endpoint: Some("POST /test".into()),
                event: None,
                schedule: None,
            },
            steps,
        }
    }

    /// A mock plugin executor that produces an invalid (non-object) payload
    /// at a specific call index, causing schema validation to fail.
    struct InvalidOutputExecutor {
        invalid_on_call: usize,
        call_count: AtomicUsize,
    }

    impl InvalidOutputExecutor {
        fn new(invalid_on_call: usize) -> Self {
            Self {
                invalid_on_call,
                call_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl PluginExecutor for InvalidOutputExecutor {
        async fn execute(
            &self,
            plugin_id: &str,
            action: &str,
            mut input: PipelineMessage,
        ) -> Result<PipelineMessage, Box<dyn EngineError>> {
            let call = self.call_count.fetch_add(1, Ordering::SeqCst);
            let schema = serde_json::json!({"type": "object"});
            if call == self.invalid_on_call {
                // Produce a string payload — not a valid object
                input.payload = TypedPayload::Custom(
                    SchemaValidated::new(serde_json::json!("invalid-string"), &serde_json::json!({"type": "string"})).unwrap(),
                );
            } else {
                let transformed = serde_json::json!({"executed_by": format!("{plugin_id}.{action}")});
                input.payload =
                    TypedPayload::Custom(SchemaValidated::new(transformed, &schema).unwrap());
            }
            Ok(input)
        }
    }

    #[tokio::test]
    async fn strict_validation_catches_invalid_intermediate_output() {
        let mock = Arc::new(InvalidOutputExecutor::new(0)); // first step produces invalid output
        let executor = PipelineExecutor::new(mock);
        let workflow = make_workflow_with_validation(
            vec![
                make_step("bad-plugin", "act"),
                make_step("good-plugin", "act"),
            ],
            crate::types::ValidationLevel::Strict,
        );

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            WorkflowError::ValidationFailed { step_index, details } => {
                assert_eq!(step_index, 0);
                assert!(details.contains("strict validation"));
            }
            other => panic!("expected ValidationFailed, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn edges_validation_allows_invalid_intermediate_but_catches_invalid_exit() {
        // Step 0 produces invalid, step 1 also produces invalid → exit validation fails
        let mock = Arc::new(InvalidOutputExecutor::new(1)); // second step produces invalid
        let executor = PipelineExecutor::new(mock);
        let workflow = make_workflow_with_validation(
            vec![
                make_step("good-plugin", "act"),
                make_step("bad-plugin", "act"),
            ],
            crate::types::ValidationLevel::Edges,
        );

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            WorkflowError::ValidationFailed { details, .. } => {
                assert!(details.contains("exit validation"));
            }
            other => panic!("expected ValidationFailed, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn edges_validation_allows_invalid_intermediate_output() {
        // Step 0 produces invalid, step 1 produces valid → edges should pass
        // (edges only checks entry and exit, not intermediate)
        let mock = Arc::new(InvalidOutputExecutor::new(0));
        let executor = PipelineExecutor::new(mock);
        let workflow = make_workflow_with_validation(
            vec![
                make_step("bad-plugin", "act"),
                make_step("good-plugin", "act"),
            ],
            crate::types::ValidationLevel::Edges,
        );

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        // Should succeed — invalid intermediate is allowed, final is valid object
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn none_validation_allows_everything() {
        let mock = Arc::new(InvalidOutputExecutor::new(1)); // last step produces invalid
        let executor = PipelineExecutor::new(mock);
        let workflow = make_workflow_with_validation(
            vec![
                make_step("good-plugin", "act"),
                make_step("bad-plugin", "act"),
            ],
            crate::types::ValidationLevel::None,
        );

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        // Should succeed — no validation at all
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn strict_validation_passes_with_all_valid_outputs() {
        let mock = Arc::new(MockPluginExecutor::new());
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow_with_validation(
            vec![
                make_step("a", "act"),
                make_step("b", "act"),
                make_step("c", "act"),
            ],
            crate::types::ValidationLevel::Strict,
        );

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        assert!(result.is_ok());
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 3);
    }

    // --- Control Flow: condition operators and error handling tests ---

    fn make_msg_with_payload(payload: serde_json::Value) -> PipelineMessage {
        let schema = serde_json::json!({"type": "object"});
        PipelineMessage {
            metadata: MessageMetadata {
                correlation_id: Uuid::new_v4(),
                source: "test:control-flow".into(),
                timestamp: Utc::now(),
                auth_context: None,
                warnings: vec![],
            },
            payload: TypedPayload::Custom(
                SchemaValidated::new(payload, &schema).unwrap(),
            ),
        }
    }

    #[tokio::test]
    async fn condition_not_equals_takes_then_when_value_differs() {
        let mock = Arc::new(MockPluginExecutor::new());
        let executor = PipelineExecutor::new(mock.clone());

        let msg = make_msg_with_payload(serde_json::json!({"status": "draft"}));

        let workflow = make_workflow(vec![make_condition_step_with_operator(
            "status",
            crate::types::ConditionOperator::NotEquals,
            serde_json::json!("published"),
            vec![make_step("draft-handler", "process")],
            vec![make_step("published-handler", "process")],
        )]);

        let result = executor.execute_workflow(&workflow, msg).await.unwrap();

        assert_eq!(mock.call_count.load(Ordering::SeqCst), 1);
        let payload_json = serde_json::to_value(&result.payload).unwrap();
        assert_eq!(
            payload_json.pointer("/data/executed_by").and_then(|v| v.as_str()),
            Some("draft-handler.process")
        );
    }

    #[tokio::test]
    async fn condition_not_equals_takes_else_when_value_matches() {
        let mock = Arc::new(MockPluginExecutor::new());
        let executor = PipelineExecutor::new(mock.clone());

        let msg = make_msg_with_payload(serde_json::json!({"status": "published"}));

        let workflow = make_workflow(vec![make_condition_step_with_operator(
            "status",
            crate::types::ConditionOperator::NotEquals,
            serde_json::json!("published"),
            vec![make_step("draft-handler", "process")],
            vec![make_step("published-handler", "process")],
        )]);

        let result = executor.execute_workflow(&workflow, msg).await.unwrap();

        assert_eq!(mock.call_count.load(Ordering::SeqCst), 1);
        let payload_json = serde_json::to_value(&result.payload).unwrap();
        assert_eq!(
            payload_json.pointer("/data/executed_by").and_then(|v| v.as_str()),
            Some("published-handler.process")
        );
    }

    #[tokio::test]
    async fn condition_exists_takes_then_when_field_present_including_null() {
        let mock = Arc::new(MockPluginExecutor::new());
        let executor = PipelineExecutor::new(mock.clone());

        // Field exists with null value — should take then branch
        let msg = make_msg_with_payload(serde_json::json!({"tag": null}));

        let workflow = make_workflow(vec![make_condition_step_with_operator(
            "tag",
            crate::types::ConditionOperator::Exists,
            serde_json::Value::Null,
            vec![make_step("has-tag", "process")],
            vec![make_step("no-tag", "process")],
        )]);

        let result = executor.execute_workflow(&workflow, msg).await.unwrap();

        let payload_json = serde_json::to_value(&result.payload).unwrap();
        assert_eq!(
            payload_json.pointer("/data/executed_by").and_then(|v| v.as_str()),
            Some("has-tag.process")
        );
    }

    #[tokio::test]
    async fn condition_exists_takes_else_when_field_missing() {
        let mock = Arc::new(MockPluginExecutor::new());
        let executor = PipelineExecutor::new(mock.clone());

        let msg = make_msg_with_payload(serde_json::json!({"other": "data"}));

        let workflow = make_workflow(vec![make_condition_step_with_operator(
            "tag",
            crate::types::ConditionOperator::Exists,
            serde_json::Value::Null,
            vec![make_step("has-tag", "process")],
            vec![make_step("no-tag", "process")],
        )]);

        let result = executor.execute_workflow(&workflow, msg).await.unwrap();

        let payload_json = serde_json::to_value(&result.payload).unwrap();
        assert_eq!(
            payload_json.pointer("/data/executed_by").and_then(|v| v.as_str()),
            Some("no-tag.process")
        );
    }

    #[tokio::test]
    async fn condition_is_empty_takes_then_for_null_empty_string_empty_array_and_absent() {
        // null → then
        let mock = Arc::new(MockPluginExecutor::new());
        let executor = PipelineExecutor::new(mock);
        let msg = make_msg_with_payload(serde_json::json!({"val": null}));
        let workflow = make_workflow(vec![make_condition_step_with_operator(
            "val",
            crate::types::ConditionOperator::IsEmpty,
            serde_json::Value::Null,
            vec![make_step("empty-handler", "process")],
            vec![make_step("non-empty-handler", "process")],
        )]);
        let result = executor.execute_workflow(&workflow, msg).await.unwrap();
        let pj = serde_json::to_value(&result.payload).unwrap();
        assert_eq!(pj.pointer("/data/executed_by").and_then(|v| v.as_str()), Some("empty-handler.process"));

        // empty string → then
        let mock2 = Arc::new(MockPluginExecutor::new());
        let executor2 = PipelineExecutor::new(mock2);
        let msg2 = make_msg_with_payload(serde_json::json!({"val": ""}));
        let workflow2 = make_workflow(vec![make_condition_step_with_operator(
            "val",
            crate::types::ConditionOperator::IsEmpty,
            serde_json::Value::Null,
            vec![make_step("empty-handler", "process")],
            vec![make_step("non-empty-handler", "process")],
        )]);
        let result2 = executor2.execute_workflow(&workflow2, msg2).await.unwrap();
        let pj2 = serde_json::to_value(&result2.payload).unwrap();
        assert_eq!(pj2.pointer("/data/executed_by").and_then(|v| v.as_str()), Some("empty-handler.process"));

        // empty array → then
        let mock3 = Arc::new(MockPluginExecutor::new());
        let executor3 = PipelineExecutor::new(mock3);
        let msg3 = make_msg_with_payload(serde_json::json!({"val": []}));
        let workflow3 = make_workflow(vec![make_condition_step_with_operator(
            "val",
            crate::types::ConditionOperator::IsEmpty,
            serde_json::Value::Null,
            vec![make_step("empty-handler", "process")],
            vec![make_step("non-empty-handler", "process")],
        )]);
        let result3 = executor3.execute_workflow(&workflow3, msg3).await.unwrap();
        let pj3 = serde_json::to_value(&result3.payload).unwrap();
        assert_eq!(pj3.pointer("/data/executed_by").and_then(|v| v.as_str()), Some("empty-handler.process"));

        // absent field → then
        let mock4 = Arc::new(MockPluginExecutor::new());
        let executor4 = PipelineExecutor::new(mock4);
        let msg4 = make_msg_with_payload(serde_json::json!({"other": "x"}));
        let workflow4 = make_workflow(vec![make_condition_step_with_operator(
            "val",
            crate::types::ConditionOperator::IsEmpty,
            serde_json::Value::Null,
            vec![make_step("empty-handler", "process")],
            vec![make_step("non-empty-handler", "process")],
        )]);
        let result4 = executor4.execute_workflow(&workflow4, msg4).await.unwrap();
        let pj4 = serde_json::to_value(&result4.payload).unwrap();
        assert_eq!(pj4.pointer("/data/executed_by").and_then(|v| v.as_str()), Some("empty-handler.process"));
    }

    #[tokio::test]
    async fn condition_is_empty_takes_else_for_non_empty_values() {
        let mock = Arc::new(MockPluginExecutor::new());
        let executor = PipelineExecutor::new(mock.clone());

        let msg = make_msg_with_payload(serde_json::json!({"val": "hello"}));

        let workflow = make_workflow(vec![make_condition_step_with_operator(
            "val",
            crate::types::ConditionOperator::IsEmpty,
            serde_json::Value::Null,
            vec![make_step("empty-handler", "process")],
            vec![make_step("non-empty-handler", "process")],
        )]);

        let result = executor.execute_workflow(&workflow, msg).await.unwrap();

        let payload_json = serde_json::to_value(&result.payload).unwrap();
        assert_eq!(
            payload_json.pointer("/data/executed_by").and_then(|v| v.as_str()),
            Some("non-empty-handler.process")
        );
    }

    #[tokio::test]
    async fn skip_strategy_appends_warning_to_message_metadata() {
        let mock = Arc::new(FailingPluginExecutor::new(0)); // fail immediately
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![
            make_step_with_strategy("failing-plugin", "crash", ErrorStrategyType::Skip),
            make_step("next-plugin", "act"),
        ]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await
            .unwrap();

        // The skipped step's error should be appended as a warning
        assert!(!result.metadata.warnings.is_empty());
        assert!(result.metadata.warnings[0].contains("failing-plugin.crash"));
        assert!(result.metadata.warnings[0].contains("skipped"));
    }

    #[tokio::test]
    async fn branch_output_feeds_next_step_after_condition_block() {
        // Condition block output (from whichever branch) feeds the next step (Req 5.1-5.2).
        let mock = Arc::new(MockPluginExecutor::new());
        let executor = PipelineExecutor::new(mock.clone());

        let msg = make_msg_with_payload(serde_json::json!({"route": "a"}));

        let workflow = make_workflow(vec![
            make_conditional_step(
                "route",
                serde_json::json!("a"),
                vec![make_step("branch-a", "transform")],
                vec![make_step("branch-b", "transform")],
            ),
            make_step("post-branch", "finalize"),
        ]);

        let result = executor.execute_workflow(&workflow, msg).await.unwrap();

        // branch-a runs, then post-branch receives branch-a's output
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 2);
        let payload_json = serde_json::to_value(&result.payload).unwrap();
        assert_eq!(
            payload_json.pointer("/data/executed_by").and_then(|v| v.as_str()),
            Some("post-branch.finalize")
        );
        // post-branch received branch-a's output as "previous"
        assert!(payload_json.pointer("/data/previous").is_some());
    }

    #[tokio::test]
    async fn missing_field_takes_else_for_not_equals_operator() {
        // Missing field should take else branch for NotEquals (Req 3.6)
        let mock = Arc::new(MockPluginExecutor::new());
        let executor = PipelineExecutor::new(mock.clone());

        let msg = make_msg_with_payload(serde_json::json!({"other": "x"}));

        let workflow = make_workflow(vec![make_condition_step_with_operator(
            "missing_field",
            crate::types::ConditionOperator::NotEquals,
            serde_json::json!("anything"),
            vec![make_step("then-handler", "act")],
            vec![make_step("else-handler", "act")],
        )]);

        let result = executor.execute_workflow(&workflow, msg).await.unwrap();

        let payload_json = serde_json::to_value(&result.payload).unwrap();
        assert_eq!(
            payload_json.pointer("/data/executed_by").and_then(|v| v.as_str()),
            Some("else-handler.act")
        );
    }

    // --- Pipeline Executor Phase 8 Tests ---

    #[tokio::test]
    async fn spawn_returns_job_id_and_sets_status_to_in_progress() {
        // Req 1.2, 7.1: spawn() returns a JobId immediately and registers InProgress.
        let notify = Arc::new(Notify::new());
        let mock = Arc::new(SlowPluginExecutor::new(Arc::clone(&notify)));
        let executor = Arc::new(PipelineExecutor::new(mock));
        let workflow = make_async_workflow(vec![make_step("slow-plugin", "process")]);
        let trigger = TriggerContext::Event {
            name: "test.event".into(),
            payload: serde_json::json!({"key": "value"}),
        };

        let job_id = executor.spawn(trigger, &workflow);

        // Give the background task a moment to register the job
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

        let status = executor.job_status(&job_id).await;
        assert_eq!(status, Some(JobStatus::InProgress));

        // Clean up: release the slow plugin
        notify.notify_one();
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let status = executor.job_status(&job_id).await;
        assert_eq!(status, Some(JobStatus::Completed));
    }

    #[tokio::test]
    async fn spawn_sets_failed_status_on_workflow_error() {
        // Req 7.3: When an async workflow fails, job status becomes Failed.
        let mock = Arc::new(FailingPluginExecutor::new(0));
        let executor = Arc::new(PipelineExecutor::new(mock));
        let workflow = make_async_workflow(vec![make_step_with_strategy(
            "bad-plugin",
            "crash",
            ErrorStrategyType::Halt,
        )]);
        let trigger = TriggerContext::Event {
            name: "test.fail".into(),
            payload: serde_json::json!({}),
        };

        let job_id = executor.spawn(trigger, &workflow);

        // Wait for background task
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let status = executor.job_status(&job_id).await;
        assert!(matches!(status, Some(JobStatus::Failed(_))));
    }

    #[tokio::test]
    async fn job_entry_contains_response_on_completion() {
        // Req 8.2: JobEntry has status, response, and created_at.
        // Req 7.2: Completed job stores the WorkflowResponse.
        let mock = Arc::new(MockPluginExecutor::new());
        let executor = Arc::new(PipelineExecutor::new(mock));
        let workflow = make_async_workflow(vec![make_step("fast-plugin", "act")]);
        let trigger = TriggerContext::Event {
            name: "test.complete".into(),
            payload: serde_json::json!({"data": "test"}),
        };

        let job_id = executor.spawn(trigger, &workflow);

        // Wait for background task
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let entry = executor.job_entry(&job_id).await;
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.status, JobStatus::Completed);
        assert!(entry.response.is_some());
        assert!(entry.created_at <= Utc::now());
    }

    #[tokio::test]
    async fn cleanup_expired_jobs_removes_old_entries() {
        // Req 7.5: Jobs exceeding TTL are evicted.
        let mock = Arc::new(MockPluginExecutor::new());
        let executor = PipelineExecutor {
            plugin_executor: mock as Arc<dyn PluginExecutor>,
            jobs: Arc::new(RwLock::new(HashMap::new())),
            event_emitter: Arc::new(NoOpEventEmitter),
            semaphore: Arc::new(Semaphore::new(DEFAULT_CONCURRENCY_LIMIT)),
            job_ttl: std::time::Duration::from_millis(1), // very short TTL
        };

        // Manually insert an old job entry
        let job_id = Uuid::new_v4();
        executor.jobs.write().await.insert(
            job_id,
            JobEntry {
                status: JobStatus::Completed,
                response: None,
                created_at: Utc::now() - chrono::Duration::hours(2),
            },
        );

        // Insert a fresh job entry
        let fresh_id = Uuid::new_v4();
        executor.jobs.write().await.insert(
            fresh_id,
            JobEntry {
                status: JobStatus::Completed,
                response: None,
                created_at: Utc::now(),
            },
        );

        let removed = executor.cleanup_expired_jobs().await;

        // The old entry should be removed, the fresh one kept
        assert_eq!(removed, 1);
        assert!(executor.job_entry(&job_id).await.is_none());
        assert!(executor.job_entry(&fresh_id).await.is_some());
    }

    #[tokio::test]
    async fn concurrency_limit_queues_excess_executions() {
        // Req 9.1, 9.2: Concurrency limit queues additional executions.
        let notify = Arc::new(Notify::new());
        let mock = Arc::new(SlowPluginExecutor::new(Arc::clone(&notify)));
        // Limit to 1 concurrent execution
        let executor = Arc::new(PipelineExecutor::with_concurrency_limit(mock.clone(), 1));
        let workflow = make_workflow(vec![make_step("slow-plugin", "process")]);

        // Start first execution (takes the only permit)
        let exec1 = {
            let ex = Arc::clone(&executor);
            let wf = workflow.clone();
            tokio::spawn(async move {
                ex.execute_workflow(&wf, make_test_message()).await
            })
        };

        // Give first task time to acquire the permit and enter the plugin
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

        // First execution entered the plugin (call_count = 1), waiting on notify
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 1);

        // Start second execution (should queue on the semaphore)
        let exec2 = {
            let ex = Arc::clone(&executor);
            let wf = workflow.clone();
            tokio::spawn(async move {
                ex.execute_workflow(&wf, make_test_message()).await
            })
        };

        // Wait a bit — second execution should still be queued (semaphore blocked)
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

        // Still only 1 call — second execution is queued waiting for the permit
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 1);

        // Release first execution
        notify.notify_one();
        let _ = exec1.await;

        // Give second execution time to acquire the permit and enter the plugin
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

        // Second execution should now be inside the plugin
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 2);

        // Release second execution
        notify.notify_one();
        let _ = exec2.await;
    }

    #[tokio::test]
    async fn custom_concurrency_limit_is_respected() {
        // Req 9.3: Configurable concurrency limit.
        let mock = Arc::new(MockPluginExecutor::new());
        let executor = PipelineExecutor::with_concurrency_limit(mock, 4);

        // Verify the executor was created with the custom limit (4 permits)
        assert_eq!(executor.semaphore.available_permits(), 4);
    }

    #[tokio::test]
    async fn pipeline_message_constructed_from_endpoint_trigger_with_metadata() {
        // Req 2.1: Endpoint trigger populates payload and auth metadata.
        let ctx = TriggerContext::Endpoint {
            method: "POST".into(),
            path: "/api/v1/emails".into(),
            body: serde_json::json!({"subject": "Test", "to": "alice@example.com"}),
            auth: Some(serde_json::json!({"user_id": "u-42"})),
        };

        let msg = build_initial_message(ctx).unwrap();

        assert_eq!(msg.metadata.source, "endpoint:POST /api/v1/emails");
        assert!(msg.metadata.auth_context.is_some());
        let auth = msg.metadata.auth_context.unwrap();
        assert_eq!(auth["user_id"], "u-42");

        let payload_json = serde_json::to_value(&msg.payload).unwrap();
        assert_eq!(payload_json["data"]["subject"], "Test");
    }

    #[tokio::test]
    async fn pipeline_message_constructed_from_schedule_trigger_empty_payload() {
        // Req 2.3: Schedule trigger maps to empty payload with workflow_id in source.
        let fired_at = Utc::now();
        let ctx = TriggerContext::Schedule {
            workflow_id: "daily-sync".into(),
            fired_at,
        };

        let msg = build_initial_message(ctx).unwrap();

        assert_eq!(msg.metadata.source, "schedule:daily-sync");
        assert!(msg.metadata.auth_context.is_none());

        let payload_json = serde_json::to_value(&msg.payload).unwrap();
        let data = &payload_json["data"];
        assert!(data.is_object());
        assert_eq!(data.as_object().unwrap().len(), 0);
    }
}
