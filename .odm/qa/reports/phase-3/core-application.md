# Core Application Review Report

## Summary

The `apps/core` crate is the main binary for Life Engine Core. It orchestrates a 10-step startup sequence, wires together subsystems (auth, storage, plugins, search, federation, identity, workflows), and exposes a full REST/GraphQL/SSE API surface via axum. The codebase is well-structured with clear module boundaries, comprehensive configuration validation, and strong security posture (TLS enforcement, credential redaction, encryption-at-rest, rate limiting). However, the review identified several issues across startup, concurrency, error handling, and security.

## File-by-file Analysis

### src/main.rs

The entry point implements a 10-step startup orchestrator with timing metrics per step. Key observations:

- The `log_step!` and `fail_step!` macros provide structured startup diagnostics. `fail_step!` calls `std::process::exit(1)` directly, which skips destructors. This is acceptable for startup failures before resources are allocated, but the Step 4 failure paths occur after storage and schema_registry are allocated.
- Step 4b (canonical schema migrations) contains inline migration logic that is ~150 lines of business logic embedded in `main()`. This is a cohesion concern — the migration engine should encapsulate this.
- The `NoOpPluginExecutor` is a placeholder that always returns errors. This is fine during transition but any workflow execution before real plugins are loaded will produce confusing errors with no indication the executor is a stub.
- `build_router()` correctly wires all routes with state. The route structure is clean and RESTful.
- `serve_tls()` properly implements connection limiting via semaphore (`MAX_TLS_CONNECTIONS = 1024`), graceful shutdown via `tokio::select!`, and per-connection error handling.
- The `dynamic_cors_middleware` reads config on every request via `RwLock`. This is correct for hot-reload but the lock acquisition on the hot path could become a bottleneck under high concurrency.
- `init_logging` intentionally leaks a span guard via `std::mem::forget(guard)`. This is documented and correct for process-lifetime spans, but static analysis tools may flag it.
- `instantiate_transports` resilience is good — one failed transport does not block others.

### src/config.rs

Comprehensive configuration system with three-layer priority (YAML file, env vars, CLI args). Key observations:

- Sensitive fields are properly redacted in `Debug` implementations (`OidcSettings`, `StorageSettings`, `PostgresSettings`) using a `REDACTED` constant.
- The `validate()` method is thorough: enforces TLS for non-localhost, blocks `local-token` auth on network-facing instances, validates all enum-like string fields, ensures OIDC/WebAuthn sections are complete when selected.
- `apply_env_overrides()` is verbose but correct — each `get_or_insert` call ensures the OIDC/WebAuthn struct exists before setting individual fields. The repetitive `get_or_insert` with default structs could be extracted into a helper.
- The `merge_partial` + `merge_json` approach for runtime config updates is sound. Validation runs on the merged result before accepting changes.
- The `startup` submodule introduces new TOML-based config types for Phase 9 transition. Both config systems run in parallel during startup, with the legacy system as primary — this is a clean migration path.
- `to_redacted_json()` redacts OIDC client_secret and PG password but does not redact `storage.passphrase`. The passphrase field on `StorageSettings` is `Option<String>` and could appear in serialized output.

### src/auth/middleware.rs

Auth middleware with per-IP rate limiting on failed attempts. Key observations:

- The rate limiter uses `tokio::sync::Mutex` wrapping a `HashMap<IpAddr, Vec<Instant>>`. The periodic cleanup (`maybe_cleanup` every 100 operations) prevents unbounded memory growth from inactive IPs.
- Auth bypass paths are hardcoded: health, token generation, storage init, login, register, WebAuthn, and OIDC well-known. These are correct and well-documented.
- X-Forwarded-For header is trusted unconditionally. The middleware does not check `config.network.behind_proxy` before trusting this header. An attacker on a directly-exposed instance could spoof their IP to bypass rate limiting.
- The `is_rate_limited` check calls `maybe_cleanup` which acquires the atomic counter — this means every rate-limit check involves an atomic operation followed by a potential full-map scan. Under high load this could cause lock contention on the Mutex.
- Expired tokens increment the failure counter. This means legitimate clients with slightly-stale tokens could get rate-limited, which may cause poor UX during token refresh races.

### src/crypto.rs

