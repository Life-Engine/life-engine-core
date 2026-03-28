//! Identity type for the workflow engine contract.
//!
//! `Identity` represents the authenticated (or guest) caller of a workflow.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Verified identity from the auth middleware.
///
/// Every `WorkflowRequest` carries an `Identity`. Authenticated routes
/// populate it from the auth token; public routes use `Identity::guest()`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

