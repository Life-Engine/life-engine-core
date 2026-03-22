---
title: "Engine — Plugin System"
tags: [life-engine, engine, plugins, wasm, extism, canonical]
created: 2026-03-14
---

# Core Plugin System

All Core features are provided by plugins. Core itself is an empty orchestrator — it loads plugins, gives them scoped access to storage and the API layer, and enforces isolation.

This design embodies several [[03 - Projects/Life Engine/Design/Principles|Design Principles]]: *Separation of Concerns* (Core owns orchestration, plugins own logic), *Open/Closed Principle* (new features are new plugins, Core does not change), *Principle of Least Privilege* (deny-by-default capabilities enforced at runtime), *Explicit Over Implicit* (all behaviour declared in manifests), and *The Pit of Success* (canonical collections are the easiest path for plugin authors).

## WASM Isolation via Extism

Core plugins run as WebAssembly modules using **Extism**:

- **Memory-isolated** — Each plugin runs in its own WASM sandbox. No shared memory with the host or other plugins.
- **Language-agnostic** — Plugins can be written in Rust, Go, C, AssemblyScript, or any language that compiles to WASM.
- **Host functions** — Core explicitly exports functions to plugins. Plugins can only call what the host exposes.
- **No direct I/O** — Plugins cannot access the filesystem, network, or OS directly. All access goes through host functions.
- **Crash isolation** — A failing plugin cannot crash Core.

## CorePlugin Trait

Every Core plugin implements this trait (via the Rust SDK or WASM equivalent):

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

## Plugin Lifecycle

```
Discover -> Load -> Init -> Running -> Stop -> Unload
```

1. **Discover** — Core reads plugin paths from config
2. **Load** — WASM module loaded into Extism runtime
3. **Init** — `on_load` called with a `PluginContext` providing scoped storage and config
4. **Running** — Plugin routes are mounted, event handlers active
5. **Stop** — `on_unload` called for cleanup
6. **Unload** — WASM module released

## Plugin Discovery

Plugins are configured in Core's YAML config:

```yaml
plugins:
  - id: notes
    path: /usr/local/lib/life-engine/plugins/notes.wasm
  - id: calendar
    path: /usr/local/lib/life-engine/plugins/calendar.wasm
```

## Capabilities

Deny-by-default. Plugins declare what they need in their manifest; Core grants or denies at install time. There are no plugin categories — any plugin can request any combination of capabilities.

Available capabilities:

- `storage:read` — Read from declared canonical and private collections
- `storage:write` — Write to declared canonical and private collections
- `http:outbound` — Make outbound HTTP requests to declared domains only
- `credentials:read` — Read from the unified credential store (scoped by type and collection)
- `credentials:write` — Write to the unified credential store (scoped)
- `events:subscribe` — Subscribe to Core events
- `events:emit` — Emit events for other plugins to consume
- `config:read` — Read plugin-specific configuration
- `logging` — Structured logging via the host

All capabilities are scoped. A plugin declaring `storage:read` on `events` cannot read `contacts`. A plugin declaring `http:outbound` for `api.google.com` cannot reach `api.github.com`. The host enforces all scoping at runtime.

Plugin routes mount under `/api/plugins/{plugin-id}/`.

## Data Model for Plugins

Plugins interact with two tiers of collections:

### Canonical Collections (use these first)

Platform-owned data types defined in the SDK (`events`, `tasks`, `contacts`, `notes`, `emails`, `files`, `credentials`). Using canonical collections is the path of least resistance — no schema definition needed, full type support from the SDK, and automatic interoperability with every other plugin in the ecosystem.

```rust
// Plugin reads canonical events — typed, documented, zero schema work
let events = ctx.store.query("events", &filters).await?;
```

### Private Collections (when canonical doesn't fit)

For data genuinely unique to the plugin. Namespaced automatically to prevent collisions. Requires a schema definition in the plugin manifest.

### Extending Canonical Data

