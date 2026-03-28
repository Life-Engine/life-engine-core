# Architecture Coherence Review

- **Scope** — Full system architecture analysis across all crates
- **Date** — 2026-03-28
- **Inputs** — Phase 1-3 review reports, ARCHITECTURE.md, Cargo.toml dependency graph, source code inspection

---

## Architecture Overview (As-Built)

Life Engine Core is a Rust workspace of 25 crates organized into four conceptual layers: foundation types/traits, infrastructure services, transport interfaces, and a top-level orchestrating binary. First-party WASM plugins live alongside the core in the monorepo.

The stated architecture in `ARCHITECTURE.md` describes a clean four-layer system where Core is a "thin orchestrator" containing only three files (`main.rs`, `config.rs`, `shutdown.rs`). The as-built reality diverges significantly: `apps/core/src/` contains 34 source files totaling approximately 22,600 lines of Rust code, housing subsystems that the architecture document says should live in independent crates.

### What actually exists

The workspace members are:

- **Foundation** — `types`, `traits`, `crypto`, `plugin-sdk-rs`
- **Infrastructure** — `storage-sqlite`, `auth`, `workflow-engine`, `plugin-system`
- **Transports** — `transport-rest`, `transport-graphql`, `transport-caldav`, `transport-carddav`, `transport-webhook`
- **Utilities** — `dav-utils`, `test-utils`, `test-fixtures`
- **Binary** — `apps/core`
- **Plugins** — 10 first-party WASM plugins under `plugins/engine/`

### What the architecture document describes but does not match

- Core should be thin (3 files): it is 34 files / 22.6k lines
- Repository structure lists directories that do not match reality (`plugins/engine/` is the actual path, not `plugins/connector-email/`)
- The dependency graph in `ARCHITECTURE.md` shows `crypto` depending on `types` (correct) but claims it depends only on `types` — it does, though the graph formatting implies a vertical chain that does not exist
- The document says plugins depend only on `plugin-sdk`, which re-exports types + traits — this is true for the SDK but the plugin-system crate also depends on workflow-engine, creating a cross-layer dependency

---

## Layer Diagram (As-Built Dependency Graph)

The actual dependency relationships from `Cargo.toml` files:

```
Layer 0 — Foundation (no internal deps)
  types

Layer 1 — Contracts (depends on Layer 0)
  traits → types
  crypto → types

Layer 2 — SDK (depends on Layers 0-1)
  plugin-sdk-rs → types, traits

Layer 3 — Infrastructure (depends on Layers 0-2)
  storage-sqlite → types, traits, crypto
  auth → types, traits, crypto
  workflow-engine → types, traits, storage-sqlite  [!]
  plugin-system → types, traits, workflow-engine   [!]

Layer 4 — Transports (depends on Layers 0-1, plus auth)
  transport-rest → types, traits, auth
  transport-graphql → types, traits

Layer 5 — Orchestrator
  apps/core → everything
```

### Layer violations

Two notable dependency direction violations exist:

- **workflow-engine depends on storage-sqlite** — The workflow engine's migration subsystem directly depends on `life-engine-storage-sqlite` and `rusqlite`. This means the workflow engine is coupled to a specific storage implementation rather than depending on the abstract `StorageBackend` trait. If the storage backend is swapped (e.g., to PostgreSQL), the workflow engine must also change. This violates the architecture's principle that the data layer is "swappable without changing any other layer."

- **plugin-system depends on workflow-engine** — The plugin system crate imports `life-engine-workflow-engine` for the `PluginExecutor` trait and event emission types. This creates a dependency from the plugin infrastructure layer up into the workflow orchestration layer. The result is that plugin-system and workflow-engine form a tightly coupled pair rather than independent modules. The `PluginExecutor` trait should live in `traits` so that both crates can depend on it independently.

---

## Coherence Assessment

### Clean separations

The following boundaries are well-maintained:

- **types has zero internal dependencies** — Correct foundation crate. All CDM types, pipeline messages, and storage query types are defined here. Other crates depend on it without creating cycles.

