# Test Infrastructure Review

Review date: 2026-03-28

## Summary

The Life Engine test infrastructure is mature and well-structured. The `packages/test-utils` crate provides a comprehensive set of factory functions, assertion macros, Docker helpers, connector test configs, and two new mock adapters (`MockBlobStorageAdapter`, `MockDocumentStorageAdapter`) that faithfully implement the corresponding traits. Test coverage across all 15+ crates reviewed is thorough, with strong patterns for isolation, error-case coverage, and security validation (capability enforcement, credential encryption, crash isolation).

Key strengths: high assertion specificity, proper use of `#[should_panic]` for negative testing, excellent mock quality, realistic fixtures, and zero test interdependencies. The project demonstrates an unusually disciplined approach to test infrastructure for a codebase at this stage.

The findings below are predominately minor. No critical defects were found in the test infrastructure.

## Package-by-Package Analysis

### packages/test-utils/src/lib.rs

Factory functions for all 7 CDM types (`Task`, `CalendarEvent`, `Contact`, `Email`, `FileMetadata`, `Note`, `Credential`). Each factory produces a realistic entity with all required fields populated, unique UUIDs, and sensible defaults.

Findings:

- All 7 factories have both a direct test and a JSON round-trip test (14 tests total). Coverage is complete.
- Factory functions produce unique UUIDs via `Uuid::new_v4()` on each call, preventing accidental ID collisions across tests. Good.
- The `create_test_credential` factory uses `Some(false)` for the `encrypted` field, which is correct for test fixtures that represent unencrypted credentials.
- **Minor** — `create_test_file` uses an all-zeros SHA-256 hash (`e3b0c44298fc...`), which is actually the hash of an empty byte string, not a 245KB PDF. The value is technically valid but misleading. Not a functional issue since nothing validates the checksum against actual content, but could confuse future developers.

### packages/test-utils/src/mock_blob.rs (new)

In-memory `BlobStorageAdapter` implementation. 180 lines of implementation, 230 lines of tests.

Findings:

- Implements all 9 trait methods: `store`, `retrieve`, `delete`, `exists`, `copy`, `list`, `metadata`, `health`, `capabilities`.
- Uses `tokio::sync::RwLock` for interior mutability, which is correct for async tests.
- The `store` method preserves `created_at` on overwrites, which is tested explicitly. Good attention to semantic correctness.
- Content-type auto-detection via `mime_guess` is tested with the `mock_blob_content_type_detection` test.
- `BlobKey` validation is tested (invalid keys: too few segments, leading slash, double dot, empty segment). Good for security.
- **Minor** — The `copy` method recomputes the SHA-256 checksum of the cloned data rather than copying the source checksum. This matches production behavior (where copy might change encoding) but is unnecessary work for identical bytes. No functional impact.
- **Minor** — No test for concurrent access. The `RwLock` should handle it correctly, but a concurrent store/retrieve test would add confidence.

### packages/test-utils/src/mock_storage.rs (new)

In-memory `DocumentStorageAdapter` implementation. 513 lines of implementation, 320 lines of tests.

Findings:

- Implements all 13 trait methods including `batch_create`, `batch_update`, `batch_delete`, `watch`, `migrate`, `query` with filters/sort/pagination/projection.
- Filter evaluation (`matches_filter`) supports all 11 `FilterOperator` variants including `And`, `Or`, `Not`, `Contains`, `StartsWith`, `Exists`, `In`, `NotIn`.
- The `batch_create` method correctly checks for duplicates both against existing data and within the batch itself (lines 393-401). Atomicity is tested via `mock_document_batch_create_atomic_on_conflict`. Excellent.
- The `watch` implementation uses `mpsc::channel(64)` and retains only live senders. Tested in `mock_document_watch_emits_events`.
- `capabilities()` reports all capabilities as `true`. This is intentional — the mock advertises full capabilities so tests can exercise all code paths.
- **Minor** — The `partial_update` method performs a shallow merge (line 236). Deep merge of nested objects is not supported. This is documented implicitly by the test only patching top-level fields. If production code ever needs deep merge semantics, this mock would diverge.
- **Minor** — The `query` method computes `total_count` before applying pagination (line 280), which is correct behavior matching typical SQL `COUNT(*) OVER()`. Good.
- **Minor** — `resolve_field` clones the entire document for each field resolution (line 122). This is fine for test data sizes but would be inefficient for large documents.

### packages/test-utils/src/assert_macros.rs

