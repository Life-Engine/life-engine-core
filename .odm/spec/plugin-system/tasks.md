<!--
domain: plugin-system
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Implementation Tasks — Plugin System

**Progress:** 0 / 24 tasks complete

## 1.1 — Manifest Types and Parsing

- [ ] Define `PluginManifest` and child structs in `packages/types`
  <!-- files: packages/types/src/plugin_manifest.rs -->
  <!-- purpose: Create all manifest data structures with TOML deserialization -->
  <!-- requirements: 1.1, 2.1, 2.2 -->

- [ ] Implement manifest validation logic
  <!-- files: packages/types/src/plugin_manifest.rs -->
  <!-- purpose: Validate required fields, action presence, schema paths, event naming conventions, and reject unknown sections -->
  <!-- requirements: 2.1, 2.2, 2.4, 2.5, 2.6, 2.7 -->

- [ ] Add unit tests for manifest parsing and validation
  <!-- files: packages/types/src/plugin_manifest.rs -->
  <!-- purpose: Test valid manifests, missing fields, unknown sections, invalid event names, and schema path resolution -->
  <!-- requirements: 2.1, 2.2, 2.6, 2.7, 2.8 -->

## 1.2 — Plugin Error Types

- [ ] Define `PluginError` enum in `packages/types`
  <!-- files: packages/types/src/plugin_error.rs -->
  <!-- purpose: Create typed error variants: CapabilityDenied, NotFound, ValidationError, StorageError, NetworkError, InternalError -->
  <!-- requirements: 6.6, 7.4 -->

- [ ] Add Display and Error trait implementations for `PluginError`
  <!-- files: packages/types/src/plugin_error.rs -->
  <!-- purpose: Enable error formatting and compatibility with the standard error trait -->
  <!-- requirements: 6.6 -->

## 1.3 — Capability Enforcement

- [ ] Define capability constants and `CapabilityChecker` in `packages/core`
  <!-- files: packages/core/src/capability_checker.rs -->
  <!-- purpose: Map manifest capability declarations to host function groups and provide check_capability and check_collection_access functions -->
  <!-- requirements: 5.1, 5.2, 5.3, 5.4, 5.5 -->

- [ ] Implement trust-level capability granting
  <!-- files: packages/core/src/capability_checker.rs -->
  <!-- purpose: Auto-grant for first-party plugins, explicit approval for third-party, fail load on unapproved capabilities -->
  <!-- requirements: 5.6, 5.7, 5.8 -->

- [ ] Add unit tests for capability enforcement
  <!-- files: packages/core/src/capability_checker.rs -->
  <!-- purpose: Test deny-by-default, first-party auto-grant, third-party approval, collection scoping, event emission scoping -->
  <!-- requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 5.7, 5.8 -->

## 1.4 — Plugin Registry

- [ ] Define `PluginRegistry` trait in `packages/traits`
  <!-- files: packages/traits/src/plugin_registry.rs -->
  <!-- purpose: Define trait for plugin registration, lookup by ID, state transitions, and listing running plugins -->
  <!-- requirements: 3.1, 3.5, 3.8 -->

- [ ] Implement in-memory `PluginRegistry`
  <!-- files: packages/core/src/plugin_registry.rs -->
  <!-- purpose: Concrete registry using HashMap for plugin entries with state tracking -->
  <!-- requirements: 3.1, 3.5, 3.7, 3.8 -->

- [ ] Add unit tests for the plugin registry
  <!-- files: packages/core/src/plugin_registry.rs -->
  <!-- purpose: Test registration, lookup, state transitions, and unload cleanup -->
  <!-- requirements: 3.1, 3.5, 3.7, 3.8 -->

## 1.5 — Plugin Loader and Lifecycle

- [ ] Implement plugin discovery (Discover phase)
  <!-- files: packages/core/src/plugin_loader.rs -->
  <!-- purpose: Scan configured plugin directory for subdirectories containing manifest.toml -->
  <!-- requirements: 3.1, 1.1 -->