- **traits depends only on types** — Correct. Infrastructure contracts (`StorageBackend`, `Transport`, `Plugin`, `EngineError`) are defined here, enabling multiple implementations.

- **crypto depends only on types** — Correct. Encryption primitives are isolated and reusable.

- **plugin-sdk-rs depends only on types + traits** — Correct. The SDK re-exports everything a plugin author needs from a single dependency. This is the intended "pit of success" for plugin developers.

- **Plugins are truly decoupled** — First-party plugins under `plugins/engine/` depend only on `plugin-sdk-rs` and serde. They have no knowledge of storage implementations, transports, or the workflow engine. This is exactly right.

- **Transport crates are lightweight** — `transport-graphql` depends only on types + traits (plus async-graphql). `transport-rest` adds auth as a dependency for middleware, which is reasonable.

### Structural problems

- **Core is not thin** — The architecture declares Core should be a thin orchestrator with 3 files. The as-built Core contains 34 modules including full implementations of: search (tantivy), federation, household management, conflict resolution, connector orchestration, credential storage, identity management, plugin signing, WASM runtime bridging, rate limiting, audit logging, and PostgreSQL storage. This is the largest crate in the workspace by far. Many of these should be extracted per the architecture plan.

- **Two config systems running in parallel** — Core's `config.rs` has a legacy YAML-based config system and a new TOML-based `startup` submodule. Both are active during startup. The transport-rest crate has its own config split (the old `config.rs` is deleted, replaced by a `config/` directory with a new `mod.rs`). This transition is in-progress but creates confusion about which config path is authoritative.

- **SchemaRegistry exists in three places** — `packages/traits/src/schema.rs` defines a `SchemaRegistry` with CDM schema validation. `packages/workflow-engine/src/schema_registry.rs` defines a `SchemaRegistry` for plugin private collection schemas. `apps/core/src/schema_registry.rs` defines a `SchemaRegistry` (with `ValidatedStorage` wrapper) that bridges the two. Three types named `SchemaRegistry` serving different purposes is a naming collision and a cohesion problem.

---

## Duplication and Inconsistency Inventory

### Type and enum duplication

1. **Capability enum (2 divergent definitions)**
   - `packages/traits/src/capability.rs` — 10 variants
   - `packages/plugin-sdk-rs/src/types.rs` — 13 variants (adds `CredentialsRead`, `CredentialsWrite`, `Logging`)
   - The SDK re-exports the traits version as `WasmCapability` and its own as `Capability`. Plugin authors see both. The three extra variants (`CredentialsRead`, `CredentialsWrite`, `Logging`) declared via the SDK's `CorePlugin::capabilities()` have no matching enforcement in the runtime capability checking system. This is the highest-priority duplication issue.

2. **SchemaError (2 definitions in traits crate)**
   - `packages/traits/src/schema.rs` — `enum SchemaError` with 4 variants implementing `EngineError`
   - `packages/traits/src/index_hints.rs` — `struct SchemaError { message: String }` without `EngineError`
   - Same name, different types, both public, within the same crate.

3. **PluginManifest (2 definitions)**
   - `packages/plugin-system/src/manifest.rs` — Full manifest with validation, actions, capabilities, events
   - `apps/core/src/manifest.rs` — Simplified runtime manifest with different fields
   - Same name, different structures, different validation rules.

4. **SchemaRegistry (3 definitions)**
   - `packages/traits/src/schema.rs` — CDM schema validation registry
   - `packages/workflow-engine/src/schema_registry.rs` — Plugin private collection schemas
   - `apps/core/src/schema_registry.rs` — Bridging wrapper
   - Three types with the same name serving different scopes.

5. **StorageCapability / Capability overlap**
   - `packages/traits/src/storage_context.rs` (orphaned) defines `StorageCapability` that duplicates the `storage:*` variants in `capability.rs::Capability`. If integrated, this creates a second capability parsing path.

6. **TriggerContext (2 definitions)**
   - `packages/types/src/identity.rs` — `TriggerContext` enum with `Endpoint`, `Event`, `Schedule` variants
   - `packages/workflow-engine/src/types.rs` — `TriggerContext` enum with `Endpoint`, `Event`, `Schedule` variants (different fields)
   - Both serve the same conceptual purpose. The workflow engine's version has request body/auth data; the types version has `source_id` and `source_type`.

