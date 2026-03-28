<!--
domain: pipeline-message
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Pipeline Message Tasks

**Progress:** 0 / 12 tasks complete

## 1.1 — Core Type Definitions

- [ ] Define `PipelineMessage` and `PipelineMetadata` structs
  <!-- files: packages/types/src/pipeline_message.rs -->
  <!-- purpose: Define the PipelineMessage and PipelineMetadata structs with serde derives -->
  <!-- requirements: 1.1, 1.2, 2.1, 2.2 -->

- [ ] Define `IdentitySummary` struct
  <!-- files: packages/types/src/pipeline_message.rs -->
  <!-- purpose: Define the IdentitySummary struct with subject and issuer fields -->
  <!-- requirements: 3.1 -->

- [ ] Define `StepTrace` and `StepOutcome` types
  <!-- files: packages/types/src/pipeline_message.rs -->
  <!-- purpose: Define StepTrace struct and StepOutcome enum for execution tracing -->
  <!-- requirements: 6.2 -->

- [ ] Re-export pipeline message types from `packages/types`
  <!-- files: packages/types/src/lib.rs -->
  <!-- purpose: Add pub mod and pub use statements so pipeline message types are accessible from the types crate root -->
  <!-- requirements: 1.1, 2.1, 3.1 -->

## 1.2 — Write-Permission Enforcement

- [ ] Implement snapshot-and-restore for read-only fields in the executor
  <!-- files: packages/workflow-engine/src/executor.rs -->
  <!-- purpose: Snapshot executor-owned metadata fields before plugin invocation and restore them after, keeping only plugin-writable fields from the returned message -->
  <!-- requirements: 4.2, 4.3 -->

- [ ] Add unit tests for write-permission enforcement
  <!-- files: packages/workflow-engine/src/executor.rs, packages/workflow-engine/tests/pipeline_message_permissions.rs -->
  <!-- purpose: Verify that plugin modifications to read-only fields are discarded and writable fields are preserved -->
  <!-- requirements: 4.1, 4.2, 4.3 -->

## 1.3 — WASM Boundary Serialisation

- [ ] Implement JSON serialisation for host-to-plugin message passing
  <!-- files: packages/workflow-engine/src/executor.rs -->
  <!-- purpose: Serialise PipelineMessage to JSON before passing to Extism plugin and deserialise the returned JSON output -->
  <!-- requirements: 5.1, 5.2 -->

- [ ] Implement SDK-side deserialisation and serialisation helpers
  <!-- files: packages/plugin-sdk-rs/src/message.rs, packages/plugin-sdk-rs/src/lib.rs -->
  <!-- purpose: Provide helper functions in the Rust plugin SDK to deserialise input PipelineMessage and serialise output PipelineMessage across the WASM boundary -->
  <!-- requirements: 5.1, 5.2 -->

- [ ] Add tests for serialisation error handling
  <!-- files: packages/workflow-engine/tests/pipeline_message_serde.rs -->
  <!-- purpose: Verify that invalid JSON or mismatched shapes result in step failure with appropriate error messages -->
  <!-- requirements: 5.3 -->

## 1.4 — Executor Integration

- [ ] Implement initial message construction from `TriggerContext`
  <!-- files: packages/workflow-engine/src/executor.rs -->
  <!-- purpose: Build the initial PipelineMessage from TriggerContext, populating payload, request_id, trigger_type, identity, params, and query -->
  <!-- requirements: 6.1, 2.3, 3.2, 3.3 -->

- [ ] Implement `StepTrace` appending and response extraction
  <!-- files: packages/workflow-engine/src/executor.rs -->
  <!-- purpose: Append StepTrace after each step completes and extract final payload into WorkflowResponse, applying status_hint if set -->
  <!-- requirements: 6.2, 6.3, 6.4, 6.5 -->

- [ ] Add integration tests for warnings accumulation and extra metadata
  <!-- files: packages/workflow-engine/tests/pipeline_message_integration.rs -->
  <!-- purpose: Verify that warnings accumulate across steps, extra entries are visible to downstream steps, and later steps can overwrite extra keys -->
  <!-- requirements: 7.1, 7.2, 8.1, 8.2, 8.3 -->
