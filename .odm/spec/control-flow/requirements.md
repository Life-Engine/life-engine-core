<!--
domain: control-flow
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Requirements Document — Control Flow

## Introduction

Control flow governs how the pipeline executor evaluates step order, conditional branches, and error handling within a workflow. The workflow engine supports three control flow primitives in v1: sequential execution, conditional branching, and per-step error handling.

Sequential execution is the default mode. Conditional branching uses a `condition` block evaluated directly by the executor (no plugin involvement) to route execution to one of two flat step lists based on a field in the current `PipelineMessage` payload. Error handling is declared per-step with three strategies: `halt`, `retry`, and `skip`. The executor clones the `PipelineMessage` before each step to enable safe retries and skips.

## Alignment with Product Vision

- **Simplicity First** — Sequential execution is the default and only execution model in v1; parallel execution is deferred to keep the initial implementation focused
- **Parse, Don't Validate** — Condition evaluation uses dot-path field access into typed `PipelineMessage` payloads, avoiding stringly-typed logic in plugins
- **Defence in Depth** — Pre-step message cloning ensures retries and skips operate on a known-good snapshot, preventing partial mutation from propagating
- **The Pit of Success** — `halt` is the default error strategy so workflows fail safely unless the author explicitly opts into retry or skip
- **Transparency** — Skipped step errors are surfaced in `WorkflowResponse.errors` rather than silently discarded, giving callers visibility into degraded execution
- **Principle of Least Surprise** — Missing condition fields take the `else` branch rather than raising an error, making conditions safe by default

## Requirements

### Requirement 1 — Sequential Step Execution

**User Story:** As a workflow author, I want steps to execute in declaration order so that I can compose simple pipelines where each step transforms and passes data to the next.

#### Acceptance Criteria

- 1.1. WHEN a workflow defines multiple steps without condition blocks THEN the executor SHALL run each step in declaration order, passing the output `PipelineMessage` of step N as input to step N+1.
- 1.2. WHEN a step succeeds THEN the executor SHALL replace the current `PipelineMessage` with the step's output before proceeding to the next step.
- 1.3. WHEN the final step completes successfully THEN the executor SHALL return a `WorkflowResponse` with status `Ok` and the final `PipelineMessage` as the result.

### Requirement 2 — Pre-Step Message Cloning

**User Story:** As a Core developer, I want the executor to snapshot the message before each step so that retries and skips can replay from a known-good state.

#### Acceptance Criteria

- 2.1. WHEN the executor is about to invoke a step THEN it SHALL clone the current `PipelineMessage` to create a pre-step snapshot before calling the step.
- 2.2. WHEN a step with `on_error: retry` fails THEN the executor SHALL use the pre-step clone as input for each retry attempt.
- 2.3. WHEN a step with `on_error: skip` fails THEN the executor SHALL pass the pre-step clone as the current message to the next step.

### Requirement 3 — Conditional Branching

**User Story:** As a workflow author, I want to branch execution based on a field value in the pipeline message so that different data shapes are routed to the correct handler.

#### Acceptance Criteria

- 3.1. WHEN a workflow step is a `condition` block THEN the executor SHALL evaluate the condition directly without invoking any plugin.
- 3.2. WHEN a `condition` block declares `field` with an `equals` operator THEN the executor SHALL take the `then` branch if the field value matches exactly, otherwise the `else` branch.
- 3.3. WHEN a `condition` block declares `field` with a `not_equals` operator THEN the executor SHALL take the `then` branch if the field value does not match, otherwise the `else` branch.
- 3.4. WHEN a `condition` block declares `field` with an `exists` operator THEN the executor SHALL take the `then` branch if the field is present in the payload (including `null` values), otherwise the `else` branch.
- 3.5. WHEN a `condition` block declares `field` with an `is_empty` operator THEN the executor SHALL take the `then` branch if the field is absent, `null`, an empty string, or an empty array, otherwise the `else` branch.
- 3.6. WHEN the `field` path in a `condition` block does not exist in the payload THEN the executor SHALL take the `else` branch.
- 3.7. WHEN a `condition` block uses exactly one operator THEN the executor SHALL accept the block. WHEN a condition block declares more than one operator THEN the executor SHALL reject it during workflow parsing.

