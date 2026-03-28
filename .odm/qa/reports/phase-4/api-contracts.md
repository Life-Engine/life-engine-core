# API Contract Consistency Review

Review date: 2026-03-28

Comprehensive cross-layer analysis of trait definitions vs implementations, type consistency across crate boundaries, error propagation, serialization contracts, REST/GraphQL API contracts, plugin manifest enforcement, event contracts, storage contracts, and configuration contracts.

---

## Summary

Life Engine's contract system spans seven crates (`types`, `traits`, `plugin-sdk-rs`, `plugin-system`, `transport-rest`, `transport-graphql`, `workflow-engine`) and the Core binary. The contracts are individually well-designed, but cross-boundary integration reveals 8 critical contract breaks, 12 major inconsistencies, and 18 minor gaps. The most severe issues are:

1. Two incompatible `Capability` enums (13 variants vs 10 variants) with no conversion bridge
2. Two incompatible `Identity` types between auth middleware and handlers
3. Two incompatible `TriggerContext` enums between the types crate and workflow engine
4. Two incompatible `PluginError` enums between the SDK and plugin-system crate
5. The plugin SDK's `StorageContext.delete()` checks `StorageWrite` instead of `StorageDelete`, while the host function correctly checks `StorageDelete`

These are not compile-time errors because they cross crate boundaries via trait objects and JSON serialization. They are runtime integration failures waiting to happen.

---

## Contract Consistency Inventory

### 1. Capability Contract

The capability system has two independent enum definitions that must stay synchronized:

- `life_engine_traits::Capability` -- 10 variants, used by the plugin-system runtime for enforcement
  - `StorageRead`, `StorageWrite`, `StorageDelete`, `StorageBlobRead`, `StorageBlobWrite`, `StorageBlobDelete`, `HttpOutbound`, `EventsEmit`, `EventsSubscribe`, `ConfigRead`
  - Has `Display`/`FromStr` for colon-separated strings (`storage:doc:read`)
  - Used by `ApprovedCapabilities`, `check_capability()`, all host functions

- `plugin_sdk_rs::types::Capability` -- 13 variants, used by the SDK for plugin declaration
  - All 10 from traits plus `CredentialsRead`, `CredentialsWrite`, `Logging`
  - Has `Serialize`/`Deserialize` but no `Display`/`FromStr` for colon-separated strings
  - Used by `CorePlugin::capabilities()`, SDK documentation
  - Re-exported as the primary `Capability` in the SDK prelude

- The traits crate's `Capability` is re-exported from the SDK as `WasmCapability` to avoid naming collision

Contract breaks found:

- **CB-1 (Critical): Three SDK capabilities have no runtime enforcement counterpart.** A plugin declaring `CredentialsRead`, `CredentialsWrite`, or `Logging` in `CorePlugin::capabilities()` will pass SDK type checks but these capabilities are invisible to the plugin-system's `ApprovedCapabilities`. The manifest parser uses `traits::Capability::from_str()` which rejects the colon-separated equivalents of these three (they don't exist in traits). If the SDK capability enum values are ever serialized to the manifest, parsing will fail at load time.

- **CB-2 (Critical): SDK `StorageContext.delete()` checks `Capability::StorageWrite`, not `Capability::StorageDelete`.** At `plugin-sdk-rs/src/storage.rs`, the delete method calls `self.require(Capability::StorageWrite, ...)`. Meanwhile, the host function `host_storage_delete` at `plugin-system/src/host_functions/storage.rs:143` correctly checks `Capability::StorageDelete`. A plugin with `StorageWrite` but not `StorageDelete` can delete through the SDK's `StorageContext` but will be blocked by the host function — or vice versa, depending on which path is taken.

- **CB-3 (Major): No conversion functions exist between the two enums.** There is no `From<sdk::Capability> for traits::Capability` or mapping function. The SDK's `storage.rs` imports `traits::Capability` directly (bypassing the SDK enum), but `CorePlugin::capabilities()` returns `Vec<types::Capability>` (the SDK enum). There is no code that converts the return value of `CorePlugin::capabilities()` into `HashSet<traits::Capability>` for runtime enforcement.

### 2. Identity Contract

Three distinct `Identity` types exist:

- `life_engine_types::identity::Identity` -- canonical type with `subject: String`, `issuer: String`, `claims: HashMap<String, Value>`
  - Used by `WorkflowRequest.identity` field
  - Used by REST handlers (`transport-rest/src/handlers/mod.rs:17`) via `Extension<Identity>`
  - Has `guest()` constructor returning `subject: "anonymous"`, `issuer: "system"`

- `transport_rest::middleware::auth::Identity` -- auth middleware type with `user_id: String`, `provider: String`, `scopes: Vec<String>`
  - Inserted as `Extension<Identity>` by the auth middleware
  - Converts from `AuthIdentity` (from `life_engine_auth` crate)
  - Different struct layout from the canonical `Identity`

- `life_engine_auth::AuthIdentity` -- source identity from auth validation with `user_id`, `provider`, `scopes`

Contract breaks found:

- **CB-4 (Critical): Auth middleware inserts `middleware::auth::Identity` but handlers extract `life_engine_types::identity::Identity`.** These are different Rust types. Axum's `Extension<T>` is type-keyed: the handler will fail to extract the extension and return a 500 error for every authenticated request. The public route bypass inserts no identity at all, so the handler will also fail on public routes (it expects `Extension<Identity>` unconditionally).

- **CB-5 (Major): Field mapping is not 1:1 between the two Identity shapes.** The auth middleware's `user_id` maps conceptually to `subject`, `provider` maps to `issuer`, but `scopes: Vec<String>` has no direct equivalent in `claims: HashMap<String, Value>`. No conversion code exists in either direction.

### 3. Error Type Contract

Three `PluginError` types exist across the system:

- `plugin_system::error::PluginError` -- 11 variants, implements `EngineError` trait
  - Error codes: `PLUGIN_001` through `PLUGIN_010`, `CAP_001`, `CAP_002`
  - Severity: Fatal for most, Retryable for `DirectoryScanFailed`, `ExecutionFailed`, `Io`
  - Used by all host functions, capability checks, manifest parsing, WASM runtime

- `plugin_sdk_rs::error::PluginError` -- 6 variants, implements `std::error::Error` but NOT `EngineError`
  - Error codes: `CAPABILITY_DENIED`, `NOT_FOUND`, `VALIDATION_ERROR`, `STORAGE_ERROR`, `NETWORK_ERROR`, `INTERNAL_ERROR`
  - Serde-serializable with `#[serde(tag = "code")]`
  - Used by `ActionContext` client traits, plugin action return types

- `storage_sqlite::error::StorageError` -- 13 variants, implements `EngineError` trait
  - Error codes: `STORAGE_001` through `STORAGE_013`
  - Source module: `"storage-sqlite"`

Contract breaks found:

- **CB-6 (Major): Two `PluginError` types with the same name but different shapes.** The plugin-system's `PluginError` has variants like `RuntimeCapabilityViolation(String)`, `ExecutionFailed(String)`. The SDK's `PluginError` has variants like `CapabilityDenied { message, detail }`, `StorageError { message, detail }`. When errors cross the WASM boundary (serialized as JSON), the format must be agreed upon by both sides. The SDK errors use `serde(tag = "code")` producing `{"code": "CAPABILITY_DENIED", ...}`, while the plugin-system errors are not serializable at all (they use `thiserror` display formatting only).

- **CB-7 (Major): SDK `PluginError` does not implement `EngineError`.** All errors in the system are expected to implement `EngineError` for structured error codes, severity, and source module. The SDK's `PluginError` has a `code()` method but it returns SCREAMING_SNAKE_CASE codes (e.g., `"CAPABILITY_DENIED"`), not the structured `CAP_002` format used by the engine. There is no `severity()` or `source_module()` method, and no `From<PluginError> for Box<dyn EngineError>` conversion.

- **CB-8 (Major): `PluginOutput::Error` severity is a `String`, not `Severity` enum.** The WASM boundary macro (`register_plugin!`) returns `PluginOutput::Error { severity: String, ... }`. The host must parse `"Fatal"`, `"Retryable"`, `"Warning"` from strings. If the severity string doesn't match (case-sensitive), the host has no way to classify the error.

### 4. TriggerContext Contract

Two incompatible `TriggerContext` enums exist:

- `life_engine_types::identity::TriggerContext` -- in the types crate
  - Variants: `Endpoint { method, path }`, `Event { event_type, source }`, `Schedule { cron_expr }`
  - `Serialize`/`Deserialize` with `serde(tag = "type", rename_all = "snake_case")`
  - Not used by any runtime code

- `workflow_engine::types::TriggerContext` -- in the workflow engine
  - Variants: `Endpoint { method, path, body, auth }`, `Event { name, payload }`, `Schedule { workflow_id, fired_at }`
  - Not `Serialize`/`Deserialize`
  - Used by `build_initial_message()`, `WorkflowEngine::handle_endpoint()`

Contract breaks found:

- **CB-9 (Major): The two TriggerContext types have different fields and are not interchangeable.** The types crate's `Endpoint` has only `method` and `path`; the workflow engine's `Endpoint` additionally has `body` and `auth`. The types crate's `Event` has `event_type` and `source`; the workflow engine's has `name` and `payload`. The types crate's `Schedule` has `cron_expr`; the workflow engine's has `workflow_id` and `fired_at`. The types crate version appears to be an earlier design that was never updated.

### 5. SchemaError Contract

Two `SchemaError` types exist in the traits crate:

- `traits::schema::SchemaError` -- enum with 4 variants: `InvalidSchema`, `ValidationFailed`, `ImmutableField`, `NamespaceViolation`
  - Implements `EngineError` with codes `SCHEMA_001` through `SCHEMA_004`

- `traits::index_hints::SchemaError` -- struct with `message: String`
  - Does NOT implement `EngineError`
  - Used for index hint parsing errors

- **CB-10 (Minor): Name collision between two public types.** Both are public and exported from the same crate. Consumers importing both modules must use fully qualified paths.

### 6. REST API Contract

The REST transport defines routes in config, translates them to `WorkflowRequest`, and maps responses to HTTP status codes.

- Route config supports methods: GET, POST, PUT, DELETE (router)
- Route config namespace rules: REST routes must start with `/api/`, GraphQL with `/graphql`
- Response contract: `{ "data": ... }` for success, `{ "error": { "code": "...", "message": "..." } }` for errors
- Status mapping: `Ok -> 200`, `Created -> 201`, `NotFound -> 404`, `Denied -> 403`, `Invalid -> 400`, `Error -> 500`

Contract breaks found:

- **CB-11 (Major): PATCH method silently dropped.** `HandlerConfig` allows any method string, but `router/mod.rs` only handles GET, POST, PUT, DELETE. PATCH routes pass config validation but are silently ignored during router construction.

- **CB-12 (Major): Router and handlers are not connected.** The router builds debug handlers returning `{"workflow": ..., "params": ...}` while `handlers/mod.rs` builds proper `WorkflowRequest` objects. These are two parallel implementations. The router's handlers never call `build_workflow_request()`.

- **CB-13 (Minor): No HTTP method validation in config.** A route with `method: "FROBNICATE"` passes config validation. The router's `_ => router` match arm silently ignores it.

### 7. GraphQL API Contract

The GraphQL layer exists in two places:

- `packages/transport-graphql/` -- stub crate with workflow translation
- `apps/core/src/routes/graphql.rs` -- production async-graphql schema

Contract breaks found:

- **CB-14 (Major): `translate_request` hardcodes workflow name `"graphql.query"` for all operations.** Both queries and mutations are translated to `workflow: "graphql.query"`. If the workflow engine dispatches based on this name, mutations will be misrouted.

- **CB-15 (Major): GraphQL mutation `collection` parameter is unvalidated.** `createRecord`, `updateRecord`, `deleteRecord` accept an arbitrary string collection name. There is no allowlist check against the 7 CDM collections. A client can target internal tables or nonexistent collections.

- **CB-16 (Major): Generated schema types in `config.rs` are dead code.** `GeneratedGraphqlType` descriptors are never consumed by the production schema. The production schema uses hand-written types. The config module's `json_type_to_graphql` passes unknown types through verbatim, producing invalid GraphQL scalar names.

### 8. Plugin Manifest Contract

The manifest parser (`plugin-system/src/manifest.rs`) enforces:

- Plugin ID format: lowercase letters, digits, hyphens, starting with a letter
- Semver version validation
- Capability strings parsed via `traits::Capability::from_str()` (10 valid strings)
- Reserved collection names: `audit_log`, `system.*`
- Collection schema references: `cdm:<name>` or file paths
- Action timeout defaults to 30,000ms
- Events: declared emit and subscribe lists
- Config: optional JSON Schema

Contract breaks found:

- **CB-17 (Major): Manifest capabilities use colon-separated strings, SDK capabilities use enum variants.** The manifest file contains strings like `"storage:doc:read"` parsed via `Capability::from_str()`. The SDK's `Capability` enum uses PascalCase variants like `StorageRead`. The two systems use the same concept with different representations and no programmatic bridge.

- **CB-18 (Major): SDK capabilities `CredentialsRead`, `CredentialsWrite`, `Logging` cannot be expressed in manifest.** `traits::Capability::from_str()` has no mapping for these three. A plugin implementing `CorePlugin::capabilities()` with `Capability::CredentialsRead` cannot declare this capability in its manifest. The manifest parser will reject any string attempting to represent them.

- **CB-19 (Minor): Manifest `events.emit` is enforced at runtime, but `events.subscribe` is not.** The `host_events_emit` function validates against `declared_emit_events`, but `host_events_subscribe` does not validate against `declared_subscribe_events`. A plugin can subscribe to any event regardless of what it declared.

### 9. Event Contract

Events flow through: plugin action -> `host_events_emit` -> `WorkflowEventEmitter` -> `EventBus` -> workflow triggers.

- Event name format: dot-separated strings (e.g., `contact.created`, `task.deleted`)
- Payload: JSON Value
- Enrichment: host wraps the plugin's payload in `{ "source": plugin_id, "depth": N, "payload": <original> }`
- Subscription registration: emits `plugin.subscription.registered` meta-event

- SDK `CoreEvent` type: `{ event_type: String, payload: Value, source_plugin: String, timestamp: DateTime<Utc> }`
- Host function `EmitRequest`: `{ event_name: String, payload: Value }`
- WASM guest `HostRequest::EventEmit`: `{ event_type: String, payload: Value }`

Contract breaks found:

- **CB-20 (Major): Event field naming inconsistency across boundaries.** The SDK's `CoreEvent` uses `event_type` and `source_plugin`. The host function `EmitRequest` uses `event_name`. The WASM guest `HostRequest::EventEmit` uses `event_type`. The enriched payload uses `source` (not `source_plugin`). A plugin author reading `CoreEvent.event_type` then constructing `HostRequest::EventEmit { event_type }` is correct, but `EmitRequest` (the host function input) uses `event_name`, requiring a field rename across the WASM boundary that doesn't happen automatically.

- **CB-21 (Minor): `CoreEvent.timestamp` is not set by the host function.** The enriched event payload from `host_events_emit` includes `source` and `depth` but not `timestamp`. The `CoreEvent` type expects a `timestamp` field. If an event subscriber receives the raw enriched payload, it won't deserialize into `CoreEvent`.

### 10. Storage Contract

The storage contract spans: `StorageBackend` trait -> `StorageQuery`/`StorageMutation` types -> SQLite implementation -> host functions -> SDK `StorageContext`.

- `StorageBackend::execute(query: StorageQuery) -> Result<Vec<PipelineMessage>, Box<dyn EngineError>>`
- `StorageBackend::mutate(op: StorageMutation) -> Result<(), Box<dyn EngineError>>`
- `StorageMutation` variants: `Insert`, `Update`, `Delete`
- All mutations carry `plugin_id` for scoping
- Host functions re-scope `plugin_id` to prevent cross-plugin access

Contract consistency verified:

- Host function `host_storage_read` correctly deserializes `StorageQuery`, re-scopes `plugin_id`, delegates to `StorageBackend::execute`, serializes results
- Host function `host_storage_write` correctly deserializes `StorageMutation`, re-scopes via `scope_mutation`, delegates to `StorageBackend::mutate`
- Host function `host_storage_delete` correctly checks `StorageDelete` capability and validates the mutation is a `Delete` variant
- SQLite implementation correctly implements `StorageBackend` with all variants

Contract breaks found:

