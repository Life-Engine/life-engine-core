# Plugin SDK Review — packages/plugin-sdk-rs

## Summary

The Plugin SDK (`life-engine-plugin-sdk`) is a well-structured crate that provides Rust plugin developers with two parallel plugin models: a native `CorePlugin` trait with async lifecycle management, and a WASM `Plugin` trait with synchronous `execute` dispatch via the `register_plugin!` macro. The SDK demonstrates strong ergonomics overall, with a thoughtful prelude, re-exports of upstream types so plugin authors need only one dependency, a fluent query builder, structured error types, and a comprehensive test mock toolkit.

The main issues found revolve around the divergence between two capability enums, the coexistence of two separate plugin trait systems that could confuse SDK consumers, and several opportunities to tighten WASM boundary type safety. The codebase is in active development (Phase 2 of an architecture redesign), so some of these issues may be intentional stepping stones.

---

## File-by-File Analysis

### Cargo.toml

- **Dependencies** — Clean dependency set. Both `life-engine-types` and `life-engine-traits` are path dependencies. All workspace-level dependencies are used correctly.
- **Features** — The `test-utils` feature correctly gates the `test` module. This is standard practice for SDK crates.
- **Missing** — No `wasm32-wasip1` conditional dependencies. This is acceptable if the WASM entry point only uses std and serde, but should be documented if the SDK intends to be dual-target (native + WASM). No `[target.'cfg(target_arch = "wasm32")'.dependencies]` section exists if Extism PDK functions are ever needed beyond the raw `extern "C"` approach.

### src/lib.rs (lines 1-148)

- **Prelude design** — The prelude re-exports are comprehensive and well-curated. Plugin authors get `CorePlugin`, `Plugin`, `Action`, `PipelineMessage`, `StorageBackend`, `PluginError`, `LifecycleHooks`, CDM types, and storage query types all from one import. This is excellent DX.
- **Dual re-export of Capability** — The SDK re-exports `life_engine_traits::Capability as WasmCapability` and separately defines `types::Capability` with different variants. This is documented at line 99-100 but will confuse plugin authors who see both `Capability` and `WasmCapability` in scope. See the critical issue below.
- **serde_json re-export** — Good practice (`pub use serde_json`) so plugin authors don't need to add it as a direct dependency.
- **Missing from prelude** — `RetryState`, `CredentialStore`, `StoredCredential`, `MockMessageBuilder`, and `MockStorageContext` are not in the prelude. `RetryState` may belong there since connectors will use it frequently; the test utilities are correctly excluded (feature-gated).
- **Module doc comments** — The crate-level documentation at lines 1-43 is thorough with a quick-start example and WASM build instructions. References a `.cargo/config.toml` that may or may not exist in the repo.

### src/types.rs (lines 1-400)

- **Capability enum** — Defines 13 variants: `StorageRead`, `StorageWrite`, `StorageDelete`, `StorageBlobRead`, `StorageBlobWrite`, `StorageBlobDelete`, `HttpOutbound`, `CredentialsRead`, `CredentialsWrite`, `EventsSubscribe`, `EventsEmit`, `ConfigRead`, `Logging`. This is the SDK-facing capability enum used by `CorePlugin::capabilities()`.
- **Divergence from traits::Capability** — The traits crate's `Capability` has only 10 variants (no `CredentialsRead`, `CredentialsWrite`, or `Logging`). The SDK's `types::Capability` adds `CredentialsRead`, `CredentialsWrite`, and `Logging` but drops nothing. These two enums serve different roles but the naming collision is a maintenance hazard and a plugin author confusion vector.
- **PluginContext** — Contains `plugin_id` and an optional `Arc<dyn CredentialAccess>`. This is the context for the `CorePlugin` model. It provides async credential access methods. Note that `ActionContext` (in `context.rs`) is a different context for the WASM `Plugin` model. Two different context types for two different plugin models.
- **CoreEvent** — Clean serialization. `timestamp` is `DateTime<Utc>`, which serializes correctly via chrono's serde.
- **HttpMethod** — Good: `serde(rename_all = "UPPERCASE")` matches HTTP convention. `Display` impl matches. Tests verify both directions.
- **PluginRoute** — Has a comment "Handler will be defined more concretely in Phase 1" at line 94. This is stale — the project is now in Phase 2+. The route struct has no handler field, making it metadata-only. `CorePlugin::handle_route` fills this gap but the comment should be updated.
- **CollectionSchema** — Uses `serde_json::Value` for the JSON schema definition. This works but provides no compile-time schema validation. Acceptable for now since JSON Schema is inherently dynamic.

