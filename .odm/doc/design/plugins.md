---
title: Plugin System
type: reference
created: 2026-03-14
updated: 2026-03-28
status: active
tags:
  - life-engine
  - core
  - plugins
  - wasm
  - extism
---

# Plugin System

Part of [[architecture/core/overview|Core Overview]] · [[architecture/core/README|Core Documentation]]

All Core features are provided by plugins. Core itself is a thin orchestrator — it loads plugins, gives them scoped access to storage, and enforces isolation via WASM sandboxing.

Plugins are WASM modules loaded at runtime via Extism. Core does not compile against any plugin — adding or removing a plugin never changes the Core binary.

## Isolation Model

- **Memory-isolated** — Each plugin runs in its own WASM sandbox
- **Language-agnostic** — Any language that compiles to WASM (Rust, Go, C, AssemblyScript)
- **Host functions** — Plugins can only call what the host explicitly exports
- **No direct I/O** — All filesystem, network, and OS access goes through host functions
- **Crash isolation** — A failing plugin cannot crash Core

## Plugin Structure

Each plugin is a directory containing a WASM binary and a manifest:

```
plugins/
  connector-email/
    plugin.wasm
    manifest.toml
```

The manifest declares metadata, actions, capabilities, collection access, and config. Core reads manifests at startup and grants or denies capabilities.

## Lifecycle

```
Discover → Load → Init → Running → Stop → Unload
```

Plugins are discovered by scanning a configured directory at startup. The lifecycle is managed entirely by Core.

## Capabilities

Deny-by-default. Plugins declare what they need in their manifest:

- `storage:doc:read`, `storage:doc:write`, `storage:doc:delete` — Document storage access
- `storage:blob:read`, `storage:blob:write`, `storage:blob:delete` — Blob storage access
- `http:outbound` — Outbound HTTP requests
- `events:emit`, `events:subscribe` — Event bus access
- `config:read` — Read own config section

First-party plugins have capabilities auto-granted. Third-party plugins require explicit approval in Core config.

## Standard Contract

Every plugin action receives and returns a `PipelineMessage` (metadata + JSON payload). This standard contract makes plugins composable — any plugin's output can be another plugin's input, as long as the schemas are compatible.

## Communication

Plugins communicate through two mechanisms (no direct plugin-to-plugin calls):

- **Workflows** — Plugins chained in a workflow receive the output of the previous step
- **Shared collections** — Multiple plugins reading/writing the same collection (e.g., both adopt the CDM `events` schema)

## Connectors

A connector is a regular plugin that fetches data from an external service, normalises it to CDM recommended schemas, and writes to shared collections. First-party connectors: Email (IMAP/SMTP), Calendar (CalDAV), Contacts (CardDAV), Filesystem (WebDAV/S3).

For the full SDK contract and host function reference, see the [[architecture/core/design/plugin-sdk/outline|Plugin SDK]] documentation.
