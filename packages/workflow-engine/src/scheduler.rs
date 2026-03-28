//! Cron-based workflow scheduler.
//!
//! Manages scheduled workflow execution by spawning a tokio task for each
//! cron-triggered workflow. Each task calculates the next fire time from
//! the cron expression, sleeps until that time, then triggers the workflow.
//!
//! Overlap prevention: if a workflow is still executing when its next tick
//! arrives, the tick is silently skipped with a debug-level log.

use std::collections::HashSet;
use std::sync::Arc;

use chrono::Utc;
use cron::Schedule;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::{debug, error, info};

use crate::executor::{build_initial_message, PipelineExecutor, WorkflowEventEmitter};
use crate::loader::TriggerRegistry;
use crate::types::{TriggerContext, WorkflowDef};

/// Tracks which scheduled workflows are currently executing.
///
/// Used for overlap prevention — if a workflow ID is present in the set,
/// the scheduler skips the tick rather than spawning a concurrent execution.
#[derive(Debug, Default, Clone)]
pub struct ScheduleJobTracker {
    in_progress: Arc<Mutex<HashSet<String>>>,
}

impl ScheduleJobTracker {
    /// Create a new empty tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark a workflow as in-progress. Returns `true` if it was not already
    /// in-progress (i.e. execution should proceed). Returns `false` if the
    /// workflow is already running (overlap detected — skip this tick).
    pub async fn try_start(&self, workflow_id: &str) -> bool {
        self.in_progress.lock().await.insert(workflow_id.to_string())
    }

    /// Mark a workflow as no longer in-progress.
    pub async fn finish(&self, workflow_id: &str) {
        self.in_progress.lock().await.remove(workflow_id);
    }

    /// Check if a workflow is currently in-progress.
    pub async fn is_in_progress(&self, workflow_id: &str) -> bool {
        self.in_progress.lock().await.contains(workflow_id)
    }
}

/// Cron scheduler that manages time-based workflow execution.
///
/// Holds handles to all spawned schedule tasks for graceful shutdown.
pub struct Scheduler {
    handles: Vec<JoinHandle<()>>,
    tracker: ScheduleJobTracker,
}

impl Scheduler {
    /// Start the scheduler, spawning a task for each schedule entry in the registry.
    ///
    /// Each task loops indefinitely: it calculates the next fire time from the
    /// cron expression, sleeps until that time, checks for overlap, then triggers
    /// the workflow. If a scheduled workflow execution fails, a
    /// `workflow.schedule.failed` event is emitted via the event bus but the
    /// scheduler continues.
    pub async fn start(
        registry: &TriggerRegistry,
        executor: Arc<PipelineExecutor>,
        event_emitter: Arc<dyn WorkflowEventEmitter>,
    ) -> Self {
        let schedules = registry.get_schedules();
        let mut handles = Vec::with_capacity(schedules.len());
        let tracker = ScheduleJobTracker::new();

        for (schedule, workflow) in schedules {
            let schedule = schedule.clone();
            let workflow = workflow.clone();
            let executor = Arc::clone(&executor);
            let event_emitter = Arc::clone(&event_emitter);
            let tracker = tracker.clone();

            info!(
                workflow_id = %workflow.id,
                "Starting cron scheduler for workflow"
            );

            let handle = tokio::spawn(async move {
                Self::run_schedule_loop(schedule, workflow, executor, event_emitter, tracker).await;
            });

            handles.push(handle);
        }

        info!(
            scheduled_workflows = handles.len(),
            "Cron scheduler started"
        );

        Self { handles, tracker }
    }

    /// Stop all scheduled tasks by aborting their handles.
    pub async fn stop(self) {
        let count = self.handles.len();
        for handle in &self.handles {
            handle.abort();
        }
        // Wait for all tasks to finish (they will return Err(JoinError) due to abort).
        for handle in self.handles {
            let _ = handle.await;
        }
        info!(stopped_tasks = count, "Cron scheduler stopped");
    }

    /// The number of active scheduled tasks.
    pub fn task_count(&self) -> usize {
        self.handles.len()
    }

    /// Get a reference to the job tracker for testing overlap state.
    pub fn tracker(&self) -> &ScheduleJobTracker {
        &self.tracker
    }

