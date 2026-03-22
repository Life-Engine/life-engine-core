<!--
domain: core
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Core Plugin System — Design

Reference: [Plugins Design Document](../../doc/Design/Core/Plugins.md)

## Purpose

This spec defines how Core discovers, loads, manages, and isolates plugins. Core is a thin orchestrator — it scans a configured plugins directory at startup, loads WASM modules via Extism, injects scoped host functions, and enforces capability-based isolation. All features are provided by plugins. Core never compiles against any plugin.

This spec absorbs the previously separate plugin-loader spec. Discovery, manifest parsing, and loading are all covered here. The Plugin SDK (`plugin-sdk-rs`) is a separate spec and is not duplicated here.

## WASM Isolation via Extism

Plugins run as WebAssembly modules using Extism:

- **Memory-isolated** — Each plugin runs in its own WASM sandbox. No shared memory with the host or other plugins.
- **Language-agnostic** — Plugins can be written in Rust, Go, C, AssemblyScript, or any language that compiles to WASM.
- **Host functions only** — Core explicitly exports functions to plugins. Plugins can only call what the host exposes.
- **No direct I/O** — Plugins cannot access the filesystem, network, or OS directly. All access goes through host functions.
- **Crash isolation** — A failing plugin cannot crash Core. The WASM sandbox contains the failure.

## Plugin Trait

Every plugin implements the `Plugin` trait (defined in `packages/traits`, re-exported via `packages/plugin-sdk`):

```rust
pub trait Plugin: Send + Sync {
    fn id(&self) -> &str;
    fn display_name(&self) -> &str;
    fn version(&self) -> &str;
    fn actions(&self) -> Vec<Action>;
    async fn execute(&self, action: &str, input: PipelineMessage) -> Result<PipelineMessage>;
}
```

- `id` — Unique plugin identifier (reverse domain notation recommended, e.g., `com.life-engine.connector-email`)
- `display_name` — Human-readable name
- `version` — Semantic version string
- `actions` — List of actions the plugin provides, each usable as a workflow step
- `execute` — Called by the workflow engine with an action name and a `PipelineMessage`, returns a `PipelineMessage`

Plugins do not register HTTP routes, handle events directly, or manage their own lifecycle callbacks. The workflow engine invokes `execute` for each step. Host functions provide all access to Core services.

## Plugin Discovery

Core scans a configured directory at startup. Each plugin is a subdirectory containing a WASM binary and a manifest:

```
plugins/
  connector-email/
    plugin.wasm       <- Compiled WASM module
    manifest.toml     <- Plugin metadata, actions, config, capabilities
  connector-calendar/
    plugin.wasm
    manifest.toml
```

The plugins directory path is configured in `config.toml`:

```toml
[plugins]
path = "./plugins/"
```

Core iterates over each subdirectory. If both `plugin.wasm` and `manifest.toml` are present, the plugin is discovered. Missing or malformed entries are logged and skipped without blocking other plugins.

## Manifest Format

The `manifest.toml` declares everything Core needs to know about a plugin:

```toml
[plugin]
id = "com.life-engine.connector-email"
name = "Email Connector"
version = "0.1.0"
description = "IMAP/SMTP email sync"

[actions.fetch]
description = "Fetch emails from configured IMAP server"
input_schema = "emails"
output_schema = "emails"

[actions.send]
description = "Send an email via SMTP"
input_schema = "custom:send-request"
output_schema = "emails"

[capabilities]
required = ["storage:read", "storage:write", "http:outbound", "config:read"]

[config]
poll_interval = { type = "string", default = "5m", description = "How often to poll for new emails" }
```

Sections:

- `[plugin]` — Required. Plugin identity: id, name, version, description.
- `[actions.*]` — One section per action. Each declares a description, input schema reference, and output schema reference. Schema references are either a canonical CDM type name (e.g., `emails`) or `custom:<schema-name>` for plugin-defined JSON Schemas.
- `[capabilities]` — Required capabilities the plugin needs at runtime.
- `[config]` — Optional. Declares config keys the plugin expects, with types, defaults, and descriptions.

## Plugin Lifecycle

```text
Discover -> Load -> Init -> Running -> Stop -> Unload
```

1. **Discover** — Core scans the plugins directory and reads each `manifest.toml`. Validates manifest structure and required fields.
2. **Load** — Capabilities declared in the manifest are checked against the approval policy. If approved, the WASM binary is loaded into the Extism runtime. Host functions matching the approved capabilities are injected.
3. **Init** — The plugin receives its configuration section and performs internal setup.
4. **Running** — The plugin's actions are available to the workflow engine. The workflow engine calls `execute(action, PipelineMessage)` for each step that references this plugin.
5. **Stop** — Cleanup signal sent to the plugin during shutdown or disable.
6. **Unload** — WASM module released from the Extism runtime. Plugin actions removed from the workflow engine.

