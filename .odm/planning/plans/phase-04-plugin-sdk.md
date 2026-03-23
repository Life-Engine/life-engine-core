<!--
project: life-engine-core
phase: 4
specs: plugin-sdk-rs
updated: 2026-03-23
-->

# Phase 4 — Plugin SDK

## Plan Overview

This phase implements `packages/plugin-sdk` — the single dependency that plugin authors need. It re-exports all types and traits from Phases 2-3, provides the `StorageContext` fluent query builder, the `register_plugin!` macro for WASM entry-point generation, test utilities for plugin development, and WASM build configuration. The SDK is the "Pit of Success" — the easy path for plugin authors is the correct path.

This phase depends on Phase 2 (types) and Phase 3 (traits, crypto). Phase 5 (data layer) and Phase 7 (workflow engine) depend on types defined here.

> spec: .odm/spec/plugin-sdk-rs/brief.md

Progress: 1 / 7 work packages complete

---

## 4.1 — Type and Trait Re-exports
> spec: .odm/spec/plugin-sdk-rs/brief.md

- [x] Configure plugin-sdk Cargo.toml dependencies and re-export all public types
  <!-- file: packages/plugin-sdk/Cargo.toml -->
  <!-- file: packages/plugin-sdk/src/lib.rs -->
  <!-- purpose: Add dependencies on life-engine-types and life-engine-traits in Cargo.toml using workspace path references. In lib.rs, re-export all public types from life-engine-types: all 7 CDM structs (CalendarEvent, Task, Contact, Note, Email, FileMetadata, Credential) and their supporting types (enums, nested structs), PipelineMessage, MessageMetadata, TypedPayload, CdmType, SchemaValidated, StorageQuery, StorageMutation, QueryFilter, FilterOp, SortField, SortDirection. Re-export from life-engine-traits: Plugin trait, Action struct, EngineError trait, Severity enum, StorageBackend trait, Capability enum, CapabilityViolation. Create a prelude module that re-exports the most commonly used types for ergonomic imports. Verify downstream consumers can import everything from the SDK without adding types or traits as direct dependencies by writing a compile-only test. -->
  <!-- requirements: from plugin-sdk-rs spec 1.1 -->
  <!-- leverage: existing packages/plugin-sdk/src/lib.rs -->

---

## 4.2 — StorageContext Query Builder
> depends: 4.1
> spec: .odm/spec/plugin-sdk-rs/brief.md

- [ ] Implement StorageContext struct with fluent query API
  <!-- file: packages/plugin-sdk/src/storage.rs -->
  <!-- purpose: Define StorageContext struct that holds a reference to a StorageBackend and the calling plugin's ID. Implement query(collection: &str) method that returns a QueryBuilder. QueryBuilder methods: where_eq(field, value) adds an Eq filter, where_gte(field, value) adds Gte filter, where_lte(field, value) adds Lte filter, where_contains(field, value) adds Contains filter, order_by(field) adds Asc sort, order_by_desc(field) adds Desc sort, limit(n: u32) sets limit (capped at 1000, silently clamped), offset(n: u32) sets offset, execute() async method that builds a StorageQuery with the plugin_id and delegates to StorageBackend::execute(). All filter values accept impl Into<serde_json::Value> for ergonomic usage. QueryBuilder consumes self and returns self for method chaining. -->
  <!-- requirements: from plugin-sdk-rs spec 1.2, data-layer spec 1.2-1.6, 8.1-8.4, 9.1-9.5 -->
  <!-- leverage: none -->

- [ ] Implement StorageContext write methods
  <!-- file: packages/plugin-sdk/src/storage.rs -->
  <!-- purpose: Add write methods to StorageContext: insert(collection: &str, message: PipelineMessage) that creates a StorageMutation::Insert with the plugin_id and delegates to StorageBackend::mutate(), update(collection: &str, id: Uuid, message: PipelineMessage, expected_version: u64) that creates a StorageMutation::Update with optimistic concurrency, delete(collection: &str, id: Uuid) that creates a StorageMutation::Delete. All methods are async and return Result<(), Box<dyn EngineError>>. Re-export StorageContext from lib.rs. -->
  <!-- requirements: from plugin-sdk-rs spec 1.2 -->
  <!-- leverage: StorageContext from previous task -->

---

## 4.3 — Registration Helper Macros
> depends: 4.1
> spec: .odm/spec/plugin-sdk-rs/brief.md

- [ ] Define register_plugin! macro for WASM entry-point generation
  <!-- file: packages/plugin-sdk/src/macros.rs -->
  <!-- purpose: Define a declarative macro register_plugin!(MyPlugin) that generates Extism WASM entry-point boilerplate. The generated code: (1) creates an extern "C" fn that Extism calls as the WASM export, (2) deserializes the input bytes into a PipelineMessage, (3) extracts the action name from the input metadata, (4) instantiates the plugin struct, (5) calls plugin.execute(action, input), (6) serializes the PipelineMessage output back to bytes, (7) handles errors by returning a serialized EngineError. The macro must NOT require the plugin author to write any unsafe code. The generated entry point should be named "execute" to match the Extism calling convention. Export the macro from lib.rs using #[macro_export]. -->
  <!-- requirements: from plugin-sdk-rs spec 1.3 -->
  <!-- leverage: none -->

---

## 4.4 — Test Utilities
> depends: 4.1, 4.2
> spec: .odm/spec/plugin-sdk-rs/brief.md

