//! Workflow engine error types implementing the EngineError trait.

use life_engine_traits::{EngineError, Severity};
use thiserror::Error;

/// Errors that can occur during workflow loading and execution.
#[derive(Debug, Error)]
pub enum WorkflowError {
    /// Duplicate workflow ID detected across files.
    #[error("duplicate workflow ID '{id}' found in '{file1}' and '{file2}'")]
    DuplicateWorkflowId {
        id: String,
        file1: String,
        file2: String,
    },

    /// Duplicate endpoint trigger detected across workflows.
    #[error("duplicate endpoint trigger '{endpoint}' in workflows '{workflow1}' and '{workflow2}'")]
    DuplicateEndpoint {
        endpoint: String,
        workflow1: String,
        workflow2: String,
    },

    /// Step execution failed and halted the pipeline.
    #[error("workflow halted at step {step_index} ({plugin}.{action}): {cause}")]
    StepHalted {
        step_index: usize,
        plugin: String,
        action: String,
        cause: String,
    },

    /// Schema validation failed on a pipeline message.
    #[error("schema validation failed at step {step_index}: {details}")]
    ValidationFailed {
        step_index: usize,
        details: String,
    },

    /// YAML parse or file I/O error during workflow loading.
    #[error("failed to load workflow from '{file}': {cause}")]
    LoadFailed {
        file: String,
        cause: String,
    },

    /// Workflow definition is missing required fields.
    #[error("invalid workflow definition '{workflow_id}': {reason}")]
    InvalidDefinition {
        workflow_id: String,
        reason: String,
    },

    /// Step retry exhausted without fallback.
    #[error("step {step_index} ({plugin}.{action}) failed after {retries} retries: {cause}")]
    RetryExhausted {
        step_index: usize,
        plugin: String,
        action: String,
        retries: u32,
        cause: String,
    },

    /// Event bus delivery failure.
    #[error("event bus error: {0}")]
    EventBusError(String),

    /// Scheduler failure.
    #[error("scheduler error: {0}")]
    SchedulerError(String),

    /// Plugin executor returned a fatal error.
    #[error("plugin execution error in '{plugin}': {cause}")]
    PluginExecutionError {
        plugin: String,
        cause: String,
    },
}

impl EngineError for WorkflowError {
    fn code(&self) -> &str {
        match self {
            WorkflowError::DuplicateWorkflowId { .. } => "WORKFLOW_001",
            WorkflowError::DuplicateEndpoint { .. } => "WORKFLOW_002",
            WorkflowError::StepHalted { .. } => "WORKFLOW_003",
            WorkflowError::ValidationFailed { .. } => "WORKFLOW_004",
            WorkflowError::LoadFailed { .. } => "WORKFLOW_005",
            WorkflowError::InvalidDefinition { .. } => "WORKFLOW_006",
            WorkflowError::RetryExhausted { .. } => "WORKFLOW_007",
            WorkflowError::EventBusError(_) => "WORKFLOW_008",
            WorkflowError::SchedulerError(_) => "WORKFLOW_009",
            WorkflowError::PluginExecutionError { .. } => "WORKFLOW_010",
        }
    }

    fn severity(&self) -> Severity {
        match self {
            WorkflowError::DuplicateWorkflowId { .. }
            | WorkflowError::DuplicateEndpoint { .. }
            | WorkflowError::LoadFailed { .. }
            | WorkflowError::InvalidDefinition { .. }
            | WorkflowError::StepHalted { .. }
            | WorkflowError::ValidationFailed { .. } => Severity::Fatal,

            WorkflowError::RetryExhausted { .. }
            | WorkflowError::PluginExecutionError { .. }
            | WorkflowError::EventBusError(_)
            | WorkflowError::SchedulerError(_) => Severity::Retryable,
        }
    }

    fn source_module(&self) -> &str {
        "workflow-engine"
    }
}
