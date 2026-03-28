# Plugin System QA Report

- **Package** — `packages/plugin-system/`
- **Reviewer** — Plugin System and WASM Runtime Expert
- **Date** — 2026-03-28
- **Scope** — All source files, host functions, tests, Cargo.toml

---

## Summary

The plugin system is architecturally sound with a well-designed two-layer capability enforcement model (injection gating + runtime checks), proper WASM sandboxing via Extism, and strong error isolation. The codebase demonstrates defence-in-depth thinking throughout.

Key strengths:

- Two-layer capability enforcement (CAP_001 at load, CAP_002 at runtime) is correctly implemented
- Storage plugin_id scoping prevents cross-plugin impersonation
- WASM trap detection classifies errors correctly (Crash vs ExecutionFailed)
- Plugin lifecycle state machine is clean and well-tested
- Manifest parser validates thoroughly: plugin ID format, semver, reserved names, cross-section consistency
- Test coverage is excellent across unit and integration tests

Issues found: 4 critical, 5 major, 8 minor.

---

## File-by-File Analysis

### Cargo.toml

Clean dependency list. Uses workspace versions consistently. The `wat = "1"` dev dependency for WASM text format compilation in tests is appropriate. No unnecessary dependencies detected.

### src/lib.rs

Well-organized public API re-exports. All modules are declared and key types are surfaced. The re-export of `discovery` and `lifecycle` modules indicates clean separation of concerns.

### src/capability.rs

Strong implementation of capability approval logic. `is_first_party()` correctly uses `canonicalize()` to resolve symlinks before path comparison, preventing symlink-based bypass. The `check_collection_access()` function properly auto-allows plugin-scoped collections (prefixed with `<plugin_id>.`). Test coverage is thorough with 14 unit tests covering first-party grants, third-party approval, runtime checks, blob capabilities, collection access, and error messages.

### src/error.rs

Clean error type hierarchy implementing the `EngineError` trait. Error codes are well-organized (PLUGIN_001 through PLUGIN_010, CAP_001, CAP_002). Severity assignments are reasonable. Minor gap: there is no PLUGIN_006 code (jumps from 005 to 007), suggesting a removed variant that left a gap.

### src/execute.rs

The `PluginSystemExecutor` correctly implements the `PluginExecutor` trait with three-step validation (lifecycle check, action existence, WASM call). Uses `std::sync::Mutex` for the handles and lifecycle maps.

**Critical issue** — `std::sync::Mutex` in async context: The executor holds `Mutex<HashMap<String, PluginHandle>>` and `Mutex<LifecycleManager>`, calling `.lock().unwrap()` inside an `async fn`. If a panic occurs while a mutex is held, the poison propagates via `unwrap()` and subsequent calls will panic too, taking down the entire executor. This should use `tokio::sync::Mutex` or handle poison explicitly.

**Major issue** — Lock contention: Both the lifecycle lock and handles lock are acquired in sequence within the same `execute()` call. The lifecycle lock is dropped before acquiring the handles lock, which is good, but the handles lock is held for the entire duration of the WASM call (line 79-108). This means only one plugin can execute at a time across the entire system. For a multi-plugin system, this is a significant bottleneck.

### src/injection.rs

The injection layer is the first capability enforcement layer. The `build_host_functions()` function correctly maps capabilities to host functions, and `host_log` is always injected (no capability required).

**Critical issue** — Blob host functions not injected: The `injected_function_names()` function lists blob capabilities (`StorageBlobRead` -> `host_blob_retrieve`, `StorageBlobWrite` -> `host_blob_store`, `StorageBlobDelete` -> `host_blob_delete`), but `build_host_functions()` does NOT actually build or inject these blob host functions. There are no `build_blob_store_function()`, `build_blob_retrieve_function()`, or `build_blob_delete_function()` builder functions. The `injected_function_names()` advertises blob support, but `build_host_functions()` silently omits them. A plugin with `StorageBlobWrite` capability would believe it has blob support via the names check, but the actual WASM sandbox would not have the host function available. This is a correctness bug that breaks the contract between the two functions and means blob storage is non-functional via the plugin system.