    /// Internal loop for a single scheduled workflow.
    async fn run_schedule_loop(
        schedule: Schedule,
        workflow: WorkflowDef,
        executor: Arc<PipelineExecutor>,
        event_emitter: Arc<dyn WorkflowEventEmitter>,
        tracker: ScheduleJobTracker,
    ) {
        loop {
            let now = Utc::now();
            let next = match schedule.upcoming(Utc).next() {
                Some(next) => next,
                None => {
                    error!(
                        workflow_id = %workflow.id,
                        "No upcoming schedule times — stopping scheduler for this workflow"
                    );
                    return;
                }
            };

            let duration = (next - now).to_std().unwrap_or_default();

            info!(
                workflow_id = %workflow.id,
                next_fire = %next,
                delay_ms = duration.as_millis() as u64,
                "Waiting for next scheduled execution"
            );

            tokio::time::sleep(duration).await;

            // Overlap prevention: skip if this workflow is still executing.
            if !tracker.try_start(&workflow.id).await {
                debug!(
                    workflow_id = %workflow.id,
                    "Skipping scheduled tick — workflow is still in progress"
                );
                continue;
            }

            let fired_at = Utc::now();
            let trigger_context = TriggerContext::Schedule {
                workflow_id: workflow.id.clone(),
                fired_at,
            };

            let initial_message = match build_initial_message(trigger_context) {
                Ok(msg) => msg,
                Err(e) => {
                    error!(
                        workflow_id = %workflow.id,
                        error = %e,
                        "Failed to build initial message for scheduled workflow"
                    );
                    event_emitter
                        .emit(
                            "workflow.schedule.failed",
                            serde_json::json!({
                                "workflow_id": workflow.id,
                                "error": e.to_string(),
                                "fired_at": fired_at.to_rfc3339(),
                            }),
                        )
                        .await;
                    tracker.finish(&workflow.id).await;
                    continue;
                }
            };

            info!(
                workflow_id = %workflow.id,
                fired_at = %fired_at,
                "Executing scheduled workflow"
            );

            if let Err(e) = executor.execute_workflow(&workflow, initial_message).await {
                error!(
                    workflow_id = %workflow.id,
                    error = %e,
                    "Scheduled workflow execution failed"
                );
                event_emitter
                    .emit(
                        "workflow.schedule.failed",
                        serde_json::json!({
                            "workflow_id": workflow.id,
                            "error": e.to_string(),
                            "fired_at": fired_at.to_rfc3339(),
                        }),
                    )
                    .await;
            }

            tracker.finish(&workflow.id).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorkflowConfig;
    use crate::executor::PluginExecutor;
    use crate::loader::load_workflows;
    use async_trait::async_trait;
    use life_engine_traits::EngineError;
    use life_engine_types::PipelineMessage;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex as StdMutex;
    use tempfile::TempDir;

    /// A mock plugin executor that counts invocations.
    struct CountingExecutor {
        call_count: AtomicUsize,
    }

    impl CountingExecutor {
        fn new() -> Self {
            Self {
                call_count: AtomicUsize::new(0),
            }
        }

        fn count(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl PluginExecutor for CountingExecutor {
        async fn execute(
            &self,
            _plugin_id: &str,
            _action: &str,
            input: PipelineMessage,
        ) -> Result<PipelineMessage, Box<dyn EngineError>> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(input)
        }
    }

    /// A mock event emitter that records emitted events.
    struct RecordingEmitter {
        events: StdMutex<Vec<(String, serde_json::Value)>>,
    }

    impl RecordingEmitter {
        fn new() -> Self {
            Self {
                events: StdMutex::new(Vec::new()),
            }
        }

        fn event_names(&self) -> Vec<String> {
            self.events
                .lock()
                .unwrap()
                .iter()
                .map(|(name, _)| name.clone())
                .collect()
        }
    }

    #[async_trait]
    impl WorkflowEventEmitter for RecordingEmitter {
        async fn emit(&self, event_name: &str, payload: serde_json::Value) {
            self.events
                .lock()
                .unwrap()
                .push((event_name.to_string(), payload));
        }
    }

    /// A mock plugin executor that always fails.
    struct FailingExecutor;

    #[async_trait]
    impl PluginExecutor for FailingExecutor {
        async fn execute(
            &self,
            _plugin_id: &str,
            _action: &str,
            _input: PipelineMessage,
        ) -> Result<PipelineMessage, Box<dyn EngineError>> {
            Err(Box::new(crate::error::WorkflowError::StepHalted {
                step_index: 0,
                plugin: "test-plugin".into(),
                action: "fail".into(),
                cause: "simulated failure".into(),
            }))
        }
    }

    fn write_yaml(dir: &Path, filename: &str, content: &str) {
        std::fs::write(dir.join(filename), content).unwrap();
    }

    fn build_test_components(
        dir: &Path,
        yaml_files: &[(&str, &str)],
        mock_executor: Arc<dyn PluginExecutor>,
    ) -> (Arc<TriggerRegistry>, Arc<PipelineExecutor>) {
        for (name, content) in yaml_files {
            write_yaml(dir, name, content);
        }
        let config = WorkflowConfig {
            path: dir.to_str().unwrap().into(),
        };
        let workflows = load_workflows(&config).unwrap();
        let registry = Arc::new(TriggerRegistry::build(workflows).unwrap());
        let pipeline = Arc::new(PipelineExecutor::new(mock_executor));
        (registry, pipeline)
    }

    #[tokio::test]
    async fn scheduler_fires_workflow_on_schedule() {
        let dir = TempDir::new().unwrap();
        let mock = Arc::new(CountingExecutor::new());
        let (registry, pipeline) = build_test_components(
            dir.path(),
            &[(
                "scheduled.yaml",
                r#"
workflows:
  every-second:
    id: every-second
    name: Every Second
    trigger:
      schedule: "* * * * * *"
    steps:
      - plugin: test-plugin
        action: run
"#,
            )],
            mock.clone() as Arc<dyn PluginExecutor>,
        );

        let emitter: Arc<dyn WorkflowEventEmitter> = Arc::new(RecordingEmitter::new());
        let scheduler = Scheduler::start(&registry, pipeline, emitter).await;

        assert_eq!(scheduler.task_count(), 1);

        // Wait enough time for at least one execution.
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

        assert!(
            mock.count() >= 1,
            "expected at least 1 execution, got {}",
            mock.count()
        );

        scheduler.stop().await;
    }

    #[tokio::test]
    async fn scheduler_continues_after_workflow_failure() {
        let dir = TempDir::new().unwrap();
        let failing = Arc::new(FailingExecutor) as Arc<dyn PluginExecutor>;
        let (registry, pipeline) = build_test_components(
            dir.path(),
            &[(
                "fail.yaml",
                r#"
workflows:
  failing:
    id: failing
    name: Failing Workflow
    trigger:
      schedule: "* * * * * *"
    steps:
      - plugin: test-plugin
        action: fail
"#,
            )],
            failing,
        );

        let emitter = Arc::new(RecordingEmitter::new());
        let emitter_ref: Arc<dyn WorkflowEventEmitter> = emitter.clone();
        let scheduler = Scheduler::start(&registry, pipeline, emitter_ref).await;

        // Wait for a couple of firings.
        tokio::time::sleep(std::time::Duration::from_millis(2500)).await;

        scheduler.stop().await;

        // Should have emitted failure events.
        let event_names = emitter.event_names();
        assert!(
            event_names
                .iter()
                .any(|n| n == "workflow.schedule.failed"),
            "expected workflow.schedule.failed events, got: {:?}",
            event_names
        );
    }

    #[tokio::test]
    async fn scheduler_stop_cancels_all_tasks() {
        let dir = TempDir::new().unwrap();
        let mock = Arc::new(CountingExecutor::new());
        let (registry, pipeline) = build_test_components(
            dir.path(),
            &[(
                "stop.yaml",
                r#"
workflows:
  task-a:
    id: task-a
    name: Task A
    trigger:
      schedule: "* * * * * *"
    steps:
      - plugin: p1
        action: a1
  task-b:
    id: task-b
    name: Task B
    trigger:
      schedule: "* * * * * *"
    steps:
      - plugin: p2
        action: a2
"#,
            )],
            mock.clone() as Arc<dyn PluginExecutor>,
        );

        let emitter: Arc<dyn WorkflowEventEmitter> = Arc::new(RecordingEmitter::new());
        let scheduler = Scheduler::start(&registry, pipeline, emitter).await;
        assert_eq!(scheduler.task_count(), 2);

        scheduler.stop().await;

        // Record the count after stop.
        let count_at_stop = mock.count();

        // Wait to confirm no more executions happen.
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

        assert_eq!(
            mock.count(),
            count_at_stop,
            "expected no more executions after stop"
        );
    }

    #[tokio::test]
    async fn scheduler_with_no_schedules_starts_empty() {
        let dir = TempDir::new().unwrap();
        let mock = Arc::new(CountingExecutor::new());
        let (registry, pipeline) = build_test_components(
            dir.path(),
            &[(
                "no-schedule.yaml",
                r#"
workflows:
  endpoint-only:
    id: endpoint-only
    name: Endpoint Only
    trigger:
      endpoint: "GET /health"
    steps:
      - plugin: health
        action: check
"#,
            )],
            mock as Arc<dyn PluginExecutor>,
        );

        let emitter: Arc<dyn WorkflowEventEmitter> = Arc::new(RecordingEmitter::new());
        let scheduler = Scheduler::start(&registry, pipeline, emitter).await;
        assert_eq!(scheduler.task_count(), 0);
        scheduler.stop().await;
    }

    // --- Overlap prevention tests ---

    #[tokio::test]
    async fn tracker_try_start_returns_true_when_not_in_progress() {
        let tracker = ScheduleJobTracker::new();
        assert!(tracker.try_start("wf-1").await);
        assert!(tracker.is_in_progress("wf-1").await);
    }

    #[tokio::test]
    async fn tracker_try_start_returns_false_when_already_in_progress() {
        let tracker = ScheduleJobTracker::new();
        assert!(tracker.try_start("wf-1").await);
        assert!(!tracker.try_start("wf-1").await);
    }

    #[tokio::test]
    async fn tracker_finish_clears_in_progress() {
        let tracker = ScheduleJobTracker::new();
        tracker.try_start("wf-1").await;
        tracker.finish("wf-1").await;
        assert!(!tracker.is_in_progress("wf-1").await);
        // Should be able to start again.
        assert!(tracker.try_start("wf-1").await);
    }

    #[tokio::test]
    async fn scheduler_clears_tracker_after_execution() {
        let dir = TempDir::new().unwrap();
        let mock = Arc::new(CountingExecutor::new());
        let (registry, pipeline) = build_test_components(
            dir.path(),
            &[(
                "tracked.yaml",
                r#"
workflows:
  tracked-wf:
    id: tracked-wf
    name: Tracked Workflow
    trigger:
      schedule: "* * * * * *"
    steps:
      - plugin: test-plugin
        action: run
"#,
            )],
            mock.clone() as Arc<dyn PluginExecutor>,
        );

        let emitter: Arc<dyn WorkflowEventEmitter> = Arc::new(RecordingEmitter::new());
        let scheduler = Scheduler::start(&registry, pipeline, emitter).await;

        // Wait for at least one complete execution cycle.
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

        assert!(
            mock.count() >= 1,
            "expected at least 1 execution, got {}",
            mock.count()
        );

        // After a fast-completing workflow, the tracker should not show it as in-progress
        // (it gets cleared after each execution).
        let in_progress = scheduler.tracker().is_in_progress("tracked-wf").await;
        assert!(
            !in_progress,
            "expected tracker to clear after workflow completes"
        );

        scheduler.stop().await;
    }

    #[tokio::test]
    async fn scheduler_clears_tracker_after_failure() {
        let dir = TempDir::new().unwrap();
        let failing = Arc::new(FailingExecutor) as Arc<dyn PluginExecutor>;
        let (registry, pipeline) = build_test_components(
            dir.path(),
            &[(
                "fail-tracked.yaml",
                r#"
workflows:
  fail-tracked:
    id: fail-tracked
    name: Fail Tracked
    trigger:
      schedule: "* * * * * *"
    steps:
      - plugin: test-plugin
        action: fail
"#,
            )],
            failing,
        );

        let emitter = Arc::new(RecordingEmitter::new());
        let emitter_ref: Arc<dyn WorkflowEventEmitter> = emitter.clone();
        let scheduler = Scheduler::start(&registry, pipeline, emitter_ref).await;

        // Wait for at least two executions to confirm the tracker clears on failure
        // (if it didn't clear, the second tick would be skipped).
        tokio::time::sleep(std::time::Duration::from_millis(2500)).await;

        scheduler.stop().await;

        let failure_count = emitter
            .event_names()
            .iter()
            .filter(|n| *n == "workflow.schedule.failed")
            .count();

        assert!(
            failure_count >= 2,
            "expected at least 2 failure events (proving tracker clears after failure), got {}",
            failure_count
        );
    }
}
