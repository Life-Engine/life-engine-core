---
title: PipelineMessage
type: adr
created: 2026-03-28
status: draft
tags:
  - architecture
  - plugin-sdk
  - pipeline-message
  - core
---

# PipelineMessage

## Overview

`PipelineMessage` is the envelope that flows between workflow steps. Every plugin action receives one as input and returns one as output. It carries the data payload, accumulated metadata, and optional status hints. The message is the only interface between the workflow engine and a plugin — plugins never see `WorkflowRequest`, `WorkflowResponse`, or any transport-level types.

## Shape

```rust
pub struct PipelineMessage {
    pub payload: Value,
    pub metadata: MessageMetadata,
}
```

The SDK provides this as a serialisable struct. Across the WASM boundary, it is passed as JSON. The SDK handles serialisation and deserialisation — plugin authors work with typed structs.

## Payload

The `payload` field is the primary data carrier. It is a JSON `Value` — an arbitrary JSON object.

- The initial payload is built from the trigger context (request body for endpoints, event payload for events, empty for schedules)
- Each step can read, modify, or replace the payload entirely
- The final step's payload becomes `WorkflowResponse.data`

There is no enforced schema on the payload between steps. Plugins are responsible for producing output that downstream steps expect. The composability contract is by convention, not enforcement — if two plugins need to interoperate, they agree on a payload shape (typically via CDM schemas).

## Metadata

```rust
pub struct MessageMetadata {
    pub request_id: String,
    pub trigger_type: String,
    pub identity: Option<IdentitySummary>,
    pub params: HashMap<String, String>,
    pub query: HashMap<String, String>,
    pub traces: Vec<StepTrace>,
    pub status_hint: Option<StatusHint>,
    pub warnings: Vec<String>,
    pub extra: HashMap<String, Value>,
}
```

### Fields

- **request_id** — Unique identifier for this workflow execution. Set by the executor. Read-only for plugins.
- **trigger_type** — How the workflow was activated: `"endpoint"`, `"event"`, or `"schedule"`. Read-only for plugins.
- **identity** — Summary of the authenticated user (if the workflow was triggered by an endpoint). `None` for event and schedule triggers. Read-only for plugins.
- **params** — Path parameters from the endpoint route (e.g., `:collection`, `:id`). Empty for event and schedule triggers. Read-only for plugins.
- **query** — Query string parameters from the endpoint, or GraphQL arguments. Empty for event and schedule triggers. Read-only for plugins.
- **traces** — Accumulated `StepTrace` entries from previous steps. Appended by the executor after each step. Read-only for plugins.
- **status_hint** — A plugin can set this to influence the `WorkflowResponse.status`. See the Status Hints section below.
- **warnings** — Non-fatal messages a plugin wants to surface in the response. Appended, not replaced.
- **extra** — Free-form key-value metadata a plugin can pass to downstream steps. Namespaced by plugin ID to avoid collisions: `extra["connector-email.sync_count"] = 42`.

### Read-Only vs Writable

Plugins can modify:

- `payload` — The primary data
- `metadata.status_hint` — To influence the response status
- `metadata.warnings` — To surface non-fatal issues
- `metadata.extra` — To pass context to downstream steps

Plugins cannot modify (the executor ignores changes to these):

- `metadata.request_id`
- `metadata.trigger_type`
- `metadata.identity`
- `metadata.params`
- `metadata.query`
- `metadata.traces`

The SDK enforces this at the type level in Rust by exposing read-only accessors for immutable fields. In other languages, the constraint is documented but not enforced at compile time — the executor strips unauthorised changes after the plugin returns.

## IdentitySummary

```rust
pub struct IdentitySummary {
    pub subject: String,
    pub name: Option<String>,
    pub email: Option<String>,
}
```

A minimal view of the authenticated user, derived from the OIDC token. Plugins use this for display purposes or to tag records with the current user. It is not a security boundary — authorisation is handled at the transport layer.

## StepTrace

```rust
pub struct StepTrace {
    pub plugin_id: String,
    pub action: String,
    pub duration_ms: u64,
    pub status: StepStatus,
}

pub enum StepStatus {
    Completed,
    Skipped,
    Failed,
}
```

The executor appends a `StepTrace` after each step completes. The full trace list is included in the `WorkflowResponse` for debugging and observability.

## Status Hints

A plugin can set `metadata.status_hint` to influence the `WorkflowResponse.status`:

```rust
pub enum StatusHint {
    Ok,
    Created,
    NotFound,
    Invalid { message: String },
}
```

- **Ok** — Default. The workflow completed successfully.
- **Created** — A new resource was created. System CRUD workflows use this.
- **NotFound** — The requested resource does not exist. Useful for plugins that perform lookups.
- **Invalid** — The input failed validation. The `message` is included in `WorkflowResponse.errors`.

If no plugin sets a status hint, the executor defaults to `Ok`. If multiple steps set a hint, the last one wins.

Plugins cannot set `Denied` or `Error` — these are reserved for the transport layer (auth failure) and executor (unrecoverable step failure) respectively.

## WASM Boundary Serialisation

Across the WASM boundary, `PipelineMessage` is serialised as JSON:

1. The executor serialises the message to a JSON byte array
2. The byte array is passed to the plugin's WASM export via Extism's input mechanism
3. The plugin deserialises it using the SDK's `from_input()` helper
4. The plugin builds a response message and serialises it via the SDK's `to_output()` helper
5. The executor deserialises the returned byte array back into a `PipelineMessage`

The SDK abstracts this entirely. A Rust plugin action looks like:

```rust
use life_engine_sdk::prelude::*;

#[plugin_action]
pub fn fetch(msg: PipelineMessage) -> Result<PipelineMessage, PluginError> {
    let emails = do_fetch(&msg)?;

    Ok(msg.with_payload(json!({ "emails": emails })))
}
```

## Message Lifecycle

1. Executor builds the initial `PipelineMessage` from `TriggerContext`
2. Before each step, the executor clones the message (pre-step snapshot for error recovery)
3. The message is serialised and passed to the plugin action
4. The plugin returns a new or modified message
5. The executor appends a `StepTrace` to metadata
6. The output message becomes the input to the next step
7. After the final step, the executor builds `WorkflowResponse` from the message

## Empty Messages

Schedule-triggered workflows start with an empty payload:

```json
{
  "payload": {},
  "metadata": {
    "request_id": "...",
    "trigger_type": "schedule",
    "identity": null,
    "params": {},
    "query": {},
    "traces": [],
    "status_hint": null,
    "warnings": [],
    "extra": {}
  }
}
```

Plugins in scheduled workflows fetch their own data (e.g., polling an external API via `http:outbound`).
