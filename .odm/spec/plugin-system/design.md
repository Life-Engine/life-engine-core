<!--
domain: core
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Core Plugin System

Reference: [[03 - Projects/Life Engine/Design/Core/Plugins]]

## Purpose

This spec defines how Core loads, manages, and isolates plugins. Core itself is an empty orchestrator — it loads plugins, gives them scoped access to storage and the API layer, and enforces isolation. All features are provided by plugins.

## WASM Isolation via Extism

Core plugins run as WebAssembly modules using Extism:

- **Memory-isolated** — Each plugin runs in its own WASM sandbox. No shared memory with the host or other plugins.
- **Language-agnostic** — Plugins can be written in Rust, Go, C, AssemblyScript, or any language that compiles to WASM.
- **Host functions only** — Core explicitly exports functions to plugins. Plugins can only call what the host exposes.
- **No direct I/O** — Plugins cannot access the filesystem, network, or OS directly. All access goes through host functions.
- **Crash isolation** — A failing plugin cannot crash Core. The WASM sandbox contains the failure.

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

- `id` — Unique plugin identifier (reverse domain notation recommended)
- `display_name` — Human-readable name
- `version` — Semantic version string
- `capabilities` — List of capabilities the plugin requires
- `on_load` — Called during initialisation with a `PluginContext` that provides scoped storage and config
- `on_unload` — Called during shutdown for cleanup
- `routes` — HTTP routes the plugin registers under `/api/plugins/{plugin-id}/`
- `handle_event` — Called when a subscribed Core event fires

## Plugin Lifecycle

```text
Discover -> Load -> Init -> Running -> Stop -> Unload
```

1. **Discover** — Core reads plugin paths from the YAML config. Each entry specifies a plugin ID and the path to its WASM binary.
2. **Load** — The WASM module is loaded into the Extism runtime. Capabilities declared in the manifest are compared against what the user has approved.
3. **Init** — `on_load` is called with a `PluginContext` providing scoped storage access, config, and event subscriptions. The plugin performs its internal setup.
4. **Running** — Plugin routes are mounted on the HTTP server, event handlers are active, and the plugin responds to requests and events.
5. **Stop** — `on_unload` is called. The plugin cleans up resources, flushes buffers, and releases handles.
6. **Unload** — The WASM module is released from the Extism runtime. Routes are unmounted.

## Plugin Discovery

Plugins are configured in Core's YAML config under the `plugins` section:

```yaml
plugins:
  paths:
    - /usr/local/lib/life-engine/plugins/
  enabled:
    - id: email-connector
      path: /usr/local/lib/life-engine/plugins/email-connector.wasm
    - id: calendar-connector
      path: /usr/local/lib/life-engine/plugins/calendar-connector.wasm
    - id: notes
      path: /usr/local/lib/life-engine/plugins/notes.wasm
```

Core loads only plugins listed in the config. The `auto_enable: false` default means newly discovered plugins must be explicitly enabled.

## Capabilities

All capabilities are deny-by-default. Plugins declare what they need in their manifest. Core grants or denies each capability at install time. There are no plugin categories — any plugin can request any combination.

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

All capabilities are scoped at runtime. A plugin declaring `storage:read` on `events` cannot read `contacts`. A plugin declaring `http:outbound` for `api.google.com` cannot reach `api.github.com`. The host enforces all scoping — WASM provides no other path.

## Host Functions

Functions Core exposes to WASM plugins:

- **Storage** — Read/write to the plugin's scoped collections (both canonical and private). Queries run through the same `StorageAdapter` trait as the REST API.
- **Credentials** — Read/write to the unified credential store. Scoped by credential type and plugin capabilities.
- **Config** — Read plugin-specific configuration from Core's YAML config.
- **Events** — Subscribe to and emit Core events. Only events matching declared capabilities are delivered.
- **Logging** — Structured logging via the host. Logs are tagged with the plugin ID.
- **HTTP (domain-scoped)** — Make outbound HTTP requests. Only domains declared in the plugin manifest are reachable.

