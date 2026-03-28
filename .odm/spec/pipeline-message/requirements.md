<!--
domain: pipeline-message
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Pipeline Message Requirements

## Requirement 1 — PipelineMessage Struct Definition

**User Story:** As a plugin author, I want a well-defined message struct so that I can read inputs and write outputs without knowing which step runs before or after me.

#### Acceptance Criteria

- 1.1. WHEN a workflow step is invoked THEN it SHALL receive a `PipelineMessage` containing a `payload` field of type `serde_json::Value` and a `metadata` field of type `PipelineMetadata`.
- 1.2. WHEN the `PipelineMessage` struct is defined THEN it SHALL derive `Serialize`, `Deserialize`, `Debug`, and `Clone`.
- 1.3. WHEN a step completes execution THEN it SHALL return a `PipelineMessage` with the same structural shape.

## Requirement 2 — PipelineMetadata Struct Definition

**User Story:** As a workflow engine developer, I want structured metadata on every message so that the executor can track context, identity, and execution history across steps.

#### Acceptance Criteria

- 2.1. WHEN `PipelineMetadata` is defined THEN it SHALL contain `request_id` (String), `trigger_type` (String), `identity` (Option of IdentitySummary), `params` (HashMap of String to String), `query` (HashMap of String to String), `traces` (Vec of StepTrace), `status_hint` (Option of WorkflowStatus), `warnings` (Vec of String), and `extra` (HashMap of String to Value).
- 2.2. WHEN `PipelineMetadata` is defined THEN it SHALL derive `Serialize`, `Deserialize`, `Debug`, and `Clone`.
- 2.3. WHEN the executor creates the initial metadata THEN `request_id` SHALL be a unique identifier and `trigger_type` SHALL be one of `"endpoint"`, `"event"`, or `"schedule"`.

## Requirement 3 — IdentitySummary Struct Definition

**User Story:** As a plugin author, I want access to the authenticated caller's identity so that I can make authorisation decisions within my action.

#### Acceptance Criteria

- 3.1. WHEN `IdentitySummary` is defined THEN it SHALL contain `subject` (String) and `issuer` (String).
- 3.2. WHEN the triggering request carried a valid authentication token THEN `identity` SHALL be `Some(IdentitySummary)` with the token's subject and issuer.
- 3.3. WHEN the trigger is unauthenticated (schedules, internal events) THEN `identity` SHALL be `None`.

## Requirement 4 — Plugin Write Permissions

**User Story:** As a workflow engine developer, I want read-only enforcement on executor-owned fields so that plugins cannot tamper with request identity, trace history, or routing parameters.

#### Acceptance Criteria

- 4.1. WHEN a plugin modifies `payload`, `status_hint`, `warnings`, or `extra` THEN those modifications SHALL be preserved in the message passed to the next step.
- 4.2. WHEN a plugin modifies `request_id`, `trigger_type`, `identity`, `params`, `query`, or `traces` THEN the executor SHALL discard those modifications and restore the executor's authoritative values.
- 4.3. WHEN the SDK deserialises a returned message from a plugin THEN the SDK SHALL merge only the writable fields (`payload`, `status_hint`, `warnings`, `extra`) into the executor's copy of the message.

## Requirement 5 — WASM Boundary Serialisation

**User Story:** As a plugin author, I want the SDK to handle serialisation transparently so that I work with native language types, not raw JSON strings.

#### Acceptance Criteria

- 5.1. WHEN a `PipelineMessage` crosses the WASM boundary into a plugin THEN it SHALL be serialised as JSON by the host and deserialised into native types by the SDK.
- 5.2. WHEN a plugin returns a `PipelineMessage` THEN the SDK SHALL serialise it as JSON and the host SHALL deserialise it back into the Rust struct.
- 5.3. WHEN serialisation or deserialisation fails THEN the step SHALL return an error and the executor SHALL treat it as a step failure.

## Requirement 6 — Message Lifecycle and Executor Integration

**User Story:** As a workflow engine developer, I want a defined message lifecycle so that the executor can build, pass, trace, and extract results from pipeline messages consistently.

#### Acceptance Criteria

- 6.1. WHEN a workflow is triggered THEN the executor SHALL build the initial `PipelineMessage` from the `TriggerContext`, populating `payload` from the request body, `params` from route parameters, `query` from query string parameters, and `identity` from the authentication token.
- 6.2. WHEN a step completes THEN the executor SHALL append a `StepTrace` to `metadata.traces` recording the step name, duration, and outcome.
- 6.3. WHEN the final step completes THEN the executor SHALL use the final message's `payload` as the `data` field of the `WorkflowResponse`.
- 6.4. WHEN any step sets `status_hint` THEN the executor SHALL use it as the HTTP response status code.
- 6.5. WHEN no step sets `status_hint` THEN the executor SHALL apply default status codes based on the workflow outcome.

## Requirement 7 — Warnings Accumulation

**User Story:** As a plugin author, I want to append warnings so that I can surface non-fatal issues without aborting the pipeline.

#### Acceptance Criteria

- 7.1. WHEN a plugin appends entries to `warnings` THEN those entries SHALL be preserved and accumulated across all steps.
- 7.2. WHEN the workflow completes THEN the accumulated `warnings` SHALL be available in the final `PipelineMessage` for inclusion in the response.

## Requirement 8 — Extra Metadata for Cross-Step Communication

**User Story:** As a plugin author, I want to store arbitrary metadata in `extra` so that downstream steps can read context I provide.

#### Acceptance Criteria

- 8.1. WHEN a plugin writes a key-value pair to `extra` THEN that entry SHALL be available to all subsequent steps in the pipeline.
- 8.2. WHEN multiple steps write to `extra` THEN entries from later steps SHALL overwrite entries with the same key from earlier steps.
- 8.3. WHEN a plugin reads from `extra` THEN it SHALL see all entries written by previous steps in the current pipeline execution.
