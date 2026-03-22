//! Workflow engine error types.

use thiserror::Error;

/// Errors that can occur during workflow execution.
#[derive(Debug, Error)]
pub enum WorkflowError {
    /// Workflow definition is invalid.
    #[error("invalid workflow definition: {0}")]
    InvalidDefinition(String),

    /// Workflow step execution failed.
    #[error("step execution failed: {0}")]
    StepFailed(String),

    /// Workflow loading failed.
    #[error("workflow loading failed: {0}")]
    LoadFailed(String),
}
