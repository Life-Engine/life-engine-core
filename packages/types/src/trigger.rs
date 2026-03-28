//! Trigger context types for the workflow engine.
//!
//! `TriggerContext` describes how a workflow was initiated — via an HTTP
//! endpoint, an internal event, or a scheduled job.

use serde::{Deserialize, Serialize};

use crate::workflow::WorkflowRequest;

/// Describes how a workflow execution was triggered.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TriggerContext {
    /// Triggered by an incoming HTTP request (REST or GraphQL).
    Endpoint(WorkflowRequest),
    /// Triggered by an internal event (e.g. a plugin emitting a domain event).
    Event {
        /// Event name (e.g. `"contact.created"`).
        name: String,
        /// Optional event payload.
        #[serde(skip_serializing_if = "Option::is_none")]
        payload: Option<serde_json::Value>,
        /// The source that emitted the event (e.g. plugin ID).
        source: String,
    },
    /// Triggered by the scheduler on a cron schedule.
    Schedule {
        /// The workflow ID being executed on schedule.
        workflow_id: String,
    },
}
