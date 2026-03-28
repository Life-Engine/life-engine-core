<!--
domain: plugin-actions
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Plugin Actions Design

## Overview

This document describes the technical design for plugin actions, lifecycle hooks, timeout enforcement, error handling, and the connector pattern. All types live in `packages/types` and trait definitions in `packages/traits`. The proc-macro crate `packages/plugin-sdk-macros` provides `#[plugin_action]` and `#[plugin_hook]`.

## Action Signature

Each action is a function annotated with `#[plugin_action]`. The macro expands the function into an Extism-compatible export.

```rust
#[plugin_action]
fn fetch(msg: PipelineMessage, ctx: PluginContext) -> Result<PipelineMessage, PluginError> {
    // perform work, return modified message
}
```

The macro expansion performs these steps in order:

- Reads raw bytes from the Extism input
- Deserialises JSON bytes into `PipelineMessage`
- Constructs `PluginContext` by creating typed clients that wrap host function calls
- Calls the user function inside `std::panic::catch_unwind`
- On `Ok`, serialises the returned `PipelineMessage` to JSON and writes to Extism output
- On `Err`, maps `PluginError` to the Extism error protocol
- On panic, returns `PluginError::InternalError` with the panic message

## PluginContext

`PluginContext` is defined in `packages/types` and constructed by the macro at each invocation.

```rust
pub struct PluginContext {
    pub storage: StorageClient,
    pub events: EventClient,
    pub config: ConfigClient,
    pub http: HttpClient,
}
```

Each client wraps calls to the corresponding host functions:

- **StorageClient** — wraps `storage_doc_get`, `storage_doc_put`, `storage_doc_delete`, `storage_doc_query`, `storage_blob_store`, `storage_blob_retrieve`, `storage_blob_delete`
- **EventClient** — wraps `emit_event`
- **ConfigClient** — wraps `config_read`
- **HttpClient** — wraps `http_request`

All client methods return `Result<T, PluginError>`. If the plugin lacks the required capability for a host function, the host returns an error code that the client maps to `PluginError::CapabilityDenied`.

## PluginError

`PluginError` is an enum defined in `packages/types`:

```rust
pub enum PluginError {
    CapabilityDenied(String),
    NotFound(String),
    ValidationError(String),
    StorageError(String),
    NetworkError(String),
    InternalError(String),
}
```

Each variant carries a human-readable message. The `#[plugin_action]` macro serialises the error as JSON with `kind` and `message` fields for the host to parse:

```json
{
  "kind": "CapabilityDenied",
  "message": "Plugin lacks 'storage:write' capability"
}
```

## Lifecycle Hooks

Lifecycle hooks use the `#[plugin_hook]` macro, which generates a simpler Extism export that receives no `PipelineMessage`.

```rust
#[plugin_hook]
fn init(ctx: PluginContext) -> Result<(), PluginError> {
    let config = ctx.config.read()?;
    if config.get("api_url").is_none() {
        return Err(PluginError::ValidationError("api_url is required".into()));
    }
    Ok(())
}

#[plugin_hook]
fn shutdown(ctx: PluginContext) -> Result<(), PluginError> {
    // flush buffers, close connections
    Ok(())
}
```

Hook invocation rules:

- `init` is called during the Load phase, after WASM instantiation and before any action invocations
- `shutdown` is called during Core shutdown, before the WASM module is unloaded
- Neither hook receives a `PipelineMessage`
- If `init` returns `Err`, the plugin fails to load and Core logs the error with the plugin ID and error details
- If a plugin omits either hook from its manifest, Core skips the call

## Manifest Declaration

Actions and hooks are declared in the plugin manifest (`plugin.toml`):

```toml
[plugin]
id = "connector-email"
version = "0.1.0"

[lifecycle]
init = true
shutdown = true

[actions.fetch]
timeout_ms = 30000

[actions.process]
timeout_ms = 10000
```

- Each `[actions.<name>]` section declares an action whose export name matches the key
- `timeout_ms` is optional; when omitted, the engine default applies
- `[lifecycle]` flags are optional booleans; when `true`, Core calls the corresponding hook export

## Timeout Enforcement

Timeouts are enforced at the Extism host level:

- When creating the Extism plugin instance, Core reads `timeout_ms` from the manifest for each action
- If `timeout_ms` is absent, Core uses the engine configuration default (e.g., `engine.default_action_timeout_ms`)
- The timeout is set on the Extism `Plugin` call configuration
- If execution exceeds the timeout, Extism cancels the WASM execution and returns an error
- The pipeline executor receives the timeout error, marks the step as failed, and applies the workflow's `on_error` strategy (`fail` or `skip`)

## Soft Warnings

Actions signal non-fatal issues by appending to `msg.metadata.warnings`:

```rust
#[plugin_action]
fn fetch(mut msg: PipelineMessage, ctx: PluginContext) -> Result<PipelineMessage, PluginError> {
    // partial sync — some records skipped
    msg.metadata.warnings.push("3 records skipped due to invalid format".into());
    Ok(msg)
}
```

Warnings do not affect step success. The pipeline executor logs them and makes them available to subsequent steps and the final workflow result.

## Connector Pattern

Connectors follow a five-step sequence inside their `fetch` action:

```rust
#[plugin_action]
fn fetch(msg: PipelineMessage, ctx: PluginContext) -> Result<PipelineMessage, PluginError> {
    // 1. Read configuration
    let config = ctx.config.read()?;
    let api_url = config.get("api_url")
        .ok_or(PluginError::ValidationError("api_url required".into()))?;

    // 2. Fetch from external API
    let response = ctx.http.get(api_url)?;

    // 3. Normalise to CDM
    let contacts: Vec<CdmContact> = normalise_contacts(&response.body)?;

    // 4. Write to shared collections
    for contact in &contacts {
        ctx.storage.doc_put("contacts", contact)?;
    }

    // 5. Emit completion event
    ctx.events.emit("connector-email.fetch.completed", &json!({
        "count": contacts.len()
    }))?;

    Ok(msg)
}
```

This pattern ensures all connectors behave uniformly: configuration-driven, CDM-normalised, storage-backed, and event-emitting.

## Crate Layout

The implementation spans three crates:

- **packages/types** — `PluginContext`, `PluginError`, client structs (`StorageClient`, `EventClient`, `ConfigClient`, `HttpClient`)
- **packages/plugin-sdk-macros** — proc-macro crate providing `#[plugin_action]` and `#[plugin_hook]`
- **packages/core** — host-side timeout enforcement, lifecycle hook invocation, manifest parsing for actions
