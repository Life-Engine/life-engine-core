<!--
domain: plugin-actions
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Plugin Actions Tasks

**Progress:** 0 / 14 tasks complete

## 1.1 — PluginError Type

- [ ] Define `PluginError` enum with all variants and JSON serialisation
  <!-- files: packages/types/src/plugin_error.rs -->
  <!-- purpose: Define the PluginError enum with CapabilityDenied, NotFound, ValidationError, StorageError, NetworkError, InternalError variants and Serialize/Deserialize impls -->
  <!-- requirements: 5.3 -->

## 1.2 — PluginContext and Client Structs

- [ ] Define `StorageClient` struct with methods wrapping storage host functions
  <!-- files: packages/types/src/storage_client.rs -->
  <!-- purpose: Typed wrapper around storage_doc_* and storage_blob_* host function calls, returning Result<T, PluginError> -->
  <!-- requirements: 2.1 -->

- [ ] Define `EventClient` struct wrapping `emit_event` host function
  <!-- files: packages/types/src/event_client.rs -->
  <!-- purpose: Typed wrapper around emit_event host function, returning Result<(), PluginError> -->
  <!-- requirements: 2.2 -->

- [ ] Define `ConfigClient` struct wrapping `config_read` host function
  <!-- files: packages/types/src/config_client.rs -->
  <!-- purpose: Typed wrapper around config_read host function, returning Result<ConfigMap, PluginError> -->
  <!-- requirements: 2.3 -->

- [ ] Define `HttpClient` struct wrapping `http_request` host function
  <!-- files: packages/types/src/http_client.rs -->
  <!-- purpose: Typed wrapper around http_request host function, returning Result<HttpResponse, PluginError> -->
  <!-- requirements: 2.4 -->

- [ ] Define `PluginContext` struct composing all four clients
  <!-- files: packages/types/src/plugin_context.rs -->
  <!-- purpose: Struct with storage, events, config, http fields and constructor -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.4, 2.5 -->

## 1.3 — Action Attribute Macro

- [ ] Implement `#[plugin_action]` proc macro with deserialisation, context construction, panic catching, and serialisation
  <!-- files: packages/plugin-sdk-macros/src/plugin_action.rs, packages/plugin-sdk-macros/src/lib.rs -->
  <!-- purpose: Generate Extism export boilerplate: deserialise PipelineMessage, construct PluginContext, call user fn inside catch_unwind, serialise result or map error -->
  <!-- requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6 -->

- [ ] Add unit tests for `#[plugin_action]` macro expansion
  <!-- files: packages/plugin-sdk-macros/tests/plugin_action_tests.rs -->
  <!-- purpose: Verify macro generates correct export signature, handles Ok/Err/panic cases -->
  <!-- requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6 -->

## 1.4 — Lifecycle Hook Macro

- [ ] Implement `#[plugin_hook]` proc macro for init and shutdown
  <!-- files: packages/plugin-sdk-macros/src/plugin_hook.rs, packages/plugin-sdk-macros/src/lib.rs -->
  <!-- purpose: Generate Extism export boilerplate for hooks: construct PluginContext, call user fn, return Result<(), PluginError> -->
  <!-- requirements: 3.3 -->

- [ ] Add unit tests for `#[plugin_hook]` macro expansion
  <!-- files: packages/plugin-sdk-macros/tests/plugin_hook_tests.rs -->
  <!-- purpose: Verify macro generates correct export for hooks with no PipelineMessage input -->
  <!-- requirements: 3.3 -->

## 1.5 — Manifest Parsing for Actions and Lifecycle

- [ ] Parse `[actions.<name>]` and `[lifecycle]` sections from plugin manifest
  <!-- files: packages/core/src/plugin/manifest.rs -->
  <!-- purpose: Deserialise action declarations (name, timeout_ms) and lifecycle flags (init, shutdown) from plugin.toml -->
  <!-- requirements: 4.1, 4.2, 3.1, 3.2, 3.5 -->

## 1.6 — Timeout Enforcement

- [ ] Configure per-action timeouts on Extism plugin instances and handle timeout errors
  <!-- files: packages/core/src/plugin/executor.rs -->
  <!-- purpose: Read timeout_ms from manifest or engine default, set on Extism call config, map timeout errors to step failure, apply on_error strategy -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4 -->

## 1.7 — Lifecycle Hook Invocation

- [ ] Call init on plugin load and shutdown on Core teardown from the plugin manager
  <!-- files: packages/core/src/plugin/manager.rs -->
  <!-- purpose: Invoke init export after WASM instantiation, fail load on error; invoke shutdown export during Core shutdown sequence -->
  <!-- requirements: 3.1, 3.2, 3.4, 3.5 -->

## 1.8 — Integration Tests

- [ ] End-to-end test: action invocation, timeout, lifecycle hooks, and error handling
  <!-- files: packages/core/tests/plugin_actions_integration.rs -->
  <!-- purpose: Test full flow: load plugin with init hook, invoke action with PipelineMessage, verify timeout enforcement, verify error mapping, call shutdown -->
  <!-- requirements: 1.1, 1.6, 3.1, 3.4, 4.3, 5.1, 5.2 -->
