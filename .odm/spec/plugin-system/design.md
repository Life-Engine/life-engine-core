<!--
domain: plugin-system
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Design Document — Plugin System

## Introduction

This document describes the technical design of the Life Engine plugin system. Plugins are WASM modules loaded via Extism that communicate with Core through host functions gated by a deny-by-default capability system. The design covers plugin structure, manifest parsing, lifecycle management, capability enforcement, host function dispatch, the SDK macro system, and the connector pattern.

## Plugin Directory Structure

Each plugin ships as a self-contained directory under the configured plugins path:

```
plugins/
  connector-email/
    plugin.wasm
    manifest.toml
    schemas/
      email-extensions.json
      sync-state.json
      config.json
```

- `plugin.wasm` — compiled WASM module containing all exported actions and hooks
- `manifest.toml` — plugin identity, actions, capabilities, collections, events, and config declarations
- `schemas/` — optional directory for JSON Schema files referenced by the manifest

## Manifest Format

The manifest is parsed as TOML. The following sections are recognised.

### Plugin Identity

```toml
[plugin]
id = "connector-email"
name = "Email Connector"
version = "1.0.0"
description = "IMAP/SMTP email sync"
author = "Life Engine"
license = "MIT"
```

Required fields: `id` (kebab-case, globally unique), `name`, `version` (semver). Optional: `description`, `author`, `license`.

### Actions

```toml
[actions.fetch]
description = "Fetch new emails from IMAP"
timeout_ms = 30000

[actions.send]
description = "Send email via SMTP"
timeout_ms = 10000
```

Each action declares a `description` (required) and optional `timeout_ms`. At least one action must be declared. Each action name must correspond to a WASM export in the module.

### Capabilities

```toml
[capabilities]
storage_doc = ["read", "write"]
storage_blob = ["read", "write"]
http_outbound = true
events_emit = true
events_subscribe = true
config_read = true
```

Omitted capabilities default to denied. Capability keys map to host function groups.

### Collections

```toml
[collections.emails]
schema = "cdm:emails"
access = "read-write"
extensions = ["ext.connector-email.thread_id", "ext.connector-email.imap_uid"]
extension_schema = "schemas/email-extensions.json"
extension_indexes = ["ext.connector-email.thread_id"]

[collections.email_sync_state]
schema = "schemas/sync-state.json"
indexes = ["last_sync"]
strict = true
```

- `schema` — `cdm:<name>` reference or relative path to a local JSON Schema
- `access` — `"read"`, `"write"`, or `"read-write"`
- `extensions` — extension field paths using `ext.<plugin-id>.<field>` convention
- `extension_schema` — relative path to JSON Schema for extension fields
- `extension_indexes`, `indexes` — fields to index
- `strict` — when `true`, enforce schema validation on write (default `false`)

### Events

```toml
[events.emit]
events = ["connector-email.fetch.completed", "connector-email.fetch.failed"]

[events.subscribe]
events = ["system.startup"]
```

Event names follow the dot-separated convention: `<plugin-id>.<action>.<outcome>`.

### Configuration

```toml
[config]
schema = "schemas/config.json"
```

Core validates the plugin's runtime config against this schema at load time.

## Data Structures

### Manifest Types

```rust
pub struct PluginManifest {
    pub plugin: PluginIdentity,
    pub actions: HashMap<String, ActionDecl>,
    pub capabilities: CapabilityDecl,
    pub collections: HashMap<String, CollectionDecl>,
    pub events: Option<EventDecl>,
    pub config: Option<ConfigDecl>,
}

pub struct PluginIdentity {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub license: Option<String>,
}

pub struct ActionDecl {
    pub description: String,
    pub timeout_ms: Option<u64>,
}

pub struct CapabilityDecl {
    pub storage_doc: Vec<String>,
    pub storage_blob: Vec<String>,
    pub http_outbound: bool,
    pub events_emit: bool,
    pub events_subscribe: bool,
    pub config_read: bool,
}

pub struct CollectionDecl {
    pub schema: String,
    pub access: String,
    pub extensions: Vec<String>,
    pub extension_schema: Option<String>,
    pub extension_indexes: Vec<String>,
    pub indexes: Vec<String>,
    pub strict: bool,
}

pub struct EventDecl {
    pub emit: Vec<String>,
    pub subscribe: Vec<String>,
}

pub struct ConfigDecl {
    pub schema: String,
}
```