Shared encryption utilities using AES-256-GCM and HKDF-SHA256. Key observations:

- `derive_key()` uses HKDF without a salt (`None`). HKDF is designed to work without salt, but including one (even a static application-level salt) would strengthen the extraction step against related-key attacks.
- `encrypt()` and `decrypt()` are correctly implemented with random nonces (via `OsRng`) and nonce-prepended ciphertext format.
- `hmac_sha256()` returns hex-encoded output — consistent and safe.
- The domain separator constants (`DOMAIN_CREDENTIAL_STORE`, `DOMAIN_IDENTITY_ENCRYPT`, `DOMAIN_IDENTITY_SIGN`) ensure key independence between subsystems. Good defence-in-depth.
- All functions are well-tested with roundtrip, uniqueness, tamper detection, and wrong-key tests.

### src/rekey.rs

Database re-encryption (passphrase change) for SQLCipher. Key observations:

- `derive_key()` (the fallback version without a DB path) uses a zeroed salt. This weakens key derivation significantly. The doc comment says "for tests" but the function is `pub` and could be called from production code.
- `run_rekey()` correctly prompts for passphrases via `rpassword` (no echo), validates non-empty, confirms match, derives keys, and verifies the rekey by re-opening with the new key.
- Passphrase strings are explicitly dropped after key derivation (`drop(current_passphrase)`) — good practice for sensitive data in memory.
- The rekey operation is not atomic — if power is lost between `PRAGMA rekey` and the verification step, the database may be in an inconsistent state. The warning to back up is appropriate.

### src/tls.rs

TLS configuration loading using rustls. Key observations:

- Supports PKCS#1, PKCS#8, and SEC1 (EC) key formats — comprehensive coverage.
- The `read_private_key` function reads all PEM items and returns the first key found. If a PEM file contains multiple keys, only the first is used (silently). A warning when multiple keys are present would be helpful for debugging.
- Error messages include file paths, which aids troubleshooting.
- Tests cover missing files, invalid PEM, and missing keys.

### src/federation.rs

Hub-to-hub federated sync between Core instances. Key observations:

- `FederationStore` uses `RwLock<HashMap>` for peers, sync history, and cursors — appropriate for read-heavy, write-light access patterns.
- `MAX_SYNC_HISTORY = 100` prevents unbounded growth of sync history. However, the trim logic should be verified (not visible in the read portion).
- The `FederationPeer` struct stores file paths to TLS certificates (`ca_cert_path`, `client_cert_path`, `client_key_path`) as `Option<String>`. These paths are not validated at peer creation time — invalid paths will only fail at sync time.
- All federation state is in-memory — it is lost on restart. For a production federation system, this needs persistence.

### src/household.rs

Multi-user household management. Key observations:

- Uses `Arc<RwLock<HashMap>>` for all stores — correct for async access patterns.
- `create_household()` does not check if the admin user is already in another household. A user could be added to multiple households, creating ambiguous state in `get_user_household()` which only returns one.
- `accept_invite()` (not fully visible in read) needs to validate invite expiration before accepting.
- The `user_household_map` maintains a 1:1 user-to-household mapping. If `create_household` is called twice for the same user, the map entry is silently overwritten.
- All state is in-memory and lost on restart.

### src/identity.rs

Identity credential storage with selective disclosure. Key observations:

- Claims are encrypted with a separate key from main storage — good defence-in-depth.
- The `disclose()` method generates time-limited tokens with HMAC-SHA256 signatures over a BTreeMap (deterministic ordering) — correct approach for reproducible signatures.
- The disclosure audit log is properly persisted with credential ID, claim names, recipient, and timestamp.
- The `IdentityStore::new()` never fails (`Result<Self>` always returns `Ok`) — the `Result` wrapper is unnecessary.
- W3C Verifiable Credentials 2.0 export is supported via `export_vc()`.

### src/install_service.rs

System service installation for Linux (systemd) and macOS (launchd). Key observations:

- Linux installer requires root and creates a dedicated `life-engine` user/group — proper service isolation.
- macOS installer runs as a user LaunchAgent (not daemon) — appropriate for a personal data tool.
- `run_cmd()` helper calls `std::process::exit(1)` on failure — no cleanup or rollback if a mid-installation step fails.
- `find_service_file()` (not visible) presumably locates the template files. If the binary is not run from the repo root, this could fail with a confusing error.