### src/traits.rs (lines 1-198)

- **CorePlugin trait** — This is the native (non-WASM) plugin trait. It has `async fn on_load(&mut self, ctx: &PluginContext)`, `on_unload`, `handle_event`, `handle_route`, `routes`, `collections`, and metadata methods.
- **Overlap with life_engine_traits::Plugin** — The `Plugin` trait from the traits crate is the WASM-oriented plugin trait with `fn execute(&self, action: &str, input: PipelineMessage)`. Both are re-exported from the SDK. A plugin author needs to know which trait to implement: `CorePlugin` for native plugins or `Plugin` for WASM plugins. This is documented in the module-level comments but could be clearer.
- **`on_load` takes `&mut self`** — This is fine for native plugins that need mutable state initialization. However, `handle_event` and `handle_route` take `&self`, which means plugins that need mutable state during event handling must use interior mutability (`Mutex`, `RwLock`).
- **`handle_route` default** — Returns `Err(anyhow!("route handling not implemented"))`. This is a reasonable default. The error message could include the plugin ID for debuggability, but this is minor.
- **Test coverage** — Tests cover lifecycle, metadata, default collections, and event handling. Good coverage.

### src/macros.rs (lines 1-347)

- **PluginInvocation / PluginOutput** — Clean envelope types for WASM boundary crossing. `PluginOutput` uses `#[serde(tag = "status")]` which produces `{"status": "ok", "message": {...}}` or `{"status": "error", "message": "...", ...}`. The discriminant tag is clean.
- **PluginOutput::Error severity as String** — The `severity` field in `PluginOutput::Error` is a `String`, not the `Severity` enum. This means the host must parse "Fatal", "Retryable", "Warning" from strings. This works but loses type safety at the boundary. However, since this crosses a WASM/JSON boundary, stringly-typed is the practical choice.
- **register_plugin! macro** — Generates a `#[cfg(target_arch = "wasm32")]` module with `extern "C"` Extism bindings. The macro:
  - Reads input via `extism_input_length`/`extism_input_offset`/`extism_load`
  - Writes output via `extism_alloc`/`extism_store`/`extism_output_set`
  - Handles deserialization errors with proper `PluginOutput::Error`
  - Handles both success and error paths from `Plugin::execute`
  - Uses `$crate::` paths for hygiene (correct)
  - Creates a new `Default::default()` instance per invocation at line 169 — this means **plugin state is not preserved across calls**. This is by design for WASM sandbox isolation but should be prominently documented.
- **Error fallback** — If even the error serialization fails (line 199-207), it falls back to `write_error` with a plain text message. This is a good defensive pattern.
- **Test coverage** — Round-trip tests for `PluginInvocation` and `PluginOutput` (both Ok and Error variants). Tests verify the `status` tag. Good.

### src/storage.rs (lines 1-541)

