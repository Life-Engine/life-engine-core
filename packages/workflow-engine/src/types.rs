//! Workflow definition types for the workflow engine.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A complete workflow definition loaded from YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDef {
    /// Unique workflow identifier.
    pub id: String,
    /// Human-readable workflow name.
    pub name: String,
    /// Execution mode (sync or async).
    #[serde(default)]
    pub mode: ExecutionMode,
    /// Validation level for pipeline messages.
    #[serde(default)]
    pub validate: ValidationLevel,
    /// Trigger that activates this workflow.
    pub trigger: TriggerDef,
    /// Ordered list of steps to execute.
    pub steps: Vec<StepDef>,
}

/// A single step in a workflow pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepDef {
    /// Plugin ID to execute.
    pub plugin: String,
    /// Action name from the plugin's manifest.
    pub action: String,
    /// Error handling strategy for this step.
    #[serde(default)]
    pub on_error: Option<ErrorStrategy>,
    /// Optional conditional branching instead of direct execution.
    #[serde(default)]
    pub condition: Option<ConditionDef>,
}

/// Trigger definition — at least one field must be set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerDef {
    /// HTTP method + path (e.g., "POST /email/sync").
    #[serde(default)]
    pub endpoint: Option<String>,
    /// Event name (e.g., "webhook.email.received").
    #[serde(default)]
    pub event: Option<String>,
    /// Cron expression (e.g., "*/5 * * * *").
    #[serde(default)]
    pub schedule: Option<String>,
}

/// Workflow execution mode.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionMode {
    /// Execute synchronously — transport blocks until completion.
    #[default]
    Sync,
    /// Execute asynchronously — return job ID immediately.
    Async,
}

/// Validation level for pipeline messages during execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ValidationLevel {
    /// Validate after every step.
    Strict,
    /// Validate only entry and exit messages.
    #[default]
    Edges,
    /// Skip all validation.
    None,
}

/// Error handling strategy for a workflow step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorStrategy {
    /// The strategy type.
    pub strategy: ErrorStrategyType,
    /// Maximum retry attempts (only used with Retry strategy, default 3).
    #[serde(default)]
    pub max_retries: Option<u32>,
    /// Fallback step to execute if all retries fail.
    #[serde(default)]
    pub fallback: Option<Box<StepDef>>,
}

/// Error strategy type.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ErrorStrategyType {
    /// Stop the pipeline immediately.
    #[default]
    Halt,
    /// Skip the failed step and continue.
    Skip,
    /// Retry with exponential backoff.
    Retry,
}

/// Context describing what triggered a workflow execution.
///
/// Used by `build_initial_message` to construct the initial `PipelineMessage`
/// that enters the pipeline.
#[derive(Debug, Clone)]
pub enum TriggerContext {
    /// Triggered by an HTTP endpoint request.
    Endpoint {
        /// HTTP method (e.g., "POST").
        method: String,
        /// Request path (e.g., "/email/sync").
        path: String,
        /// Request body as JSON.
        body: serde_json::Value,
        /// Authenticated identity, if available (serialized as JSON value).
        auth: Option<serde_json::Value>,
    },
    /// Triggered by a named event.
    Event {
        /// Event name (e.g., "webhook.email.received").
        name: String,
        /// Event payload as JSON.
        payload: serde_json::Value,
    },
    /// Triggered by a cron schedule.
    Schedule {
        /// The workflow ID being triggered.
        workflow_id: String,
        /// When the schedule fired.
        fired_at: DateTime<Utc>,
    },
}

/// Conditional branching definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionDef {
    /// Dot-notation path to the field to evaluate (e.g., "payload.category").
    pub field: String,
    /// Value to compare against.
    pub equals: serde_json::Value,
    /// Steps to execute if the condition matches.
    pub then_steps: Vec<StepDef>,
    /// Steps to execute if the condition does not match.
    pub else_steps: Vec<StepDef>,
}
