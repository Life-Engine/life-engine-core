# Traits and Interfaces Review

Review of `packages/traits/` covering trait design, type safety, object safety, error handling, async patterns, and overall architecture.

## Summary

The `packages/traits/` crate defines the foundational infrastructure contracts for Life Engine: storage backends, transports, plugins, capabilities, schema validation, and schema versioning. The **compiled modules** (those declared in `lib.rs`) are generally well-designed with clean trait definitions, proper object safety, and solid test coverage. However, there are three **orphaned source files** (`blob.rs`, `storage_context.rs`, `storage_router.rs`) that are not declared in the module tree, import types that do not exist in this crate, and would not compile. These appear to be work-in-progress for the new architecture but represent a significant integration gap.

## File-by-file Analysis

### Cargo.toml

The dependency list is reasonable for the crate's scope. Notable observations:

- `toml = "0.8"` is pinned without using workspace version management, unlike other dependencies
- `tokio` is not listed as a dependency, yet three orphaned source files depend on it heavily
- `chrono` and `uuid` are listed as dependencies but only used by orphaned files and `schema.rs` (which uses them transitionally during validation)

### src/lib.rs

Clean module declarations with appropriate re-exports. The crate doc comment accurately describes the four core contracts. The module list includes `index_hints`, `schema`, and `schema_versioning` but does **not** include `blob`, `storage_context`, or `storage_router`, leaving those files as dead code.

### src/error.rs

Well-designed error trait and severity enum.

- `EngineError` requires `std::error::Error + Send + Sync + 'static` -- correct supertraits for use in async contexts and trait objects
- `Severity` is `Copy`, `Eq`, `Hash` -- appropriate for a classification enum
- Object safety is maintained: all methods return owned or reference types, no `Self` in return position
- The convenience methods (`is_fatal`, `is_retryable`, `is_warning`) are useful and well-tested

### src/capability.rs

Solid capability model with good separation of concerns.

- `Capability` enum is `Copy + Eq + Hash` -- correct for set membership checks
- `Display`/`FromStr` round-trip is tested and uses the `storage:doc:read` namespaced format
- `ParseCapabilityError` implements `std::error::Error` manually rather than using `thiserror` -- minor inconsistency since `thiserror` is a dependency
- `CapabilityViolation` correctly implements both `std::error::Error` and `EngineError`
- The `at_load_time` boolean provides useful differentiation between error codes `CAP_001` and `CAP_002`
- Test coverage is thorough with round-trip, display format, rejection, and violation code tests

### src/storage.rs

Minimal trait definition for the legacy storage backend.

- `StorageBackend` is `Send + Sync` with `async_trait` -- correct
- The `init` associated function has a `Self: Sized` bound, which is correct since it cannot be called on trait objects
- Error type is `Box<dyn EngineError>` -- consistent with the trait-object-based error model
- The `key: [u8; 32]` parameter in `init` exposes encryption key handling at the trait level; this is a design choice worth noting
- This trait appears to be the legacy interface, with `DocumentStorageAdapter` being the new one (referenced in orphaned files)

### src/transport.rs

Clean transport abstraction.

- `Transport` is `Send + Sync` with `async_trait`
- `TransportConfig` uses `serde` derives and provides a sensible default bind address
- `TlsConfig` has a duplicate doc comment line ("Common configuration shared by all transports") -- copy-paste error from `TransportConfig`
- `start()` takes `toml::Value` which couples the trait to the TOML format; a more abstract config type would be more flexible
- `name()` returns `&str` which is object-safe

### src/plugin.rs

Good plugin contract definition.

- `Plugin` is `Send + Sync` but notably does **not** use `async_trait` -- `execute()` is synchronous
- This is a deliberate design choice consistent with WASM plugin execution being synchronous from the host's perspective
- `Action` struct uses builder pattern with `with_input_schema`/`with_output_schema` -- clean API
- `Action` derives `PartialEq` and `Serialize/Deserialize` with `skip_serializing_if` for optional fields
- The `execute` method returns `Box<dyn EngineError>` which is consistent

### src/types.rs

Empty module with only a doc comment. Should either contain types or be removed.

### src/index_hints.rs

Well-implemented index hint parsing and merging logic.

