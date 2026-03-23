<!--
project: life-engine-core
phase: 7
specs: workflow-engine
updated: 2026-03-23
-->

# Phase 7 — Workflow Engine

## Plan Overview

This phase implements the workflow engine (`packages/workflow-engine/`): the central orchestration layer that connects transports to plugin steps via declarative YAML pipelines. Work covers YAML workflow loading and validation, the trigger registry (endpoint, event, schedule), the sequential step executor with sync/async execution modes, error handling strategies (halt, skip, retry with fallback), conditional branching, pipeline validation, the event bus for async event-driven execution, and the cron scheduler for time-based triggers.

This phase depends on Phase 2 (types), Phase 3 (traits), and Phase 4 (plugin SDK). Phase 8 (plugin system) and Phase 9 (Core startup) depend on the workflow engine.

> spec: .odm/spec/workflow-engine/brief.md

Progress: 0 / 17 work packages complete

---

## 7.1 — Crate Scaffold
> spec: .odm/spec/workflow-engine/brief.md

- [x] Set up workflow-engine crate with standard layout and dependencies
  <!-- file: packages/workflow-engine/Cargo.toml -->
  <!-- file: packages/workflow-engine/src/lib.rs -->
  <!-- purpose: Ensure Cargo.toml has name = "life-engine-workflow-engine" with dependencies on life-engine-types (workspace), life-engine-traits (workspace), serde (workspace), serde_json (workspace), serde_yaml = "0.9" (workflow YAML parsing), tokio (workspace with rt-multi-thread, sync, time features), tracing (workspace), thiserror (workspace), uuid (workspace), chrono (workspace), cron = "0.13" (cron expression parsing). Ensure src/lib.rs declares modules: config, error, types, loader, executor, event_bus, scheduler, and re-exports the public API: WorkflowEngine (main entry point), WorkflowConfig. Ensure src/config.rs has WorkflowConfig struct with path (String — directory containing YAML files). Ensure src/error.rs has WorkflowError enum implementing EngineError with codes WORKFLOW_001 through WORKFLOW_010. Verify crate compiles with no warnings. -->
  <!-- requirements: from workflow-engine spec 1.1 -->
  <!-- leverage: Phase 1 scaffold -->

---

## 7.2 — Workflow Types
> spec: .odm/spec/workflow-engine/brief.md

- [x] Define all workflow definition types with serde derives
  <!-- file: packages/workflow-engine/src/types.rs -->
  <!-- purpose: Define WorkflowDef struct: id (String — unique workflow identifier), name (String — human-readable), mode (ExecutionMode), validate (ValidationLevel), trigger (TriggerDef), steps (Vec<StepDef>). Define StepDef struct: plugin (String — plugin ID), action (String — action name from manifest), on_error (Option<ErrorStrategy>), condition (Option<ConditionDef>). Define TriggerDef struct with all optional fields: endpoint (Option<String> — HTTP method + path like "POST /email/sync"), event (Option<String> — event name like "webhook.email.received"), schedule (Option<String> — cron expression like "*/5 * * * *"). Define ExecutionMode enum: Sync, Async with Default = Sync, serde rename_all = "lowercase". Define ValidationLevel enum: Strict, Edges, None with Default = Edges, serde rename_all = "lowercase". Define ErrorStrategy struct: strategy (ErrorStrategyType enum: Halt, Skip, Retry), max_retries (Option<u32> — only for Retry, default 3), fallback (Option<StepDef> — fallback step if all retries fail). Default ErrorStrategy is Halt. Define ConditionDef struct: field (String — dot-notation path like "payload.category"), equals (serde_json::Value — comparison value), then_steps (Vec<StepDef>), else_steps (Vec<StepDef>). All types derive Serialize, Deserialize, Debug, Clone. -->
  <!-- requirements: from workflow-engine spec 1.2 -->
  <!-- leverage: none -->

---

## 7.3 — YAML Workflow Loader
> depends: 7.2
> spec: .odm/spec/workflow-engine/brief.md

