//! Workflow step executor.
//!
//! Provides the `PipelineExecutor` which runs workflow steps sequentially,
//! passing each step's output as input to the next step.

use async_trait::async_trait;
use life_engine_traits::EngineError;
use life_engine_types::PipelineMessage;
use std::sync::Arc;
use tracing::info;

use crate::error::WorkflowError;
use crate::types::WorkflowDef;

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

/// Executes workflow steps sequentially, passing each step's output
/// as input to the next step.
pub struct PipelineExecutor {
    plugin_executor: Arc<dyn PluginExecutor>,
}

impl PipelineExecutor {
    /// Create a new pipeline executor with the given plugin executor.
    pub fn new(plugin_executor: Arc<dyn PluginExecutor>) -> Self {
        Self { plugin_executor }
    }

    /// Execute a workflow's steps sequentially.
    ///
    /// Starts with `initial_message` and passes each step's output as
    /// input to the next step. Returns the final step's output.
    pub async fn execute_workflow(
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

            current_message = self
                .plugin_executor
                .execute(&step.plugin, &step.action, current_message)
                .await
                .map_err(|e| WorkflowError::StepHalted {
                    step_index: index,
                    plugin: step.plugin.clone(),
                    action: step.action.clone(),
                    cause: e.to_string(),
                })?;
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
    use chrono::Utc;
    use life_engine_types::{MessageMetadata, SchemaValidated, TypedPayload};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use uuid::Uuid;

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
            mode: crate::types::ExecutionMode::Sync,
            validate: crate::types::ValidationLevel::None,
            trigger: TriggerDef {
                endpoint: Some("POST /test".into()),
                event: None,
                schedule: None,
            },
            steps,
        }
    }

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
}
