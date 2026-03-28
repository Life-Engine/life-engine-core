<!--
domain: plugin-system
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Requirements Document — Plugin System

## Introduction

The plugin system is the primary extension mechanism for Life Engine Core. Plugins are WASM modules loaded at runtime via Extism. Core acts as a thin orchestrator that discovers, validates, loads, and manages plugins through a defined lifecycle. All plugin interactions with Core pass through host functions gated by a deny-by-default capability system. Every plugin action follows the PipelineMessage contract, enabling composability within the workflow engine.

This document covers plugin structure, manifest validation, lifecycle management, capability enforcement, host function dispatch, action execution, error handling, and the connector pattern.

## Alignment with Product Vision

- **Defence in Depth** — WASM sandboxing, deny-by-default capabilities, and manifest validation create multiple layers of isolation
- **Parse, Don't Validate** — Manifests are fully validated at load time; by the time a plugin reaches the Running state, all declarations are known-good
- **Principle of Least Privilege** — Plugins declare only the capabilities they need; Core grants the minimum required access
- **Open/Closed Principle** — New plugins extend Core without modifying the Core binary; the plugin interface is stable
- **The Pit of Success** — The SDK macro system (`#[plugin_action]`, `#[plugin_hook]`) makes the correct implementation path the easiest path
- **Crash Isolation** — Extism traps WASM faults so a failing plugin cannot bring down the engine

## Requirements

### Requirement 1 — Plugin Directory Structure

**User Story:** As a plugin author, I want a clear directory structure so that I know exactly what files to ship and where to place them.

#### Acceptance Criteria

- 1.1. WHEN a plugin is packaged THEN it SHALL be a directory containing at minimum a `plugin.wasm` file and a `manifest.toml` file.
- 1.2. WHEN a plugin declares collection schemas THEN it SHALL include a `schemas/` subdirectory containing the referenced JSON Schema files.
- 1.3. WHEN a plugin declares a config schema THEN the referenced schema file SHALL exist at the path specified in `[config].schema`.

### Requirement 2 — Manifest Validation

**User Story:** As a maintainer, I want manifests validated at load time so that misconfigured plugins are rejected with clear error messages before they can execute.

#### Acceptance Criteria

- 2.1. WHEN Core loads a plugin THEN it SHALL validate that the `[plugin]` section contains non-empty `id`, `name`, and `version` fields.
- 2.2. WHEN Core loads a plugin THEN it SHALL validate that at least one action is declared in the `[actions]` section.
- 2.3. WHEN Core loads a plugin THEN it SHALL verify that each declared action references a valid WASM export in the module.
- 2.4. WHEN a collection declares a `schema` with a `cdm:` prefix THEN Core SHALL resolve it to an SDK-shipped schema file and reject the plugin if the schema does not exist.
- 2.5. WHEN a collection declares a local schema path THEN Core SHALL verify the file exists and is valid JSON Schema.
- 2.6. WHEN events are declared in `[events.emit]` or `[events.subscribe]` THEN Core SHALL validate that each event name follows the dot-separated naming convention (`<plugin-id>.<action>.<outcome>`).
- 2.7. WHEN a manifest contains unknown top-level sections THEN Core SHALL reject the manifest with an error identifying the unknown section.
- 2.8. WHEN manifest validation fails THEN Core SHALL log the specific validation error and abort loading that plugin without affecting other plugins.

### Requirement 3 — Plugin Lifecycle

**User Story:** As a maintainer, I want Core to manage the full plugin lifecycle so that plugins are loaded, initialised, and shut down in a predictable order.

#### Acceptance Criteria