- [x] Implement workflow directory scanning and YAML parsing
  <!-- file: packages/workflow-engine/src/loader.rs -->
  <!-- purpose: Implement pub fn load_workflows(config: &WorkflowConfig) -> Result<Vec<WorkflowDef>, WorkflowError>. Logic: (1) scan the configured directory for all *.yaml and *.yml files, (2) parse each file using serde_yaml into a top-level struct containing a workflows map (HashMap<String, WorkflowDef>), (3) validate required fields: every workflow must have an id and at least one step, (4) validate each step references a plugin and action, (5) detect duplicate workflow IDs across all files — reject with WORKFLOW_001 error identifying both files, (6) detect duplicate endpoint triggers across all files — reject with WORKFLOW_002 error, (7) return the flattened Vec<WorkflowDef>. Produce clear errors with the filename and line context for parse failures. Log info-level summary: "Loaded N workflows from M files". -->
  <!-- requirements: from workflow-engine spec 2.1 -->
  <!-- leverage: none -->

---

## 7.4 — Trigger Registry
> depends: 7.3
> spec: .odm/spec/workflow-engine/brief.md

- [x] Build immutable trigger registry from loaded workflows
  <!-- file: packages/workflow-engine/src/loader.rs -->
  <!-- file: packages/workflow-engine/src/types.rs -->
  <!-- purpose: Define TriggerRegistry struct with three internal maps: endpoints (HashMap<(HttpMethod, String), WorkflowDef> — maps HTTP method + path to workflow), events (HashMap<String, Vec<WorkflowDef>> — maps event name to list of workflows, multiple workflows can trigger on the same event), schedules (Vec<(CronExpression, WorkflowDef)> — list of cron-triggered workflows). Implement TriggerRegistry::build(workflows: Vec<WorkflowDef>) -> Result<TriggerRegistry> that populates all three maps from the workflow trigger definitions. Implement lookup methods: find_endpoint(method: &str, path: &str) -> Option<&WorkflowDef>, find_event(event_name: &str) -> Vec<&WorkflowDef>, get_schedules() -> &[(CronExpression, WorkflowDef)]. The registry is immutable after construction — rebuilt on workflow reload. Add tests: endpoint lookup by method+path, event lookup returns multiple workflows, schedule enumeration, missing endpoint returns None. -->
  <!-- requirements: from workflow-engine spec 2.2 -->
  <!-- leverage: loader.rs -->

---

## 7.5 — Sequential Step Executor
> depends: 7.2
> spec: .odm/spec/workflow-engine/brief.md

- [x] Implement pipeline executor for sequential step execution
  <!-- file: packages/workflow-engine/src/executor.rs -->
  <!-- purpose: Define PipelineExecutor struct holding a reference to the plugin execution bridge (trait-based, not concrete). Implement pub async fn execute_workflow(&self, workflow: &WorkflowDef, initial_message: PipelineMessage) -> Result<PipelineMessage, WorkflowError>. Logic: (1) start with the initial PipelineMessage, (2) for each step in workflow.steps, call the plugin execution bridge with (step.plugin, step.action, current_message), (3) capture the returned PipelineMessage as input for the next step, (4) return the final step's output as the workflow result. Define a PluginExecutor trait with async fn execute(plugin_id: &str, action: &str, input: PipelineMessage) -> Result<PipelineMessage, Box<dyn EngineError>> — the plugin system (Phase 8) will implement this trait. For now, the executor depends on this trait abstraction. Log each step execution: step index, plugin ID, action name. -->
  <!-- requirements: from workflow-engine spec 3.1 -->
  <!-- leverage: none -->

---

## 7.6 — Sync and Async Execution Modes
> depends: 7.5
> spec: .odm/spec/workflow-engine/brief.md

- [ ] Implement sync and async execution mode handling
  <!-- file: packages/workflow-engine/src/executor.rs -->
  <!-- purpose: Extend execute_workflow to handle ExecutionMode. For ExecutionMode::Sync: await all steps sequentially and return the final PipelineMessage result directly — the transport blocks until completion. For ExecutionMode::Async: generate a UUID job_id, spawn the pipeline execution on a tokio background task using tokio::spawn, return immediately with a PipelineMessage containing the job_id in metadata. When the background task completes, emit a system event "workflow.completed" or "workflow.failed" with the job_id, workflow_id, and result/error via the event bus. Define JobStatus enum: Running, Completed, Failed. Store job status in an in-memory HashMap<Uuid, JobStatus> wrapped in Arc<RwLock> for status queries. Add tests: sync mode returns result after all steps, async mode returns job_id immediately, async completion emits event. -->
  <!-- requirements: from workflow-engine spec 3.2 -->
  <!-- leverage: executor.rs -->