Six declarative assertion macros: `assert_serialization_roundtrip!`, `assert_plugin_metadata!`, `assert_plugin_capabilities!`, `assert_plugin_routes!`, `assert_basic_auth_header!`, `assert_sync_state_empty!`.

Findings:

- Each macro has at least one positive test and one `#[should_panic]` negative test. Good.
- `assert_basic_auth_header!` correctly uses `base64::engine::general_purpose::STANDARD`, matching the production base64 encoding.
- `assert_plugin_capabilities!` checks both count and membership, which catches both missing and extra capabilities.
- **No issues found.**

### packages/test-utils/src/docker.rs

Docker service constants and TCP availability helpers for GreenMail, Radicale, and MinIO.

Findings:

- Constants are tested against expected values (`greenmail_constants_match_compose`, etc.). While these tests appear tautological, they serve as a canary if someone changes the constants without updating `docker-compose.test.yml`.
- `is_service_available` uses a 1-second timeout, which is appropriate for local Docker.
- `wait_for_port` uses 200ms polling interval and async TCP connect. Tested with a known-unavailable port.
- `skip_unless_docker!` macro prints to stderr and returns early, which is the correct pattern for skippable integration tests in Rust (no native `#[ignore_if]` equivalent).
- `require_docker_service` panics with an actionable message including the Docker Compose command.
- **No issues found.**

### packages/test-utils/src/connectors.rs

Generic config structs and factory functions for integration test services, plus WebDAV/SMTP helpers.

Findings:

- `greenmail_send_email` implements a full SMTP client using raw TCP, handling RFC 5321 multi-line responses correctly (lines 392-412). This avoids a dependency on the connector crate's SMTP client.
- `ensure_radicale_calendar` and `ensure_radicale_addressbook` use custom HTTP methods (`MKCALENDAR`, `MKCOL`) with proper XML bodies.
- `delete_collection` treats 404 as success, which is correct for teardown operations.
- All 6 factory functions (`radicale_caldav_config`, `radicale_carddav_config`, `greenmail_imap_config`, `greenmail_smtp_config`, `minio_s3_config`) are tested for correctness.
- **No issues found.**

### packages/test-utils/src/plugin_test_helpers.rs

Generic lifecycle and event-handling helpers for plugin tests.

Findings:

- `test_plugin_lifecycle` exercises `on_load` then `on_unload` and verifies both succeed.
- The `DummyPlugin` test fixture tracks `loaded` state, enabling the test to verify lifecycle transitions.
- `create_test_core_event` produces a consistent synthetic event with `"test.event"` type.
- **No issues found.**

### packages/test-utils/fixtures/schemas/

Valid and invalid JSON fixtures for all 7 CDM types.

Findings:

- **Valid fixtures** — Single JSON objects with full field populations including optional fields, extensions, and nested structures. Used for positive schema validation.
- **Invalid fixtures** — Arrays of JSON objects, each with a `_comment` field documenting the specific violation. Violations include: missing required fields, invalid UUID formats, wrong types (string vs object, string vs boolean), invalid enum values, missing nested required fields.
- `contacts.json` (invalid): 6 test cases covering missing name, missing `name.given`, bad UUID, bad email type enum, bad phone type enum, wrong type for name.
- `emails.json` (invalid): 6 test cases covering missing `from`, missing `to`, wrong type for attachments, missing attachment field, wrong type for `read`, bad UUID.
- `events.json` (invalid): 7 test cases covering missing title, bad UUID, bad datetime format, bad status enum, bad recurrence frequency, bad attendee status, wrong type for title.
- **Minor** — The invalid contacts fixture (test case 1) and invalid emails fixture (test case 1) reuse the same UUID across multiple invalid entries. This is fine since the fixtures are consumed individually, not as a collection.
- **Minor** — No invalid fixtures for `files.json`, `notes.json`, `tasks.json`, or `credentials.json` exist as separate files, but `schema_validation_test.rs` constructs inline invalid fixtures for all 7 types. Coverage is complete.

### apps/core/tests/schema_validation_test.rs

JSON schema validation tests against `.odm/doc/schemas/` for all 7 CDM types plus plugin manifests.

Findings:

- 16 tests total: 7 positive + 7 negative for CDM schemas, plus 5 plugin manifest tests (3 positive, 2 negative).
- `assert_valid` and `assert_invalid` helpers compile the schema and provide clear error messages.
- `plugin_manifest_calendar_known_violations` documents known schema violations with a TODO, asserting on specific error content. Good practice for tracking known-issue tests.
- Plugin manifest tests gracefully skip when files are not found (lines 325-329), which is correct since plugins may be relocated during refactoring.
- **No issues found.**