- **StorageContext** — Generic over `S: StorageBackend`. Holds the backend, plugin_id, and `HashSet<Capability>` (from `life_engine_traits::Capability`, not `types::Capability`). This is the correct choice since `StorageBackend` is from the traits crate.
- **QueryBuilder** — Fluent API with `where_eq`, `where_gte`, `where_lte`, `where_contains`, `order_by`, `order_by_desc`, `limit`, `offset`, `execute`. The API is ergonomic and well-documented.
- **Limit cap at 1000** — `limit(n: u32)` clamps to 1000 via `n.min(1000)`. This is a sensible safety valve. The default when no limit is set is... no limit (just `None`). The backend decides what to do with `None`. This could lead to unbounded queries if the backend doesn't enforce its own limit.
- **Capability checks** — All mutation operations check `Capability::StorageWrite`, reads check `Capability::StorageRead`. The delete method checks `StorageWrite`, not `StorageDelete` (even though a `StorageDelete` variant exists in the traits crate). This may be intentional (treating delete as a write) or may be an oversight.
- **Optimistic concurrency** — The `update` method takes `expected_version: u64` for optimistic locking. This is good for preventing lost updates.
- **Test coverage** — Comprehensive tests with a `MockBackend` that records all queries and mutations. Tests cover capability enforcement for all operations.

### src/context.rs (lines 1-311)

- **ActionContext** — This is the WASM-oriented plugin context. It holds `Arc<dyn StorageClient>`, `Arc<dyn EventClient>`, `Arc<dyn ConfigClient>`, `Arc<dyn HttpClient>`. All client fields are `pub`, so plugins access them directly as `ctx.storage.doc_read(...)`.
- **Client traits** — `StorageClient`, `EventClient`, `ConfigClient`, `HttpClient` are all `async_trait` with `Send + Sync`. They return `Result<T, PluginError>` instead of `Result<T, Box<dyn EngineError>>`. This is a deliberate design choice — plugin actions use `PluginError`, not `EngineError`.
- **HttpResponse** — Simple struct with `status: u16`, `headers: Value`, `body: String`. Not serializable (no `Serialize`/`Deserialize` derives). This is fine if it only exists in memory during a plugin call, but it means it can't cross the WASM boundary as-is.
- **All four clients required** — `ActionContext::new` requires all four client `Arc`s. If a plugin only needs storage, it still needs to provide event/config/http mocks. This is a minor DX friction point. Could be addressed with `Option<Arc<...>>` or a builder pattern, but the current approach is simpler.
- **Test coverage** — Good coverage including a `DeniedStorage` mock that returns `PluginError::CapabilityDenied`. Tests verify the error code matches.

### src/error.rs (lines 1-222)

- **PluginError enum** — Six variants: `CapabilityDenied`, `NotFound`, `ValidationError`, `StorageError`, `NetworkError`, `InternalError`. Each has `message: String` and `detail: Option<String>`.
- **Serde tagging** — Uses `#[serde(tag = "code")]` with rename to SCREAMING_SNAKE_CASE. This produces `{"code": "STORAGE_ERROR", "message": "...", ...}`. Clean design for crossing the WASM boundary.
- **std::error::Error impl** — Implemented. This means `PluginError` can be used with `?` operator and boxed as `Box<dyn Error>`.
- **Missing: From impls** — No `From<anyhow::Error>` or `From<serde_json::Error>` for convenient conversion. Plugin authors will need to manually map errors. This is a minor DX gap.
- **Missing: Retryable flag** — The error type doesn't indicate whether an error is retryable. The pipeline executor needs this information. Currently, the executor would need to match on variant names to decide (e.g., `NetworkError` might be retryable, `ValidationError` is not). A `retryable()` method or a `Retryable` variant would help.
- **Test coverage** — Thorough: tests all variant codes, message/detail accessors, Display, serde round-trip, and std::error::Error trait.

### src/lifecycle.rs (lines 1-168)

