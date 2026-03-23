//! YAML-defined workflow execution engine for Life Engine.

pub mod config;
pub mod error;
pub mod event_bus;
pub mod executor;
pub mod loader;
pub mod scheduler;
pub mod types;

pub use config::WorkflowConfig;
pub use error::WorkflowError;
pub use loader::{load_workflows, HttpMethod, TriggerRegistry};
pub use executor::{
    JobStatus, NoOpEventEmitter, PipelineExecutor, PluginExecutor, WorkflowEventEmitter,
};
pub use types::{
    ConditionDef, ErrorStrategy, ErrorStrategyType, ExecutionMode, StepDef, TriggerDef,
    ValidationLevel, WorkflowDef,
};

#[cfg(test)]
mod tests;