**Major issue** — `StorageDelete` missing from `build_host_functions` and `injected_function_names` mismatch: The `build_host_functions()` does include `StorageDelete`, but the `injected_function_names()` test at line 528 expects 11 functions (10 capabilities + host_log). The test at line 209 (`injected_function_names_matches_build_host_functions`) only passes because it tests with `StorageRead` + `HttpOutbound`, not the full set. If you called `build_host_functions()` with all 10 capabilities including blob caps, it would only produce 8 functions (7 doc/event/http/config + host_log), while `injected_function_names()` would return 11. This contract mismatch would be caught if the integration test in `injection_test.rs` tested all 10 capabilities with `build_host_functions()`.

### src/loader.rs

Well-structured loading pipeline. Key features:

- Duplicate plugin ID detection via `seen_ids` HashSet
- Graceful degradation: one bad plugin doesn't prevent others from loading
- Clear error messages with plugin directory and error context

No issues found. The test coverage is excellent with 8 tests covering valid loading, missing manifests, unapproved capabilities, corrupt WASM, multiple plugins, first-party grants, empty directories, and duplicate IDs.

### src/manifest.rs

Thorough manifest parser with strong validation:

- Plugin ID format validation (lowercase, hyphens, starts with letter)
- Semver validation
- Reserved name checking (`audit_log`, `system.*` prefix)
- `deny_unknown_fields` on the raw TOML struct prevents typos
- Cross-section consistency checks (events without capability, config without capability)
- Extension naming convention enforcement (`ext.` prefix)
- At least one action required

**Minor issue** — Event naming validation is weak: The check at line 517 only validates that event names have 3+ dot-separated parts. It does not verify that the first part matches the plugin ID, which the naming convention (`<plugin-id>.<action>.<outcome>`) implies. A plugin could declare events with a different plugin's namespace.

**Minor issue** — No maximum length validation: Plugin IDs, action names, and collection names have no length limits. An extremely long plugin ID could cause issues with path construction in blob storage (`scoped_key()`) and logging.

### src/runtime.rs

Clean WASM runtime wrapper around Extism. Notable design decisions:

- 64 MB default memory limit (1024 WASM pages), configurable
- 30-second default execution timeout, configurable
- Memory limit clamped to 4 GiB (WASM spec maximum)
- WASI enabled for all plugins
- Trap detection via keyword matching in error messages

**Major issue** — WASI enabled unconditionally: Line 182 calls `.with_wasi(true)` for all plugins. WASI gives plugins access to clock functions, random number generation, and potentially environment variables. This should be configurable per-plugin or at least documented as a security consideration. For a sandboxing system focused on capability enforcement, unconditional WASI access is permissive.

**Minor issue** — Trap detection is fragile: The `is_wasm_trap()` function at line 89 relies on substring matching against known error message patterns. If Extism or Wasmtime changes error message formatting, traps could be misclassified as `ExecutionFailed` (which is `Retryable`) instead of `Crash` (which is `Fatal`). This could cause the system to retry operations that caused genuine WASM traps.

### src/discovery.rs

Simple, correct directory scanner. Sorted output ensures deterministic loading order. Does not recurse into nested directories. Test coverage covers all edge cases (missing files, non-directories, sorting, nonexistent paths).

### src/lifecycle.rs

Clean state machine with six phases (Discovered -> Loaded -> Initialized -> Running -> Stopped -> Unloaded). The `force_unload()` method enables error recovery from any state. The `start_all()` and `stop_all()` methods handle bulk operations with error collection. Reverse-order shutdown is correctly implemented.

**Minor issue** — No timestamp tracking: State transitions are logged but not stored with timestamps. For debugging production issues, knowing when a plugin entered a state (and how long it stayed) would be valuable.

### src/host_functions/mod.rs

Simple module declaration. Declares `blob`, `config`, `events`, `http`, `logging`, `storage` submodules.

### src/host_functions/storage.rs

Strong implementation with defence-in-depth. Key security features:

- Capability check at the top of every function
- `query.plugin_id` is forcibly overwritten with the caller's actual plugin ID (line 58)
- `scope_mutation()` replaces the `plugin_id` in all mutation variants
- `host_storage_delete()` validates the mutation is actually a `Delete` variant

Test coverage is excellent, including a plugin impersonation prevention test.

### src/host_functions/events.rs

Correct capability enforcement for emit and subscribe. Notable features:

- Event enrichment: the host adds `source` and `depth` fields to emitted events, preventing plugins from spoofing the source
- Declared event validation: if `declared_emit_events` is set, only listed events can be emitted
- Execution depth tracking for cascading event detection

**Minor issue** — `declared_emit_events` is always `None` in injection: In `injection.rs` line 365, the `EventsHostContext` is always constructed with `declared_emit_events: None`, which means manifest-declared event restrictions are never enforced at runtime. The validation code exists but is dead in practice.

**Minor issue** — Execution depth is always 0: Similarly, `execution_depth` is hardcoded to 0 in the injection builder (line 366). The depth tracking infrastructure exists but is never wired up to detect cascading events.

### src/host_functions/http.rs

Well-designed HTTP outbound control with multiple safety layers:

- Capability check
- URL scheme validation (only http/https)
- Domain allowlist enforcement
- Request timeout (30 seconds)
- Response body size limit (10 MB)

**Critical issue** — No SSRF protection for internal networks: The URL scheme check blocks `file://` and `ftp://`, but does not block requests to internal network addresses (127.0.0.1, 169.254.x.x, 10.x.x.x, 192.168.x.x, fd00::/8, etc.). When `allowed_domains` is `None` (backwards compat mode), a malicious plugin could probe internal services, access cloud metadata endpoints (169.254.169.254), or attack other services on the local network. This is a Server-Side Request Forgery (SSRF) vulnerability.

**Major issue** — Response body size check after full download: The body size check at line 200 happens after `response.bytes().await` has already downloaded the entire body into memory (line 193). A malicious server could send a multi-gigabyte response, causing OOM before the size check triggers. The correct approach is to use a streaming reader with a size-limited wrapper.

**Minor issue** — `allowed_domains` is always `None` in injection: In `injection.rs` line 324, `allowed_domains` is hardcoded to `None`, meaning domain restrictions are never enforced in practice. The feature exists but is not wired up from manifest data.

### src/host_functions/logging.rs

Clean implementation with per-plugin rate limiting (100 entries/second). The rate limiter uses a sliding window approach. Rate-limited entries are silently dropped (returns `Ok` with empty response), which is the correct behavior to avoid confusing plugins.

No issues found.

### src/host_functions/config.rs

Simple, correct implementation. The host only returns the calling plugin's own config section, never another plugin's. Missing config returns empty JSON object.

No issues found.

### src/host_functions/blob.rs

Well-designed blob storage host functions with automatic key scoping (`<plugin_id>/blobs/<user_key>`). Capability checks for read, write, and delete are correct. Base64 encoding/decoding handles binary data across the JSON boundary.

**Critical issue** — No blob size limit: There is no maximum size check on the decoded blob data in `host_blob_store()`. A plugin could attempt to store arbitrarily large blobs, limited only by available memory during base64 decoding. The `data_base64` field in the request is a JSON string with no size limit, and `base64::decode()` will allocate the full decoded output in memory. This could be used for denial-of-service.

**Major issue** — Key path traversal: The `scoped_key()` function at line 98 uses simple string concatenation: `format!("{plugin_id}/blobs/{user_key}")`. If a plugin passes `user_key = "../../other-plugin/blobs/secret"`, the resulting key would be `my-plugin/blobs/../../other-plugin/blobs/secret`. Depending on the blob backend implementation, this could allow path traversal to access other plugins' blobs. The key should be validated to reject `..` components and other path traversal sequences.

### tests/capability_enforcement.rs

Comprehensive integration test covering all five capability enforcement scenarios: first-party auto-grant, third-party approval, third-party unapproved capabilities, CAP_001 rejection, and config modification enabling load. Well-structured with clear test names.

