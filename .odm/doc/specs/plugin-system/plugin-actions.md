---
title: Plugin Actions Specification
type: reference
created: 2026-03-28
status: active
tags:
  - spec
  - plugin
  - actions
---

# Plugin Actions Specification

An action is a named entry point within a plugin that the [[workflow-engine-contract|workflow engine]] invokes as a pipeline step. Every action receives a [[pipeline-message|PipelineMessage]], performs work, and returns a modified `PipelineMessage`. Actions are declared in the plugin's [[plugin-manifest|manifest]] and compiled into the `.wasm` module.

## Action Signature

Each action is a function annotated with the `#[plugin_action]` attribute macro:

```rust
#[plugin_action]
fn fetch(msg: PipelineMessage, ctx: PluginContext) -> Result<PipelineMessage, PluginError> {
    // perform work, return modified message
}
```

The `#[plugin_action]` macro handles:

- Generating the Extism export boilerplate so the function is callable from the host
- Deserialising the incoming JSON bytes into a `PipelineMessage`
- Constructing the `PluginContext` with host function clients
- Serialising the returned `PipelineMessage` back to JSON
- Mapping `PluginError` into the Extism error protocol

Plugin authors write plain Rust functions. The macro handles all WASM boundary mechanics.

## PluginContext

The `PluginContext` struct provides typed access to all [[host-functions|host functions]]:

```rust
pub struct PluginContext {
    pub storage: StorageClient,
    pub events: EventClient,
    pub config: ConfigClient,
    pub http: HttpClient,
}
```

- **storage** — Document and blob storage operations. Methods correspond to `storage_doc_*` and `storage_blob_*` host functions.
- **events** — Event emission via `emit_event`.
- **config** — Plugin configuration access via `config_read`.
- **http** — Outbound HTTP requests via `http_request`.

Each client method returns `Result<T, PluginError>`. Calling a method that requires an ungrated capability returns `CapabilityDenied`.

## Lifecycle Hooks

Plugins may declare two optional lifecycle hooks in their manifest. These are not workflow steps and do not receive a `PipelineMessage`.

- **init** — Called once immediately after the WASM module is instantiated, during the Load phase of the [[plugin-system#Lifecycle|plugin lifecycle]]. Use for setup tasks: validating configuration, warming caches, or verifying external service connectivity.
- **shutdown** — Called once when Core is shutting down, before the module is unloaded. Use for cleanup: flushing buffers, closing connections, or writing final state.

Both hooks receive only a `PluginContext` and return `Result<(), PluginError>`:

```rust
#[plugin_hook]
fn init(ctx: PluginContext) -> Result<(), PluginError> {
    // setup logic
}

#[plugin_hook]
fn shutdown(ctx: PluginContext) -> Result<(), PluginError> {
    // cleanup logic
}
```

If `init` returns an error, the plugin fails to load and Core logs the failure.

## Timeouts

Each action may declare a `timeout_ms` value in the manifest:

```toml
[actions.fetch]
timeout_ms = 30000
```

The Extism host enforces this timeout at the WASM execution level. If an action exceeds its timeout:

- Extism terminates the WASM execution.
- The step is marked as failed.
- The [[pipeline-executor]] applies the workflow's `on_error` strategy (fail or skip).

If `timeout_ms` is omitted, Core applies a default timeout defined in the engine configuration.

## Error Handling

Actions report outcomes through two mechanisms:

- **Hard failure** — Return `Err(PluginError)`. The step fails immediately. The executor applies the workflow's `on_error` strategy. Common error types include `CapabilityDenied`, `NotFound`, `ValidationError`, `StorageError`, `NetworkError`, and `InternalError`.
- **Soft warning** — Return `Ok(msg)` with warnings appended to `msg.metadata.warnings`. The step succeeds, but the caller sees degradation signals. Use for non-fatal issues like partial sync or skipped records.

Plugins must not panic. The `#[plugin_action]` macro catches panics and converts them to `InternalError`.

## Connector Pattern

Connectors are plugins that synchronise data from external services. The recommended action implementation for a connector's `fetch` action follows this sequence:

1. Read configuration (server URL, credentials, sync interval) via `ctx.config`.
2. Fetch data from the external API via `ctx.http`.
3. Normalise the fetched data to [[cdm-specification|CDM]] schemas.
4. Write normalised documents to shared collections via `ctx.storage`.
5. Emit a completion event (e.g., `connector-email.fetch.completed`) via `ctx.events`.

This pattern ensures all connectors behave uniformly and integrate cleanly with the workflow engine and [[event-bus]].