- **CB-22 (Major): SDK `StorageContext` uses `traits::Capability` directly, bypassing `types::Capability`.** The SDK's `StorageContext::new()` takes `HashSet<life_engine_traits::Capability>`. But `CorePlugin::capabilities()` returns `Vec<sdk::types::Capability>`. There is no code that converts between these. A plugin using the `CorePlugin` model cannot construct a `StorageContext` from its own declared capabilities without manual mapping.

- **CB-23 (Minor): WASM guest `HostRequest::StoreDelete` exists but the SDK's `StorageContext` also has `delete()`.** Two parallel paths to delete: the WASM guest `HostRequest` enum route (used by `register_plugin!` macro plugins) and the SDK `StorageContext` route (used by `CorePlugin` plugins). The capability check differs between them (see CB-2).

### 11. Configuration Contract

Configuration flows through TOML files parsed at multiple layers:

- `Transport::start()` takes `toml::Value` -- couples all transports to TOML format
- `RestTransportConfig` in `lib.rs`: `{ host: String, port: u16 }`
- `ListenerConfig` in `config/mod.rs`: `{ address: String, port: u16, tls: Option, auth: Option, ... }`
- `GraphqlTransportConfig`: `{ host: String, port: u16 }` with default port 4000
- `TransportConfig` in traits: `{ bind_address: String, port: u16, tls: Option }`

Contract breaks found:

- **CB-24 (Minor): Three competing address/port config shapes for REST transport.** `RestTransportConfig` uses `host`/`port`, `ListenerConfig` uses `address`/`port`, and `TransportConfig` uses `bind_address`/`port`. The field names differ for the same concept.

- **CB-25 (Minor): `Transport::start()` takes `toml::Value` even though `from_config()` already parsed it.** The `RestTransport::start()` receives a `toml::Value` that it ignores (it already parsed config in `from_config()`). The trait signature forces every transport to accept a `toml::Value` parameter it may not need.

---

## Type Duplication and Divergence Map

Types that appear in multiple crates with the same name but different definitions:

- `Capability` -- `traits::Capability` (10 variants) vs `plugin_sdk_rs::types::Capability` (13 variants)
- `Identity` -- `types::identity::Identity` (subject/issuer/claims) vs `transport_rest::middleware::auth::Identity` (user_id/provider/scopes)
- `PluginError` -- `plugin_system::error::PluginError` (11 variants, EngineError) vs `plugin_sdk_rs::error::PluginError` (6 variants, std::error::Error)
- `TriggerContext` -- `types::identity::TriggerContext` (3 variants, serde) vs `workflow_engine::types::TriggerContext` (3 variants, different fields)
- `SchemaError` -- `traits::schema::SchemaError` (enum, 4 variants) vs `traits::index_hints::SchemaError` (struct, 1 field)
- `HttpMethod` -- `plugin_sdk_rs::types::HttpMethod` (7 variants) vs `workflow_engine::loader::HttpMethod` (4 variants, used for trigger matching)
- `CollectionSchema` -- `plugin_sdk_rs::types::CollectionSchema` (name + schema Value) vs `traits::index_hints::CollectionDescriptor` (different purpose but overlapping concept)
- `BlobMeta` -- `traits::blob::BlobMeta` (orphaned, key: String) vs `plugin_system::host_functions::blob::BlobMeta` (key: String, compiled)

---

## Breaking Contract Issues

Ranked by severity and integration impact:

1. **Dual Capability enums (CB-1, CB-3, CB-17, CB-18)** -- Blocks plugins from declaring and enforcing 3 capabilities. No conversion bridge exists. The SDK re-exports the traits version as `WasmCapability`, creating confusion. Fix: add `CredentialsRead`, `CredentialsWrite`, `Logging` to `traits::Capability` and remove the SDK's separate enum.

2. **Dual Identity types (CB-4, CB-5)** -- Auth middleware inserts one type, handlers extract another. Every authenticated request will fail with a 500 at integration time. Fix: remove `middleware::auth::Identity`, convert `AuthIdentity` directly to `types::identity::Identity` in the middleware.

3. **SDK delete capability mismatch (CB-2)** -- The SDK checks `StorageWrite` for delete, the host function checks `StorageDelete`. Plugin behavior will differ depending on the code path. Fix: change `StorageContext::delete()` to check `Capability::StorageDelete`.

