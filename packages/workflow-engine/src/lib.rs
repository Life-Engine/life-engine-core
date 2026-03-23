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

#[cfg(test)]
mod tests;
