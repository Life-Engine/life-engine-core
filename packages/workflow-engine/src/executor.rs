//! Workflow step executor.
//!
//! Provides the `PipelineExecutor` which runs workflow steps sequentially,
//! passing each step's output as input to the next step. Supports both
//! sync (blocking) and async (background) execution modes.

use async_trait::async_trait;
use chrono::Utc;
use life_engine_traits::EngineError;
use life_engine_types::{MessageMetadata, PipelineMessage, SchemaValidated, TypedPayload};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::error::WorkflowError;
use crate::types::{ErrorStrategyType, ExecutionMode, TriggerContext, WorkflowDef};

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
    Running,
    /// Job completed successfully.
    Completed,
    /// Job failed with an error message.
    Failed(String),
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
        },
        payload: TypedPayload::Custom(validated),
    })
}

/// Executes workflow steps sequentially, passing each step's output
/// as input to the next step. Supports sync and async execution modes.
pub struct PipelineExecutor {
    plugin_executor: Arc<dyn PluginExecutor>,
    jobs: Arc<RwLock<HashMap<Uuid, JobStatus>>>,
    event_emitter: Arc<dyn WorkflowEventEmitter>,
}

impl PipelineExecutor {
    /// Create a new pipeline executor with the given plugin executor.
    pub fn new(plugin_executor: Arc<dyn PluginExecutor>) -> Self {
        Self {
            plugin_executor,
            jobs: Arc::new(RwLock::new(HashMap::new())),
            event_emitter: Arc::new(NoOpEventEmitter),
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
        }
    }

    /// Query the status of an async job by its ID.
    pub async fn job_status(&self, job_id: &Uuid) -> Option<JobStatus> {
        self.jobs.read().await.get(job_id).cloned()
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
    /// final result is returned directly.
    async fn execute_sync(
        &self,
        workflow: &WorkflowDef,
        initial_message: PipelineMessage,
    ) -> Result<PipelineMessage, WorkflowError> {
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

        // Mark the job as running
        self.jobs.write().await.insert(job_id, JobStatus::Running);

        // Clone what the background task needs
        let jobs = Arc::clone(&self.jobs);
        let event_emitter = Arc::clone(&self.event_emitter);
        let plugin_executor = Arc::clone(&self.plugin_executor);
        let workflow = workflow.clone();
        let correlation_id = initial_message.metadata.correlation_id;
        let auth_context = initial_message.metadata.auth_context.clone();

        tokio::spawn(async move {
            let bg_executor = PipelineExecutor {
                plugin_executor,
                jobs: Arc::clone(&jobs),
                event_emitter: Arc::clone(&event_emitter),
            };

            match bg_executor.run_steps(&workflow, initial_message).await {
                Ok(_result) => {
                    jobs.write().await.insert(job_id, JobStatus::Completed);
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
                    jobs.write()
                        .await
                        .insert(job_id, JobStatus::Failed(err_msg.clone()));
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

    /// Run the steps of a workflow sequentially, respecting each step's error strategy.
    async fn run_steps(
        &self,
        workflow: &WorkflowDef,
        initial_message: PipelineMessage,
    ) -> Result<PipelineMessage, WorkflowError> {
        info!(
            workflow_id = %workflow.id,
            workflow_name = %workflow.name,
            step_count = workflow.steps.len(),
            "starting workflow execution"
        );

        let mut current_message = initial_message;

        for (index, step) in workflow.steps.iter().enumerate() {
            info!(
                workflow_id = %workflow.id,
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
                    current_message = output;
                }
                Err(plugin_error) => {
                    let strategy = step
                        .on_error
                        .as_ref()
                        .map(|s| &s.strategy)
                        .unwrap_or(&ErrorStrategyType::Halt);

                    match strategy {
                        ErrorStrategyType::Halt => {
                            error!(
                                workflow_id = %workflow.id,
                                step_index = index,
                                plugin = %step.plugin,
                                action = %step.action,
                                error = %plugin_error,
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
                            warn!(
                                workflow_id = %workflow.id,
                                step_index = index,
                                plugin = %step.plugin,
                                action = %step.action,
                                error = %plugin_error,
                                "step failed with skip strategy — skipping and continuing"
                            );
                            // Pass the previous step's output (current_message) to the next step.
                            // current_message is unchanged — the failed step's output is discarded.
                        }
                        ErrorStrategyType::Retry => {
                            let max_retries = step
                                .on_error
                                .as_ref()
                                .and_then(|s| s.max_retries)
                                .unwrap_or(3);

                            let mut last_error = plugin_error;
                            let mut succeeded = false;

                            for attempt in 1..=max_retries {
                                let backoff = std::time::Duration::from_secs(1 << (attempt - 1));
                                warn!(
                                    workflow_id = %workflow.id,
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
                                            workflow_id = %workflow.id,
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

                            if !succeeded {
                                // Check for fallback step
                                let fallback = step
                                    .on_error
                                    .as_ref()
                                    .and_then(|s| s.fallback.as_ref());

                                if let Some(fallback_step) = fallback {
                                    warn!(
                                        workflow_id = %workflow.id,
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

        info!(
            workflow_id = %workflow.id,
            "workflow execution completed"
        );

        Ok(current_message)
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
            life_engine_traits::Severity::Fatal
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
            make_step("step-b", "act"),
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

        // The plugin hasn't been released yet — job should be Running
        let job_id: Uuid = data["job_id"].as_str().unwrap().parse().unwrap();
        let status = executor.job_status(&job_id).await;
        assert_eq!(status, Some(JobStatus::Running));

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
        let workflow = make_async_workflow(vec![make_step("bad-plugin", "crash")]);

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
    async fn default_strategy_is_halt_when_on_error_is_none() {
        // Steps with no on_error should behave identically to explicit halt
        let mock = Arc::new(FailingPluginExecutor::new(1));
        let executor = PipelineExecutor::new(mock.clone());
        let workflow = make_workflow(vec![
            make_step("step-a", "act"),
            make_step("step-b", "act"), // no on_error → defaults to halt
            make_step("step-c", "act"),
        ]);

        let result = executor
            .execute_workflow(&workflow, make_test_message())
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            WorkflowError::StepHalted { step_index, .. } => {
                assert_eq!(step_index, 1);
            }
            other => panic!("expected StepHalted, got: {other:?}"),
        }
        // step-c should not have been called
        assert_eq!(mock.call_count.load(Ordering::SeqCst), 2);
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
}