### src/manifest.rs

Plugin manifest reading and validation. Key observations:

- Validates reverse-domain ID format, semver version, Web Components element naming (requires hyphen), non-empty required fields, and capability lists.
- The `PluginManifest` struct uses `serde_json::Value` for extensible fields (`author`, `collections`, `settings`, `slots`) — good for forward compatibility.
- Duplicate `PluginManifest` struct exists in `plugin_loader.rs` with different fields. The two should be unified to avoid confusion.

### src/message_bus.rs

In-process event bus using `tokio::sync::broadcast`. Key observations:

- `DEFAULT_CAPACITY = 256` — if a slow subscriber falls behind, events are dropped and the subscriber sees `Lagged(n)`. The audit subscriber in `audit.rs` handles this correctly with a warning.
- `publish()` returns 0 when there are no subscribers — fire-and-forget semantics are correct for an event bus.
- The `BusEvent` enum is `Clone` and `Serialize/Deserialize` — needed for broadcast channel semantics and SSE serialization.

### src/pg_storage.rs

PostgreSQL storage adapter with connection pooling. Key observations:

- TLS is properly supported with `rustls` for the PostgreSQL connection, defaulting to `Require` mode.
- `make_rustls_config()` loads native root certificates and silently ignores individual cert load failures (`root_store.add(cert).ok()`). If all certs fail to load, the root store would be empty, causing all TLS connections to fail.
- The `PgSslMode::Prefer` variant is mapped to the same code path as `Require` — it does not actually implement fallback-to-plaintext behavior.
- Connection pool uses `deadpool-postgres` with configurable pool size (default 16).

### src/sqlite_storage.rs

SQLite storage adapter with WAL mode and change events. Key observations:

- Uses a single `Mutex<Connection>` for all access. The doc comments correctly note this serializes all database access and suggest `r2d2_sqlite` if it becomes a bottleneck.
- WAL mode and foreign keys are enabled on connection creation.
- `open_with_key()` validates the key by reading `sqlite_master` after setting `PRAGMA key` — correct verification pattern.
- `AuditLogger::cleanup_old_entries()` is called on every connection open — this keeps the audit log bounded but adds startup latency proportional to log size.
- `open_encrypted()` validates that the derived hex key contains only hex characters before interpolation into the PRAGMA statement — prevents SQL injection via crafted keys.

### src/storage_migration.rs

SQLite-to-PostgreSQL migration with transactional safety. Key observations:

- Migration is performed within a single PostgreSQL transaction with per-collection record count verification before commit — strong data integrity guarantee.
- Batch size of 500 records balances memory usage and throughput.
- `ON CONFLICT (id) DO NOTHING` makes the migration idempotent — safe to retry after partial failure.
- The per-collection verification (rather than global COUNT) correctly handles pre-existing rows from previous runs.
- Progress callback provides real-time migration status.

### src/plugin_loader.rs

Plugin lifecycle management. Key observations:

- Two `PluginManifest` structs exist: one in `manifest.rs` (detailed, for the manifest schema spec) and one here (simplified, for runtime). They serve different purposes but the naming overlap is confusing.
- `load_all()` returns a `Vec` of errors rather than failing on first error — correct for plugin isolation (one bad plugin should not block others).
- Schema registry integration registers plugin collections under `{plugin_id}/{collection_name}` namespacing — prevents cross-plugin collection conflicts.
- Credential store integration provides scoped access via `PluginCredentialBridge`.

### src/plugin_signing.rs

Ed25519 plugin signing and verification. Key observations:

- Signature covers `SHA-256(wasm_bytes || manifest_hash)` — binding the binary to its manifest prevents post-signing capability tampering.
- Revocation list with normalized lowercase hex keys — consistent comparison.
- Three verification tiers (Unverified, Reviewed, Official) — good for progressive trust.
- `sign_plugin()` uses `expect` on hex decode of its own output — safe since `compute_manifest_hash` always produces valid hex.

### src/rate_limit.rs

General per-IP rate limiting using the `governor` crate. Key observations:

- Uses `DashMap` for lock-free concurrent access — good for high-throughput scenarios.
- `reconfigure()` replaces the entire limiter, discarding all per-IP state. This means a config change resets all clients to fresh budgets.
- X-Forwarded-For is trusted without checking `behind_proxy` config — same issue as in auth middleware.
- Health endpoint is exempt from rate limiting — correct.
- The `RwLock<Arc<KeyedLimiter>>` pattern allows hot-reload without blocking concurrent readers.

### src/search.rs

Full-text search using tantivy. Key observations:

- Index is in-memory (`Index::create_in_ram`) — search state is lost on restart. For a personal data tool this may be acceptable if re-indexing is fast, but a persistent index would improve restart time.
- `index_record()` commits after every single document. The `index_records_bulk()` method commits once at the end — the bulk method should be preferred for batch operations.
- Search result limit is capped at 100 (`limit.min(100)`) — prevents excessive memory usage.
- Collection filtering uses a `BooleanQuery` with `MUST` clauses — correct AND semantics.

### src/sync_primitives.rs

Shared sync primitives for federation and app sync. Key observations:

- `apply_change()` correctly implements last-write-wins (LWW) conflict resolution using version numbers.
- Delete operations check version before deleting — stale deletes are skipped.
- Update operations create the record if it doesn't exist locally — handles the case where a create was missed.
- Tests use a purpose-built `TestStorage` with `std::sync::Mutex` — acceptable for synchronous test code.

### src/wasm_runtime.rs

WASM plugin host bridge. Key observations:

- Capability enforcement is thorough — every host function checks for the required capability before proceeding.
- Collection scoping ensures plugins can only access their declared collections.
- Rate limiting is per-plugin with a configurable calls-per-second limit.
- `MAX_LOG_MESSAGE_LEN = 1_000` prevents plugins from flooding logs.
- `handle_http_request` (not fully visible) should validate URLs against `allowed_http_domains` to prevent SSRF.
- `handle_config_get` always returns null — marked as "config store TBD".

### src/wasm_adapter.rs

Adapter wrapping native plugins through the WASM host bridge for migration validation. Key observations:

- Clean adapter pattern — delegates `CorePlugin` trait methods to the inner plugin while routing storage/event operations through the bridge.
- Used during Phase 4 migration to verify WASM bridge parity with native execution.
- Default resource limits: 64 MB memory, 30s timeout, 1000 calls/sec.

### src/credential_store.rs

Encrypted credential storage. Key observations:

- The doc comment says "XOR-based key derivation" but the actual implementation uses AES-256-GCM via `crypto::encrypt/decrypt` — the doc comment is outdated and misleading.
- Credentials are scoped by `(plugin_id, key)` composite primary key — correct isolation.
- `encrypt()` uses `expect` on AES-256-GCM encryption — valid because encryption with a correct-length key cannot fail.
- Values are never logged — only plugin_id and key appear in trace output.

### src/conflict.rs

Conflict resolution for local-first sync. Key observations:

- Three resolution strategies: LastWriteWins (default), FieldLevelMerge (contacts, events), ManualResolution (notes).
- `resolve_field_merge()` correctly handles: both-changed-to-same-value (no conflict), one-side-changed (take that side), both-changed-differently (escalate to manual).
- Uses `std::sync::Mutex` with a documented justification — short lock durations on in-memory HashMap, avoiding tokio::sync::Mutex overhead.
- `detect_conflict()` requires both sides to have diverged from the base version — correct three-way merge detection.

### src/connector.rs

External service connector trait and sync backoff. Key observations:

- The `Connector` trait defines a clean lifecycle: `authenticate -> sync -> disconnect`.
- `SyncBackoff` implements exponential backoff with configurable threshold and cap — `exponent.min(10)` prevents overflow on the bit shift.
- `record_failure()` returns `None` when below threshold (normal interval) or `Some(delay)` when backoff is active — clean API.

### src/audit.rs

Audit event subscriber for the message bus. Key observations:

- Spawns a background task that persists audit entries for storage mutations and plugin lifecycle events.
- Non-auditable events (`NewRecords`, `SyncComplete`) are explicitly skipped.
- Uses `tokio::sync::Mutex` for the SQLite connection — consistent with the rest of the codebase.
- The `Lagged` case logs a warning but does not attempt recovery — acceptable since audit events are best-effort.
- Tests use `tokio::time::sleep(50ms)` for synchronization — fragile under load; a channel-based notification would be more reliable.