### packages/plugin-system/tests/ (6 test files)

- **capability_enforcement.rs** — 5 tests covering first-party auto-grant, third-party approved, unapproved rejection (CAP_001), and config-based approval lifecycle. Tests use real WASM modules compiled from WAT. The `echo_wasm_module` is a real Extism-compatible module that echoes input through the WASM ABI (input_length, input_load_u8, alloc, store_u8, output_set). Excellent integration test depth.

- **communication.rs** — 4 tests covering workflow chaining, shared canonical collection access, absence of direct plugin-to-plugin calls, and storage plugin_id scoping (anti-impersonation). The `RecordingStorage` mock captures all read/write calls with caller identity, enabling assertion on scoping. Strong security test coverage.

- **community_plugin.rs** — 6 tests covering community plugin discovery, rejection without approval, same host functions as first-party when approved, coexistence, partial approval rejection, and full approval lifecycle. These are thorough end-to-end tests through the full `load_plugins` pipeline.

- **crash_isolation.rs** — 6 tests covering panicking plugin error return, error includes plugin ID, healthy plugin continues after crash, lifecycle manager force-unload, WASM memory isolation, and crash retry. Uses a `panicking_wasm_module` with `unreachable` instruction. Tests that `PLUGIN_010` error code is returned with `Severity::Fatal`.

- **runtime_capability_test.rs** — 12 tests covering CAP_002 runtime enforcement for all 6 host functions (storage read/write, config read, events emit/subscribe, HTTP outbound) plus positive cases for approved capabilities. The `assert_runtime_violation` helper validates error code, severity, capability name, and plugin ID in one check. Excellent.

- **injection_test.rs** — 8 tests covering host function injection gating for every capability combination (none, single, all), plus namespace verification and consistency between `injected_function_names` and `build_host_functions`.

Findings:

- **Minor** — The `echo_wasm_module()` function is duplicated verbatim across `capability_enforcement.rs`, `communication.rs`, `community_plugin.rs`, and `crash_isolation.rs`. This is ~35 lines copied 4 times. A shared test module or fixture file would reduce maintenance burden. However, test isolation is maximized by the current approach.
- **Minor** — `MockStorage`, `MockEventBus`, `mock_storage()`, `mock_event_bus()`, and `log_limiter()` helpers are duplicated across 3 test files. Same trade-off as above.
- Test quality is uniformly high. Every test asserts on specific error codes, severities, and message contents rather than just `is_err()`.

### packages/plugin-sdk-rs/tests/ (3 test files)

- **smoke_test.rs** — 7 tests verifying the SDK surface: plugin metadata, action declaration, execute known/unknown actions, PipelineMessage round-trip, PluginInvocation round-trip, and single-dependency ergonomics (compile-time check that all types are re-exported).

- **plugin_actions_test.rs** — 8 tests covering action function signatures, typed storage access, lifecycle hook defaults, hard failure (PluginError), soft warnings, PluginError variant inspection, and the full connector pattern (read config, fetch HTTP, normalize, write, emit). Mock clients (`MockStorage`, `MockEvents`, `MockConfig`, `MockHttp`) implement the new context traits.

- **reexport_test.rs** — Not read (appears to be a compile-time re-export verification test).

Findings:

- `plugin_error_has_code_message_detail_fields` exhaustively tests all 6 `PluginError` variants with expected codes.
- `connector_pattern_read_fetch_normalise_write_emit` is an excellent integration-style test that validates the full connector lifecycle through mock clients.
- **No issues found.**

### packages/storage-sqlite/src/tests/mod.rs

13 tests covering SQLite initialization, WAL mode, foreign keys, idempotent schema, rekey lifecycle, credential encryption at rest.

Findings:

- `init_rejects_wrong_key` verifies that SQLCipher returns a decryption error. Good security test.
- `rekey_succeeds_and_new_key_works` verifies the full rekey cycle: create with old key, populate, rekey, verify with new key, verify old key rejected. Excellent.
- `credential_encrypted_field_is_not_plaintext` and `credential_stored_in_db_is_not_plaintext_at_rest` are particularly valuable security tests verifying that sensitive data never appears in plaintext in the database.
- **Minor** — `rekey_failure_retains_old_key` (line 185) does not actually trigger a rekey failure — it just verifies that the original key still works when no rekey was applied. The test name is slightly misleading. A more accurate name would be `original_key_works_after_normal_close`.