- **LifecycleHooks trait** — Has `init` and `shutdown`, both with default no-op implementations. Both take `&ActionContext`. This is the correct design — plugins only override what they need.
- **Return type** — Both methods return `Result<(), PluginError>`. Returning `Err` from `init` aborts the plugin load. Good.
- **Not composable with CorePlugin** — `LifecycleHooks` is a standalone trait, not part of `CorePlugin`. `CorePlugin` has its own `on_load`/`on_unload`. For WASM plugins, `LifecycleHooks` is the mechanism. For native plugins, `CorePlugin::on_load`/`on_unload` is the mechanism. This is a clear separation but may confuse developers who aren't sure which model they're using.
- **Test coverage** — Tests cover default no-ops, overridden init that fails validation, and the test helper infrastructure. Good.

### src/credential_store.rs (lines 1-146)

- **CredentialStore trait** — Five methods: `store`, `retrieve`, `delete`, `delete_all_for_plugin`, `list_keys`. All async, all return `anyhow::Result`. Plugin-scoped by passing `plugin_id` to every method.
- **StoredCredential** — Custom `Debug` impl that redacts the `value` field. This is a security best practice.
- **Not `Serialize`/`Deserialize`** — `StoredCredential` doesn't derive serde traits. This is intentional — credential values should not be casually serialized.
- **Overlap with CredentialAccess** — `types.rs` defines `CredentialAccess` (another credential trait) that is used by `PluginContext`. `CredentialStore` is a different, lower-level trait. The SDK exposes both. Plugin authors need to understand the layering: `CredentialStore` is what Core implements; `CredentialAccess` is the higher-level interface plugins use through `PluginContext`.
- **Test coverage** — Construction, debug redaction, and clone tests. No async tests since the trait is only a definition here.

### src/retry.rs (lines 1-195)

- **RetryState** — Simple exponential backoff tracker. Fields are public: `failure_count`, `max_retries`, `backoff_min_secs`, `backoff_max_secs`.
- **Public fields** — All fields are `pub`, meaning plugin authors can mutate `failure_count` directly, bypassing `record_failure`/`record_success`. This is a minor encapsulation issue but provides flexibility.
- **Exponent cap** — `min(31)` prevents overflow in `1u64 << exponent`. Good defensive coding.
- **No jitter** — Exponential backoff without jitter can cause thundering herd problems when many plugins retry simultaneously. Adding optional jitter would improve production behavior but is not critical for a local-first engine.
- **Test coverage** — Tests cover initial state, doubling, cap, reset on success, exhaustion, and custom config. Good.

### src/wasm_guest.rs (lines 1-372)

- **HostRequest enum** — 18 variants covering document storage, blob storage, config, events, logging, and HTTP. Uses `#[serde(tag = "type")]` for dispatch.
- **HostResponse** — Success/error with `data: Option<Value>` and `error: Option<String>`. The `into_result` method is ergonomic.
- **Capability string inconsistency** — Doc comments reference `storage:doc:read`, `storage:doc:write` etc. (matching `traits::Capability::Display`), but the SDK's `types::Capability` enum uses PascalCase variants like `StorageRead` with no Display impl for the colon-separated format. The WASM host presumably uses the colon-separated strings, while native code uses the enum. This inconsistency should be reconciled.
- **No actual `host_call` FFI** — The `HostRequest` and `HostResponse` types are defined but there is no `extern "C" fn host_call(...)` import or wrapper function. The actual FFI bridge that a WASM plugin would use to call `host_call` is not yet implemented. This is noted as a Phase-gated TODO.
- **Limits module** — `DEFAULT_MEMORY_BYTES` (64MB), `DEFAULT_TIMEOUT_SECS` (30s), `DEFAULT_RATE_LIMIT` (1000/s). These are informational constants for plugin developers. Good reference material.
- **Test coverage** — All 18 request variants tested for serialization round-trip. HttpMethod serialization and limits constants tested.

### src/test/mod.rs (lines 1-18)

- Clean module with `MockMessageBuilder` and `MockStorageContext` re-exports.
- Feature-gated behind `test-utils`. Correct.

### src/test/mock_message.rs (lines 1-399)