- [ ] Implement mock StorageContext for plugin testing
  <!-- file: packages/plugin-sdk/src/test/mock_storage.rs -->
  <!-- purpose: Define MockStorageContext that stores data in an in-memory HashMap<String, Vec<PipelineMessage>> keyed by collection name. Implement the same fluent query API as the real StorageContext: query(), insert(), update(), delete(). The mock query builder supports where_eq filtering by checking JSON field values, order_by sorting, limit/offset pagination. insert() adds to the in-memory store, update() replaces by ID, delete() removes by ID. Provide assert_inserted(collection, count), assert_contains(collection, id), and dump() methods for test assertions. This allows plugin authors to test their plugins without a real database. -->
  <!-- requirements: from plugin-sdk-rs spec 1.4 -->
  <!-- leverage: none -->

- [ ] Implement mock PipelineMessage builder for plugin testing
  <!-- file: packages/plugin-sdk/src/test/mock_message.rs -->
  <!-- purpose: Define MockMessageBuilder that creates PipelineMessage instances with sensible defaults: auto-generated UUID correlation_id, current timestamp, "test" as source, None as auth_context. Builder methods: with_payload(TypedPayload) sets the payload, with_cdm(CdmType) sets a CDM payload, with_custom(serde_json::Value, schema: serde_json::Value) sets a validated custom payload, with_source(String) overrides the source, with_correlation_id(Uuid) overrides the correlation ID, with_auth(serde_json::Value) sets auth context, build() -> PipelineMessage. Provide convenience constructors: MockMessageBuilder::event(CalendarEvent), MockMessageBuilder::task(Task), etc. for each CDM type. -->
  <!-- requirements: from plugin-sdk-rs spec 1.4 -->
  <!-- leverage: none -->

- [ ] Create test module and re-export test utilities
  <!-- file: packages/plugin-sdk/src/test/mod.rs -->
  <!-- file: packages/plugin-sdk/src/lib.rs -->
  <!-- purpose: Create src/test/mod.rs that re-exports MockStorageContext and MockMessageBuilder. In lib.rs, add pub mod test gated behind #[cfg(any(test, feature = "test-utils"))] so test utilities are available to plugin authors who enable the feature but not included in production WASM builds. Add the "test-utils" feature to Cargo.toml as an optional feature. -->
  <!-- requirements: from plugin-sdk-rs spec 1.4 -->
  <!-- leverage: none -->

---

## 4.5 — WASM Build Configuration
> depends: 4.1, 4.2, 4.3
> spec: .odm/spec/plugin-sdk-rs/brief.md

- [ ] Configure WASM build target and verify compatibility
  <!-- file: packages/plugin-sdk/Cargo.toml -->
  <!-- file: packages/plugin-sdk/.cargo/config.toml -->
  <!-- purpose: In Cargo.toml, ensure all SDK dependencies are WASM-compatible — no host-only crates (no tokio, no std::net, no filesystem operations in the SDK itself). Add conditional compilation flags for wasm32-wasi target. Create .cargo/config.toml with [build] target = "wasm32-wasi" as a reference for plugin authors (not enforced on the SDK itself, which must compile for both native and WASM). Verify cargo build --target wasm32-wasi succeeds for the SDK crate and produces a valid WASM module. Document the WASM build command in the crate-level doc comment in lib.rs. Note: async-trait and tokio are NOT available in WASM — StorageContext methods in WASM use synchronous host function calls instead. -->
  <!-- requirements: from plugin-sdk-rs spec 1.5 -->
  <!-- leverage: none -->

---

## 4.6 — Action Type Definition
> depends: 4.1
> spec: .odm/spec/plugin-sdk-rs/brief.md

- [ ] Define Action struct in traits crate and re-export through SDK
  <!-- file: packages/traits/src/plugin.rs -->
  <!-- file: packages/plugin-sdk/src/lib.rs -->
  <!-- purpose: Verify the Action struct defined in Phase 3 (WP 3.5) has all required fields: name (String), description (String), input_schema (Option<String> — JSON Schema string), output_schema (Option<String> — JSON Schema string). Add serde Serialize/Deserialize derives. Add a builder method Action::new(name, description) -> Action with optional .with_input_schema(schema) and .with_output_schema(schema) chainable methods for ergonomic construction. Re-export Action through the plugin SDK. Verify it round-trips through serde_json serialization. -->
  <!-- requirements: from plugin-sdk-rs spec 1.6 -->
  <!-- leverage: packages/traits/src/plugin.rs from Phase 3 -->

---

## 4.7 — Integration Smoke Test
> depends: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6
> spec: .odm/spec/plugin-sdk-rs/brief.md

- [ ] Create minimal example plugin and smoke test
  <!-- file: packages/plugin-sdk/examples/hello_plugin.rs -->
  <!-- file: packages/plugin-sdk/tests/smoke_test.rs -->
  <!-- purpose: Create an example plugin in examples/hello_plugin.rs that: (1) defines a HelloPlugin struct, (2) implements Plugin trait with id "hello-plugin", display_name "Hello Plugin", version "0.1.0", (3) declares one action "greet" that takes a PipelineMessage with a Contact payload and returns a PipelineMessage with a Note payload containing a greeting, (4) uses register_plugin!(HelloPlugin) macro. Write a test in tests/smoke_test.rs that: (1) creates a MockStorageContext, (2) creates a MockMessageBuilder with a Contact CDM payload, (3) calls HelloPlugin.execute("greet", message), (4) asserts the output is a PipelineMessage with a Note payload, (5) asserts unknown action returns an error. Verify the example compiles to wasm32-wasi (cargo build --target wasm32-wasi --example hello_plugin). Verify plugin authors need only life-engine-plugin-sdk in their Cargo.toml. -->
  <!-- requirements: from plugin-sdk-rs spec 1.7 -->
  <!-- leverage: all previous WPs in this phase -->
