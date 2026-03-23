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
pub use event_bus::EventBus;
pub use executor::{
    build_initial_message, ExecutionLog, ExecutionStatus, JobStatus, NoOpEventEmitter,
    PipelineExecutor, PluginExecutor, StepErrorLog, StepLog, StepStatus, WorkflowEventEmitter,
};
pub use loader::{load_workflows, HttpMethod, TriggerRegistry};
pub use scheduler::Scheduler;
pub use types::{
    ConditionDef, ErrorStrategy, ErrorStrategyType, ExecutionMode, StepDef, TriggerContext,
    TriggerDef, ValidationLevel, WorkflowDef,
};

use std::sync::Arc;

use life_engine_types::PipelineMessage;
use tracing::info;

/// Main entry point for the workflow engine.
///
/// Holds the trigger registry, pipeline executor, event bus, and scheduler.
/// Transports call `has_endpoint` to check if an incoming request path matches
/// a workflow endpoint trigger, and `handle_endpoint` to execute the workflow.
pub struct WorkflowEngine {
    registry: Arc<TriggerRegistry>,
    executor: Arc<PipelineExecutor>,
    event_bus: Arc<EventBus>,
    /// Held to keep scheduler tasks alive; dropped on engine shutdown.
    _scheduler: Scheduler,
}

impl WorkflowEngine {
    /// Create a new workflow engine from configuration.
    ///
    /// Loads workflow definitions from YAML files, builds the trigger registry,
    /// creates the executor and event bus, and starts the cron scheduler.
    pub async fn new(
        config: WorkflowConfig,
        plugin_executor: Arc<dyn PluginExecutor>,
    ) -> Result<Self, WorkflowError> {
        let workflows = load_workflows(&config)?;
        let workflow_count = workflows.len();
        let registry = Arc::new(TriggerRegistry::build(workflows)?);

        let executor = Arc::new(PipelineExecutor::new(plugin_executor));

        let event_bus = Arc::new(EventBus::new(
            Arc::clone(&registry),
            Arc::clone(&executor),
        ));

        let scheduler = Scheduler::start(
            &registry,
            Arc::clone(&executor),
            Arc::clone(&event_bus) as Arc<dyn WorkflowEventEmitter>,
        )
        .await;

        info!(
            workflows = workflow_count,
            "Workflow engine initialized"
        );

        Ok(Self {
            registry,
            executor,
            event_bus,
            _scheduler: scheduler,
        })
    }

    /// Check if a request path matches a workflow endpoint trigger.
    ///
    /// Transports call this during request routing — if `true`, the request
    /// should be delegated to `handle_endpoint` instead of built-in handlers.
    pub fn has_endpoint(&self, method: &str, path: &str) -> bool {
        self.registry.find_endpoint(method, path).is_some()
    }

    /// Handle an incoming endpoint request by executing the matched workflow.
    ///
    /// Looks up the path in the trigger registry, builds an initial
    /// `PipelineMessage` from the endpoint trigger context, executes the
    /// workflow, and returns the result.
    pub async fn handle_endpoint(
        &self,
        method: &str,
        path: &str,
        body: serde_json::Value,
        auth: Option<serde_json::Value>,
    ) -> Result<PipelineMessage, WorkflowError> {
        let workflow = self
            .registry
            .find_endpoint(method, path)
            .ok_or_else(|| WorkflowError::InvalidDefinition {
                workflow_id: String::new(),
                reason: format!("no workflow registered for {} {}", method, path),
            })?
            .clone();

        info!(
            workflow_id = %workflow.id,
            method = %method,
            path = %path,
            "Routing endpoint request to workflow"
        );

        let trigger_context = TriggerContext::Endpoint {
            method: method.to_string(),
            path: path.to_string(),
            body,
            auth,
        };

        let initial_message = build_initial_message(trigger_context)?;
        self.executor
            .execute_workflow(&workflow, initial_message)
            .await
    }

    /// Get a reference to the trigger registry.
    pub fn registry(&self) -> &TriggerRegistry {
        &self.registry
    }

    /// Get a reference to the event bus.
    pub fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }
}

#[cfg(test)]
mod tests;
