<!--
domain: pipeline-message
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Pipeline Message Design

## Overview

The `PipelineMessage` is the universal data envelope for the Life Engine workflow engine. It flows through every step in a pipeline, carrying both the working data (`payload`) and contextual metadata. This document describes the data structures, write-permission enforcement strategy, WASM serialisation approach, and executor integration points.

## Data Structures

All types are defined in `packages/types/src/` as the authoritative source.

### PipelineMessage

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineMessage {
    pub payload: Value,
    pub metadata: PipelineMetadata,
}
```

- `payload` — The step's primary data. Steps read, modify, or replace this entirely.
- `metadata` — Contextual information about the request, identity, and execution trace.

### PipelineMetadata

```rust
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

Field categories by write permission:

- **Executor-owned (read-only to plugins)** — `request_id`, `trigger_type`, `identity`, `params`, `query`, `traces`
- **Plugin-writable** — `payload`, `status_hint`, `warnings`, `extra`

### IdentitySummary

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentitySummary {
    pub subject: String,
    pub issuer: String,
}
```

- `subject` — The authenticated user's identifier (user ID or email).
- `issuer` — The identity provider that issued the token (`"local"` or an OIDC issuer URL).

Present only when the triggering request carried a valid authentication token. `None` for unauthenticated triggers such as schedules or internal events.

### StepTrace

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepTrace {
    pub step_name: String,
    pub duration_ms: u64,
    pub outcome: StepOutcome,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepOutcome {
    Success,
    Error(String),
}
```

Appended by the executor after each step completes. Plugins must not modify the `traces` vec.

## Write-Permission Enforcement

The executor enforces write permissions using a snapshot-and-restore strategy:

1. Before invoking a plugin step, the executor snapshots the current values of all read-only fields (`request_id`, `trigger_type`, `identity`, `params`, `query`, `traces`).
2. The full `PipelineMessage` is serialised and passed to the plugin.
3. The plugin returns a modified `PipelineMessage`.
4. The executor deserialises the returned message and restores all read-only fields from the snapshot.
5. Only `payload`, `status_hint`, `warnings`, and `extra` from the plugin's returned message are kept.

This approach means the plugin SDK does not need to strip fields or enforce permissions on the plugin side. The executor is the single point of enforcement.

```rust
fn apply_plugin_result(
    snapshot: &PipelineMetadata,
    returned: PipelineMessage,
) -> PipelineMessage {
    PipelineMessage {
        payload: returned.payload,
        metadata: PipelineMetadata {
            // Restore executor-owned fields
            request_id: snapshot.request_id.clone(),
            trigger_type: snapshot.trigger_type.clone(),
            identity: snapshot.identity.clone(),
            params: snapshot.params.clone(),
            query: snapshot.query.clone(),
            traces: snapshot.traces.clone(),
            // Keep plugin-writable fields
            status_hint: returned.metadata.status_hint,
            warnings: returned.metadata.warnings,
            extra: returned.metadata.extra,
        },
    }
}
```

## WASM Boundary Serialisation

The `PipelineMessage` is serialised as JSON when crossing the WASM boundary. The flow is:

1. **Host to plugin** — The executor serialises `PipelineMessage` to a JSON byte array using `serde_json::to_vec`. The Extism host passes this as the plugin input.
2. **Plugin entry** — The plugin SDK deserialises the JSON input into the language-native `PipelineMessage` type (Rust struct or equivalent in other SDK languages).
3. **Plugin exit** — The plugin SDK serialises the modified `PipelineMessage` back to JSON and writes it as the Extism output.
4. **Host receives** — The executor deserialises the JSON output back into the Rust `PipelineMessage` struct and applies write-permission enforcement.

Error handling:

- If serialisation fails on the host side, the step is marked as failed with an `StepOutcome::Error` describing the serialisation issue.
- If the plugin returns invalid JSON or a JSON shape that does not match `PipelineMessage`, the executor treats the step as failed.

## Message Lifecycle

1. The pipeline executor receives a `TriggerContext` from the transport layer (HTTP endpoint, event bus, or scheduler).
2. The executor builds the initial `PipelineMessage`:
   - `payload` is populated from the request body (or `Value::Null` if no body).
   - `metadata.request_id` is generated (UUID v4).
   - `metadata.trigger_type` is set based on the trigger source.
   - `metadata.identity` is set from the authentication token, or `None`.
   - `metadata.params` is populated from route path parameters.
   - `metadata.query` is populated from query string parameters.
   - `metadata.traces` starts as an empty vec.
   - `metadata.status_hint` starts as `None`.
   - `metadata.warnings` starts as an empty vec.
   - `metadata.extra` starts as an empty map.
3. Each workflow step receives the current message, executes, and returns a modified message.
4. After each step, the executor appends a `StepTrace` to `metadata.traces`.
5. The final message's `payload` becomes the `data` field of the `WorkflowResponse`.
6. If `status_hint` is set, it becomes the HTTP response status code. Otherwise, default codes apply.

## File Locations

- `packages/types/src/pipeline_message.rs` — `PipelineMessage`, `PipelineMetadata`, `IdentitySummary`, `StepTrace`, `StepOutcome` structs
- `packages/types/src/lib.rs` — Re-exports for the pipeline message types
- `packages/workflow-engine/src/executor.rs` — Message construction from `TriggerContext`, write-permission enforcement, trace appending
- `packages/plugin-sdk-rs/src/` — SDK-side deserialisation/serialisation helpers for the WASM boundary

## Conventions

- All struct fields use `snake_case`.
- All structs derive `Debug`, `Clone`, `Serialize`, `Deserialize`.
- `trigger_type` is a String (not an enum) to allow future trigger sources without breaking the schema. Validated values are `"endpoint"`, `"event"`, and `"schedule"`.
- `WorkflowStatus` is assumed to be defined elsewhere in `packages/types` (likely wrapping HTTP status codes).
- `StepTrace` is append-only; the executor never modifies existing trace entries.