- 3.1. WHEN Core starts THEN it SHALL scan the configured plugin directory and discover all subdirectories containing a `manifest.toml` (Discover phase).
- 3.2. WHEN a plugin is discovered THEN Core SHALL validate the manifest and instantiate the WASM module via Extism (Load phase).
- 3.3. WHEN a plugin declares an `init` action THEN Core SHALL call it immediately after module instantiation (Init phase).
- 3.4. WHEN `init` returns an error THEN Core SHALL fail the plugin load and log the error.
- 3.5. WHEN a plugin completes Init successfully THEN it SHALL enter the Running state and be available for workflow invocation.
- 3.6. WHEN Core shuts down THEN it SHALL call the `shutdown` action on each plugin that declares one (Stop phase).
- 3.7. WHEN shutdown completes THEN Core SHALL deallocate the WASM instance and remove the plugin from the registry (Unload phase).
- 3.8. WHEN Core manages lifecycle transitions THEN plugins SHALL NOT control their own state transitions.

### Requirement 4 — WASM Isolation

**User Story:** As a maintainer, I want each plugin to run in its own WASM sandbox so that a crashing or misbehaving plugin cannot affect Core or other plugins.

#### Acceptance Criteria

- 4.1. WHEN a plugin is loaded THEN it SHALL run in its own memory-isolated WASM sandbox managed by Extism.
- 4.2. WHEN a plugin crashes (WASM fault) THEN Extism SHALL trap the fault and Core SHALL handle it as a step failure.
- 4.3. WHEN a plugin attempts direct filesystem, network, or OS access THEN the WASM sandbox SHALL prevent it — all such access must go through host functions.
- 4.4. WHEN a plugin is instantiated THEN it SHALL have no access to another plugin's memory space.

### Requirement 5 — Capability Enforcement

**User Story:** As an administrator, I want deny-by-default capabilities so that plugins can only access resources they have been explicitly granted.

#### Acceptance Criteria

- 5.1. WHEN a plugin declares capabilities in `[capabilities]` THEN Core SHALL record the requested capabilities at load time.
- 5.2. WHEN a plugin calls a host function without the required capability THEN Core SHALL return `CapabilityDenied`.
- 5.3. WHEN a plugin accesses a collection not declared in its `[collections]` section THEN Core SHALL return `CapabilityDenied`.
- 5.4. WHEN a plugin emits an event not declared in `[events.emit]` THEN Core SHALL return `CapabilityDenied`.
- 5.5. WHEN a capability is not declared in the manifest THEN it SHALL default to denied.
- 5.6. WHEN a first-party plugin is loaded THEN all declared capabilities SHALL be auto-granted.
- 5.7. WHEN a third-party plugin is loaded THEN each declared capability SHALL require explicit approval in Core's configuration file.
- 5.8. WHEN a third-party plugin has any unapproved capability THEN Core SHALL fail the plugin load with an error listing the unapproved capabilities.

### Requirement 6 — Host Function Dispatch

**User Story:** As a plugin author, I want typed host functions so that I can interact with Core services (storage, events, config, HTTP) through a well-defined API.

#### Acceptance Criteria

- 6.1. WHEN Core exports host functions THEN it SHALL provide document storage functions: `storage_doc_get`, `storage_doc_list`, `storage_doc_count`, `storage_doc_create`, `storage_doc_update`, `storage_doc_partial_update`, `storage_doc_delete`, `storage_doc_batch_create`, `storage_doc_batch_update`, `storage_doc_batch_delete`.
- 6.2. WHEN Core exports host functions THEN it SHALL provide blob storage functions: `storage_blob_store`, `storage_blob_retrieve`, `storage_blob_exists`, `storage_blob_list`, `storage_blob_metadata`, `storage_blob_delete`.
- 6.3. WHEN Core exports host functions THEN it SHALL provide `emit_event` for event emission, `config_read` for plugin configuration, and `http_request` for outbound HTTP.
- 6.4. WHEN a host function is called THEN Core SHALL validate the calling plugin's capabilities before executing the operation.
- 6.5. WHEN blob storage functions are called THEN Core SHALL automatically prefix keys with the calling plugin's ID for scope isolation.
- 6.6. WHEN a host function fails THEN it SHALL return a typed `PluginError` (one of `CapabilityDenied`, `NotFound`, `ValidationError`, `StorageError`, `NetworkError`, `InternalError`).

### Requirement 7 — Action Execution

**User Story:** As a workflow author, I want every plugin action to follow the same input/output contract so that actions are composable as workflow steps.