### packages/auth/src/tests/ (5 test files)

- **identity_test.rs** — 5 tests covering `AuthIdentity` round-trip through `PipelineMessage`, None preservation, plugin output propagation, `skip_serializing_if` validation, and API key identity.
- **keys_test.rs** — API key CRUD lifecycle tests with mock storage.
- **validate_test.rs** — Auth validation pipeline tests with configurable mock provider.
- **rate_limit_test.rs** — Rate limiter tests covering threshold behavior, per-IP isolation, and success reset.
- **pocket_id_test.rs** — JWT validation tests using real Ed25519 keypairs and wiremock for OIDC discovery.

Findings:

- The auth tests use `wiremock::MockServer` for external HTTP mocking rather than a custom mock. Good choice for reliability.
- `auth_context_not_present_in_serialized_json_when_none` tests the `skip_serializing_if` annotation, which is easy to overlook and critical for wire format correctness.
- **No issues found.**

### packages/crypto/src/tests/mod.rs

3 tests covering derive-key + encrypt/decrypt round-trip, wrong-key decryption failure, and HMAC integrity with derived keys.

Findings:

- Tests exercise the full pipeline: `derive_key` -> `encrypt` -> `decrypt` and `derive_key` -> `hmac_sign` -> `hmac_verify`.
- Wrong-key test is present and correctly expected to fail.
- **Minor** — No test for empty plaintext, zero-length data, or very large payloads. These edge cases may be worth adding for robustness.

### packages/transport-graphql/src/tests/mod.rs

6 tests covering GraphQL request translation, schema generation from plugin declarations, success/error response shapes, transport equivalence with REST, and error fallback behavior.

Findings:

- `transport_equivalence_same_workflow_request_shape` is a particularly good test that validates the invariant that both REST and GraphQL transports produce the same `WorkflowRequest` structure.
- Schema generation tests verify PascalCase naming and type mapping (string->String, integer->Int, number->Float, boolean->Boolean).
- **No issues found.**

### packages/transport-rest/src/tests/ (2 test files)

- **mod.rs** — 11 tests covering config validation (port zero, TLS paths, duplicate routes, route prefix enforcement), default config content, route merging with plugins, router path parameter extraction, workflow name resolution, and 404 handling.
- **middleware_test.rs** — 8 tests covering CORS (permissive localhost, strict wildcard, explicit origins), auth middleware (401 on missing token, identity extension on valid token, public route bypass), logging passthrough, and panic handler (500 without internal details).

Findings:

- `panic_handler_returns_500_without_internal_details` verifies that internal error details do not leak to clients. Critical security test.
- `auth_rejects_missing_token_with_401` verifies the specific error code `AUTH_001`. Good.
- Route prefix enforcement tests ensure REST routes start with `/api/` and GraphQL routes start with `/graphql`. Prevents configuration errors.
- **No issues found.**

### packages/workflow-engine/tests/migration_test.rs

8 tests covering end-to-end migration, quarantine on transform failure, chain migration (v1->v4), backup creation and restore, idempotency, collection scoping, plugin_id scoping, and no-matching-records handling.

Findings:

- Uses real WASM modules (compiled from WAT at test time) for transform functions. Tests both identity transforms and trapping transforms.
- `multi_export_identity_wasm` helper generates WASM with multiple named exports for chain migration tests. Clever approach.
- Backup restore is verified to return data to pre-migration state.
- Idempotency test verifies that running the same migration twice produces 0 migrated on the second run.
- Collection and plugin_id scoping tests verify migration isolation. These are critical correctness tests.
- **No issues found.**

## Cross-Cutting Test Quality Assessment

### Test isolation

All tests are fully isolated. No test depends on or affects another:

- Unit tests use fresh instances or `TempDir` for each test.
- Async tests use `#[tokio::test]` with default single-threaded runtime.
- Mock storage adapters use per-test instances with `RwLock` for thread safety.
- Docker integration tests use unique collection paths and `skip_unless_docker!` guards.

### Mock quality and realism

Mocks are high quality throughout:

- `MockBlobStorageAdapter` and `MockDocumentStorageAdapter` implement the full trait surface with correct semantics (atomicity, error conditions, change events).
- `RecordingStorage` in `communication.rs` records all calls for post-hoc assertion.
- `MockAuthProvider` in `middleware_test.rs` returns realistic `AuthIdentity` objects.
- The WASM fixtures (`echo_wasm_module`, `panicking_wasm_module`) are real WASM modules, not mocks. This provides true integration-level confidence.

