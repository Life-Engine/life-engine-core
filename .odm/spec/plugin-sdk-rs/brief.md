<!--
domain: sdk
status: draft
tier: 1
updated: 2026-03-23
-->

# Plugin SDK RS Spec

## Overview

This spec defines the Rust SDK (`life-engine-plugin-sdk`) for Life Engine plugin authors. The SDK is the single dependency a plugin author adds to their `Cargo.toml`. It re-exports everything from `packages/types` (CDM types, `PipelineMessage`, envelopes) and `packages/traits` (`Plugin` trait, `EngineError` trait), and provides additional DX features: a `StorageContext` query builder, helper macros for registration, and test utilities.

Plugin authors never directly depend on `packages/types` or `packages/traits`. Internal module developers (building storage backends, transports) depend on `types` + `traits` directly.

## Goals

- **Single dependency** — Plugin authors add one crate and get everything they need.
- **Type safety** — CDM types, `PipelineMessage`, and `EngineError` are fully typed Rust structs with serde derives.
- **Isolation** — The SDK has no dependency on Core internals. Plugins depend only on the public SDK crate.
- **Ergonomic authoring** — Implement the `Plugin` trait, declare actions, compile to WASM. No boilerplate.
- **Testability** — Mock `StorageContext` and `PipelineMessage` builders ship in the SDK for unit testing.
- **Versioning** — Major-version compatibility windows let authors upgrade at their own pace.

## User Stories

- As a plugin author, I want to implement the `Plugin` trait so that Core can load and manage my plugin.
- As a plugin author, I want typed CDM structs and `PipelineMessage` envelopes so that I can read and write platform data safely.
- As a plugin author, I want a `StorageContext` query builder so that I can read and write collections without importing database crates.
- As a plugin author, I want to declare actions so that workflows can invoke my plugin's functionality.
- As a plugin author, I want to compile to `wasm32-wasi` so that my plugin runs in the Core sandbox.
- As a plugin author, I want mock `StorageContext` and `PipelineMessage` builders so that I can unit-test my plugin without a running Core.
- As a plugin author, I want helper macros so that plugin registration requires minimal boilerplate.

## Functional Requirements

- The SDK must re-export all public types from `packages/types` (CDM types, `PipelineMessage`, `TypedPayload`, `MessageMetadata`).
- The SDK must re-export the `Plugin` trait and `EngineError` trait from `packages/traits`.
- The SDK must provide a `StorageContext` with a fluent query builder API.
- The SDK must provide helper macros for plugin registration boilerplate.
- The SDK must provide test utilities: mock `StorageContext` and mock `PipelineMessage` builders.
- The SDK must compile to the `wasm32-wasi` target.
- The SDK must be versioned independently from Core with additive-only minor releases.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