### Plugin Registry Entry

```rust
pub struct PluginEntry {
    pub manifest: PluginManifest,
    pub instance: ExtismPlugin,
    pub state: PluginState,
    pub trust_level: TrustLevel,
    pub granted_capabilities: HashSet<String>,
}

pub enum PluginState {
    Discovered,
    Loaded,
    Initialised,
    Running,
    Stopped,
    Unloaded,
}

pub enum TrustLevel {
    FirstParty,
    ThirdParty,
}
```

### PluginContext

```rust
pub struct PluginContext {
    pub storage: StorageClient,
    pub events: EventClient,
    pub config: ConfigClient,
    pub http: HttpClient,
}
```

Each client wraps calls to the corresponding host functions. Methods return `Result<T, PluginError>`.

### PluginError

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

## Lifecycle Flow

The lifecycle proceeds through six states in order:

1. **Discover** — Core scans the plugin directory at startup. For each subdirectory containing `manifest.toml`, Core records a `PluginEntry` in `Discovered` state.

2. **Load** — Core parses the manifest, validates all fields, checks capability grants against the trust model, and instantiates the WASM module via `Extism::Plugin::new()`. The entry moves to `Loaded` state. If validation or instantiation fails, the plugin is skipped and the error is logged.

3. **Init** — If the manifest declares an `init` action, Core calls it with a `PluginContext`. On success, the entry moves to `Initialised`. On failure, the plugin is unloaded and the error is logged.

4. **Running** — The plugin is registered as available for workflow invocation. Actions can be called by the pipeline executor.

5. **Stop** — During Core shutdown, Core calls the `shutdown` action (if declared) on each running plugin. The entry moves to `Stopped`.

6. **Unload** — Core drops the Extism plugin instance and removes the entry from the registry.

## Capability Enforcement Design

Capabilities are enforced at the host function dispatch layer:

```rust
fn check_capability(
    plugin_id: &str,
    required: &str,
    registry: &PluginRegistry,
) -> Result<(), PluginError> {
    let entry = registry.get(plugin_id)?;
    if entry.granted_capabilities.contains(required) {
        Ok(())
    } else {
        Err(PluginError::CapabilityDenied(
            format!("Plugin '{}' lacks capability '{}'", plugin_id, required)
        ))
    }
}
```

Capability granting at load time:

- For `TrustLevel::FirstParty` — all declared capabilities are added to `granted_capabilities`
- For `TrustLevel::ThirdParty` — only capabilities listed in Core's `approved_capabilities` config section are added; any unapproved capability causes load failure

Collection access is also enforced: a plugin calling `storage_doc_get("tasks", id)` must have `"tasks"` in its `collections` map with at least `"read"` access.

## Host Function Dispatch

Each host function is registered with Extism as a callable import. The dispatch pattern:

```rust
fn host_storage_doc_get(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
) -> Result<(), Error> {
    let plugin_id = plugin.id();
    let (collection, id) = deserialise_args(plugin, inputs)?;

    // Check capability
    check_capability(plugin_id, "storage:doc:read", &registry)?;

    // Check collection access
    check_collection_access(plugin_id, &collection, "read", &registry)?;

    // Execute operation
    let doc = storage_context.get(&collection, &id)?;

    // Return result
    serialise_result(plugin, outputs, &doc)
}
```

Host functions that are never exposed to plugins: `transaction`, `watch`, `migrate`, `health`, `copy`.

## SDK Macro System

### `#[plugin_action]` Macro

Transforms a plain function into an Extism-compatible WASM export:

```rust
// What the author writes:
#[plugin_action]
fn fetch(msg: PipelineMessage, ctx: PluginContext) -> Result<PipelineMessage, PluginError> {
    let config = ctx.config.read()?;
    let response = ctx.http.request(/* ... */)?;
    // normalise and store
    Ok(msg.with_payload(normalised_data))
}

// What the macro generates (simplified):
#[no_mangle]
pub extern "C" fn fetch() -> i32 {
    let result = std::panic::catch_unwind(|| {
        let input = extism_pdk::input::<String>().unwrap();
        let msg: PipelineMessage = serde_json::from_str(&input).unwrap();
        let ctx = PluginContext::from_host();
        match _user_fetch(msg, ctx) {
            Ok(output) => {
                let json = serde_json::to_string(&output).unwrap();
                extism_pdk::output(json).unwrap();
                0
            }
            Err(e) => {
                extism_pdk::error(e.to_string());
                1
            }
        }
    });
    match result {
        Ok(code) => code,
        Err(_) => {
            extism_pdk::error("InternalError: plugin panicked");
            1
        }
    }
}
```

### `#[plugin_hook]` Macro

Similar to `#[plugin_action]` but for lifecycle hooks that do not receive or return a `PipelineMessage`:

```rust
#[plugin_hook]
fn init(ctx: PluginContext) -> Result<(), PluginError> {
    let config = ctx.config.read()?;
    // validate config, warm caches
    Ok(())
}
```

## Connector Pattern

Connectors are regular plugins that follow a standardised fetch sequence:

```rust
#[plugin_action]
fn fetch(msg: PipelineMessage, ctx: PluginContext) -> Result<PipelineMessage, PluginError> {
    // 1. Read configuration
    let config: EmailConfig = serde_json::from_value(ctx.config.read()?)?;

    // 2. Fetch from external service
    let response = ctx.http.request(HttpRequest {
        method: "GET".into(),
        url: format!("{}:{}", config.imap_host, config.imap_port),
        headers: None,
        body: None,
    })?;

    // 3. Normalise to CDM schemas
    let emails = normalise_to_cdm(&response)?;

    // 4. Write to shared collections
    for email in &emails {
        ctx.storage.doc_create("emails", &serde_json::to_string(email)?)?;
    }

    // 5. Emit completion event
    ctx.events.emit("connector-email.fetch.completed", Some(json!({
        "count": emails.len()
    })))?;

    Ok(msg.with_payload(json!({ "synced": emails.len() })))
}
```

First-party connectors shipping with v1:

- **Email** — IMAP for retrieval, SMTP for sending
- **Calendar** — CalDAV sync
- **Contacts** — CardDAV sync
- **Filesystem** — WebDAV and S3-compatible object storage

## Error Handling Strategy

- **Hard failure** — Action returns `Err(PluginError)`. The pipeline executor marks the step as failed and applies the workflow's `on_error` strategy (fail or skip).
- **Soft warning** — Action returns `Ok(msg)` with entries appended to `msg.metadata.warnings`. The step succeeds but degradation is recorded.
- **Panic recovery** — The `#[plugin_action]` macro wraps execution in `std::panic::catch_unwind` and converts panics to `InternalError`.
- **WASM fault** — Extism traps the fault. Core logs it and marks the step as failed.
- **Timeout** — Extism terminates execution after `timeout_ms`. The step is marked as failed.

## File Locations

Key implementation files (planned):

- `packages/types/src/plugin_manifest.rs` — manifest types and TOML deserialisation
- `packages/types/src/plugin_error.rs` — `PluginError` enum
- `packages/traits/src/plugin_registry.rs` — `PluginRegistry` trait
- `packages/core/src/plugin_loader.rs` — discovery, validation, loading, lifecycle management
- `packages/core/src/host_functions.rs` — host function implementations and registration
- `packages/core/src/capability_checker.rs` — capability enforcement logic
- `packages/plugin-sdk/src/lib.rs` — `PluginContext`, client types, macro re-exports
- `packages/plugin-sdk-macros/src/lib.rs` — `#[plugin_action]` and `#[plugin_hook]` proc macros