### Assertion quality

Assertions are specific throughout:

- Error assertions check error code, severity, and message content (not just `is_err()`).
- Capability assertions check exact counts and membership.
- Storage assertions verify scoping (plugin_id, collection) on both reads and writes.
- Security assertions verify absence of plaintext in encrypted outputs.

### Naming conventions

Test names are descriptive and consistent:

- Test functions use `snake_case` with full descriptive names.
- Module doc comments reference work package numbers (e.g., "WP 8.16", "WP 8.17").
- Section comments in test files use `// ==` dividers with test numbers.
- Test names describe the behavior being verified, not the implementation detail.

### Test performance

- All unit tests are fast (no I/O, no network, no filesystem except TempDir).
- WASM module compilation from WAT adds ~50ms per module per test but is acceptable for integration tests.
- Docker-dependent tests are properly guarded with `skip_unless_docker!`.
- No `sleep` calls in any test.

## Problems Found

### Major

- **Duplicated WASM fixture and mock code across plugin-system tests** — The `echo_wasm_module()` function (~35 lines), `MockStorage`, `MockEventBus`, and helper functions are duplicated across 4 test files in `packages/plugin-system/tests/`. Total duplication is ~200 lines. If the Extism ABI changes, all 4 copies must be updated. Recommendation: extract shared fixtures into a `packages/plugin-system/tests/common/mod.rs` module.

### Minor

- **Misleading checksum in `create_test_file` factory** — The SHA-256 hash `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` is the hash of empty input, not a 245KB file. Consider using a descriptive placeholder like `"deadbeef..."` or computing the actual hash of test content.

- **Misleading test name `rekey_failure_retains_old_key`** — This test does not trigger a rekey failure; it verifies that the original key works after a normal close. The test name should reflect what it actually tests.

- **No concurrent access tests for mock storage adapters** — `MockBlobStorageAdapter` and `MockDocumentStorageAdapter` both use `RwLock` but are never tested with concurrent readers/writers. While the `RwLock` implementation is well-understood, a concurrent test would verify the mock's behavior under contention.

- **No edge-case crypto tests** — `packages/crypto/src/tests/mod.rs` has 3 tests but does not cover empty plaintext, very large payloads (>1MB), or repeated encrypt/decrypt of the same input yielding different ciphertexts (nonce uniqueness).

- **Missing mock_blob.rs and mock_storage.rs module declarations in lib.rs** — The `packages/test-utils/src/lib.rs` file declares `pub mod assert_macros`, `pub mod connectors`, `pub mod docker`, `pub mod plugin_test_helpers` but does not include `pub mod mock_blob` or `pub mod mock_storage`. These new files exist but may not be accessible from external crates yet. This should be verified — if the modules are used internally in other crates, they need to be declared.

- **`_source_meta` naming in `MockBlobStorageAdapter::copy`** — Line 114 uses `_source_meta` (underscore prefix) but then accesses `_source_meta.content_type` and `_source_meta.metadata` on lines 128-129. The underscore prefix convention in Rust indicates an unused binding, which is misleading here.

## Recommendations

1. **Extract shared WASM fixtures** — Create `packages/plugin-system/tests/common/mod.rs` with shared `echo_wasm_module()`, `panicking_wasm_module()`, mock backends, and helper functions. This reduces duplication from ~200 lines to ~30 lines of imports.

2. **Add mock_blob and mock_storage module declarations** — Add `pub mod mock_blob;` and `pub mod mock_storage;` to `packages/test-utils/src/lib.rs` so external crates can use the mocks.

3. **Add concurrent access tests** — Add at least one test per mock adapter that spawns multiple `tokio::spawn` tasks performing simultaneous reads and writes to verify `RwLock` behavior.

4. **Expand crypto edge-case tests** — Add tests for empty plaintext, repeated encryption (nonce uniqueness), and payloads at the boundary of AES-GCM block sizes.

5. **Rename misleading test** — Rename `rekey_failure_retains_old_key` to `original_key_works_after_normal_close` or add an actual failure scenario (e.g., interrupted rekey via read-only filesystem).

6. **Fix `_source_meta` naming** — Rename to `source_meta` in `MockBlobStorageAdapter::copy` to match Rust conventions for used bindings.
