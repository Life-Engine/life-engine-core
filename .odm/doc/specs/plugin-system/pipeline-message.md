---
title: Pipeline Message Specification
type: reference
created: 2026-03-28
status: active
tags:
  - spec
  - pipeline
  - message
---

# Pipeline Message Specification

The `PipelineMessage` is the universal data envelope passed between workflow steps. Every [[plugin-actions|plugin action]] receives a `PipelineMessage` as input and returns a modified `PipelineMessage` as output. This uniform shape makes all actions composable within the [[workflow-engine-contract|workflow engine]].

## Message Shape

```rust
pub struct PipelineMessage {
    pub payload: Value,
    pub metadata: PipelineMetadata,
}
```

- **payload** — The step's primary data. A `serde_json::Value` that steps read, modify, or replace entirely.
- **metadata** — Contextual information about the request, identity, and execution trace.

## Metadata

```rust
pub struct PipelineMetadata {
    pub request_id: String,
    pub trigger_type: String,
    pub identity: Option<IdentitySummary>,
    pub params: HashMap<String, String>,
    pub query: HashMap<String, String>,
    pub traces: Vec<StepTrace>,
    pub status_hint: Option<WorkflowStatus>,
    pub warnings: Vec<String>,
    pub extra: HashMap<String, Value>,
}
```

- **request_id** — Unique identifier for this pipeline execution. Set by the executor at creation. Must not be modified by plugins.
- **trigger_type** — How the workflow was triggered. One of `"endpoint"`, `"event"`, or `"schedule"`. Set by the executor. Must not be modified by plugins.
- **identity** — The authenticated caller, if present. See IdentitySummary below. Must not be modified by plugins.
- **params** — Path parameters extracted from the matched route (e.g., `{id}` in `/tasks/{id}`). Must not be modified by plugins.
- **query** — Query string parameters from the originating HTTP request. Must not be modified by plugins.
- **traces** — Ordered list of `StepTrace` entries appended by the executor after each step completes. Must not be modified by plugins.
- **status_hint** — Optional hint from a plugin indicating the desired HTTP response status. Plugins may set this.
- **warnings** — List of non-fatal warning messages. Plugins may append to this.
- **extra** — Arbitrary key-value metadata. Plugins may read and write entries here.

## IdentitySummary

```rust
pub struct IdentitySummary {
    pub subject: String,
    pub issuer: String,
}
```

- **subject** — The authenticated user's identifier (e.g., a user ID or email).
- **issuer** — The identity provider that issued the token (e.g., `"local"` or an OIDC issuer URL).

Present only when the triggering request carried a valid authentication token. `None` for unauthenticated triggers (schedules, internal events).

## Plugin Write Permissions

Plugins have restricted write access to the message:

- **Writable** — `payload`, `status_hint`, `warnings`, `extra`
- **Read-only** — `request_id`, `trigger_type`, `identity`, `params`, `query`, `traces`

The SDK enforces these restrictions. Attempting to modify read-only fields has no effect; the executor's copy of those fields is authoritative.

## WASM Boundary

The `PipelineMessage` is serialised as JSON when crossing the WASM boundary. The plugin SDK handles deserialisation on entry and serialisation on exit. Plugins work with native language types, not raw JSON strings.

## Lifecycle

1. The [[pipeline-executor]] builds the initial `PipelineMessage` from the `TriggerContext` (route params, query params, request body, identity).
2. Each workflow step receives the current message, performs its work, and returns a modified message.
3. After each step completes, the executor appends a `StepTrace` to `metadata.traces` recording the step name, duration, and outcome.
4. The final message's `payload` becomes the `data` field of the `WorkflowResponse` returned to the caller.
5. If any step sets `status_hint`, the executor uses it as the HTTP response status code. Otherwise, the executor applies default status codes based on the workflow outcome.