Plugins that need custom fields on canonical records use the namespaced `extensions` field. See [[03 - Projects/Life Engine/Design/Core/Data#Extensions on Canonical Data|Data — Extensions]].

## Promoting Ecosystem Interoperability

The design makes canonical the default and private the exception:

- **SDK ships ready-to-use types** — Plugin authors import canonical types directly. Autocomplete, documentation, zero schema work.
- **First-party plugins set the example** — Every first-party plugin uses canonical collections as its primary data model.
- **Plugin store surfaces compatibility** — Plugins using canonical collections get a "Works with your data" badge. Users see which plugins are composable.
- **Extensions prevent unnecessary private collections** — If the only reason to create a private collection is "I need extra fields," the `extensions` object solves that within canonical.
- **Canonical schema evolution is governed** — Versioned with the SDK. Adding fields is non-breaking. Removals require major version bump + migration path.

## Host Functions

Functions Core exposes to WASM plugins:

- **Storage** — Read/write to the plugin's scoped tables (canonical and private)
- **Credentials** — Read/write to the unified credential store (scoped by type)
- **Config** — Read plugin-specific configuration
- **Events** — Subscribe to and emit Core events
- **Logging** — Structured logging via the host
- **HTTP (scoped)** — Make outbound requests to declared domains only

Plugins cannot bypass these — WASM provides no other path to the host.

## Plugin-to-Plugin Communication

Plugins communicate through three mechanisms. Direct plugin-to-plugin calls are not supported — all interaction goes through the host.

- **Shared canonical collections** — Two plugins reading/writing the same canonical collection (e.g., both work with `events`)
- **Core events** — One plugin emits an event, another subscribes to it
- **Workflows** — Plugins chained in a workflow receive the output of the previous step as input. See [[03 - Projects/Life Engine/Design/Core/Workflow]]

## SDK Contract

Two SDKs exist for two plugin targets. Both define the same canonical collection schemas:

- `plugin-sdk-rs` — For Core plugins (Rust, compiled to WASM). Defines `CorePlugin` trait, `Store` trait, `Route` type.
- `plugin-sdk-js` — For App plugins (JavaScript, running in the Tauri webview). Defines the shell data API bindings and canonical types.

Versioning (both SDKs):

- Versioned independently from Core
- Additive only (new methods, new optional interfaces, no removals) for minor versions
- Breaking changes only if necessary for major versions, with 12-month support overlap
- New capabilities expressed as optional traits, not by expanding the core interface

## Connector Plugins

A connector is a regular plugin that declares `http:outbound` and `credentials:read` capabilities. It fetches data from an external service, normalises it, and writes to canonical collections. There is no special trait or category — connectors are just plugins that happen to talk to external APIs.

See [[03 - Projects/Life Engine/Design/Core/Connectors]] for details on the protocol-first approach, normalisation, and sync strategies.

## First-Party Plugins

Located in `plugins/core/` in the monorepo:

- **Email Connector** — IMAP/SMTP, writes to `emails` canonical collection
- **Calendar Connector** — CalDAV, writes to `events` canonical collection
- **Contacts Connector** — CardDAV, writes to `contacts` canonical collection
- **Files Connector** — WebDAV/S3, writes to `files` canonical collection
- **Identity** — Credential store, verification
- **Sync** — Sync engine

## Core Configuration Endpoints

Core exposes system configuration endpoints under `/api/system/config` (see [[03 - Projects/Life Engine/Design/Core/API#System Configuration Endpoints]]). These are a Core-level feature, not a plugin — they are built into the Core binary and always available. The App's Core Configuration plugin (`com.life-engine.core-config`) consumes these endpoints to provide a web UI for editing Core's YAML config without direct file access.

## Community Plugins

Third-party authors create an independent repo, add `plugin-sdk-rs`, implement `CorePlugin`, compile to WASM, and distribute. No monorepo forking or internal tooling knowledge required.
