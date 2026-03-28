<!--
domain: control-flow
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Design Document — Control Flow

## Introduction

This document describes the technical design for control flow within the pipeline executor. It covers sequential step execution, conditional branching, error handling strategies, and the `PipelineMessage` cloning lifecycle. All control flow logic lives in the `le_workflow_engine` crate.

## Step Representation

Each workflow step is represented as an enum that distinguishes plugin steps from condition blocks:

```rust
/// A single step in a workflow pipeline.
pub enum WorkflowStep {
    /// Invoke a plugin action.
    Plugin(PluginStep),
    /// Evaluate a condition and branch.
    Condition(ConditionBlock),
}

pub struct PluginStep {
    pub plugin: String,
    pub action: String,
    pub on_error: Option<ErrorStrategy>,
}
```

## Condition Block

The condition block is a data structure parsed from the YAML workflow definition:

```rust
pub struct ConditionBlock {
    pub field: String,
    pub operator: ConditionOperator,
    pub then_steps: Vec<PluginStep>,
    pub else_steps: Vec<PluginStep>,
}

pub enum ConditionOperator {
    Equals(serde_json::Value),
    NotEquals(serde_json::Value),
    Exists,
    IsEmpty,
}
```

Design notes:

- `field` is a dot-separated path (e.g. `payload.category`) resolved against the current `PipelineMessage` payload
- `then_steps` and `else_steps` contain only `PluginStep` values, enforcing the one-level nesting rule at the type level
- Each condition block uses exactly one operator, expressed as an enum variant

## Condition Evaluation

The executor evaluates conditions using a `resolve_field` function that walks the dot-separated path into the `PipelineMessage` payload:

```rust
fn resolve_field(payload: &serde_json::Value, path: &str) -> Option<serde_json::Value> {
    let mut current = payload;
    for segment in path.split('.') {
        current = current.get(segment)?;
    }
    Some(current.clone())
}
```

Evaluation logic per operator:

- `Equals(v)` — Resolves the field; takes `then` if the resolved value equals `v`, otherwise `else`
- `NotEquals(v)` — Resolves the field; takes `then` if the resolved value does not equal `v`, otherwise `else`
- `Exists` — Takes `then` if `resolve_field` returns `Some`, otherwise `else`
- `IsEmpty` — Takes `then` if the field is absent (`None`), `null`, empty string (`""`), or empty array (`[]`), otherwise `else`

When `resolve_field` returns `None` (field path does not exist), the `else` branch is taken for all operators. This is the safe-by-default rule.

## Error Handling Strategies

Error strategies are declared per-step and parsed into a struct:

```rust
pub struct ErrorStrategy {
    pub strategy: ErrorStrategyKind,
    pub max_retries: Option<u32>,
    pub fallback: Option<PluginStep>,
}

pub enum ErrorStrategyKind {
    Halt,
    Retry,
    Skip,
}
```

When no `on_error` block is present, the executor applies `ErrorStrategyKind::Halt` as the default.

## Retry Backoff

The retry strategy uses exponential backoff with a base delay:

```rust
/// Calculate the delay for attempt N (0-indexed).
fn retry_delay(attempt: u32) -> Duration {
    let base_ms = 100;
    let delay_ms = base_ms * 2u64.pow(attempt);
    Duration::from_millis(delay_ms.min(30_000)) // cap at 30 seconds
}
```

Retry sequence for `max_retries: 3`:

- Attempt 0: 100ms delay
- Attempt 1: 200ms delay
- Attempt 2: 400ms delay

After all attempts are exhausted, the executor checks for a `fallback` step. If present, it executes the fallback with the pre-step clone. If the fallback fails or no fallback is declared, the workflow halts.

## Execution Loop

The main execution loop processes a `Vec<WorkflowStep>` and maintains the current `PipelineMessage`:

```rust
pub async fn execute_steps(
    steps: &[WorkflowStep],
    mut message: PipelineMessage,
    ctx: &ExecutionContext,
) -> WorkflowResponse {
    let mut errors: Vec<StepError> = Vec::new();

    for step in steps {
        match step {
            WorkflowStep::Plugin(plugin_step) => {
                let snapshot = message.clone();
                match invoke_plugin(plugin_step, &message, ctx).await {
                    Ok(output) => message = output,
                    Err(err) => {
                        match handle_error(plugin_step, snapshot, err, ctx).await {
                            ErrorOutcome::Halted(err) => {
                                return WorkflowResponse::error(err);
                            }
                            ErrorOutcome::Skipped(warning) => {
                                message = snapshot;
                                errors.push(warning);
                            }
                            ErrorOutcome::Recovered(output) => {
                                message = output;
                            }
                        }
                    }
                }
            }
            WorkflowStep::Condition(block) => {
                let branch = evaluate_condition(block, &message);
                let branch_result = execute_steps(branch, message.clone(), ctx).await;
                match branch_result.status {
                    Status::Ok => {
                        message = branch_result.output;
                        errors.extend(branch_result.errors);
                    }
                    Status::Error => return branch_result,
                }
            }
        }
    }

    WorkflowResponse::ok(message, errors)
}
```

Key design decisions in the execution loop:

- The `snapshot` clone is taken before every plugin step invocation, providing the pre-step state for retry and skip
- Condition blocks recurse into `execute_steps` with the branch step list, maintaining the same error handling semantics inside branches
- Branch errors (warnings from skipped steps) are accumulated into the parent response
- A halt inside a branch propagates immediately to the top-level response

## Error Outcome

The `handle_error` function returns one of three outcomes:

```rust
enum ErrorOutcome {
    /// Workflow must stop.
    Halted(StepError),
    /// Step was skipped; error recorded as warning.
    Skipped(StepError),
    /// Step succeeded on retry or fallback.
    Recovered(PipelineMessage),
}
```

The `handle_error` function implements the strategy dispatch:

- `Halt` — Returns `ErrorOutcome::Halted` immediately
- `Retry` — Loops up to `max_retries`, applying `retry_delay` between attempts. On success, returns `ErrorOutcome::Recovered`. On exhaustion, tries the fallback if declared, then returns `Halted` or `Recovered`
- `Skip` — Returns `ErrorOutcome::Skipped` with the error converted to a warning

## Workflow Response

The `WorkflowResponse` captures the final state:

```rust
pub struct WorkflowResponse {
    pub status: Status,
    pub output: PipelineMessage,
    pub errors: Vec<StepError>,
}

pub enum Status {
    Ok,
    Error,
}

pub struct StepError {
    pub step_plugin: String,
    pub step_action: String,
    pub message: String,
    pub is_warning: bool,
}
```

When the workflow completes with skipped steps, `status` is `Ok` but `errors` contains entries with `is_warning: true`. Callers inspect this list to detect degraded execution.

## YAML Parsing and Validation

Workflow definitions are parsed from YAML. The parser enforces structural constraints before execution begins:

- Each `condition` block must declare exactly one operator (`equals`, `not_equals`, `exists`, or `is_empty`)
- `then` and `else` branches must contain only plugin steps (no nested condition blocks)
- `max_retries` is required when `strategy` is `retry`
- `fallback` is optional and only valid when `strategy` is `retry`

Parse errors include the step index and a descriptive message so the workflow author can locate the issue.

## File Locations

All control flow types and logic reside in the `le_workflow_engine` crate:

- `crates/le_workflow_engine/src/control_flow/mod.rs` — Module root, re-exports
- `crates/le_workflow_engine/src/control_flow/types.rs` — `WorkflowStep`, `ConditionBlock`, `ConditionOperator`, `ErrorStrategy` structs
- `crates/le_workflow_engine/src/control_flow/condition.rs` — `resolve_field`, `evaluate_condition` functions
- `crates/le_workflow_engine/src/control_flow/error_handling.rs` — `handle_error`, `retry_delay`, `ErrorOutcome`
- `crates/le_workflow_engine/src/control_flow/executor.rs` — `execute_steps` loop
- `crates/le_workflow_engine/src/control_flow/parse.rs` — YAML parsing and structural validation