### src/routes/data.rs

CRUD routes for `/api/data/{collection}`. Key observations:

- `create_record()` strips client-supplied `_user_id` and `_household_id` before injecting from the authenticated identity — prevents identity spoofing.
- Pagination is clamped via `Pagination::clamped()` — prevents excessive result sets.
- Error responses follow a consistent `{ "error": { "code": "...", "message": "..." } }` structure.
- All routes extract storage from `AppState.storage` with a `None` check — correct for the deferred-init pattern.

### src/routes/events.rs

SSE event stream. Key observations:

- `RecordChanged` and `RecordDeleted` events are explicitly excluded from the SSE stream — these are internal events for the search indexer and audit subscriber.
- Keep-alive interval of 15 seconds with "ping" text prevents connection timeouts.
- Collection and event_type filters are applied server-side before sending to the client.

### src/routes/graphql.rs

GraphQL API auto-generated from CDM schemas. Key observations:

- GraphQL types mirror CDM schemas (Task, Contact, CalendarEvent, etc.) with proper enum mappings.
- CalendarEvent has a nested `attendees -> contacts` resolver — demonstrates the cross-entity query capability.
- Uses `async-graphql` with proper State injection.

### src/routes/health.rs

Health check endpoint. Key observations:

- `AppState` is the central shared state struct — contains all subsystem handles as `Option<Arc<T>>`.
- Health response includes version, uptime, and per-plugin status.
- The health check acquires the plugin_loader Mutex — under extreme contention this could slow health checks. Consider a cached plugin count.

### src/routes/household.rs

Household management routes. Key observations:

- All endpoints require an authenticated identity with a `user_id`.
- Role-based access control: only admins can invite members or update roles.
- Error responses are consistent with the rest of the API.

### tests/schema_validation_test.rs

Integration tests validating fixtures against JSON schemas. Key observations:

- Tests both positive (valid fixture passes) and negative (invalid fixture fails) cases for each CDM schema.
- Schema files are loaded from `.odm/doc/schemas/` relative to the repo root.
- Covers tasks, events, contacts, emails, files, notes, credentials, and plugin manifests.

### deploy/docker-compose.yml

Docker deployment configuration. Key observations:

- Container binds to `0.0.0.0` with `local-token` auth — this would fail the config validator for non-localhost addresses unless `behind_proxy` is set. The compose file does not set `LIFE_ENGINE_BEHIND_PROXY=true`.
- Memory limit of 256M is reasonable for a personal data tool.
- Health check uses `wget --spider` against the health endpoint.

## Problems Found

### Critical

- **X-Forwarded-For trusted without proxy check (auth/middleware.rs:128-140, rate_limit.rs:113-125)** — Both the auth middleware and rate limiter trust the `X-Forwarded-For` header unconditionally. On a directly-exposed instance (not behind a proxy), an attacker can spoof their IP to bypass rate limiting. The `config.network.behind_proxy` flag exists but is not checked before trusting this header.

- **Docker compose config contradiction (deploy/docker-compose.yml:15)** — The compose file sets `LIFE_ENGINE_AUTH_PROVIDER=local-token` with `LIFE_ENGINE_CORE_HOST=0.0.0.0`. The config validator rejects `local-token` on non-localhost addresses. Either `behind_proxy` must be set to `true` or the auth provider must be changed.

### Major

- **In-memory federation/household state lost on restart (federation.rs, household.rs)** — `FederationStore` and `HouseholdStore` use in-memory `HashMap`s. All peer registrations, household memberships, and sync cursors are lost on restart. These need persistence (SQLite tables or similar).

- **`to_redacted_json()` does not redact storage passphrase (config.rs:940-960)** — The `storage.passphrase` field is a plaintext `Option<String>` that is not redacted in the JSON output exposed via the `GET /api/system/config` endpoint.

- **`PgSslMode::Prefer` does not implement fallback behavior (pg_storage.rs:151)** — The `Prefer` variant is mapped to the same TLS-required code path as `Require`. If TLS negotiation fails, the connection fails entirely rather than falling back to plaintext.