#### Acceptance Criteria

- 7.1. WHEN a workflow step invokes a plugin action THEN the action SHALL receive a `PipelineMessage` as input and return a modified `PipelineMessage` as output.
- 7.2. WHEN the `PipelineMessage` crosses the WASM boundary THEN it SHALL be serialised as JSON; the SDK handles deserialisation on entry and serialisation on exit.
- 7.3. WHEN an action modifies the message THEN it SHALL only write to `payload`, `status_hint`, `warnings`, and `extra` — the fields `request_id`, `trigger_type`, `identity`, `params`, `query`, and `traces` are read-only.
- 7.4. WHEN an action returns `Err(PluginError)` THEN the step SHALL be marked as failed and the executor SHALL apply the workflow's `on_error` strategy.
- 7.5. WHEN an action returns `Ok(msg)` with entries in `msg.metadata.warnings` THEN the step SHALL succeed with non-fatal warnings recorded.
- 7.6. WHEN a plugin panics THEN the `#[plugin_action]` macro SHALL catch the panic and convert it to `InternalError`.

### Requirement 8 — Action Timeouts

**User Story:** As a maintainer, I want action timeouts enforced so that a hung plugin does not block the workflow engine indefinitely.

#### Acceptance Criteria

- 8.1. WHEN an action declares `timeout_ms` in the manifest THEN Extism SHALL enforce that timeout at the WASM execution level.
- 8.2. WHEN an action exceeds its timeout THEN Extism SHALL terminate execution and the step SHALL be marked as failed.
- 8.3. WHEN an action omits `timeout_ms` THEN Core SHALL apply a default timeout defined in the engine configuration.

### Requirement 9 — Plugin Communication

**User Story:** As a plugin author, I want clear communication boundaries so that I understand how my plugin interacts with other plugins.

#### Acceptance Criteria

- 9.1. WHEN plugins communicate THEN they SHALL do so only through workflows (PipelineMessage chaining) or shared collections — direct plugin-to-plugin calls are prohibited.
- 9.2. WHEN multiple plugins declare access to the same collection THEN they SHALL read and write documents through storage host functions.
- 9.3. WHEN a workflow chains actions THEN the output PipelineMessage of one step SHALL become the input of the next step.

### Requirement 10 — Connector Pattern

**User Story:** As a connector author, I want a standard pattern for synchronising external data so that all connectors behave uniformly.

#### Acceptance Criteria

- 10.1. WHEN a connector plugin fetches data THEN it SHALL follow the sequence: read config via `config_read`, fetch via `http_request`, normalise to CDM schemas, write via storage host functions, emit a completion event.
- 10.2. WHEN a connector completes a fetch THEN it SHALL emit an event following the naming convention `<plugin-id>.fetch.completed`.
- 10.3. WHEN a connector fetch fails THEN it SHALL emit an event following the naming convention `<plugin-id>.fetch.failed`.
- 10.4. WHEN v1 ships THEN the following first-party connectors SHALL be available: Email (IMAP/SMTP), Calendar (CalDAV), Contacts (CardDAV), Filesystem (WebDAV/S3).

### Requirement 11 — Plugin SDK Macros

**User Story:** As a plugin author, I want attribute macros so that I can write plain Rust functions without handling WASM boundary boilerplate.

#### Acceptance Criteria

- 11.1. WHEN a function is annotated with `#[plugin_action]` THEN the macro SHALL generate the Extism export boilerplate, deserialise the incoming JSON into a `PipelineMessage`, construct a `PluginContext`, and serialise the returned `PipelineMessage`.
- 11.2. WHEN a function is annotated with `#[plugin_hook]` THEN the macro SHALL generate the Extism export boilerplate and construct a `PluginContext` — lifecycle hooks do not receive or return a `PipelineMessage`.
- 11.3. WHEN the `PluginContext` is constructed THEN it SHALL expose typed clients: `StorageClient`, `EventClient`, `ConfigClient`, `HttpClient`.
- 11.4. WHEN a client method is called for a capability the plugin lacks THEN it SHALL return `PluginError::CapabilityDenied`.