7. **PluginError (2 definitions)**
   - `packages/plugin-system/src/error.rs` — Host-side plugin errors (PLUGIN_001-010, CAP_001-002)
   - `packages/plugin-sdk-rs/src/error.rs` — Guest-side plugin errors (CAPABILITY_DENIED, NOT_FOUND, etc.)
   - These serve legitimately different sides of the WASM boundary but share the same name, which can confuse when debugging cross-boundary issues.

### Pattern inconsistencies

1. **Error handling patterns vary across crates**
   - `traits` — `Box<dyn EngineError>` for trait boundary errors
   - `plugin-sdk-rs` — `PluginError` enum with `thiserror`
   - `workflow-engine` — `WorkflowError` enum with `thiserror`, implements `EngineError`
   - `plugin-system` — `PluginError` enum with `thiserror`, implements `EngineError`
   - `storage-sqlite` — `StorageError` enum with `thiserror`
   - `apps/core` — Mix of `anyhow::Result`, module-specific error types, and `Box<dyn EngineError>`
   - The traits crate defines the canonical `EngineError` pattern but not all crates implement it consistently. `storage-sqlite::StorageError` does not implement `EngineError`.

2. **Mutex usage (std vs tokio)**
   - `plugin-system/src/execute.rs` — Uses `std::sync::Mutex` in async context (identified as critical bug)
   - `apps/core/src/auth/middleware.rs` — Uses `tokio::sync::Mutex`
   - `apps/core/src/conflict.rs` — Uses `std::sync::Mutex` (with documented justification)
   - `apps/core/src/household.rs` — Uses `Arc<RwLock<HashMap>>` (tokio)
   - `workflow-engine/src/scheduler.rs` — Uses `tokio::sync::Mutex` (unnecessarily, per review)
   - No consistent policy on when to use std vs tokio synchronization primitives.

3. **Config parsing**
   - Traits crate `Transport::start()` takes `toml::Value` — couples to TOML format
   - `storage-sqlite::init()` takes `toml::Value` — same coupling
   - Plugin manifests use TOML
   - Workflow definitions use YAML
   - Core config uses YAML (legacy) and TOML (new startup module)
   - The dual config format is documented but creates an inconsistent developer experience.

---

## Data Flow Analysis

### Happy path: External data ingestion

```
1. Transport receives HTTP request (e.g., POST /email/sync)
2. Transport checks WorkflowEngine::has_endpoint()
3. WorkflowEngine builds TriggerContext, creates initial PipelineMessage
4. PipelineExecutor runs steps sequentially:
   a. Step 1: connector-email.fetch
      - Plugin calls host_storage_read (host function)
      - Plugin calls host_http_request (host function)
      - Plugin returns PipelineMessage with fetched emails
   b. Step 2: search-indexer.index
      - Plugin calls host_storage_write (host function)
      - Plugin returns PipelineMessage
5. PipelineExecutor returns result through WorkflowEngine
6. Transport sends HTTP response
```

### Data flow gaps

1. **Storage writes bypass the workflow engine** — The Core binary's `/api/data/{collection}` routes write directly to storage via `ValidatedStorage`, bypassing the workflow engine entirely. This means CRUD operations through the REST API do not trigger workflows, event emissions, or search indexing. Only workflow-mediated operations (via endpoint triggers) go through the pipeline. This creates two write paths with different side effects.

2. **Search indexing is disconnected from the pipeline** — The search indexer exists as a plugin (`search-indexer`) designed to be a workflow step, but Core also has an in-memory tantivy search index (`apps/core/src/search.rs`) that is indexed via the message bus subscriber pattern. These are two separate search systems that do not coordinate.

