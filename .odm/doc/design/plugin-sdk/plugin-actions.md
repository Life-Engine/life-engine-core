---
title: Plugin Actions
type: adr
created: 2026-03-28
status: draft
tags:
  - architecture
  - plugin-sdk
  - actions
  - lifecycle
  - core
---

# Plugin Actions

## Overview

A plugin action is a named function that the workflow engine invokes as a step in a pipeline. Each action receives a `PipelineMessage`, performs work, and returns a `PipelineMessage`. Actions are the fundamental unit of plugin behaviour — everything a plugin does happens inside an action.

## Action Signature

Every action has the same signature:

```rust
fn action_name(input: PipelineMessage) -> Result<PipelineMessage, PluginError>;
```

The WASM binary must export a function for each action declared in `manifest.toml`. The SDK provides a macro to handle the Extism export boilerplate:

```rust
use life_engine_sdk::prelude::*;

#[plugin_action]
pub fn fetch(msg: PipelineMessage) -> Result<PipelineMessage, PluginError> {
    let config: EmailConfig = ctx.config().read()?;

    let emails = imap_fetch(&config)?;

    for email in &emails {
        ctx.storage().doc("emails").create(json!({
            "subject": email.subject,
            "from": email.from,
            "body": email.body,
            "ext": {
                "connector-email": {
                    "imap_uid": email.uid,
                    "folder": "INBOX"
                }
            }
        }))?;
    }

    ctx.events().emit("connector-email.fetch.completed", Some(json!({
        "count": emails.len()
    })))?;

    Ok(msg.with_payload(json!({ "fetched": emails.len() })))
}
```

The `#[plugin_action]` macro:

1. Generates the Extism export wrapper function
2. Deserialises the `PipelineMessage` from the WASM input
3. Creates a `PluginContext` (`ctx`) with access to host functions
4. Serialises the returned `PipelineMessage` to the WASM output
5. Maps `PluginError` to a structured error response the executor can interpret

## PluginContext

The `PluginContext` is the SDK's entry point to all host functions. It is created automatically by the `#[plugin_action]` macro and available as `ctx` within the action body.

```rust
impl PluginContext {
    pub fn storage(&self) -> StorageClient;
    pub fn events(&self) -> EventClient;
    pub fn config(&self) -> ConfigClient;
    pub fn http(&self) -> HttpClient;
}
```

Each client is a thin wrapper around the corresponding host functions. Calling a method on a client that requires a capability the plugin has not declared returns `PluginError::CapabilityDenied`.

## Lifecycle Hooks

Beyond regular actions, plugins can declare two optional lifecycle hooks:

### `init`

Called once when Core loads the plugin, after the manifest is validated and capabilities are granted. Use it for one-time setup that does not belong in individual actions.

```rust
#[plugin_init]
pub fn init() -> Result<(), PluginError> {
    // Validate config, warm caches, etc.
    let config: EmailConfig = ctx.config().read()?;
    validate_imap_connection(&config)?;
    Ok(())
}
```

- Runs during Core startup, in the `Init` phase of the plugin lifecycle
- Has access to `config_read` only — storage and events are not yet available
- If `init` returns an error, the plugin fails to load and Core logs the error
- Optional — if not declared, the plugin skips straight to `Running`

Declaration in manifest:

```toml
[lifecycle]
init = true
```

### `shutdown`

Called once when Core is shutting down, before the plugin is unloaded. Use it for graceful cleanup.

```rust
#[plugin_shutdown]
pub fn shutdown() -> Result<(), PluginError> {
    // Flush buffers, close connections, etc.
    Ok(())
}
```

- Runs during Core shutdown, in the `Stop` phase of the plugin lifecycle
- Has access to storage and config (best-effort — Core is shutting down)
- If `shutdown` returns an error, Core logs it and continues shutting down
- Optional — most plugins do not need this

Declaration in manifest:

```toml
[lifecycle]
shutdown = true
```

## Action Timeout

Each action has a configurable timeout declared in `manifest.toml`:

```toml
[actions.fetch]
timeout_ms = 30000
```

If the action exceeds its timeout, the Extism host cancels the WASM execution and returns `PluginError::Timeout` to the workflow engine. The step's `on_error` strategy then applies (halt, retry, or skip).

The default timeout is 5000ms (5 seconds). Connectors that poll external APIs should set higher timeouts.

## Action Error Handling

When an action encounters an error, it has two choices:

### Propagate the error

Return `Err(PluginError)`. The workflow engine treats this as a step failure and applies the step's `on_error` strategy.

```rust
#[plugin_action]
pub fn fetch(msg: PipelineMessage) -> Result<PipelineMessage, PluginError> {
    let emails = imap_fetch(&config).map_err(|e| PluginError::External(e.to_string()))?;
    Ok(msg.with_payload(json!({ "emails": emails })))
}
```

### Handle gracefully

Catch the error, add a warning, and return a valid `PipelineMessage`. The workflow continues normally, but the caller sees the warning in the response.

```rust
#[plugin_action]
pub fn fetch(msg: PipelineMessage) -> Result<PipelineMessage, PluginError> {
    match imap_fetch(&config) {
        Ok(emails) => Ok(msg.with_payload(json!({ "emails": emails }))),
        Err(e) => {
            let mut out = msg.with_payload(json!({ "emails": [] }));
            out.metadata.warnings.push(format!("IMAP fetch failed: {}", e));
            Ok(out)
        }
    }
}
```

The right choice depends on whether the step is critical to the workflow. Connectors that sync data should typically propagate (triggering retry). Enrichment plugins that add optional data should typically handle gracefully.

## PipelineMessage Helpers

The SDK provides convenience methods on `PipelineMessage`:

```rust
impl PipelineMessage {
    /// Replace the payload, preserving metadata
    pub fn with_payload(self, payload: Value) -> Self;

    /// Set a status hint
    pub fn with_status(self, hint: StatusHint) -> Self;

    /// Add a warning
    pub fn with_warning(self, message: impl Into<String>) -> Self;

    /// Set an extra metadata value (namespaced by plugin ID)
    pub fn with_extra(self, key: impl Into<String>, value: Value) -> Self;

    /// Read a typed value from the payload
    pub fn payload_as<T: DeserializeOwned>(&self) -> Result<T, PluginError>;

    /// Read a field from the payload by dot-separated path
    pub fn payload_field(&self, path: &str) -> Option<&Value>;

    /// Read from extra metadata
    pub fn extra(&self, key: &str) -> Option<&Value>;
}
```

## Statelessness

Plugin actions are stateless between invocations. WASM linear memory is reset between action calls. A plugin cannot store data in memory across invocations — use storage host functions for persistence or `metadata.extra` for passing context between steps in the same workflow.

The `init` hook is the exception — it runs once and any setup it performs is available for the lifetime of the plugin instance. However, mutable state across action calls is not supported.

## Connector Pattern

Connectors are regular plugins that follow a common pattern:

1. Read config (API credentials, sync interval, source URL)
2. Make outbound HTTP requests to fetch data from an external service
3. Normalise the data to CDM recommended schemas
4. Write to shared collections via storage host functions
5. Emit a completion event

The SDK does not enforce this pattern, but it is the recommended approach for data ingestion plugins. A typical connector declares these capabilities:

```toml
[capabilities]
storage_doc = ["read", "write"]
storage_blob = ["read", "write"]
http_outbound = true
events_emit = true
config_read = true
```

And accesses shared CDM collections:

```toml
[collections.emails]
schema = "cdm:emails"
access = "read-write"

[collections.contacts]
schema = "cdm:contacts"
access = "read-write"
```
