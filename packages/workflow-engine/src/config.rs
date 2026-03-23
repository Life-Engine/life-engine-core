//! Workflow engine configuration.

use serde::{Deserialize, Serialize};

/// Configuration for the workflow engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowConfig {
    /// Directory containing YAML workflow definition files.
    pub path: String,
}
