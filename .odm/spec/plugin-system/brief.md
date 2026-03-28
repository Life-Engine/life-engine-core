<!--
domain: plugin-system
status: draft
tier: 1
updated: 2026-03-28
-->

# Plugin System Spec

## Overview

The plugin system extends Life Engine Core with isolated, language-agnostic WASM modules loaded at runtime via Extism. Every plugin runs in its own memory-isolated sandbox and communicates with Core exclusively through host functions and the PipelineMessage contract. Core is a thin orchestrator — it loads plugins, gives them scoped access to storage, enforces capabilities, and manages the full plugin lifecycle. Adding or removing a plugin never changes the Core binary.

Plugins ship as directories containing a compiled `.wasm` module, a `manifest.toml` declaring identity, actions, capabilities, collections, events, and configuration, and optional JSON Schema files. Core validates manifests at load time and rejects plugins with invalid or incomplete declarations.

## Goals

- Memory-isolated execution — each plugin runs in its own WASM sandbox via Extism, preventing one plugin from affecting another or crashing Core
- Language-agnostic — any language that compiles to WASM is supported (Rust, Go, C, AssemblyScript)
- Deny-by-default capabilities — plugins declare what they need and Core grants or denies at load time based on trust level
- Uniform action contract — every action receives and returns a PipelineMessage, making plugins composable within workflows
- Managed lifecycle — Core controls all state transitions (Discover, Load, Init, Running, Stop, Unload); plugins never control their own lifecycle
- First-party and third-party trust model — first-party capabilities are auto-granted; third-party capabilities require explicit approval
- Connector pattern — standardised approach for plugins that synchronise data from external services

## User Stories

- As a plugin author, I want to write actions in any WASM-compatible language so that I am not locked into a single technology.
- As a plugin author, I want to declare capabilities in my manifest so that the system enforces least-privilege access to host functions.
- As a plugin author, I want to declare collections in my manifest so that Core scopes my storage access to only what I need.
- As a plugin author, I want lifecycle hooks (init, shutdown) so that my plugin can perform setup and cleanup.
- As a workflow author, I want all plugins to use the same PipelineMessage contract so that I can chain any action as a workflow step.
- As a connector author, I want to follow a standard pattern (config, fetch, normalise, store, emit) so that external data sync is consistent.
- As an administrator, I want third-party plugins to require explicit capability approval so that untrusted code cannot access sensitive resources.
- As a maintainer, I want Core to handle plugin crashes gracefully so that a failing plugin does not bring down the engine.
- As a maintainer, I want manifest validation at load time so that misconfigured plugins are rejected with clear error messages.

## Functional Requirements

- Core discovers plugins by scanning a configured directory for subdirectories containing a `manifest.toml`.
- Core validates every manifest at load time, checking required fields, action declarations, schema references, event naming conventions, and capability declarations.
- Core instantiates each plugin as a WASM module via Extism with memory isolation.
- Core manages six lifecycle states: Discover, Load, Init, Running, Stop, Unload.
- Core calls the optional `init` hook after module instantiation and the optional `shutdown` hook before unloading.
- Core enforces deny-by-default capabilities — plugins can only call host functions for which they have declared and been granted the corresponding capability.
- Core auto-grants capabilities for first-party plugins and requires explicit approval in configuration for third-party plugins.
- Core exports host functions for document storage, blob storage, event emission, configuration reading, and outbound HTTP.
- Every action receives a PipelineMessage and returns a modified PipelineMessage.
- Core enforces action timeouts at the WASM execution level via Extism.
- A crashing plugin is trapped by Extism and handled as a step failure — it cannot crash Core.
- Plugins communicate only through workflows (PipelineMessage chaining) and shared collections — direct plugin-to-plugin calls are prohibited.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