---

## 7.7 — Initial PipelineMessage Construction
> depends: 7.5
> spec: .odm/spec/workflow-engine/brief.md

- [ ] Construct initial PipelineMessage from trigger context
  <!-- file: packages/workflow-engine/src/executor.rs -->
  <!-- purpose: Implement pub fn build_initial_message(trigger_context: TriggerContext) -> PipelineMessage. Define TriggerContext enum: Endpoint { method: String, path: String, body: serde_json::Value, auth: Option<AuthIdentity> }, Event { name: String, payload: serde_json::Value }, Schedule { workflow_id: String, fired_at: DateTime<Utc> }. Construction logic: (1) generate a new UUID v4 correlation_id, (2) set source string from trigger type: "endpoint:POST /email/sync", "event:webhook.email.received", "schedule:sync-email", (3) set timestamp to current UTC time, (4) for Endpoint: set payload to TypedPayload::Custom with the request body, set auth_context from the AuthIdentity, (5) for Event: set payload to TypedPayload::Custom with the event payload, auth_context is None, (6) for Schedule: set payload to TypedPayload::Custom with empty object {}, auth_context is None. Add tests for each trigger type. -->
  <!-- requirements: from workflow-engine spec 3.3 -->
  <!-- leverage: executor.rs -->

---

## 7.8 — Halt Error Strategy
> depends: 7.5
> spec: .odm/spec/workflow-engine/brief.md

- [ ] Implement halt error strategy (default behavior)
  <!-- file: packages/workflow-engine/src/executor.rs -->
  <!-- file: packages/workflow-engine/src/error.rs -->
  <!-- purpose: When a step fails and its on_error strategy is Halt (or on_error is None, which defaults to Halt): immediately stop the workflow, do not execute any subsequent steps, construct a WorkflowError with code "WORKFLOW_003" and Severity::Fatal including context: failed step index, plugin ID, action name, and the underlying error message. Return this error from execute_workflow. Define WorkflowStepError struct with fields: step_index (usize), plugin_id (String), action (String), cause (Box<dyn EngineError>). Add tests: step 2 of 3 fails with halt → step 3 does not execute, error includes step index and plugin ID, severity is Fatal. -->
  <!-- requirements: from workflow-engine spec 4.1 -->
  <!-- leverage: executor.rs -->

---

## 7.9 — Skip Error Strategy
> depends: 7.5
> spec: .odm/spec/workflow-engine/brief.md