3. **Event bus duplication** — Core has its own `MessageBus` (`apps/core/src/message_bus.rs`) using `tokio::sync::broadcast`, and the workflow engine has its own `EventBus` (`packages/workflow-engine/src/event_bus.rs`) also using `tokio::sync::broadcast`. Events published on one bus are not visible on the other. The Core message bus handles storage mutations and audit events; the workflow engine event bus handles plugin-emitted events and workflow triggers. This split means a plugin event cannot trigger a Core subscriber (e.g., audit logging), and a storage mutation cannot trigger a workflow.

4. **Plugin host functions call through to Core, not through the workflow engine** — When a WASM plugin calls `host_storage_write`, the host function in `plugin-system` writes directly using the injected `StorageHostContext`. This bypasses schema validation in the workflow engine's `SchemaRegistry`, bypasses the Core's `ValidatedStorage` wrapper, and bypasses the Core message bus (so no audit events or search index updates for plugin-initiated writes). The plugin-system's storage host function does enforce plugin_id scoping and capability checks, but the downstream side effects are lost.

5. **Blob storage is not wired end-to-end** — The traits crate has orphaned `blob.rs` defining `BlobStorageAdapter`. The plugin-system has `host_functions/blob.rs` with blob storage host functions. The storage-sqlite crate has an orphaned `blob_fs.rs`. But the injection layer in plugin-system does not build blob host functions, and the storage crate does not export a blob backend. Blob storage is designed but not connected.

---

## Error Propagation Analysis

### The intended pattern

`ARCHITECTURE.md` defines a clean error model:

- Each module has its own error type
- At module boundaries, errors implement `EngineError`
- The workflow engine uses `Severity` (Fatal/Retryable/Warning) for control flow

### As-built deviations

1. **storage-sqlite does not implement EngineError** — `StorageError` defines error variants but does not implement the `EngineError` trait. This means storage errors cannot carry structured error codes or severity levels through the workflow engine. The plugin-system's storage host function catches storage errors and wraps them, but the original structure is lost.

2. **PluginError (SDK) does not carry severity** — The SDK's `PluginError` has no `severity()` method or `Severity` field. When a plugin action fails, the pipeline executor cannot determine retry behavior from the error type. The Phase 2 report notes this gap and recommends adding `is_retryable()`.

3. **Warning severity is partially handled** — The workflow engine executor recognizes `Warning` severity but skips step logging for warning-severity errors (a bug identified in the Phase 2 review). Warning-level errors also have inconsistent handling: the `PluginOutput::Error` type from WASM sends severity as a string, which must be parsed back to the enum.

4. **RwLock/Mutex poisoning is handled inconsistently**
   - `plugin-system/execute.rs` — `.lock().unwrap()` (panics on poison)
   - `workflow-engine/schema_registry.rs` — `.read().ok()?` (silently returns None)
   - `apps/core/federation.rs` — `.write().unwrap()` (panics on poison)
   - No consistent policy: some crates panic, some silently degrade, some log warnings.

---

## Configuration Architecture

### Config flow

```
CLI args + env vars + YAML file → CoreConfig (in apps/core)
  → Each module gets its section as toml::Value
  → Module parses its own config internally
```

### Issues

1. **Two config formats in transition** — YAML (legacy) and TOML (new). Both active during startup. The `CoreConfig` type handles both, but downstream modules (transports, storage) receive `toml::Value`, coupling them to the TOML format regardless of the source.

2. **Config coupling via toml::Value** — The `Transport::start()` trait method accepts `toml::Value`. The `StorageBackend::init()` (in practice, `SqliteStorage::init`) also takes `toml::Value`. This means every implementer of these traits is coupled to the TOML library. A more abstract config type (e.g., `serde_json::Value` or a custom `ConfigSection` type) would decouple modules from the serialization format.

3. **Redaction gap** — `CoreConfig::to_redacted_json()` redacts OIDC secrets and PostgreSQL passwords but does not redact `storage.passphrase`. The passphrase is a plaintext `Option<String>` that can appear in the `GET /api/system/config` response.

4. **No config validation in workflow engine** — `WorkflowConfig` has only a `path` field. There is no validation for event bus capacity, scheduler concurrency, job TTL, executor timeout, or other operational parameters that are currently hardcoded.

---

## State Management

### In-memory state (lost on restart)

