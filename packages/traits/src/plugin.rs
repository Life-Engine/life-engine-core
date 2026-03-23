//! Plugin trait and action types for WASM plugin contracts.
//!
//! Defines the `Plugin` trait that WASM modules implement via the SDK,
//! and the `Action` struct describing declared plugin actions.

use life_engine_types::PipelineMessage;
use serde::{Deserialize, Serialize};

use crate::error::EngineError;

/// Describes a single action exposed by a plugin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Action {
    /// Unique action name within the plugin (e.g., "sync_emails", "transform_contact").
    pub name: String,
    /// Human-readable description of what the action does.
    pub description: String,
    /// Optional JSON Schema for validating the action's input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<String>,
    /// Optional JSON Schema for validating the action's output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<String>,
}

impl Action {
    /// Create a new action with the given name and description.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema: None,
            output_schema: None,
        }
    }

    /// Set the JSON Schema for validating this action's input.
    pub fn with_input_schema(mut self, schema: impl Into<String>) -> Self {
        self.input_schema = Some(schema.into());
        self
    }

    /// Set the JSON Schema for validating this action's output.
    pub fn with_output_schema(mut self, schema: impl Into<String>) -> Self {
        self.output_schema = Some(schema.into());
        self
    }
}

/// Trait for WASM plugin contracts.
///
/// Every plugin must declare its identity, version, and the actions it supports.
/// The workflow engine calls `execute` to run a named action with a pipeline message.
pub trait Plugin: Send + Sync {
    /// Unique plugin identifier (e.g., "google-calendar", "email-sync").
    fn id(&self) -> &str;

    /// Human-readable display name.
    fn display_name(&self) -> &str;

    /// Semver version string (e.g., "1.0.0").
    fn version(&self) -> &str;

    /// List of actions this plugin declares.
    fn actions(&self) -> Vec<Action>;

    /// Execute a named action with the given input message.
    fn execute(
        &self,
        action: &str,
        input: PipelineMessage,
    ) -> Result<PipelineMessage, Box<dyn EngineError>>;
}