- [ ] Implement skip error strategy
  <!-- file: packages/workflow-engine/src/executor.rs -->
  <!-- purpose: When a step fails and its on_error strategy is Skip: log a warning with the step index, plugin ID, action, and error message using tracing::warn!, skip the failed step entirely, pass the previous step's PipelineMessage (the input to the failed step, not its output) as input to the next step. The pipeline continues as if the failed step was never in the chain. Add tests: step 2 of 3 fails with skip → step 3 receives step 1's output (not step 2's), warning is logged, pipeline completes successfully. Test edge case: first step fails with skip → second step receives the initial PipelineMessage. -->
  <!-- requirements: from workflow-engine spec 4.2 -->
  <!-- leverage: executor.rs -->

---

## 7.10 — Retry Error Strategy with Fallback
> depends: 7.5
> spec: .odm/spec/workflow-engine/brief.md

- [ ] Implement retry error strategy with exponential backoff and fallback
  <!-- file: packages/workflow-engine/src/executor.rs -->
  <!-- purpose: When a step fails and its on_error strategy is Retry: retry the step with exponential backoff delays (1s, 2s, 4s, 8s...) up to max_retries attempts (default 3). On each retry, call the same plugin action with the same input PipelineMessage. If any retry succeeds, continue the workflow with the successful output. If all retries are exhausted: if a fallback StepDef is declared, execute the fallback step with the original input and continue the pipeline with its output; if no fallback is declared, halt the workflow with a WorkflowError including the retry count and all error messages. Use tokio::time::sleep for backoff delays. Add tests: (1) retry succeeds on second attempt → pipeline continues, (2) all retries fail with fallback → fallback executes, (3) all retries fail without fallback → pipeline halts, (4) exponential backoff timing is correct, (5) retry count is configurable per step. -->
  <!-- requirements: from workflow-engine spec 4.3 -->
  <!-- leverage: executor.rs -->

---

## 7.11 — EngineError Severity Handling
> depends: 7.8, 7.9, 7.10
> spec: .odm/spec/workflow-engine/brief.md

- [ ] Handle EngineError severity from plugin responses
  <!-- file: packages/workflow-engine/src/executor.rs -->
  <!-- purpose: When a plugin returns an error implementing EngineError, check its severity() before applying the step's declared error strategy. Severity overrides: (1) Severity::Fatal — always halt the pipeline regardless of declared strategy (even if step says skip or retry), (2) Severity::Retryable — treat as retryable regardless of declared strategy (retry with default max_retries if step doesn't declare retry), (3) Severity::Warning — log the warning via tracing::warn! and continue execution without applying any error strategy (the step's output is used as-is, or if the plugin returned no output, pass through the input). This creates a hierarchy: plugin severity > step strategy for Fatal and Warning, step strategy wins for Retryable when the step also declares retry. Add tests for each severity override scenario. -->
  <!-- requirements: from workflow-engine spec 4.4 -->
  <!-- leverage: executor.rs -->

---

## 7.12 — Conditional Branching
> depends: 7.5
> spec: .odm/spec/workflow-engine/brief.md

- [ ] Implement conditional step branching
  <!-- file: packages/workflow-engine/src/executor.rs -->
  <!-- purpose: When a StepDef has a condition (ConditionDef) instead of a plugin/action: evaluate the condition against the current PipelineMessage. Evaluation logic: (1) resolve the field path using dot-notation (e.g., "payload.category" → navigate into the serialized PipelineMessage JSON), (2) compare the resolved value with condition.equals using serde_json::Value equality, (3) if match, execute condition.then_steps sequentially as a sub-pipeline, (4) if no match, execute condition.else_steps sequentially as a sub-pipeline, (5) the sub-pipeline's final output becomes the input for the next step in the parent pipeline. Support nested field paths: "metadata.source", "payload.data.status", etc. Handle missing fields: if the path doesn't resolve, treat as non-matching (else branch). Add tests: condition matches → then branch executes, condition doesn't match → else branch executes, missing field → else branch, nested field access works. -->
  <!-- requirements: from workflow-engine spec 5.1 -->
  <!-- leverage: executor.rs -->

---

## 7.13 — Pipeline Validation
> depends: 7.5
> spec: .odm/spec/workflow-engine/brief.md

- [ ] Implement configurable schema validation per workflow
  <!-- file: packages/workflow-engine/src/executor.rs -->
  <!-- purpose: Based on the workflow's validate field (ValidationLevel): For Strict — after every step, validate the output PipelineMessage against the expected schema. If the payload is TypedPayload::Cdm, validate against the canonical JSON Schema for that CDM type. If TypedPayload::Custom, validate against the step's output_schema from the plugin's Action definition. Validation failure produces WorkflowError with code "WORKFLOW_004" and Severity::Fatal. For Edges — validate only the initial PipelineMessage (entry) and the final step's output (exit). For None — skip all validation. Validation uses the jsonschema crate. Log a debug message for each validation check showing what was validated. Add tests: strict catches invalid intermediate output, edges allows invalid intermediate but catches invalid final output, none allows everything. -->
  <!-- requirements: from workflow-engine spec 6.1 -->
  <!-- leverage: jsonschema crate -->

---

## 7.14 — Event Bus
> depends: 7.4
> spec: .odm/spec/workflow-engine/brief.md

- [ ] Implement event bus for async event-driven workflow triggering
  <!-- file: packages/workflow-engine/src/event_bus.rs -->
  <!-- purpose: Define EventBus struct using tokio::sync::broadcast channel for event distribution. Implement pub async fn emit(&self, event_name: String, payload: serde_json::Value) that: (1) looks up matching workflows in the TriggerRegistry, (2) for each matching workflow, spawn a new tokio task to execute it with an Event TriggerContext, (3) log the event emission and number of triggered workflows. Implement pub fn subscribe(&self) -> broadcast::Receiver<(String, serde_json::Value)> for components that want to listen to all events (e.g., logging, metrics). System events to emit: "plugin.loaded", "plugin.error", "workflow.completed", "workflow.failed", "storage.error". The event bus is thread-safe (Arc<EventBus>) and shared across the workflow engine, plugin system, and transports. Add tests: emit event triggers matching workflow, emit event with no matching workflows is a no-op, multiple workflows matching same event all execute independently. -->
  <!-- requirements: from workflow-engine spec 7.1 -->
  <!-- leverage: TriggerRegistry from WP 7.4 -->

---

## 7.15 — Cron Scheduler
> depends: 7.4
> spec: .odm/spec/workflow-engine/brief.md

- [ ] Implement cron scheduler for time-based workflow triggering
  <!-- file: packages/workflow-engine/src/scheduler.rs -->
  <!-- purpose: Define Scheduler struct that manages scheduled workflow execution. Implement pub async fn start(&self, registry: &TriggerRegistry, executor: Arc<PipelineExecutor>) that: (1) iterates all schedule entries from the registry, (2) for each cron expression, spawn a tokio task that loops: calculate next fire time from the cron expression using the cron crate, sleep until that time using tokio::time::sleep_until, then trigger the workflow with a Schedule TriggerContext, (3) if a scheduled workflow execution fails, emit a "workflow.failed" event via the event bus but continue scheduling — never stop the scheduler due to a workflow failure. Implement pub async fn stop(&self) that cancels all scheduled tasks using tokio::task::JoinHandle abort. Track each scheduled task's handle for graceful shutdown. Add tests: workflow fires at correct interval (use controlled time), failed workflow emits error event, scheduler continues after failure, stop cancels all tasks. -->
  <!-- requirements: from workflow-engine spec 7.2 -->
  <!-- leverage: cron crate, event_bus from WP 7.14 -->

---

## 7.16 — Endpoint Trigger Wiring
> depends: 7.4, 7.5
> spec: .odm/spec/workflow-engine/brief.md

- [ ] Expose endpoint matching method for transports
  <!-- file: packages/workflow-engine/src/lib.rs -->
  <!-- purpose: Define the WorkflowEngine struct as the main public entry point. It holds the TriggerRegistry, PipelineExecutor, EventBus, and Scheduler. Implement pub async fn handle_endpoint(&self, method: &str, path: &str, body: serde_json::Value, auth: Option<AuthIdentity>) -> Result<PipelineMessage, WorkflowError> that: (1) looks up the path in the trigger registry, (2) if found, builds an initial PipelineMessage from the endpoint trigger context, (3) executes the workflow, (4) returns the result. Implement pub fn has_endpoint(&self, method: &str, path: &str) -> bool for transports to check if a path is a workflow endpoint before routing. Transports call has_endpoint during request routing — if true, delegate to handle_endpoint instead of built-in handlers. Add a pub async fn new(config: WorkflowConfig, plugin_executor: Arc<dyn PluginExecutor>) -> Result<Self> constructor that loads workflows, builds the registry, and creates the executor, event bus, and scheduler. -->
  <!-- requirements: from workflow-engine spec 8.1 -->
  <!-- leverage: all previous WPs -->

---

## 7.17 — Execution Logging
> depends: 7.5
> spec: .odm/spec/workflow-engine/brief.md

- [ ] Implement structured execution logging with per-step timing
  <!-- file: packages/workflow-engine/src/executor.rs -->
  <!-- purpose: Instrument the step executor to capture timing and status per step. Define ExecutionLog struct: workflow_id (String), trigger_type (String), trigger_value (String), started_at (DateTime<Utc>), completed_at (DateTime<Utc>), total_duration_ms (u64), status (ExecutionStatus enum: Completed, Failed, PartiallyFailed), steps (Vec<StepLog>). Define StepLog struct: index (usize), plugin_id (String), action (String), status (StepStatus enum: Completed, Failed, Skipped, Retried), duration_ms (u64), error (Option<StepErrorLog>), retry_count (Option<u32>). Define StepErrorLog struct: message (String), code (String), severity (String), input_summary (String — truncated serialization of input PipelineMessage for debugging). After each workflow execution (success or failure), emit the ExecutionLog as a structured tracing event at info level using tracing::info!(execution_log = ?log). For failures, log at error level with the full error context. Skipped steps log the skip reason and original error. -->
  <!-- requirements: from workflow-engine spec 9.1, 9.2 -->
  <!-- leverage: executor.rs -->
