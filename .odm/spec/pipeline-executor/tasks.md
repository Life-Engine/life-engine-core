<!--
domain: pipeline-executor
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Implementation Tasks — Pipeline Executor

**Progress:** 0 / 16 tasks complete

## 1.1 — Workflow Types

- [ ] Define `TriggerContext` enum
  <!-- files: packages/types/src/workflow/trigger.rs -->
  <!-- purpose: Add TriggerContext enum with Endpoint, Event, and Schedule variants -->
  <!-- requirements: 2.1, 2.2, 2.3 -->

- [ ] Define `StepTrace` and `StepStatus`
  <!-- files: packages/types/src/workflow/trace.rs -->
  <!-- purpose: Add StepTrace struct and StepStatus enum (Completed, Skipped, Failed) -->
  <!-- requirements: 5.1, 5.2, 5.3 -->

- [ ] Define `WorkflowResponse` and `ResponseMeta`
  <!-- files: packages/types/src/workflow/response.rs -->
  <!-- purpose: Add WorkflowResponse struct with status, data, errors, and meta fields; add ResponseMeta with request_id, duration_ms, and traces -->
  <!-- requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6 -->

- [ ] Define `JobId`, `JobEntry`, and `JobStatus`
  <!-- files: packages/types/src/workflow/job.rs -->
  <!-- purpose: Add JobId type alias, JobEntry struct, and JobStatus enum for async job tracking -->
  <!-- requirements: 7.1, 8.1, 8.2 -->

## 1.2 — Workflow Error Types

- [ ] Define `WorkflowLoadError` and `ExecutionError`
  <!-- files: packages/types/src/workflow/error.rs -->
  <!-- purpose: Add error enums for workflow loading (DuplicateId, ParseError, IoError) and execution (WorkflowNotFound, ConcurrencyLimitExceeded, PluginActionFailed) -->
  <!-- requirements: 3.3, 3.4, 10.1 -->

## 2.1 — TriggerContext Conversion

- [ ] Implement `From<TriggerContext> for PipelineMessage`
  <!-- files: packages/types/src/workflow/trigger.rs -->
  <!-- purpose: Map each TriggerContext variant to a PipelineMessage with correct payload and metadata -->
  <!-- requirements: 2.1, 2.2, 2.3 -->

- [ ] Add unit tests for TriggerContext conversion
  <!-- files: packages/types/src/workflow/trigger.rs -->
  <!-- purpose: Test Endpoint, Event, and Schedule variants produce correct PipelineMessage payloads and metadata -->
  <!-- requirements: 2.1, 2.2, 2.3 -->

## 3.1 — Workflow Loader

- [ ] Implement `load_workflows` function
  <!-- files: packages/workflow-engine/src/loader.rs -->
  <!-- purpose: Scan YAML directory, deserialize WorkflowDefinitions, build HashMap, reject duplicates -->
  <!-- requirements: 3.1, 3.2, 3.3 -->

- [ ] Add unit tests for workflow loader
  <!-- files: packages/workflow-engine/src/loader.rs, packages/workflow-engine/tests/fixtures/ -->
  <!-- purpose: Test successful loading, duplicate ID rejection, and invalid YAML handling -->
  <!-- requirements: 3.1, 3.3 -->

## 4.1 — JobRegistry

- [ ] Implement `JobRegistry`
  <!-- files: packages/workflow-engine/src/registry.rs -->
  <!-- purpose: Build in-memory registry with register, complete, fail, get, and evict_expired operations using RwLock<HashMap> -->
  <!-- requirements: 7.1, 7.2, 7.3, 7.5, 8.1, 8.2, 8.3 -->

- [ ] Add TTL eviction background task
  <!-- files: packages/workflow-engine/src/registry.rs -->
  <!-- purpose: Spawn a periodic Tokio task that calls evict_expired to remove entries older than the configured TTL -->
  <!-- requirements: 7.5 -->

- [ ] Add unit tests for JobRegistry
  <!-- files: packages/workflow-engine/src/registry.rs -->
  <!-- purpose: Test register, complete, fail, get, and eviction lifecycle -->
  <!-- requirements: 7.1, 7.2, 7.3, 7.5, 8.3 -->

## 5.1 — WorkflowExecutor Core

- [ ] Implement `WorkflowExecutor` with `execute()` and `spawn()`
  <!-- files: packages/workflow-engine/src/executor.rs -->
  <!-- purpose: Build the executor struct with semaphore, registry, and workflow map; implement sync execute and async spawn entry points -->
  <!-- requirements: 1.1, 1.2, 1.3, 1.4, 9.1, 9.2, 9.3 -->

- [ ] Implement the step execution loop
  <!-- files: packages/workflow-engine/src/executor.rs -->
  <!-- purpose: Iterate steps sequentially, clone pre-step snapshot, call plugin action, record StepTrace, apply on_error strategy -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4, 5.1, 5.2, 5.3 -->

- [ ] Implement `WorkflowResponse` construction in the executor
  <!-- files: packages/workflow-engine/src/executor.rs -->
  <!-- purpose: Build final response from pipeline state, status_hint, warnings, and traces -->
  <!-- requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6 -->

## 6.1 — Deferred Feature Guards

- [ ] Add deferred-feature validation to workflow loader
  <!-- files: packages/workflow-engine/src/loader.rs -->
  <!-- purpose: Reject workflow definitions that request parallel step execution; log warning for workflow-level timeouts -->
  <!-- requirements: 10.1, 10.2 -->