- `IndexHint` is properly `Serialize/Deserialize` with `skip_serializing_if` for optional `name`
- `CollectionDescriptor` bundles collection metadata cleanly
- `parse_index_hints` provides detailed error messages with index position context (e.g., `indexes[0]: missing required field 'fields'`)
- `merge_index_hints` correctly uses sorted field sets for order-independent conflict detection
- **Issue**: `SchemaError` is defined here as a simple struct, duplicating the name with the more comprehensive `SchemaError` enum in `schema.rs`. While they're in separate modules, this creates confusion. The index_hints `SchemaError` is a simpler type that doesn't implement `EngineError`

### src/schema.rs

The most substantial module -- implements the centralized schema registry with CDM schema loading, validation pipeline, strict mode, extension namespace isolation, and immutable field protection.

- `SchemaError` enum uses `thiserror::Error` with four variants covering invalid schema, validation failure, immutable field, and namespace violation
- Implements `EngineError` trait with structured error codes (`SCHEMA_001` through `SCHEMA_004`)
- `SchemaRegistry` uses `HashMap` for O(1) lookups keyed by `(plugin_id, collection_name)` tuples
- CDM schemas are embedded at compile time via `include_str!` -- ensures availability without filesystem access
- `validate_write` implements a 10-step validation sequence that handles system field stripping/re-injection, immutable field checks, namespace isolation, body validation, strict mode, and extension validation
- **Issue**: The `validate_write` method modifies the document (inserts placeholder system fields for validation, then restores originals), which makes the function impure despite taking `&Value`. It clones the document internally to avoid mutating the input, but the overall approach is complex
- **Issue**: `get_schema_properties` falls back through CDM values then schema_values map, and returns `None` for cases where the schema value wasn't stored. This means strict mode silently becomes permissive if the schema value lookup fails
- **Issue**: `let` chains use `let ... && let ...` syntax which is a nightly/unstable Rust feature (`let_chains`). If the project targets stable Rust, this won't compile
- Test coverage is excellent with 17 test cases covering all validation paths

### src/schema_versioning.rs

Comprehensive schema compatibility checker.

- `CompatibilityResult` enum clearly separates `Compatible` from `Breaking(Vec<BreakingChange>)`
- `ChangeKind` covers all spec requirements: field removal, rename, type change, required field addition, enum value removal, constraint tightening, and default change
- `DeprecationTracker` correctly scans for `deprecated: true` and `x-deprecated-*` extension keywords
- **Issue**: `FieldRenamed` variant exists in `ChangeKind` but is never produced by the comparator -- renames are detected as `FieldRemoved` + new field appears. The variant is unused dead code
- **Issue**: `compare_enums` reports `EnumValueRemoved` when the entire `enum` keyword is removed from the new schema. This is debatable -- removing an enum constraint is actually relaxing (making it accept more values), not tightening
- The recursive comparison (`compare_schemas` -> `compare_properties` -> `compare_schemas`) handles nested objects correctly
- `compare_defs` handles `$defs` section changes
- Constraint comparison covers `maxLength`, `minLength`, `maximum`, `minimum`, `pattern`, and `format`

### src/blob.rs (orphaned -- NOT in module tree)

Well-designed blob storage adapter trait and types, but **not compiled** as part of the crate.

- `BlobKey` has proper validation: rejects `..`, leading `/`, empty segments, and requires at least 3 segments
- `BlobStorageAdapter` is `async_trait` with `Send + Sync` -- correct
- Object safety is verified with a compile-time test
- `BlobAdapterCapabilities` has a sensible `Default` implementation
- **Critical**: Imports `crate::storage::{HealthReport, StorageError}` -- these types do not exist in `storage.rs`
- Has comprehensive unit tests for `BlobKey` validation and type construction

### src/storage_context.rs (orphaned -- NOT in module tree)

Enforcement layer with capability checks, collection scoping, schema validation, extension namespace isolation, and audit events. Not compiled.

- `StorageCapability` enum duplicates capability string parsing from `Capability` in `capability.rs`
- `CallerIdentity` enum provides a clean `Plugin` vs `System` discrimination
- `StorageContext` wraps `StorageRouter` with `Arc` for shared ownership -- appropriate for async
- Uses `tokio::sync::mpsc::UnboundedSender` for audit events -- fire-and-forget pattern with `let _ = self.audit_tx.send(event)`
- **Critical**: `tokio` is not in `Cargo.toml` dependencies
- **Critical**: Imports `DocumentStorageAdapter`, `QueryDescriptor`, `DocumentList`, `FilterNode`, `HealthReport`, `HealthCheck`, `HealthStatus`, `AdapterCapabilities`, `ChangeEvent`, `CollectionDescriptor` from `crate::storage` -- none of these exist there
- The inline mock adapters in tests are thorough but very verbose (~170 lines of mock code)