- **Federation peers and sync cursors** — `FederationStore` uses `HashMap` in memory
- **Household memberships and invites** — `HouseholdStore` uses `HashMap` in memory
- **Search index** — tantivy `Index::create_in_ram()`
- **Conflict store** — `ConflictStore` uses `HashMap` in memory
- **Rate limiter state** — Per-IP counters in `DashMap`
- **Plugin lifecycle state** — `LifecycleManager` tracks plugin phases in `HashMap`
- **Workflow job registry** — `PipelineExecutor` tracks async job status in `HashMap` (unbounded, never cleaned automatically)

### Persisted state

- **CDM data** — SQLite/SQLCipher via storage-sqlite
- **Credentials** — Encrypted in SQLite via credential_store
- **Audit log** — SQLite table via audit module
- **Migration history** — SQLite table via migration engine

### State management gaps

- Federation and household state must be persisted for production use. These are identified in the Phase 3 report as major issues.
- The search index must be either persisted or rebuilt on startup. Current in-memory implementation means all search results are lost on restart.
- The workflow job registry grows without bound. No automatic cleanup exists.

---

## Orphaned and Dead Code Inventory

### Orphaned files (not in module tree, would not compile)

- `packages/traits/src/blob.rs` — Well-designed blob storage trait, imports types that do not exist in the traits crate
- `packages/traits/src/storage_context.rs` — Storage enforcement layer with capability checks, imports missing types and missing `tokio` dependency
- `packages/traits/src/storage_router.rs` — Storage dispatcher with timeout enforcement, same missing imports/dependencies
- `packages/storage-sqlite/src/blob_fs.rs` — Filesystem-backed blob storage implementation, not wired into lib.rs
- `packages/storage-sqlite/src/health.rs` — Health check types, not wired into lib.rs

### Dead code in compiled modules

- `packages/traits/src/types.rs` — Empty module (only a doc comment), declared in lib.rs
- `packages/traits/src/schema_versioning.rs::ChangeKind::FieldRenamed` — Variant defined but never produced by the comparator
- `packages/plugin-system/src/host_functions/events.rs` — `declared_emit_events` field exists but is always `None` at injection time; event name validation is dead code in practice
- `packages/plugin-system/src/host_functions/events.rs` — `execution_depth` tracking exists but is always 0; cascade detection is dead code
- `packages/plugin-system/src/host_functions/http.rs` — `allowed_domains` field exists but is always `None`; domain restriction checking is dead code
- `packages/plugin-system/src/injection.rs` — `injected_function_names()` lists blob host functions, but `build_host_functions()` does not build them; blob capabilities advertised but non-functional

### New untracked files (work-in-progress)

- `packages/types/src/identity.rs` — New identity types, declared in lib.rs but not re-exported at crate root
- `packages/types/src/workflow.rs` — New workflow request/response types, same issue
- `packages/plugin-sdk-rs/src/context.rs` — ActionContext for WASM plugin model
- `packages/plugin-sdk-rs/src/error.rs` — PluginError for guest-side errors
- `packages/plugin-sdk-rs/src/lifecycle.rs` — LifecycleHooks trait
- `packages/transport-rest/src/config/` — New modular config directory replacing deleted `config.rs`
- `packages/transport-rest/src/middleware/` — New middleware directory (auth, cors, error handling, logging)
- `packages/transport-rest/src/router/` — New router directory
- `packages/transport-rest/src/listener.rs` — TLS listener
- `packages/workflow-engine/src/triggers/` — New triggers directory
- Various new test files across multiple crates

---

## Plugin Architecture Assessment

### The SDK / System / Runtime split

The plugin architecture has three components:

- **plugin-sdk-rs** — The crate plugin authors depend on. Re-exports types and traits, provides the `register_plugin!` macro, `StorageContext`, mock test helpers, and the action context. Clean API surface.

- **plugin-system** — The host-side crate that Core uses to discover, load, validate, and execute WASM plugins. Handles manifest parsing, capability enforcement (two-layer: injection gating + runtime checks), host function injection, and lifecycle management.