- [ ] Implement plugin loading and WASM instantiation (Load phase)
  <!-- files: packages/core/src/plugin_loader.rs -->
  <!-- purpose: Parse manifest, validate, check capabilities, instantiate WASM module via Extism -->
  <!-- requirements: 3.2, 2.3, 4.1 -->

- [ ] Implement init and shutdown lifecycle hooks
  <!-- files: packages/core/src/plugin_loader.rs -->
  <!-- purpose: Call init action after load, call shutdown before unload, handle errors -->
  <!-- requirements: 3.3, 3.4, 3.6, 3.7 -->

- [ ] Add integration tests for the full plugin lifecycle
  <!-- files: packages/core/tests/plugin_lifecycle.rs -->
  <!-- purpose: Test discover-load-init-running-stop-unload flow with mock WASM modules -->
  <!-- requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 3.8 -->

## 1.6 — Host Function Implementation

- [ ] Register document storage host functions with Extism
  <!-- files: packages/core/src/host_functions.rs -->
  <!-- purpose: Implement storage_doc_get, storage_doc_list, storage_doc_count, storage_doc_create, storage_doc_update, storage_doc_partial_update, storage_doc_delete, storage_doc_batch_create, storage_doc_batch_update, storage_doc_batch_delete -->
  <!-- requirements: 6.1, 6.4 -->

- [ ] Register blob storage, events, config, and HTTP host functions with Extism
  <!-- files: packages/core/src/host_functions.rs -->
  <!-- purpose: Implement storage_blob_*, emit_event, config_read, http_request with capability checks and scope isolation -->
  <!-- requirements: 6.2, 6.3, 6.4, 6.5 -->

- [ ] Add integration tests for host function dispatch
  <!-- files: packages/core/tests/host_functions.rs -->
  <!-- purpose: Test capability checks, collection scoping, blob key prefixing, error responses -->
  <!-- requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6 -->

## 1.7 — Plugin SDK and Macros

- [ ] Implement `PluginContext` and client types in `packages/plugin-sdk`
  <!-- files: packages/plugin-sdk/src/lib.rs, packages/plugin-sdk/src/context.rs -->
  <!-- purpose: Create StorageClient, EventClient, ConfigClient, HttpClient wrapping host function calls -->
  <!-- requirements: 11.3, 11.4 -->

- [ ] Implement `#[plugin_action]` proc macro
  <!-- files: packages/plugin-sdk-macros/src/lib.rs -->
  <!-- purpose: Generate Extism export boilerplate, PipelineMessage deserialization, PluginContext construction, panic recovery -->
  <!-- requirements: 11.1, 7.1, 7.2, 7.6 -->

- [ ] Implement `#[plugin_hook]` proc macro
  <!-- files: packages/plugin-sdk-macros/src/lib.rs -->
  <!-- purpose: Generate Extism export boilerplate for lifecycle hooks without PipelineMessage -->
  <!-- requirements: 11.2 -->

- [ ] Add tests for SDK macros with a test plugin
  <!-- files: packages/plugin-sdk/tests/macro_tests.rs -->
  <!-- purpose: Compile a minimal test plugin using both macros and verify the generated WASM exports -->
  <!-- requirements: 11.1, 11.2, 11.3 -->

## 1.8 — Action Execution and Timeouts

- [ ] Implement action invocation in the pipeline executor
  <!-- files: packages/core/src/plugin_loader.rs -->
  <!-- purpose: Call plugin actions via Extism with PipelineMessage serialisation, timeout enforcement, and error mapping -->
  <!-- requirements: 7.1, 7.2, 7.3, 7.4, 7.5, 8.1, 8.2, 8.3 -->

- [ ] Add integration tests for action execution and timeouts
  <!-- files: packages/core/tests/action_execution.rs -->
  <!-- purpose: Test PipelineMessage round-trip, read-only field enforcement, timeout termination, panic recovery, warning propagation -->
  <!-- requirements: 7.1, 7.3, 7.4, 7.5, 7.6, 8.1, 8.2, 8.3, 4.2 -->
