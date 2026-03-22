<!--
domain: sdk
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Plugin SDK RS — Tasks

## 1.1 — Re-export types and traits
<!-- spec: ./brief.md | depends: none | est: 20m | status: todo -->
> spec: ./brief.md
> depends: none

- Configure `packages/plugin-sdk/Cargo.toml` to depend on `packages/types` and `packages/traits`
- In `packages/plugin-sdk/src/lib.rs`, re-export all public types from `packages/types`: CDM structs, `PipelineMessage`, `MessageMetadata`, `TypedPayload`, `CdmType`, `SchemaValidated`
- Re-export `Plugin` trait and `EngineError` trait from `packages/traits`
- Re-export `Severity` enum from `packages/traits`
- Verify that downstream consumers can import everything from the SDK without adding `types` or `traits` as direct dependencies

**Files:** `packages/plugin-sdk/Cargo.toml`, `packages/plugin-sdk/src/lib.rs`
**Est:** 20 min

## 1.2 — StorageContext query builder
<!-- spec: ./brief.md | depends: 1.1 | est: 40m | status: todo -->
> spec: ./brief.md
> depends: 1.1

- Define `StorageContext` struct in `packages/plugin-sdk/src/storage.rs`
- Implement fluent query builder: `storage.query("collection")` returns a `QueryBuilder`
- Implement `QueryBuilder` methods: `.where_eq(field, value)`, `.order_by(field)`, `.limit(n)`, `.execute()` returning `Result<Vec<PipelineMessage>>`
- Implement write methods on `StorageContext`: `.insert(collection, message)`, `.update(collection, id, message)`, `.delete(collection, id)`
- Define `StorageQuery` and `StorageMutation` value types that the query builder produces
- Export `StorageContext` and related types from `packages/plugin-sdk/src/lib.rs`

**Files:** `packages/plugin-sdk/src/storage.rs`, `packages/plugin-sdk/src/lib.rs`
**Est:** 40 min

## 1.3 — Registration helper macros
<!-- spec: ./brief.md | depends: 1.1 | est: 25m | status: todo -->
> spec: ./brief.md
> depends: 1.1

- Define `register_plugin!` macro in `packages/plugin-sdk/src/macros.rs`
- The macro SHALL generate Extism WASM entry-point boilerplate for a struct implementing `Plugin`
- The generated code SHALL wire `execute` to the WASM callable export
- The macro SHALL NOT require the plugin author to write any unsafe code
- Export the macro from `packages/plugin-sdk/src/lib.rs`

**Files:** `packages/plugin-sdk/src/macros.rs`, `packages/plugin-sdk/src/lib.rs`
**Est:** 25 min

## 1.4 — Test utilities
<!-- spec: ./brief.md | depends: 1.1, 1.2 | est: 30m | status: todo -->
> spec: ./brief.md
> depends: 1.1, 1.2

- Create `packages/plugin-sdk/src/test/mod.rs` with test utility exports
- Implement `mock_storage()` returning a `MockStorageContext` that stores data in memory and supports the same fluent query API as the real `StorageContext`
- Implement `mock_message()` returning a `MockMessageBuilder` with sensible defaults for metadata (auto-generated correlation ID, current timestamp, test auth context)
- `MockMessageBuilder` SHALL support `.with_payload()`, `.with_source()`, `.with_correlation_id()`, and `.build()`
- Export test utilities under `life_engine_plugin_sdk::test`

**Files:** `packages/plugin-sdk/src/test/mod.rs`, `packages/plugin-sdk/src/test/mock_storage.rs`, `packages/plugin-sdk/src/test/mock_message.rs`, `packages/plugin-sdk/src/lib.rs`
**Est:** 30 min

## 1.5 — WASM build configuration
<!-- spec: ./brief.md | depends: 1.1, 1.2, 1.3 | est: 20m | status: todo -->
> spec: ./brief.md
> depends: 1.1, 1.2, 1.3

- Add `wasm32-wasi` target configuration to `packages/plugin-sdk/Cargo.toml`
- Ensure all SDK dependencies are WASM-compatible (no host-only crates)
- Create a `.cargo/config.toml` with default WASM build settings for plugin authors to reference
- Verify `cargo build --target wasm32-wasi` succeeds and produces a valid WASM module
- Document the build command in the crate-level doc comment

**Files:** `packages/plugin-sdk/Cargo.toml`, `packages/plugin-sdk/.cargo/config.toml`
**Est:** 20 min

## 1.6 — Action type definition
<!-- spec: ./brief.md | depends: 1.1 | est: 15m | status: todo -->
> spec: ./brief.md
> depends: 1.1

- Define `Action` struct in `packages/traits/src/plugin.rs` (or appropriate location in `packages/traits`)
- `Action` SHALL include: `name: String`, `description: String`, `input_schema: String`, `output_schema: String`
- Add serde derives for serialisation
- Re-export `Action` through the SDK

**Files:** `packages/traits/src/plugin.rs`, `packages/plugin-sdk/src/lib.rs`
**Est:** 15 min

## 1.7 — Integration smoke test
<!-- spec: ./brief.md | depends: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6 | est: 30m | status: todo -->
> spec: ./brief.md
> depends: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6

- Create a minimal example plugin in `packages/plugin-sdk/examples/` that implements `Plugin`
- The example plugin SHALL declare at least one action, use `StorageContext` in `execute`, and return a `PipelineMessage`
- Write a test that uses mock test utilities to exercise the example plugin
- Verify the example compiles to `wasm32-wasi` without errors
- Verify plugin authors need only `life-engine-plugin-sdk` in their `Cargo.toml`

**Files:** `packages/plugin-sdk/examples/hello_plugin.rs`, `packages/plugin-sdk/tests/smoke_test.rs`
**Est:** 30 min
