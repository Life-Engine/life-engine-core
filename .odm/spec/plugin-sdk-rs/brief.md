<!--
domain: sdk
status: draft
tier: 1
updated: 2026-03-22
-->

# Plugin SDK RS Spec

## Overview

This spec defines the Rust SDK for Core plugin authors. The SDK provides the trait definitions, types, and helpers that plugin authors need to implement a Core plugin, compile it to WASM, and load it into the Core runtime.

## Goals

- Ergonomic authoring — Plugin authors implement a single trait and compile to WASM with no boilerplate.
- Type safety — Canonical collection types ship as Rust structs with serde derives.
- Isolation — The SDK has no dependency on Core internals; plugins depend only on the public crate.
- Versioning — Major-version compatibility windows let authors upgrade at their own pace.

## User Stories

- As a plugin author, I want to implement the `CorePlugin` trait so that Core can load and manage my plugin.
- As a plugin author, I want a builder pattern so that I can configure my plugin without repetitive code.
- As a plugin author, I want typed canonical structs so that I can read and write platform data safely.
- As a plugin author, I want to declare capabilities so that Core grants only the permissions I need.
- As a plugin author, I want to compile to `wasm32-wasi` so that my plugin runs in the Core sandbox.

## Functional Requirements

- The SDK must expose a `CorePlugin` trait with lifecycle, route, and event methods.
- The SDK must provide a `PluginContext` struct granting scoped access to storage, config, events, and logging.
- The SDK must define a `Capability` enum covering all permission types.
- The SDK must support route registration under the plugin namespace.
- The SDK must include serde-ready Rust structs for all 7 canonical collection types.
- The SDK must provide a builder pattern for constructing plugin instances.
- The SDK must compile to the `wasm32-wasi` target.
- The SDK must be versioned independently from Core with additive-only minor releases.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
