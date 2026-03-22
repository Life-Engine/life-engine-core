<!--
domain: sdk
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Spec — Plugin SDK RS

## Contents

- [[#Purpose]]
- [[#Package]]
- [[#CorePlugin Trait]]
- [[#PluginContext]]
- [[#Capability Enum]]
- [[#Route Registration]]
- [[#Canonical Collection Types]]
- [[#WASM Target]]
- [[#Builder Pattern]]
- [[#Versioning]]
- [[#Acceptance Criteria]]

## Purpose

This spec defines the Rust SDK for Core plugin authors. The SDK provides the trait definitions, types, and helpers that plugin authors need to implement a Core plugin, compile it to WASM, and load it into the Core runtime.

Reference: [[03 - Projects/Life Engine/Design/Core/Plugins]]

## Package

The SDK is published as the `life-engine-plugin-sdk` crate on crates.io. Plugin authors add it as a dependency in their `Cargo.toml`:

```toml
[dependencies]
life-engine-plugin-sdk = "1"
```

The crate re-exports all public types, traits, and macros needed to author a Core plugin. It has no dependency on Core internals.

## CorePlugin Trait

Every Core plugin must implement the `CorePlugin` trait. This is the single entry point that Core uses to manage the plugin lifecycle, discover routes, and dispatch events.

```rust
#[async_trait]
pub trait CorePlugin: Send + Sync {
    fn id(&self) -> &str;
    fn display_name(&self) -> &str;
    fn version(&self) -> &str;
    fn capabilities(&self) -> Vec<Capability>;
    async fn on_load(&mut self, ctx: &PluginContext) -> Result<()>;
    async fn on_unload(&mut self) -> Result<()>;
    fn routes(&self) -> Vec<PluginRoute>;
    async fn handle_event(&self, event: &CoreEvent) -> Result<()>;
}
```

Method responsibilities:

- **id** — Returns the unique plugin identifier (reverse-domain format, e.g. `com.life-engine.google-calendar`).
- **display_name** — Returns a human-readable name for UI and logging.
- **version** — Returns the plugin version string (semver).
- **capabilities** — Declares which scoped capabilities this plugin requires. Core grants only the requested capabilities at load time.
- **on_load** — Called when Core loads the plugin. Receives a `PluginContext` for accessing storage, config, events, and logging. Use this for initialisation logic.
- **on_unload** — Called when Core unloads the plugin. Use this for cleanup (close connections, flush buffers).
- **routes** — Returns the HTTP routes this plugin exposes. Core mounts them under the plugin namespace.
- **handle_event** — Called when an event the plugin subscribed to is emitted on the event bus.

## PluginContext

The `PluginContext` struct is passed to the plugin during `on_load`. It provides scoped access to Core services. A plugin can only use the services matching its declared capabilities.

PluginContext provides:

- **Scoped storage access** — Read and write to the plugin's own namespace within the data store. Plugins cannot access another plugin's storage directly.
- **Config access** — Read the plugin's configuration values as set by the user.
- **Event bus** — Subscribe to and emit events on the Core event bus. Subscriptions are filtered by the plugin's declared event capabilities.
- **Logging** — Structured logging that Core collects and surfaces in diagnostics. Log entries are tagged with the plugin ID automatically.

## Capability Enum

The `Capability` enum defines the permissions a plugin can request. Core enforces these at runtime — any operation outside the granted capabilities returns an error.

```rust
pub enum Capability {
    StorageRead,
    StorageWrite,
    HttpOutbound,
    CredentialsRead,
    CredentialsWrite,
    EventsSubscribe,
    EventsEmit,
    ConfigRead,
    Logging,
}
```

All capabilities are scoped to the requesting plugin's namespace. For example, `StorageRead` grants read access to the plugin's own storage partition, not the entire data store. `HttpOutbound` allows the plugin to make outbound HTTP requests (subject to allowlisting in future versions).

## Route Registration

Plugins expose HTTP endpoints by returning a `Vec<PluginRoute>` from the `routes()` method. Core mounts all plugin routes under the namespace:

```
/api/plugins/{plugin-id}/
```

For example, a plugin with ID `com.life-engine.todos` that registers a route `/items` is reachable at:

```
/api/plugins/com.life-engine.todos/items
```

Each `PluginRoute` specifies the HTTP method, path, and handler function. Core handles authentication and authorisation before dispatching to the plugin handler.

## Canonical Collection Types

The SDK includes Rust struct definitions for the 7 canonical collection types. These structs have `serde::Serialize` and `serde::Deserialize` derives, making them ready for JSON serialisation out of the box.

The canonical types are:

- **Event** — Calendar events with recurrence support
- **Task** — Actionable items with status and priority
- **Contact** — People with structured name, email, phone, and address fields
- **Note** — Text content with optional tags
- **Email** — Email messages with threading and attachments
- **File** — File metadata with checksum
- **Credential** — Secure credential storage with typed claims

Full schema definitions for each type are in [[03 - Projects/Life Engine/Planning/specs/sdk/Canonical Data Models]].

## WASM Target

Plugins compile to the `wasm32-wasi` target. The Core runtime uses Extism to load and execute plugin WASM modules.

To build a plugin:

```bash
cargo build --target wasm32-wasi --release
```

The resulting `.wasm` file is the distributable plugin artifact. Core loads it at runtime without requiring the plugin source code or a Rust toolchain on the host machine.

WASI provides the plugin with a sandboxed environment — file system access, environment variables, and network access are all mediated by the runtime and constrained by the plugin's declared capabilities.

## Builder Pattern

The SDK provides a builder pattern for constructing plugin instances, reducing boilerplate for common configurations:

```rust
use life_engine_plugin_sdk::PluginBuilder;

let plugin = PluginBuilder::new("com.example.my-plugin")
    .display_name("My Plugin")
    .version("0.1.0")
    .capability(Capability::StorageRead)
    .capability(Capability::StorageWrite)
    .capability(Capability::EventsSubscribe)
    .route(Method::GET, "/items", handle_list_items)
    .route(Method::POST, "/items", handle_create_item)
    .on_load(init)
    .on_unload(cleanup)
    .build();
```

The builder validates required fields at compile time where possible and returns clear errors for missing configuration at runtime.

## Versioning

The SDK is versioned independently from Core.

- **v1.x** — Additive changes only. New capabilities, new canonical fields, new helper methods. No removals or breaking signature changes.
- **v2.x** — Breaking changes permitted. When v2 ships, v1 continues to receive security fixes for 12 months. Core maintains compatibility with both v1 and v2 plugins during the overlap window.

Plugin authors pin to a major version in their `Cargo.toml` and receive non-breaking updates automatically.

## Acceptance Criteria

A community plugin author can:

1. Add `life-engine-plugin-sdk` as a Cargo dependency
2. Implement the `CorePlugin` trait on their own struct
3. Use the builder pattern for convenience if desired
4. Compile the plugin to `wasm32-wasi` target
5. Load the resulting `.wasm` file in Core and have it respond to events and serve routes
