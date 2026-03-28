//! Identity and trigger context types for the workflow engine contract.
//!
//! `Identity` represents the authenticated (or guest) caller of a workflow.
//! `TriggerContext` describes how a workflow was triggered.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Verified identity from the auth middleware.
///
/// Every `WorkflowRequest` carries an `Identity`. Authenticated routes
/// populate it from the auth token; public routes use `Identity::guest()`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Identity {
    /// The authenticated subject (e.g. user ID, or `"anonymous"` for guests).
    pub subject: String,
    /// The token issuer (e.g. `"life-engine"`, or `"system"` for guests).
    pub issuer: String,
    /// Arbitrary claims extracted from the auth token.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub claims: HashMap<String, serde_json::Value>,
}

impl Identity {
    /// Create a guest/anonymous identity for public routes.
    pub fn guest() -> Self {
        Self {
            subject: "anonymous".into(),
            issuer: "system".into(),
            claims: HashMap::new(),
        }
    }
}

/// Describes how a workflow was triggered.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TriggerContext {
    /// Triggered by an HTTP/GraphQL endpoint.
    Endpoint {
        /// HTTP method (e.g. `"GET"`, `"POST"`).
        method: String,
        /// Request path (e.g. `"/api/v1/tasks"`).
        path: String,
    },
    /// Triggered by an internal event.
    Event {
        /// Event type name (e.g. `"record.created"`).
        event_type: String,
        /// Source plugin or subsystem.
        source: String,
    },
    /// Triggered by a cron schedule.
    Schedule {
        /// Cron expression (e.g. `"0 */5 * * *"`).
        cron_expr: String,
    },
}