### src/storage_router.rs (orphaned -- NOT in module tree)

Storage operation dispatcher with timeout enforcement and health aggregation. Not compiled.

- `TimeoutConfig` with per-operation-class timeouts is a good pattern
- `with_timeout` uses `tokio::time::timeout` for deadline enforcement
- Health aggregation with `worst_status` function is clean
- **Critical**: Same import issues as `storage_context.rs` -- references types not in this crate
- **Critical**: `tokio` not in dependencies

### src/tests/mod.rs

Good integration-level tests for the compiled modules.

- Tests `EngineError` object safety by boxing a `CapabilityViolation`
- Tests `Capability` round-trip but only covers 6 of 10 variants (missing `StorageDelete`, `StorageBlobRead`, `StorageBlobWrite`, `StorageBlobDelete`)
- Tests `Action` serialization including `skip_serializing_if` behavior
- Tests re-export accessibility as a compile-time check

### tests/index_hints_tests.rs

Thorough external integration tests for index hint parsing and merging.

- 14 test cases covering valid input, edge cases, and all error paths
- Tests multi-field conflict with order independence
- Tests `CollectionDescriptor` construction
- Tests `IndexHint` serde round-trip including `skip_serializing_if`

### tests/schema_versioning_tests.rs

Comprehensive integration tests for the schema compatibility checker.

- 18 test cases covering all spec requirements (Req 2-5, 10)
- Tests nested object recursion
- Tests deprecation tracking end-to-end
- Tests `$defs` changes
- Tests edge cases (empty schemas, adding properties to empty)

### tests/schema_integration_tests.rs

Cross-module integration tests combining schema registry, index hints, and versioning.

- 12 test cases exercising the full validation pipeline
- Tests CDM schema loading + validation + accept/reject
- Tests extension namespace isolation end-to-end
- Tests strict mode with unknown field rejection
- Tests full lifecycle: register -> validate -> schema evolution -> re-register -> re-validate
- Tests schema registry with index hints stored

## Problems Found

### Critical

- **Orphaned files cannot compile** — `blob.rs`, `storage_context.rs`, and `storage_router.rs` are not declared in `lib.rs` and import types (`DocumentStorageAdapter`, `StorageError`, `DocumentList`, `QueryDescriptor`, `FilterNode`, `HealthReport`, `HealthCheck`, `AdapterCapabilities`, `ChangeEvent`, `HealthStatus`) that do not exist anywhere in the traits crate. These files reference a `crate::storage` module that only contains `StorageBackend`, not the new adapter interfaces. Adding these modules to `lib.rs` without first defining the missing types would produce compilation errors.
- **Missing `tokio` dependency** — `storage_context.rs` and `storage_router.rs` use `tokio::sync::mpsc` and `tokio::time::timeout` but `tokio` is not in `Cargo.toml`
- **Potentially unstable Rust features** — `schema.rs` uses `let_chains` syntax (`if let ... && let ...`) which requires either nightly Rust or the `#![feature(let_chains)]` flag. If the project targets stable Rust, this code won't compile even if the module is otherwise correct. (Note: `let_chains` was stabilized in Rust 1.87.0, so this is only an issue if the project's MSRV is below that.)

### Major

- **Duplicate `SchemaError` types** — `index_hints.rs` defines `pub struct SchemaError { pub message: String }` while `schema.rs` defines `pub enum SchemaError { InvalidSchema, ValidationFailed, ImmutableField, NamespaceViolation }`. Both are public. If a consumer imports both modules, they'll need fully-qualified paths. The `index_hints::SchemaError` should be merged into `schema::SchemaError` or renamed to avoid confusion.
- **Duplicate `StorageCapability` / `Capability` string parsing** — `storage_context.rs::StorageCapability` duplicates the `storage:doc:*` and `storage:blob:*` parsing already present in `capability.rs::Capability`. The storage-specific variants in `Capability` (`StorageRead`, `StorageWrite`, `StorageDelete`, `StorageBlobRead`, `StorageBlobWrite`, `StorageBlobDelete`) map 1:1 with `StorageCapability`. This duplication will lead to inconsistency as the crate evolves.
- **`FieldRenamed` variant is dead code** — `schema_versioning.rs::ChangeKind::FieldRenamed` is defined but never produced by `check_compatibility()`. Renames are detected as `FieldRemoved`. The variant should either be wired into the detection logic or removed.
- **Strict mode silently degrades to permissive** — In `schema.rs`, `get_schema_properties()` returns `None` if the schema JSON value can't be found, causing the strict mode check to be silently skipped. This means a misconfiguration (e.g., CDM schema not stored in `cdm_values` at the time of strict check) would allow unknown fields through without any warning or error.
- **`types.rs` is empty** — The module is declared in `lib.rs` but contains only a doc comment. It should either contain actual types or be removed.
- **Missing `CollectionDescriptor` definition for the new storage system** — `storage_context.rs` imports `CollectionDescriptor` from `crate::storage` but this type only exists in `index_hints.rs` as a different struct (no `migrate` or schema-aware fields). The `storage_router.rs` also imports `CollectionDescriptor` from `crate::storage`. There's a naming collision between the index hints descriptor and the storage migration descriptor.