- **MockMessageBuilder** — Convenience constructors for all 7 CDM types: `event`, `task`, `contact`, `note`, `email`, `file`, `credential`. Also `with_cdm`, `with_custom`, and builder methods for `source`, `correlation_id`, `auth`.
- **All CDM types covered** — Every CDM variant has a constructor. This is excellent DX for plugin test authoring.
- **SchemaValidated integration** — `with_custom` calls `SchemaValidated::new()` for validated custom payloads. Returns the validation error if the value doesn't match the schema.
- **Test coverage** — All constructors tested, builder overrides tested, custom payload tested. Comprehensive.

### src/test/mock_storage.rs (lines 1-494)

- **MockStorageContext** — In-memory mock that mirrors the `StorageContext` fluent API but is synchronous (no async, no capability checks).
- **Query execution** — Supports `Eq`, `NotEq`, `Gte`, `Lte`, `Contains` filter operations with JSON field extraction. Supports sorting and pagination.
- **Field extraction** — Uses serde serialization to extract fields from `PipelineMessage` payloads. Navigates into `data.value` for CDM payloads. Supports dot-notation for nested fields.
- **Update/delete by correlation_id** — Uses `metadata.correlation_id` as the record ID. This is a testing simplification; production uses the CDM record `id`. Plugin authors need to be aware of this difference.
- **Assertion helpers** — `assert_inserted` and `assert_contains` provide test-friendly assertions with clear failure messages. `dump()` exposes the full data for debugging.
- **No capability checking** — The mock doesn't enforce capabilities. This is intentional — mock storage is for unit tests, not capability enforcement tests. The real `StorageContext` handles capability checks.
- **Test coverage** — Tests cover insert, query, filters, limit/offset, delete, update, ordering, empty collection, and assertion failure. Good.

### tests/smoke_test.rs (lines 1-193)

- **End-to-end smoke test** — Implements both `Plugin` trait and invokes `register_plugin!`. Tests metadata, actions, execute, error handling, serialization, and type accessibility.
- **Single-dependency ergonomics test** — Verifies that all essential types (`Action`, `PipelineMessage`, `Severity`, `StorageContext`, `WasmCapability`, and all CDM types) are accessible from the SDK crate alone. This is a clever compile-time test.
- **Test coverage** — Covers the primary WASM plugin pathway. Good integration test.

### tests/plugin_actions_test.rs (lines 1-360)

- **Action contract tests** — Tests the `fn(PipelineMessage, &ActionContext) -> Result<PipelineMessage, PluginError>` action signature, typed storage access, lifecycle hooks, hard failure, soft warnings, and the full connector pattern (read config, fetch HTTP, normalise, write storage, emit event).
- **Connector pattern test** — The `connector_pattern_read_fetch_normalise_write_emit` test validates the full data flow a connector plugin would use. This is excellent for demonstrating the SDK's intended usage.
- **Soft warning pattern** — Tests that warnings can be appended to `input.metadata.warnings`. This is a good pattern for non-fatal issues.

---

## Problems Found

### Critical

- **Dual Capability enums with divergent variants** — `types::Capability` (SDK) has 13 variants including `CredentialsRead`, `CredentialsWrite`, and `Logging`. `traits::Capability` (WASM runtime) has 10 variants without those three. The `storage.rs` module uses `traits::Capability` for enforcement, but `CorePlugin::capabilities()` returns `Vec<types::Capability>`. A plugin could declare `Capability::CredentialsRead` in `CorePlugin::capabilities()` but this variant doesn't exist in the traits crate's capability enforcement system. The two enums need to be unified or their relationship made explicit with conversion functions.
  - **Location** — `src/types.rs:17-45` vs `packages/traits/src/capability.rs:13-34`
  - **Impact** — Plugin authors declaring `CredentialsRead`, `CredentialsWrite`, or `Logging` capabilities in `CorePlugin::capabilities()` may believe they are requesting those permissions, but the runtime enforcement layer (`traits::Capability`) doesn't know about them. This could lead to capabilities being declared but never checked, creating a false sense of security.