- **apps/core/src/wasm_runtime.rs** — The WASM host bridge in the Core binary. Provides the actual host function implementations that delegate to Core subsystems (storage, events, config).

### Assessment

The SDK/system split is conceptually sound. Plugin authors depend on one crate (SDK) and the host depends on another (system). However:

1. **The two plugin trait models create confusion** — `CorePlugin` (native, async) and `Plugin` (WASM, sync) coexist in the SDK. Both are re-exported in the prelude. There is no compile-time or documentation-level guidance on which to use. The `CorePlugin` model appears to be a transitional artifact from before the WASM architecture, but it is still actively maintained and tested.

2. **The WASM guest-side bridge is incomplete** — `wasm_guest.rs` defines `HostRequest` and `HostResponse` envelope types for all 18 host function variants, but does not implement the actual FFI function that calls the host. WASM plugins currently cannot call host functions through the SDK. This is the most critical functional gap in the plugin architecture.

3. **Blob storage is designed but not wired** — Blob types exist in traits (orphaned), host functions exist in plugin-system, storage implementation exists in storage-sqlite (orphaned), but the injection layer never connects them. Blob storage is non-functional across the entire stack.

4. **Capability enforcement has gaps** — The injection layer never populates `declared_emit_events`, `execution_depth`, or `allowed_domains` from manifest data. These features are implemented in the host functions but are permanently in their default (disabled) states.

---

## Migration Path Assessment

### What ARCHITECTURE.md says

The migration plan lists 11 steps for incremental extraction from the monolithic Core into independent crates. Steps 1-9 create the package structure; step 10 slims Core; step 11 converts plugins to WASM.

### Current progress

Steps 1-9 are substantially complete — all listed crates exist and have meaningful implementations:

- `packages/types` — Complete
- `packages/traits` — Complete (compiled portions), with three orphaned files for the next phase
- `packages/crypto` — Complete
- `packages/plugin-sdk-rs` — Substantially complete, WASM bridge incomplete
- `packages/storage-sqlite` — Complete (compiled portions), blob storage orphaned
- `packages/auth` — Complete
- `packages/workflow-engine` — Complete with migration subsystem
- `packages/transport-rest` — In active redesign (config split, new middleware/router structure)
- `packages/transport-graphql` — Complete

Steps 10-11 are in progress but far from complete:

- **Step 10 (slim Core)** — Core still contains 34 modules / 22.6k lines. The following should be extracted: search, federation, household, conflict resolution, credential store, identity management, plugin signing, audit, sync primitives, PostgreSQL storage, rate limiting, connector orchestration. This is the largest remaining extraction effort.

- **Step 11 (WASM plugins)** — First-party plugins exist as compiled Rust crates targeting WASM, but the SDK lacks the FFI bridge for guest-to-host calls. Plugins can be loaded and their `execute` entry point called, but they cannot interact with storage, events, HTTP, or config through the host function system end-to-end.

### Coherence of the migration

The incremental approach is sound — each crate can be developed and tested independently, and the workspace compiles at each step. The orphaned files in traits and storage-sqlite represent a clear next phase: once the missing types are defined, these modules can be wired in.

The main coherence risk is the **dual write path problem**: as functionality is extracted from Core into crates, there is a transition period where both Core's internal implementation and the crate implementation exist. Currently, Core has its own search, schema registry, credential store, and WASM runtime alongside the extracted crates. The path to consolidation (replacing Core's internal modules with crate calls) needs explicit sequencing to avoid prolonged duplication.

---

## Recommendations

### Priority 1 — Fix layer violations

1. **Move `PluginExecutor` trait from workflow-engine to traits** — The `PluginExecutor` trait is the contract between the workflow engine and the plugin system. Both crates depend on each other through this trait. Moving it to the traits crate breaks the cycle: workflow-engine depends on traits (for PluginExecutor), and plugin-system depends on traits (for PluginExecutor), with no direct dependency between them.

2. **Remove workflow-engine's dependency on storage-sqlite** — The migration subsystem should depend on `StorageBackend` (from traits), not on `SqliteStorage` directly. Alternatively, extract the migration subsystem into its own crate (`packages/migration-engine`) that is allowed to know about SQLite.