### Minor

- **`TlsConfig` doc comment is copy-pasted from `TransportConfig`** — Line 13 of `transport.rs` says "Common configuration shared by all transports" which describes `TransportConfig`, not `TlsConfig`
- **`toml` dependency version not workspace-managed** — `toml = "0.8"` is pinned directly while other deps use `workspace = true`
- **Incomplete capability round-trip test in `tests/mod.rs`** — The `capability_fromstr_display_round_trip_all_variants` test only covers 6 of the 10 `Capability` variants, missing `StorageDelete`, `StorageBlobRead`, `StorageBlobWrite`, and `StorageBlobDelete`. The test in `capability.rs` itself does cover all 10.
- **`ParseCapabilityError` doesn't use `thiserror`** — Manually implements `Display` and `std::error::Error` despite `thiserror` being a crate dependency. Minor inconsistency.
- **`BlobInput` is not `Serialize/Deserialize`** — `BlobMeta` is, but `BlobInput` is not, which limits serialization round-trips in tests and persistence of pending inputs
- **`BlobMeta.key` is `String` not `BlobKey`** — After validating the key as `BlobKey` on input, the metadata stores it as a raw `String`, losing the type-level validation guarantee
- **Enum removal detection is debatable** — `compare_enums` treats removing the entire `enum` keyword as `EnumValueRemoved`. Removing an enum constraint actually relaxes validation (accepts any value), so this could be classified as compatible rather than breaking. The current behavior is conservative, which is defensible, but may produce false positives.
- **`SchemaRegistry` is not `Send + Sync`** — `jsonschema::Validator` may not be `Send + Sync` depending on the version. If `SchemaRegistry` needs to be shared across async tasks (as `StorageContext` does with `Arc<SchemaRegistry>`), this could be a problem at integration time.

## Recommendations

1. **Decide the integration path for orphaned files** — The three orphaned files (`blob.rs`, `storage_context.rs`, `storage_router.rs`) contain well-structured code but need a home. Either:
   - Define the missing types (`DocumentStorageAdapter`, `StorageError`, `DocumentList`, `QueryDescriptor`, `FilterNode`, `HealthReport`, etc.) in a new or existing module within the traits crate, then wire the modules into `lib.rs`
   - Move these files to a separate crate (e.g., `packages/storage-traits/`) that depends on `life-engine-traits`
   - If these types should live in `storage.rs`, expand that module significantly to replace the current `StorageBackend` trait with the new adapter-based architecture

2. **Resolve the `SchemaError` duplication** — Rename `index_hints::SchemaError` to something like `IndexHintError` or `ParseError`, or consolidate it as a variant in `schema::SchemaError`

3. **Eliminate `StorageCapability` duplication** — Use `Capability` from `capability.rs` directly in `StorageContext` rather than maintaining a parallel enum. A method like `Capability::is_storage()` or a conversion trait could bridge the gap.

4. **Remove or implement `FieldRenamed`** — Either wire rename detection into the comparator (detect when a removal + addition at the same level have the same type, suggesting a rename) or remove the variant to avoid confusion

5. **Add `tokio` to `Cargo.toml`** — Once the orphaned files are integrated, `tokio` will be needed with at least `sync` and `time` features

6. **Remove or populate `types.rs`** — The empty module adds noise

7. **Fix the `TlsConfig` doc comment** — Should describe TLS configuration, not repeat the transport config description

8. **Make strict mode failure explicit** — When `get_schema_properties()` returns `None` for a strict-mode collection, emit a warning or error rather than silently skipping the strict check