- **Two parallel plugin models without clear migration path** — The SDK exposes two plugin traits: `CorePlugin` (native, async, with `PluginContext`) and `Plugin` (WASM, sync, with `ActionContext`). There is no documentation guiding plugin authors on which to use, when the `CorePlugin` model will be deprecated, or how to migrate between them. The prelude exports both, making it easy for a developer to implement the wrong one.
  - **Location** — `src/traits.rs` (`CorePlugin`) vs `packages/traits/src/lib.rs` (`Plugin`)
  - **Impact** — Plugin authors may implement `CorePlugin` when they should implement `Plugin` (or vice versa), leading to plugins that don't work at runtime.

### Major

- **StorageContext.delete checks StorageWrite, not StorageDelete** — The `StorageContext::delete` method at line 127 calls `self.require(Capability::StorageWrite, ...)` but the traits crate defines a separate `Capability::StorageDelete` variant. The SDK's own `types::Capability` also defines `StorageDelete`. This means a plugin with only `StorageWrite` can delete records, and a plugin with only `StorageDelete` cannot use `StorageContext::delete`.
  - **Location** — `src/storage.rs:127`
  - **Impact** — Capability enforcement for delete operations is incorrect. Plugins that should only be able to write (not delete) can currently delete records through the StorageContext.

- **No actual `host_call` FFI in wasm_guest** — The `HostRequest` and `HostResponse` types are defined but there is no function that actually calls the host. Plugin authors have the request/response types but no way to use them from WASM guest code. The module describes the architecture in comments but doesn't implement the bridge.
  - **Location** — `src/wasm_guest.rs`
  - **Impact** — WASM plugins that need to call host functions (storage, events, config, HTTP) cannot do so through the SDK. They would need to implement their own FFI bridge.

- **register_plugin! creates a new instance per call** — Line 169: `let plugin = <$plugin_type>::default()`. Every invocation creates a fresh plugin instance. Any state set during init (if a separate init export existed) would be lost. This is by design for WASM sandbox isolation, but it's not documented. Plugin authors coming from native plugin development may expect state persistence.
  - **Location** — `src/macros.rs:169`
  - **Impact** — Plugin developers may write stateful plugins expecting state to persist across calls, leading to subtle bugs.

- **PluginContext and ActionContext serve overlapping roles** — `PluginContext` (in `types.rs`) provides credential access for `CorePlugin::on_load`. `ActionContext` (in `context.rs`) provides storage, events, config, HTTP for WASM plugin actions. Both are contexts, both have `plugin_id()`, but they provide different services and are used by different plugin models. This isn't a bug but is a major DX confusion risk.
  - **Location** — `src/types.rs:152-238` and `src/context.rs:94-136`

### Minor

- **Stale Phase 1 comment in PluginRoute** — Line 94 of `types.rs`: "Handler will be defined more concretely in Phase 1". The project is past Phase 1. This creates confusion about the project's current state.
  - **Location** — `src/types.rs:94-95`

- **No `From` impls for PluginError** — Plugin code frequently needs to convert `anyhow::Error`, `serde_json::Error`, or `std::io::Error` into `PluginError`. Without `From` impls, every conversion requires manual mapping. This adds boilerplate to plugin code.
  - **Location** — `src/error.rs`

- **No retryable indicator on PluginError** — The pipeline executor needs to decide whether to retry a failed action. Currently it must match on variant names. A `fn is_retryable(&self) -> bool` method (where `NetworkError` returns `true`, `ValidationError` returns `false`, etc.) would make retry logic cleaner.
  - **Location** — `src/error.rs`

- **MockStorageContext update/delete uses correlation_id instead of record ID** — The mock uses `metadata.correlation_id` to identify records for update/delete. Production storage uses the CDM record's `id` field. This mismatch means tests may pass with the mock but fail in production if a plugin relies on record ID for updates.
  - **Location** — `src/test/mock_storage.rs:66-94`

