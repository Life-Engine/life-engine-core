---
title: "Core — Plugin System"
tags: [life-engine, core, plugins, wasm, extism]
created: 2026-03-14
updated: 2026-03-23
---

# Plugin System

All Core features are provided by plugins. Core itself is a thin orchestrator — it loads plugins, gives them scoped access to storage, and enforces isolation via WASM sandboxing.

Plugins are WASM modules loaded at runtime. Core does not compile against any plugin — adding or removing a plugin never changes the Core binary.

## WASM Isolation via Extism

Plugins run as WebAssembly modules using Extism:

- **Memory-isolated** — Each plugin runs in its own WASM sandbox. No shared memory with the host or other plugins.
- **Language-agnostic** — Plugins can be written in Rust, Go, C, AssemblyScript, or any language that compiles to WASM.
- **Host functions** — Core explicitly exports functions to plugins. Plugins can only call what the host exposes.
- **No direct I/O** — Plugins cannot access the filesystem, network, or OS directly. All access goes through host functions.
- **Crash isolation** — A failing plugin cannot crash Core.

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

Plugins declare the actions they provide. Each action is a step that can be used in a workflow pipeline. Every action receives a `PipelineMessage` and returns a `PipelineMessage`.

## Plugin Discovery

Core scans a configured directory at startup. Each plugin is a directory containing a WASM binary and a manifest:

```
plugins/
  connector-email/
    plugin.wasm       <- Compiled WASM module
    manifest.toml     <- Plugin metadata, actions, config, capabilities
  connector-calendar/
    plugin.wasm
    manifest.toml
```

The manifest declares everything Core needs to know about the plugin:

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

## Plugin Lifecycle

```
Discover -> Load -> Init -> Running -> Stop -> Unload
```

1. **Discover** — Core scans the plugins directory, reads each `manifest.toml`
2. **Load** — WASM module loaded into Extism runtime
3. **Init** — Plugin receives its config and scoped capabilities
4. **Running** — Plugin actions are available to workflows
5. **Stop** — Cleanup signal sent to plugin
6. **Unload** — WASM module released

## Capabilities

Deny-by-default. Plugins declare what they need in their manifest. Core grants or denies at load time.

Available capabilities:

- `storage:read` — Read from collections via StorageContext
- `storage:write` — Write to collections via StorageContext
- `http:outbound` — Make outbound HTTP requests
- `events:emit` — Emit events into the workflow engine
- `events:subscribe` — Listen for events
- `config:read` — Read own config section

All capabilities are enforced at runtime by the WASM host. A plugin that calls a host function it wasn't granted receives an error.

### Approval Policy

- **First-party plugins** (in the monorepo) — capabilities auto-granted. These are trusted.
- **Third-party plugins** — capabilities must be explicitly approved in Core config:

```toml
[plugins.some-third-party]
approved_capabilities = ["storage:read", "http:outbound"]
```

If a manifest declares a capability not in the approved list, Core refuses to load the plugin.

## Standard Input/Output

Every plugin action receives and returns a `PipelineMessage`:

```rust
struct PipelineMessage {
    metadata: MessageMetadata,
    payload: TypedPayload,
}
```

`TypedPayload` is either a CDM type (one of the 7 canonical collection types) or a custom type validated against a JSON Schema declared in the plugin manifest. See Data.md for details.

This standard contract is what makes plugins composable — any plugin's output can be another plugin's input, as long as the schemas are compatible.

## Host Functions

Functions Core exposes to WASM plugins (gated by capabilities):

- **Storage** — Read/write via StorageContext (query builder API)
- **Config** — Read plugin-specific configuration
- **Events** — Emit events for workflows to consume
- **HTTP (scoped)** — Make outbound requests
- **Logging** — Structured logging via the host

Plugins cannot bypass these — WASM provides no other path to the host.

## Plugin-to-Plugin Communication

Plugins communicate through two mechanisms. Direct plugin-to-plugin calls are not supported — all interaction goes through the host.

- **Workflows** — Plugins chained in a workflow receive the output of the previous step as input. This is the primary communication mechanism.
- **Shared canonical collections** — Two plugins reading/writing the same canonical collection (e.g., both work with `events`).

## Connector Plugins

A connector is a regular plugin that declares `http:outbound` and `storage:write` capabilities. It fetches data from an external service, normalises it to CDM types, and writes to canonical collections. There is no special trait or category — connectors are just plugins that happen to talk to external APIs.

First-party connectors:

- **Email** — IMAP/SMTP, writes to `emails` canonical collection
- **Calendar** — CalDAV, writes to `events` canonical collection
- **Contacts** — CardDAV, writes to `contacts` canonical collection
- **Filesystem** — WebDAV/S3, writes to `files` canonical collection

## SDK Contract

Plugin authors depend on a single crate: `life-engine-plugin-sdk`.

The SDK provides:

- Re-exports of `Plugin` trait, CDM types, and `PipelineMessage`
- `StorageContext` query builder for storage interactions
- Helper macros for plugin registration boilerplate
- Test utilities (mock `StorageContext`, mock `PipelineMessage` builders)

Plugin authors never directly depend on `packages/types` or `packages/traits` — the SDK re-exports everything they need.

Versioning:

- Versioned independently from Core
- Additive only (new methods, new optional interfaces) for minor versions
- Breaking changes require major versions with 12-month support overlap

## Community Plugins

Third-party authors create an independent repo, add `life-engine-plugin-sdk`, implement `Plugin`, compile to WASM, and distribute. No monorepo forking or internal tooling knowledge required. Drop the plugin directory into Core's plugins path, approve capabilities in config, and it runs.
