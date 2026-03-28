<!--
domain: plugin-actions
updated: 2026-03-28
-->

# Plugin Actions Spec

## Overview

This spec defines how plugins expose named actions that the workflow engine invokes as pipeline steps. An action is a Rust function annotated with `#[plugin_action]` that receives a `PipelineMessage` and a `PluginContext`, performs work, and returns a modified `PipelineMessage`. The spec also covers lifecycle hooks (`init` and `shutdown`), per-action timeouts, error handling, and the connector pattern for external data synchronisation.

Actions are declared in the plugin manifest, compiled into the `.wasm` module, and invoked by the Extism host at runtime. The `#[plugin_action]` attribute macro hides all WASM boundary mechanics so plugin authors write plain Rust functions.

## Goals

- Ergonomic action authoring — plugin authors write plain Rust functions; the `#[plugin_action]` macro handles serialisation, deserialisation, and Extism export boilerplate
- Typed host access — `PluginContext` provides typed clients for storage, events, config, and HTTP so plugins never call raw host functions directly
- Lifecycle management — optional `init` and `shutdown` hooks let plugins perform setup and teardown outside of workflow execution
- Configurable timeouts — each action declares a `timeout_ms` in the manifest; the Extism host enforces it at the WASM execution level
- Structured error handling — hard failures via `PluginError` and soft warnings via `PipelineMessage.metadata.warnings` give workflows clear signals for error strategies
- Connector uniformity — a documented connector pattern ensures all data-fetching plugins follow the same fetch-normalise-store-emit sequence

## User Stories

- As a plugin author, I want to annotate a function with `#[plugin_action]` so that it becomes a callable pipeline step without writing Extism boilerplate.
- As a plugin author, I want typed access to storage, events, config, and HTTP through `PluginContext` so that I can interact with the host safely.
- As a plugin author, I want to declare `init` and `shutdown` hooks so that my plugin can validate config on load and flush state on teardown.
- As a workflow author, I want per-action timeouts enforced by the host so that a misbehaving plugin cannot block the pipeline indefinitely.
- As a workflow author, I want actions to report hard errors and soft warnings so that the pipeline executor can apply the correct `on_error` strategy.
- As a connector developer, I want a standard fetch-normalise-store-emit pattern so that all connectors integrate uniformly with the workflow engine.

## Functional Requirements Summary

- The system must provide a `#[plugin_action]` attribute macro that generates Extism export boilerplate, deserialises `PipelineMessage` from JSON, constructs `PluginContext`, serialises the return value, and maps errors.
- The system must define `PluginContext` with typed clients: `StorageClient`, `EventClient`, `ConfigClient`, and `HttpClient`.
- The system must support optional `init` and `shutdown` lifecycle hooks via `#[plugin_hook]`, each receiving only `PluginContext` and returning `Result<(), PluginError>`.
- The system must fail plugin loading when `init` returns an error and log the failure.
- The system must enforce per-action `timeout_ms` from the manifest, falling back to an engine-level default when omitted.
- The system must terminate WASM execution, mark the step as failed, and apply the workflow's `on_error` strategy when a timeout is exceeded.
- The system must define `PluginError` variants including `CapabilityDenied`, `NotFound`, `ValidationError`, `StorageError`, `NetworkError`, and `InternalError`.
- The system must catch panics inside `#[plugin_action]` and convert them to `InternalError`.
- The system must support soft warnings via `PipelineMessage.metadata.warnings` for non-fatal degradation signals.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