Plugins cannot bypass these — WASM provides no other path to the host.

## Plugin-to-Plugin Communication

Plugins communicate through three mechanisms. Direct plugin-to-plugin calls are not supported — all interaction goes through Core.

- **Shared canonical collections** — Two plugins reading/writing the same canonical collection (e.g., both work with `events`). This is the simplest form of communication and requires no special mechanism.
- **Core events** — One plugin emits an event via the `events:emit` capability, another subscribes to it via `events:subscribe`. Events are delivered asynchronously.
- **Workflows** — Plugins chained in a workflow receive the output of the previous step as input. See [[03 - Projects/Life Engine/Planning/specs/core/Workflow Engine]].

## Data Model for Plugins

Plugins interact with two tiers of collections. See [[03 - Projects/Life Engine/Planning/specs/core/Data Layer]] for full details.

### Canonical Collections (use first)

Platform-owned data types defined in the SDK (`events`, `tasks`, `contacts`, `notes`, `emails`, `files`, `credentials`). Using canonical collections is the path of least resistance — no schema definition needed, full type support from the SDK, and automatic interoperability with every other plugin.

```rust
// Plugin reads canonical events — typed, documented, zero schema work
let events = ctx.store.query("events", &filters).await?;
```

### Private Collections (when canonical doesn't fit)

For data genuinely unique to the plugin. Namespaced automatically to prevent collisions. Requires a JSON Schema definition in the plugin manifest.

### Extending Canonical Data

Plugins that need custom fields on canonical records use the namespaced `extensions` field. See [[03 - Projects/Life Engine/Design/Core/Data#Extensions on Canonical Data]].

## Connector Plugins

A connector is a regular plugin that declares `http:outbound` and `credentials:read`/`credentials:write` capabilities. It fetches data from an external service, normalises it, and writes to canonical collections. There is no special trait or category — connectors are just plugins that happen to talk to external APIs.

See [[03 - Projects/Life Engine/Planning/specs/core/Connector Architecture]] for the full connector spec.

## SDK Contract

Two SDKs exist for two plugin targets. Both define the same canonical collection schemas:

- `plugin-sdk-rs` — For Core plugins (Rust, compiled to WASM). Defines the `CorePlugin` trait, `Store` trait, and `Route` type.
- `plugin-sdk-js` — For App plugins (JavaScript, running in the Tauri webview). Defines the shell data API bindings and canonical types.

SDK versioning (applies to both):

- Versioned independently from Core
- Additive only for minor versions — new methods, new optional interfaces, no removals
- Breaking changes only for major versions, with a 12-month support overlap
- New capabilities expressed as optional traits, not by expanding the core interface

## First-Party Plugins

Located in `plugins/core/` in the monorepo:

- **Email Connector** — IMAP/SMTP, writes to the `emails` canonical collection
- **Calendar Connector** — CalDAV, writes to the `events` canonical collection
- **Contacts Connector** — CardDAV, writes to the `contacts` canonical collection
- **Files Connector** — WebDAV/S3, writes to the `files` canonical collection
- **Identity** — Credential store and verification
- **Sync** — Sync engine

## Community Plugins

Third-party authors create an independent repo, add `plugin-sdk-rs` as a dependency, implement the `CorePlugin` trait, compile to WASM, and distribute. No monorepo forking or internal tooling knowledge required.

## Acceptance Criteria

- Plugins load from configured paths and register routes on the HTTP server
- Capabilities are enforced at runtime — a plugin requesting an unapproved capability is denied
- A failing plugin cannot crash Core — the WASM sandbox contains the failure
- Plugin-to-plugin communication works via shared canonical collections and Core events
- Plugin routes mount under `/api/plugins/{plugin-id}/` and are removed when the plugin is unloaded
- `on_load` and `on_unload` are called at the correct lifecycle points
- Host function scoping is enforced — a plugin cannot access collections or domains outside its declared scope