### Requirement 4 — Condition Nesting Limits

**User Story:** As a workflow author, I want clear nesting rules so that workflow definitions remain flat and predictable.

#### Acceptance Criteria

- 4.1. WHEN a `condition` block declares `then` and `else` branches THEN those branches SHALL contain only flat step lists (plugin steps or further condition blocks are not permitted within branches).
- 4.2. WHEN a workflow definition contains a nested condition block inside a `then` or `else` branch THEN the executor SHALL reject the workflow during parsing with a clear error message.

### Requirement 5 — Branch Rejoining

**User Story:** As a workflow author, I want execution to continue after a condition block so that post-branch steps receive the output from whichever branch was taken.

#### Acceptance Criteria

- 5.1. WHEN a conditional branch completes THEN the executor SHALL use the final step output from the taken branch as the current `PipelineMessage`.
- 5.2. WHEN the next step after a condition block executes THEN it SHALL receive the branch output as its input, maintaining the linear `PipelineMessage` passing model.

### Requirement 6 — Halt Error Strategy

**User Story:** As a workflow author, I want workflows to stop immediately on failure by default so that errors do not propagate silently.

#### Acceptance Criteria

- 6.1. WHEN a step fails and declares `on_error: { strategy: halt }` THEN the executor SHALL stop the workflow immediately and return a `WorkflowResponse` with status `Error` containing the step's error detail.
- 6.2. WHEN a step fails and does not declare an `on_error` block THEN the executor SHALL apply the `halt` strategy as the default.

### Requirement 7 — Retry Error Strategy

**User Story:** As a workflow author, I want to retry transient failures with exponential backoff so that temporary issues are recovered automatically.

#### Acceptance Criteria

- 7.1. WHEN a step fails and declares `on_error: { strategy: retry, max_retries: N }` THEN the executor SHALL retry the step up to N times, replaying the pre-step `PipelineMessage` clone as input on each attempt.
- 7.2. WHEN the executor retries a step THEN it SHALL apply exponential backoff between attempts.
- 7.3. WHEN retries are exhausted and a `fallback` step is declared THEN the executor SHALL execute the fallback step with the pre-step `PipelineMessage` clone as input.
- 7.4. WHEN retries are exhausted and no `fallback` step is declared THEN the executor SHALL halt the workflow.
- 7.5. WHEN the fallback step itself fails THEN the executor SHALL halt the workflow. There is no fallback-of-fallback.

### Requirement 8 — Skip Error Strategy

**User Story:** As a workflow author, I want to skip non-critical steps on failure so that the overall workflow completes in a degraded state rather than halting.

#### Acceptance Criteria

- 8.1. WHEN a step fails and declares `on_error: { strategy: skip }` THEN the executor SHALL log the error, skip the step, and pass the pre-step `PipelineMessage` clone to the next step.
- 8.2. WHEN a step is skipped THEN the executor SHALL append the error to `WorkflowResponse.errors` as a non-fatal warning.
- 8.3. WHEN a workflow completes with skipped steps THEN the `WorkflowResponse` status SHALL remain `Ok`, and the caller can inspect `errors` to detect degradation.

### Requirement 9 — Error Handling in Conditional Branches

**User Story:** As a workflow author, I want steps inside condition branches to declare their own error strategies so that error handling is consistent regardless of where a step appears.

#### Acceptance Criteria

- 9.1. WHEN a step inside a `then` or `else` branch declares `on_error` THEN the executor SHALL apply that strategy to the step independently of the parent workflow.
- 9.2. WHEN a step inside a branch fails with `halt` strategy THEN the executor SHALL halt the entire workflow, not just the branch.
- 9.3. WHEN a condition block itself is evaluated THEN no error strategy applies because condition evaluation cannot fail (missing fields take the `else` branch per Requirement 3.6).
