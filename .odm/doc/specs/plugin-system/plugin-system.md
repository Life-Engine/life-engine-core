---
title: Plugin System Specification
type: reference
created: 2026-03-28
status: active
tags:
  - spec
  - plugin
  - wasm
---

# Plugin System Specification

The plugin system extends Core with isolated, language-agnostic WASM modules loaded at runtime via Extism. Every plugin runs in its own sandbox and communicates with Core exclusively through [[host-functions|host functions]] and the [[pipeline-message|PipelineMessage]] contract.

## Isolation Model

- Each plugin compiles to a single `.wasm` module and runs in its own memory-isolated sandbox.
- Extism manages module instantiation, memory, and host function dispatch.
- Any language that compiles to WASM is supported: Rust, Go, C, AssemblyScript, and others.
- A plugin can only call host functions that Core explicitly exports. No other system access is available.
- All filesystem, network, and OS access must go through host functions. Direct I/O is prohibited.
- A crashing plugin cannot bring down Core. Extism traps WASM faults and Core handles them as step failures.

## Plugin Structure

Every plugin ships as a directory containing:

```
plugins/
  connector-email/
    plugin.wasm
    manifest.toml
    schemas/
      email-thread.json
```

- `plugin.wasm` — The compiled WASM module.
- `manifest.toml` — Declares identity, actions, capabilities, collections, events, and config. See [[plugin-manifest]].
- `schemas/` — Optional directory for plugin-specific JSON Schemas referenced by the manifest.

## Lifecycle

A plugin passes through six states in order:

```
Discover → Load → Init → Running → Stop → Unload
```

- **Discover** — Core scans the configured plugin directory at startup and locates directories containing a `manifest.toml`.
- **Load** — Core validates the manifest, checks capability grants, and instantiates the WASM module via Extism.
- **Init** — Core calls the optional `init` action if declared. The plugin performs setup, connection validation, or cache warming.
- **Running** — The plugin is available for workflow invocation. Actions are called as workflow steps.
- **Stop** — Core calls the optional `shutdown` action if declared. The plugin flushes buffers and releases resources.
- **Unload** — Core deallocates the WASM instance and removes the plugin from the registry.

Core manages the full lifecycle. Plugins do not control their own state transitions.

## Capabilities

All capabilities are deny-by-default. A plugin must declare required capabilities in its [[plugin-manifest|manifest]]:

- `storage:doc:read` — Read documents from declared collections
- `storage:doc:write` — Create and update documents in declared collections
- `storage:doc:delete` — Delete documents from declared collections
- `storage:blob:read` — Retrieve blobs from plugin-scoped storage
- `storage:blob:write` — Store blobs in plugin-scoped storage
- `storage:blob:delete` — Delete blobs from plugin-scoped storage
- `http:outbound` — Make outbound HTTP requests
- `events:emit` — Emit events to the [[event-bus]]
- `events:subscribe` — Subscribe to events via workflow triggers
- `config:read` — Read the plugin's runtime configuration

Trust model:

- **First-party plugins** — Capabilities are auto-granted at load time.
- **Third-party plugins** — Capabilities require explicit approval in Core configuration. Unapproved capabilities cause load failure.

## Standard Contract

Every [[plugin-actions|action]] receives a [[pipeline-message|PipelineMessage]] and returns a modified `PipelineMessage`. This uniform interface makes plugins composable: any action can appear as a step in any [[workflow-engine-contract|workflow]].

## Communication

Plugins communicate through exactly two mechanisms. Direct plugin-to-plugin calls are not permitted.

- **Workflows** — Actions are chained as steps in a workflow definition. The output `PipelineMessage` of one step becomes the input of the next.
- **Shared collections** — Multiple plugins declare access to the same collection in their manifests. They read and write documents through [[host-functions|storage host functions]].

## Connectors

A connector is a regular plugin whose purpose is to synchronise data with an external service. It follows the [[plugin-actions#Connector Pattern|connector pattern]]:

1. Read configuration (server details, credentials) via `config:read`
2. Fetch data from the external service via `http:outbound`
3. Normalise fetched data to [[cdm-specification|CDM]] schemas
4. Write normalised documents to shared collections via storage host functions
5. Emit a completion event (e.g., `connector-email.fetch.completed`)

First-party connectors shipping with v1:

- **Email** — IMAP for retrieval, SMTP for sending
- **Calendar** — CalDAV sync
- **Contacts** — CardDAV sync
- **Filesystem** — WebDAV and S3-compatible object storage