### tests/communication.rs

Excellent integration tests for plugin-to-plugin communication patterns: workflow chaining, shared canonical collections, absence of direct plugin calls, and storage plugin_id scoping. The test at line 452 (`no_host_function_exists_for_direct_plugin_calls`) is particularly valuable as a negative test ensuring no cross-plugin invocation path exists.

**Minor issue** — Test expects 7 host functions but blob functions exist: The test at line 479 asserts `fn_names.len() == 7`, but `injected_function_names()` would return 11 for all 10 capabilities. The test only passes because it uses 6 capabilities (excludes blob and delete). This test will break when blob injection is wired up.

### tests/community_plugin.rs

Six integration tests covering community plugin discovery, approval requirements, host function parity with first-party, coexistence, partial approval rejection, and full loading lifecycle. Coverage is thorough.

### tests/crash_isolation.rs

Strong crash isolation tests validating: WASM traps return errors without crashing Core, error codes are correct (PLUGIN_010), healthy plugins continue after another crashes, lifecycle manager can force-unload, memory isolation prevents corruption, and crashed plugins can be retried.

### tests/runtime_capability_test.rs

Systematic tests for every host function's runtime capability check (CAP_002). Each test verifies the correct error code, severity (Fatal), and that the error message includes both the capability name and plugin ID. Also tests the positive path (approved capability allows execution).

### tests/injection_test.rs

Integration tests for the injection layer, verifying that `build_host_functions()` produces the correct set of Extism `Function` objects. Includes a namespace verification test ensuring all functions use the `life_engine` namespace. The `injected_function_names_matches_build_host_functions` test is valuable but only covers a subset of capabilities.

---

## Problems Found

### Critical

1. **Blob host functions not actually injected** (`src/injection.rs`)
   - `build_host_functions()` does not build or inject blob host functions despite `injected_function_names()` advertising them. Blob storage is non-functional via the plugin system.
   - Fix: Add `build_blob_store_function()`, `build_blob_retrieve_function()`, and `build_blob_delete_function()` builders in `injection.rs`, gated on `StorageBlobWrite`, `StorageBlobRead`, and `StorageBlobDelete` respectively. Requires wiring in a `BlobBackend` dependency through `InjectionDeps`.

2. **SSRF vulnerability in HTTP host function** (`src/host_functions/http.rs`)
   - No protection against requests to internal/private network addresses when `allowed_domains` is `None`. Plugins can probe internal services, cloud metadata endpoints, and local network.
   - Fix: Add private IP range blocking as a baseline, independent of `allowed_domains`. Block RFC 1918, link-local, loopback, and cloud metadata addresses unless explicitly allowed.

3. **No blob size limit** (`src/host_functions/blob.rs`)
   - No maximum size enforcement on blob data. A plugin can attempt to store arbitrarily large blobs, potentially causing OOM.
   - Fix: Add a `MAX_BLOB_SIZE` constant (e.g., 50 MB) and check `data.len()` after base64 decoding.

4. **Mutex poison in async executor** (`src/execute.rs`)
   - `std::sync::Mutex` with `.unwrap()` in async context propagates poison from any panic, making the entire executor permanently unusable.
   - Fix: Either use `tokio::sync::Mutex` or handle `PoisonError` explicitly to recover gracefully.

### Major

1. **Executor serializes all plugin execution** (`src/execute.rs:79`)
   - The handles mutex is held for the entire WASM call duration. Only one plugin can execute at a time.
   - Fix: Either use per-plugin locks, or clone/extract the needed handle before the WASM call so the global lock is released.

2. **WASI enabled unconditionally** (`src/runtime.rs:182`)
   - All plugins get WASI access regardless of trust level or declared capabilities. This provides file system access paths, environment variables, and clock functions that may not be appropriate for sandboxed third-party plugins.
   - Fix: Make WASI enablement configurable per-plugin, defaulting to disabled for third-party plugins.

