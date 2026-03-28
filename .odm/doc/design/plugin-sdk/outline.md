---
title: Plugin SDK Outline
type: adr
created: 2026-03-28
status: draft
tags:
  - architecture
  - plugin-sdk
  - core
---

# Plugin SDK Outline

## Scope

The Plugin SDK is the developer-facing toolkit for building workflow plugins — WASM modules that run as steps in Life Engine workflows. The SDK provides language-specific bindings, the manifest schema, CDM recommended schemas, and the types needed to produce and consume `PipelineMessage` values.

Plugin authors never interact with Core internals. They receive a `PipelineMessage`, call host functions for storage and event access, and return a `PipelineMessage`. The SDK is the contract between the plugin and the host.

## What the SDK Provides

- **Language bindings** — Idiomatic wrappers around host function imports and Extism guest exports. Rust is first-class; Go, AssemblyScript, and C are supported through the same WASM interface.
- **PipelineMessage types** — Serialisable structs for the message envelope, metadata, and status hints.
- **Host function stubs** — Typed signatures for every host function (storage, events, config, HTTP). The SDK generates the correct WASM import declarations.
- **Manifest schema** — A TOML schema definition and validation tool for `manifest.toml`.
- **CDM recommended schemas** — JSON Schema files for common collections (events, tasks, contacts, notes, emails, credentials). Plugins can adopt these by referencing `cdm:<name>` in their manifest.
- **Build tooling** — A CLI helper that compiles the plugin to WASM, validates the manifest, and packages the output directory.

## What the SDK Does Not Provide

- **Runtime** — The SDK is a compile-time dependency. Core provides the runtime (Extism host, StorageContext, event bus).
- **Direct adapter access** — Plugins interact with storage through host functions only.
- **Plugin-to-plugin calls** — Not supported. Plugins communicate through workflows and shared collections.
- **Transport awareness** — Plugins never see HTTP, GraphQL, or any wire format.

## SDK Components

Each component is documented separately:

- [[manifest]] — Full `manifest.toml` specification
- [[pipeline-message]] — `PipelineMessage` shape, metadata, and status hints
- [[host-functions]] — Complete host function reference
- [[plugin-actions]] — Action signatures, lifecycle hooks, and configuration access

## Language Support

The WASM interface is language-agnostic. Any language that compiles to WASM and supports Extism's PDK (Plugin Development Kit) can build a plugin.

First-class SDK support (typed bindings, idiomatic wrappers):

- **Rust** — Via `life-engine-plugin-sdk` crate. Zero-cost abstractions over the raw host functions.

Community-supported (raw host function imports, manual serialisation):

- **Go** — Via Extism Go PDK with Life Engine host function declarations.
- **AssemblyScript** — Via Extism AS PDK.
- **C/C++** — Via Extism C PDK.

The Rust SDK is the reference implementation. Other language SDKs follow the same contract and produce identical WASM imports.

## Plugin Directory Structure

A built plugin is a directory containing:

```
connector-email/
  plugin.wasm        # compiled WASM binary
  manifest.toml      # plugin metadata, actions, capabilities, collections
  schemas/           # optional JSON Schema files for custom collections
    email-thread.json
```

Core discovers plugins by scanning the configured plugin directory at startup.

## Design Principles

- **Plugins are black boxes** — Core calls actions and receives output. It does not inspect plugin internals.
- **Deny-by-default** — Every capability must be declared and granted. Undeclared access is rejected at runtime.
- **Composable** — Any plugin's output can be another plugin's input through the `PipelineMessage` contract.
- **Crash-safe** — A failing plugin cannot crash Core. WASM sandboxing guarantees isolation.
- **Deterministic builds** — The same source and SDK version produce the same WASM binary.