- **Zeroed salt in `rekey::derive_key()` fallback (rekey.rs:91-92)** — The public `derive_key()` function uses a zeroed salt, which significantly weakens Argon2id key derivation. While documented as "for tests", it is `pub` and could be accidentally used in production.

- **Outdated doc comment on credential store (credential_store.rs:11-12)** — The module doc says "XOR-based key derivation" but the implementation uses AES-256-GCM. This is misleading and could cause confusion during security audits.

- **Duplicate `PluginManifest` struct (manifest.rs vs plugin_loader.rs)** — Two different structs named `PluginManifest` with overlapping but different fields. This creates confusion about which is authoritative.

- **Search index is in-memory only (search.rs:70)** — `Index::create_in_ram()` means all indexed data is lost on restart. Re-indexing from storage on startup could be slow with large datasets.

### Minor

- **Step 4b migration logic inlined in main() (main.rs:329-499)** — ~170 lines of canonical schema migration logic is embedded directly in the startup function. This should be extracted into a dedicated module for testability and readability.

- **`init_logging` leaks span guard (main.rs:1137-1138)** — `std::mem::forget(guard)` is intentional and documented but may trigger warnings from static analysis tools. A comment at the call site explains the reasoning.

- **Health check acquires plugin_loader Mutex (routes/health.rs:88)** — Under high contention from plugin operations, health checks could be delayed. Consider caching the plugin count or using a `RwLock`.

- **HKDF without salt in crypto.rs:19** — `Hkdf::<Sha256>::new(None, ...)` omits the salt parameter. While HKDF is designed to work without salt, a static application-level salt would strengthen the extraction step.

- **Multiple private keys silently ignored (tls.rs:111)** — If a PEM file contains multiple private keys, only the first is used with no warning. This could cause confusion when the wrong key is silently selected.

- **Audit subscriber tests use sleep-based synchronization (audit.rs:154, 188, 254, 289)** — `tokio::time::sleep(Duration::from_millis(50))` is fragile under CI load. Channel-based synchronization would be more reliable.

- **`IdentityStore::new()` returns `Result` but never fails (identity.rs:178-186)** — The constructor always returns `Ok`. The `Result` wrapper is unnecessary.

- **Rate limiter `reconfigure()` discards all per-IP state (rate_limit.rs:65-71)** — A config change resets all rate-limit budgets. This could allow a brief window where previously-throttled IPs can send a burst of requests.

- **`ConflictStore` and `HouseholdStore` use `Arc` inside `Arc` (household.rs:72-76)** — The fields are `Arc<RwLock<HashMap>>` but the store itself is wrapped in `Arc` at the call site. The inner `Arc` is redundant since the outer `Arc` provides shared ownership.

## Recommendations

1. **Fix X-Forwarded-For trust** — Check `config.network.behind_proxy` before parsing `X-Forwarded-For` in both auth middleware and rate limit middleware. When not behind a proxy, always use `ConnectInfo<SocketAddr>`.

2. **Persist federation and household state** — Add SQLite tables for federation peers, sync cursors, households, and invites. Load on startup, write on mutation.

3. **Redact storage passphrase** — Add `storage.passphrase` to the redaction logic in `to_redacted_json()`.

4. **Fix docker-compose** — Either add `LIFE_ENGINE_BEHIND_PROXY=true` or change the auth provider to `oidc`/`webauthn` in the compose file.

5. **Fix PgSslMode::Prefer** — Implement actual fallback behavior: try TLS first, catch the error, retry with `NoTls`.

6. **Extract migration logic from main()** — Move the Step 4b canonical migration logic into a `canonical_migrations` module with its own tests.

7. **Update credential store doc comment** — Replace the "XOR-based" description with the actual AES-256-GCM implementation description.

8. **Unify plugin manifest types** — Either merge the two `PluginManifest` structs or rename one to avoid confusion (e.g., `DiscoveredManifest` vs `ManifestSpec`).

9. **Consider persistent search index** — Use `Index::create_in_dir()` instead of `create_in_ram()` for production deployments to avoid re-indexing on restart.

10. **Mark `rekey::derive_key()` as `#[cfg(test)]`** — Prevent the zeroed-salt fallback from being callable in production code.