3. **HTTP response body downloaded fully before size check** (`src/host_functions/http.rs:193-200`)
   - `response.bytes().await` downloads the entire body into memory before the 10 MB check. A malicious server could send a multi-gigabyte response.
   - Fix: Use `response.bytes()` with a streaming approach or `content_length()` pre-check, and limit the read incrementally.

4. **Blob key path traversal** (`src/host_functions/blob.rs:98`)
   - `scoped_key()` does not sanitize the user-provided key. `../` sequences could escape the plugin's namespace depending on the blob backend.
   - Fix: Validate that `user_key` does not contain `..`, does not start with `/`, and normalize the path before constructing the scoped key.

5. **`injected_function_names()` / `build_host_functions()` contract mismatch** (`src/injection.rs`)
   - The two functions disagree on what functions exist for blob capabilities. The names function returns blob names, but build does not create them.
   - Fix: This is a consequence of the blob injection gap (Critical #1). Fixing that issue resolves this one.

### Minor

1. **`declared_emit_events` never wired up** (`src/injection.rs:365`)
   - Manifest-declared event restrictions exist in code but are never populated at runtime. Events validation is effectively disabled.
   - Fix: Pass `manifest.events.emit` to the `EventsHostContext` during injection.

2. **`execution_depth` always 0** (`src/injection.rs:366`)
   - Cascading event depth tracking exists but is never incremented. The depth field in emitted events is always 0.
   - Fix: Wire up depth tracking in the workflow engine when a plugin call is triggered by another plugin's event.

3. **`allowed_domains` never populated** (`src/injection.rs:324`)
   - HTTP domain restrictions exist in the host function but are never configured from manifest data.
   - Fix: Parse manifest `http_outbound` domains and pass them to `HttpHostContext`.

4. **Event naming validation doesn't check plugin ID prefix** (`src/manifest.rs:517`)
   - Event names are validated for 3-part dot-separated format but not that the first part matches the declaring plugin's ID.
   - Fix: Compare `parts[0]` against `plugin.id`.

5. **Trap detection relies on string matching** (`src/runtime.rs:71-92`)
   - WASM trap classification depends on error message substrings. Version changes in Extism/Wasmtime could break this.
   - Fix: Check if Extism provides a typed error enum or error code for traps, falling back to string matching only as a last resort.

6. **No maximum length for plugin ID / action names** (`src/manifest.rs`)
   - No length caps could cause issues with path construction, logging, and storage key generation.
   - Fix: Add reasonable length limits (e.g., 64 chars for plugin ID, 128 for action names).

7. **Error code gap** (`src/error.rs`)
   - PLUGIN_006 is missing from the error code sequence. This is cosmetic but could cause confusion.
   - Fix: Either add PLUGIN_006 or document the gap.

8. **Communication test will break when blob injection is added** (`tests/communication.rs:479`)
   - The test hardcodes `fn_names.len() == 7` for all capabilities, but this count will change when blob functions are injected.
   - Fix: Update the expected count when blob injection is implemented, or restructure the test to check for specific names rather than counts.

---

## Recommendations

1. **Priority 1: Fix blob injection gap** — This is the most impactful issue. The blob storage host function code is written and tested in isolation, but never wired into the injection layer. This means no plugin can actually use blob storage through the WASM sandbox. Add blob builders to `injection.rs` and wire `BlobBackend` through `InjectionDeps`.

2. **Priority 1: Address SSRF** — Add private IP range blocking to the HTTP host function. This is a security vulnerability that allows malicious plugins to access internal services.

3. **Priority 2: Fix executor locking** — The single mutex around all plugin handles serializes execution. Consider per-plugin `Mutex<PluginInstance>` or extracting handles into a concurrent-safe structure.

4. **Priority 2: Wire up manifest-declared restrictions** — `declared_emit_events`, `allowed_domains`, and `execution_depth` are all implemented but disconnected. Wiring these up would complete the capability enforcement picture.

5. **Priority 3: Add blob size limits and key sanitization** — These are straightforward fixes that close denial-of-service and path traversal vectors.

6. **Priority 3: Make WASI configurable** — Third-party plugins should not get WASI access by default. Add a manifest-level or config-level WASI toggle.