4. **Dual PluginError types (CB-6, CB-7, CB-8)** -- Two different error shapes cross the WASM boundary. The host must deserialize SDK errors but the formats don't align. Fix: define a shared error serialization format (the SDK's `serde(tag = "code")` format is the better design) and implement `EngineError` for the SDK's `PluginError`.

5. **Dual TriggerContext types (CB-9)** -- The types crate version is stale and unused. Fix: remove `types::identity::TriggerContext` and use the workflow engine's version as the single source of truth, or consolidate them.

6. **Event field naming inconsistency (CB-20)** -- `event_type` vs `event_name` across three layers. Fix: standardize on one field name (`event_type` matches the SDK) and rename `EmitRequest.event_name` to `event_type`.

7. **Router/handler disconnect (CB-12)** -- Two parallel handler implementations in the REST transport. Fix: wire the router to use the handlers module's `handle_with_body`/`handle_without_body` instead of inline closures.

8. **PATCH method dropped (CB-11)** -- Silent data loss for any PATCH-configured route. Fix: add `routing::patch` to the router builder.

---

## Recommendations

### Immediate (blocking integration)

1. **Unify `Capability` to a single enum in `traits` crate.** Add `CredentialsRead`, `CredentialsWrite`, `Logging` variants. Remove the SDK's `types::Capability`. Update `CorePlugin::capabilities()` to return `Vec<traits::Capability>`. Remove the `WasmCapability` alias.

2. **Fix the Identity type mismatch.** In `transport-rest/src/middleware/auth.rs`, convert `AuthIdentity` to `life_engine_types::identity::Identity` directly. Map `user_id -> subject`, `provider -> issuer`, `scopes` -> `claims` (as a JSON array value). Remove the middleware's private `Identity` struct.

3. **Fix `StorageContext::delete()` capability check.** Change the `require` call from `Capability::StorageWrite` to `Capability::StorageDelete` in `plugin-sdk-rs/src/storage.rs`.

4. **Standardize event field naming.** Rename `EmitRequest.event_name` to `event_type` to match the SDK's `CoreEvent` and `HostRequest::EventEmit`.

### Short-term (required for end-to-end workflows)

5. **Bridge the two `PluginError` types.** Either implement `EngineError` for the SDK's `PluginError` (mapping `CAPABILITY_DENIED -> CAP_002`, `STORAGE_ERROR -> STORAGE_001`, etc.) or define a shared serialization format for the WASM boundary.

6. **Consolidate `TriggerContext`.** Remove the unused `types::identity::TriggerContext`. If it needs to be in the types crate, move the workflow engine's version there (with `Serialize`/`Deserialize`).

7. **Wire the REST router to the handlers module.** Replace the router's inline debug closures with calls to `handle_with_body`/`handle_without_body`.

8. **Add PATCH method support to the router.** Add a `"PATCH" => router.route(path, routing::patch(handler))` arm.

9. **Validate GraphQL mutation collection names.** Add an allowlist check against the 7 CDM collection names before dispatching to storage.

### Medium-term (contract hardening)

10. **Rename `SchemaError` in `index_hints.rs`** to `IndexHintError` or merge it into `schema::SchemaError`.

11. **Consolidate REST config types.** Remove `RestTransportConfig` from `lib.rs` and use `ListenerConfig` throughout, or vice versa.

12. **Add conversion traits between crate error types.** Implement `From<StorageError> for plugin_system::PluginError` and `From<plugin_sdk::PluginError> for Box<dyn EngineError>` to make error propagation explicit and type-safe.

13. **Validate manifest event subscriptions at runtime.** Add `declared_subscribe_events` to `EventsHostContext` and validate in `host_events_subscribe`, matching the enforcement already done for `host_events_emit`.

14. **Add a `Display`/`FromStr` implementation to the SDK's `HttpMethod`.** This allows consistent string representation across the WASM boundary and manifest files.

15. **Fix the Transport trait signature.** Either remove the `config: toml::Value` parameter from `Transport::start()` (since transports already parse config in their constructors) or make it `config: &dyn Any` to decouple from the TOML format.