## Capabilities

Deny-by-default. Plugins declare required capabilities in their `manifest.toml`. Core grants or denies each capability at load time. Host functions injected into the WASM runtime are gated by the granted capability set.

Available capabilities:

- `storage:read` — Read from collections via StorageContext
- `storage:write` — Write to collections via StorageContext
- `http:outbound` — Make outbound HTTP requests
- `events:emit` — Emit events into the workflow engine
- `events:subscribe` — Listen for events
- `config:read` — Read own config section

All capabilities are enforced at runtime by the WASM host. A plugin that calls a host function it was not granted receives an error.

### Approval Policy

- **First-party plugins** (shipped in the monorepo `plugins/` directory) — capabilities auto-granted. These are trusted.
- **Third-party plugins** — capabilities must be explicitly approved in Core config:

```toml
[plugins.some-third-party]
approved_capabilities = ["storage:read", "http:outbound"]
```

If a manifest declares a capability not in the approved list, Core refuses to load the plugin and logs a warning identifying the unapproved capability.

## Host Functions

Functions Core exposes to WASM plugins, gated by capabilities:

- **Storage** — Read/write via StorageContext (query builder API). Routes through the `StorageBackend` trait.
- **Config** — Read plugin-specific configuration section from `config.toml`.
- **Events** — Emit events for workflows to consume, subscribe to event types.
- **HTTP** — Make outbound HTTP requests (requires `http:outbound`).
- **Logging** — Structured logging via the host. Logs tagged with the calling plugin's ID. Always available (no capability required).

Plugins cannot bypass these — WASM provides no other path to the host.

## Plugin Actions and Execution

Each action declared in the manifest is a step the workflow engine can invoke. The workflow engine calls `execute(action, PipelineMessage)` on the plugin:

```rust
// Workflow engine pseudocode
let output = plugin.execute("fetch", input_message).await?;
```

Every action receives a `PipelineMessage` and returns a `PipelineMessage`. This standard contract makes plugins composable — any plugin's output can be another plugin's input, as long as the schemas are compatible.

## Plugin-to-Plugin Communication

Plugins communicate through two mechanisms only. Direct plugin-to-plugin calls are not supported — all interaction goes through Core.

- **Workflows** — Plugins chained in a workflow receive the output `PipelineMessage` of the previous step as input. This is the primary communication mechanism. Output-to-input chaining is declared in the workflow YAML.
- **Shared canonical collections** — Two plugins reading/writing the same canonical collection (e.g., both work with `events`). Both must hold the appropriate `storage:read` or `storage:write` capability.

## Connector Plugins

A connector is a regular plugin that declares `http:outbound` and `storage:write` capabilities. It fetches data from an external service, normalises it to CDM types, and writes to canonical collections. There is no special trait or category — connectors are just plugins that happen to talk to external APIs.

First-party connectors:

- **Email** — IMAP/SMTP, writes to `emails` canonical collection
- **Calendar** — CalDAV, writes to `events` canonical collection
- **Contacts** — CardDAV, writes to `contacts` canonical collection
- **Filesystem** — WebDAV/S3, writes to `files` canonical collection

## Community Plugins

Third-party authors create an independent repo, add `life-engine-plugin-sdk` as a dependency, implement the `Plugin` trait, compile to WASM, and distribute. No monorepo forking or internal tooling knowledge required.

To install a community plugin:

1. Drop the plugin directory (containing `plugin.wasm` and `manifest.toml`) into Core's configured plugins path.
2. Approve the plugin's capabilities in `config.toml`.
3. Restart Core (or trigger a plugin reload if supported).

If the manifest declares any capability not in the approved list, Core refuses to load the plugin.

## Acceptance Criteria

- Core scans the configured plugins directory and discovers plugins by the presence of `plugin.wasm` + `manifest.toml`.
- `manifest.toml` is parsed for plugin identity, actions, capabilities, and config schema.
- First-party plugin capabilities are auto-granted; third-party capabilities require explicit approval.
- Unapproved capabilities cause Core to refuse loading the plugin.
- Plugins load into isolated Extism WASM instances with no shared memory.
- Host functions are gated by the plugin's approved capability set.
- The workflow engine invokes `execute(action, PipelineMessage)` and receives a `PipelineMessage` response.
- A failing plugin cannot crash Core — the WASM sandbox contains the failure.
- Plugin-to-plugin communication works via workflow chaining and shared canonical collections only. No direct calls.
- The six-phase lifecycle (Discover, Load, Init, Running, Stop, Unload) is enforced in order.