### Priority 2 — Unify duplicated concepts

3. **Unify the Capability enums** — Add `CredentialsRead`, `CredentialsWrite`, and `Logging` to `traits::Capability`. Remove the parallel enum from `plugin-sdk-rs::types`. The traits version becomes the single source of truth.

4. **Rename duplicate types** — Rename `index_hints::SchemaError` to `IndexHintError`. Rename Core's `PluginManifest` to `DiscoveredManifest` or `RuntimeManifest`. Distinguish the three `SchemaRegistry` types by scope: `CdmSchemaRegistry`, `PluginSchemaRegistry`, `ValidatingSchemaRegistry`.

5. **Consolidate TriggerContext** — The types crate and workflow engine both define `TriggerContext`. Determine which is canonical and remove the other, or merge them.

### Priority 3 — Connect the data flow

6. **Bridge the two event buses** — Core's `MessageBus` and the workflow engine's `EventBus` must be connected. Storage mutations published on the Core bus should be visible as events in the workflow engine, enabling workflows to trigger on data changes. Plugin-emitted events should reach Core subscribers (audit, search indexing).

7. **Wire up blob storage end-to-end** — Define `BlobStorageAdapter` types in traits (un-orphan `blob.rs`), implement in storage-sqlite (un-orphan `blob_fs.rs`), add blob builders to the plugin-system injection layer, and thread the blob backend through `InjectionDeps`.

8. **Ensure plugin writes trigger side effects** — When a WASM plugin writes to storage via `host_storage_write`, the write should publish an event to the Core message bus (or a unified bus) so that search indexing and audit logging occur.

### Priority 4 — Continue extraction from Core

9. **Extract subsystems from Core in order of independence:**
   - `packages/search` — Extract tantivy search from Core (no dependencies on other Core modules)
   - `packages/federation` — Extract federation (needs persistence first)
   - `packages/household` — Extract household management (needs persistence first)
   - `packages/identity` — Extract identity/credential management
   - `packages/rate-limit` — Extract rate limiting
   - `packages/audit` — Extract audit logging

10. **Persist in-memory state** — Federation peers, household memberships, and search index must be persisted to SQLite or a similar store before they can be extracted into independent crates.

### Priority 5 — Standardize cross-cutting patterns

11. **Establish a Mutex policy** — Document when to use `std::sync::Mutex` vs `tokio::sync::Mutex`. General rule: use `std::sync::Mutex` when the lock is never held across `.await` points and the critical section is short. Use `tokio::sync::Mutex` when the lock must be held across async operations.

12. **Establish a poison handling policy** — Decide whether to panic (current default via `.unwrap()`), silently degrade (`.ok()?`), or log and recover. Apply consistently across all crates.

13. **Decouple config from TOML** — Replace `toml::Value` in trait signatures (`Transport::start`, `StorageBackend::init`) with `serde_json::Value` or a custom config type. This decouples module implementations from the serialization format.

14. **Ensure all crate errors implement EngineError** — `storage-sqlite::StorageError` should implement `EngineError` so that error codes and severity propagate through the workflow engine correctly.

---

## Summary Assessment

The architecture is fundamentally sound. The four-layer design with types at the bottom, traits for contracts, independent infrastructure crates, and a thin orchestrator on top is the right approach. The workspace structure, the plugin SDK model, and the WASM sandboxing design are well-conceived.

The primary gap is between the documented architecture and the as-built system. Core is still a large monolith containing substantial business logic that the architecture says should be extracted. The extraction is clearly in progress (most crates exist and are functional), but the halfway state creates duplication (two event buses, two search systems, three schema registries) and disconnected data flows (plugin writes bypass Core side effects).

The dependency graph is almost a clean DAG, with two exceptions (workflow-engine depending on storage-sqlite, and plugin-system depending on workflow-engine). Fixing these two edges would restore the clean layering.

The most impactful near-term work is: unifying the Capability enums, bridging the event buses, completing the WASM guest-side FFI bridge, and continuing the Core extraction. Each of these directly addresses the gap between the designed architecture and the built system.