- **No jitter in RetryState** — Exponential backoff without jitter can cause thundering herd when multiple plugins retry simultaneously. For a local-first engine this is low-impact, but worth noting for future multi-instance deployment.
  - **Location** — `src/retry.rs:94-102`

- **HttpResponse not serializable** — `HttpResponse` in `context.rs` has no `Serialize`/`Deserialize` derives. If it needs to cross the WASM boundary, this would need to be added.
  - **Location** — `src/context.rs:72-80`

- **No `where_not_eq` on QueryBuilder** — The `FilterOp::NotEq` variant exists in the types crate but `QueryBuilder` in `storage.rs` doesn't expose a `where_not_eq` method. The mock storage's `MockQueryBuilder` also lacks it despite `match_filter` handling `NotEq`.
  - **Location** — `src/storage.rs:152-238`

- **StorageContext has no default limit** — When no `limit()` is called, `StorageQuery.limit` is `None`. If the backend doesn't enforce a default, queries could return unbounded result sets. The mock caps at 1000 via `unwrap_or(1000)` in `mock_storage.rs:171`, but the real `StorageContext` passes `None` through.
  - **Location** — `src/storage.rs:226-238`

- **Public fields on RetryState** — All fields (`failure_count`, `max_retries`, `backoff_min_secs`, `backoff_max_secs`) are `pub`, allowing plugins to directly mutate state and bypass the `record_failure`/`record_success` API. This is a minor encapsulation leak.
  - **Location** — `src/retry.rs:37-46`

- **Referenced .cargo/config.toml may not exist** — The crate-level docs at `lib.rs:40` mention "A reference .cargo/config.toml is included in this crate's directory" but no such file was found during review.
  - **Location** — `src/lib.rs:40`

---

## Recommendations

1. **Unify or explicitly bridge the two Capability enums** — Either make `traits::Capability` the single source of truth and add `CredentialsRead`, `CredentialsWrite`, `Logging` to it, or provide documented conversion functions (`types::Capability -> traits::Capability`) that return errors for unmapped variants. The current silent divergence is the highest-risk issue.

2. **Document the two plugin models clearly** — Add a section to the crate-level docs explaining when to use `CorePlugin` vs `Plugin`, which context type each uses, and the planned convergence path. Consider adding compile-time guidance (e.g., a deprecated attribute on `CorePlugin` if WASM `Plugin` is the future).

3. **Fix delete capability check** — Change `StorageContext::delete` to check `Capability::StorageDelete` instead of `Capability::StorageWrite`.

4. **Implement the WASM host_call bridge** — Add a `host_call(request: &HostRequest) -> Result<HostResponse, PluginError>` function to `wasm_guest.rs` that serializes the request, calls the host import, and deserializes the response. This completes the guest-side FFI story.

5. **Document statelessness of register_plugin!** — Add prominent doc comments to the `register_plugin!` macro explaining that each invocation creates a fresh instance and state is not preserved across calls. Include guidance on using host storage for persistent state.

6. **Add `From` impls for common error types into PluginError** — At minimum: `From<serde_json::Error>` -> `PluginError::InternalError` and a `PluginError::from_anyhow(e: anyhow::Error)` method.

7. **Add `is_retryable()` to PluginError** — Return `true` for `NetworkError` and `StorageError`, `false` for `ValidationError`, `CapabilityDenied`, `NotFound`, and `InternalError`.

8. **Add `where_not_eq` to QueryBuilder** — Expose the `NotEq` filter operation that already exists in the types crate.

9. **Consider a default limit on unbounded queries** — When `QueryBuilder::execute` is called without `limit()`, default to 1000 instead of passing `None` to the backend.

10. **Update or remove stale Phase 1 comments** — Clean up TODO comments that reference completed phases.
