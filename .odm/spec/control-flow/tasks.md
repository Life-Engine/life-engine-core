<!--
domain: control-flow
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Tasks — Control Flow

**Progress:** 0 / 14 tasks complete

## 1.1 — Control Flow Types

- [ ] Define core control flow types
  <!-- files: crates/le_workflow_engine/src/control_flow/mod.rs, crates/le_workflow_engine/src/control_flow/types.rs -->
  <!-- purpose: Create WorkflowStep enum, PluginStep, ConditionBlock, ConditionOperator, ErrorStrategy, ErrorStrategyKind structs -->
  <!-- requirements: R1, R3, R6, R7, R8 -->

- [ ] Define error outcome and response types
  <!-- files: crates/le_workflow_engine/src/control_flow/types.rs -->
  <!-- purpose: Create ErrorOutcome enum, WorkflowResponse, Status, and StepError structs -->
  <!-- requirements: R6, R7, R8 -->

## 1.2 — Condition Evaluation

- [ ] Implement field resolution
  <!-- files: crates/le_workflow_engine/src/control_flow/condition.rs -->
  <!-- purpose: Implement resolve_field function that walks a dot-separated path into a serde_json::Value payload -->
  <!-- requirements: R3 -->

- [ ] Implement condition evaluation logic
  <!-- files: crates/le_workflow_engine/src/control_flow/condition.rs -->
  <!-- purpose: Implement evaluate_condition for equals, not_equals, exists, is_empty operators with safe-by-default missing field handling -->
  <!-- requirements: R3 -->

- [ ] Add unit tests for condition evaluation
  <!-- files: crates/le_workflow_engine/src/control_flow/condition.rs -->
  <!-- purpose: Test all four operators, missing field path (else branch), null values, empty strings, empty arrays -->
  <!-- requirements: R3 -->

## 1.3 — Error Handling

- [ ] Implement retry with exponential backoff
  <!-- files: crates/le_workflow_engine/src/control_flow/error_handling.rs -->
  <!-- purpose: Implement retry loop with retry_delay, pre-step clone replay, and fallback execution -->
  <!-- requirements: R7 -->

- [ ] Implement halt and skip strategies
  <!-- files: crates/le_workflow_engine/src/control_flow/error_handling.rs -->
  <!-- purpose: Implement handle_error dispatch for halt (immediate stop) and skip (warning append, pre-step passthrough) -->
  <!-- requirements: R2, R6, R8 -->

- [ ] Add unit tests for error handling strategies
  <!-- files: crates/le_workflow_engine/src/control_flow/error_handling.rs -->
  <!-- purpose: Test halt stops workflow, retry exhaustion with and without fallback, skip appends warning, fallback failure halts -->
  <!-- requirements: R6, R7, R8 -->

## 1.4 — Execution Loop

- [ ] Implement sequential step execution loop
  <!-- files: crates/le_workflow_engine/src/control_flow/executor.rs -->
  <!-- purpose: Implement execute_steps with pre-step cloning, plugin invocation, and error strategy dispatch -->
  <!-- requirements: R1, R2, R6 -->

- [ ] Implement condition block handling in execution loop
  <!-- files: crates/le_workflow_engine/src/control_flow/executor.rs -->
  <!-- purpose: Add condition block evaluation and recursive branch execution with error accumulation and halt propagation -->
  <!-- requirements: R3, R4, R5, R9 -->

- [ ] Add integration tests for execution loop
  <!-- files: crates/le_workflow_engine/src/control_flow/executor.rs -->
  <!-- purpose: Test sequential flow, branch rejoining, error handling in branches, halt propagation from branch, skipped step warning accumulation -->
  <!-- requirements: R1, R2, R3, R5, R8, R9 -->

## 1.5 — YAML Parsing and Validation

- [ ] Implement workflow YAML parser
  <!-- files: crates/le_workflow_engine/src/control_flow/parse.rs -->
  <!-- purpose: Parse workflow YAML into Vec<WorkflowStep>, enforcing single operator per condition and max_retries requirement for retry strategy -->
  <!-- requirements: R3, R4, R7 -->

- [ ] Implement nesting depth validation
  <!-- files: crates/le_workflow_engine/src/control_flow/parse.rs -->
  <!-- purpose: Reject nested condition blocks inside then/else branches with descriptive error including step index -->
  <!-- requirements: R4 -->

- [ ] Add unit tests for YAML parsing
  <!-- files: crates/le_workflow_engine/src/control_flow/parse.rs -->
  <!-- purpose: Test valid workflows parse correctly, nested conditions rejected, multiple operators rejected, missing max_retries rejected, clear error messages -->
  <!-- requirements: R3, R4, R7 -->
