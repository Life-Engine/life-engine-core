# Life Engine Core — Executive Summary

Reviewed: 2026-03-28

## Executive Summary

Life Engine Core is a Rust monorepo implementing a local-first personal data engine with encrypted storage, a WASM plugin sandbox, multi-protocol transport layers (REST, GraphQL, CalDAV, CardDAV, webhooks), and a YAML-driven workflow engine. The foundational infrastructure — type system, trait contracts, cryptographic primitives, and SQLite storage — is architecturally sound, well-tested, and demonstrates a security-first mindset. Correct algorithm choices (AES-256-GCM, Argon2id, HMAC-SHA256), defence-in-depth credential encryption, and thorough capability enforcement through the WASM sandbox provide a strong base.

However, the codebase is in an active architecture transition (Phase 2+) and shows the strain of this migration. Several modules exist as non-functional stubs (DAV transports, webhook delivery, plugin pipeline integration), critical security issues remain in cryptographic salt handling and request header trust, and significant gaps in SQL injection protection, GraphQL query limits, and search multi-tenancy need resolution before production use. The project's greatest strength — deep test coverage across all crates — provides high confidence that fixes can be applied without regression.

---

## Project Health Scorecard

Each area is rated 1 (critical concerns) through 5 (production-ready):

- **Types and Data Models** — 4/5. Clean CDM, strong serde patterns, thorough tests. Minor spec divergences and a silent test bug.
- **Traits and Interfaces** — 3/5. Compiled modules are well-designed, but three orphaned files with missing dependencies cannot compile. Duplicate error types.
- **Cryptography** — 2/5. Correct algorithms, but hardcoded/zeroed salts in production paths, missing key zeroization, and duplicated implementations across three crates.
- **Storage (SQLite)** — 3/5. Security-conscious design, but SQL injection vectors in migration executor and sort fields, missing transaction boundaries, and credential re-encryption gap after rekey.
- **Authentication** — 3/5. Sound architecture, but X-Forwarded-For spoofing bypasses rate limiting, API key validation is O(n), key hashes exposed via list endpoint, duplicate rate limiters.
- **Build and Configuration** — 3/5. Workspace management is mostly clean, but a stale Dockerfile, config format mismatch (TOML vs YAML), version pinning inconsistencies, and broken Docker Compose port mapping.
- **Plugin System** — 3/5. Strong capability enforcement design, but blob host functions not wired, SSRF vulnerability in HTTP host function, executor serializes all plugin execution behind one lock.
- **Plugin SDK** — 3/5. Good DX and ergonomics, but divergent Capability enums, two undocumented plugin models, missing WASM host_call bridge.
- **Transport REST** — 3/5. Well-structured, but auth bypass broken for parameterized routes, dual Identity types cause runtime extraction failure, no request body size limits.
- **Transport GraphQL** — 2/5. Functional schema, but no query depth/complexity limits, introspection enabled in production, no auth at resolver level, no collection validation on mutations.
- **Workflow Engine** — 3/5. Clean pipeline design, but race condition in job registration, unbounded job registry, event depth tracking defeated, per-record WASM instantiation in migrations.
- **DAV and Webhooks** — 1/5. Transport crates are empty stubs with no protocol logic. Webhook delivery is unimplemented. CalDAV/CardDAV have zero RFC compliance.
- **Core Application** — 3/5. Well-orchestrated startup, but in-memory state lost on restart (federation, households), X-Forwarded-For trust without proxy check, storage passphrase not redacted.
- **Connector Plugins** — 3/5. Consistent architecture and good normalizers, but all `execute()` methods are no-op passthroughs, no retry/backoff on most connectors, S3 credential stored directly.
- **API Plugins (CalDAV/CardDAV)** — 2/5. Good serialization foundations, but SDK lacks PROPFIND/REPORT HTTP methods (blocking), UTF-8 line folding corruption, incomplete RFC compliance.
- **Backup and Webhooks** — 2/5. Sound backup architecture, but fixed cryptographic salt, webhook sender does not actually send, unencrypted manifests leak metadata.
- **Search Indexer** — 2/5. Working core engine, but no multi-tenancy isolation, per-document commits cause write amplification, plugin is a hollow shell, volatile in-memory index.
- **Test Infrastructure** — 4/5. Excellent mock quality, high assertion specificity, thorough coverage. Minor duplication of WASM fixtures.

---

## Critical Findings

These issues represent security vulnerabilities, data corruption risks, or fundamental functional gaps that must be addressed before any production deployment.

### Security: Cryptographic Salt Failures

- **Production SQLCipher uses all-zeros salt** — `apps/core/src/rekey.rs:88-93`. The `derive_key()` fallback uses `[0u8; SALT_LENGTH]`. The only production call site in `sqlite_storage.rs:111` hits this path, meaning all databases derived from the same passphrase share identical encryption keys.
- **Backup plugin uses hardcoded fixed salt** — `plugins/engine/backup/src/crypto.rs:17`. The constant `b"life-engine-salt"` means identical passphrases produce identical keys across all installations and all backups.
- **Salt generation uses `thread_rng()` instead of `OsRng`** — `apps/core/src/rekey.rs:31`. Security-critical material should use the OS entropy source directly.

### Security: SQL Injection

- **Migration executor interpolates field names into DDL** — `packages/storage-sqlite/src/migration/executor.rs:77-83, 97-103`. Plugin-provided field names are inserted directly into `CREATE INDEX` statements. A malicious plugin manifest could execute arbitrary SQL.
- **Sort field injection in backend queries** — `packages/storage-sqlite/src/backend.rs:183`. The `ORDER BY` clause interpolates caller-provided field names directly into SQL.

### Security: Rate Limit Bypass via X-Forwarded-For

- **Trusted unconditionally across three locations** — `apps/core/src/auth/middleware.rs:128-133`, `apps/core/src/rate_limit.rs:113-118`, `packages/transport-rest/src/middleware/auth.rs:69-74`. An attacker on a directly-exposed instance can spoof their IP to bypass all rate limiting. The `behind_proxy` config flag exists but is never checked.

### Security: GraphQL Query Abuse

- **No query depth limit** — `apps/core/src/routes/graphql.rs:1366`. No `.limit_depth()` on the schema. Arbitrarily nested queries can cause stack overflow or memory exhaustion.
- **No query complexity limit** — No `.limit_complexity()`. A single query can request all collections with all fields and nested resolvers.
- **Introspection enabled in production** — The full schema is exposed to unauthenticated clients. No `.disable_introspection()` for production builds.
- **No authentication at GraphQL resolver level** — Mutations accept any request with no authorization check. Identity is not propagated into the async-graphql context.

### Security: Plugin System SSRF

- **HTTP host function allows internal network requests** — `packages/plugin-system/src/host_functions/http.rs`. When `allowed_domains` is `None` (the current default), plugins can probe internal services, cloud metadata endpoints (169.254.169.254), and the local network. No private IP range blocking exists.

### Security: API Key Material Exposure

- **`list_keys` returns key hashes and salts** — `packages/auth/src/handlers/keys.rs:103-127`. Full `ApiKeyRecord` objects including cryptographic material are returned, enabling offline brute-force attacks if this data reaches an API response.

### Functional: Credential Re-encryption Missing After Rekey

- **Master key rotation breaks all credential reads** — `packages/storage-sqlite/src/credentials.rs` + `src/lib.rs`. Per-credential keys are derived from the master key via HMAC. After `rekey()`, derived keys change but stored ciphertext remains encrypted under old keys. All credential reads will fail with decryption errors.

### Functional: Transport Stubs

- **CalDAV, CardDAV, and Webhook transports are non-functional** — All three transport crates implement `Transport::start()` as a no-op that logs but does not bind sockets or serve requests. Zero protocol logic exists.
- **Webhook sender does not actually send** — `plugins/engine/webhook-sender/src/lib.rs:261-271`. The `handle_event` method logs matches but never dispatches HTTP requests. `reqwest` is unused.

### Functional: REST Auth Middleware Broken for Parameterized Routes

- **Public route bypass uses concrete paths** — `packages/transport-rest/src/middleware/auth.rs:56-60`. Route keys use pattern syntax (`:collection`) but matching uses concrete request paths (`tasks`). Any parameterized route marked as public still requires authentication.
- **Dual Identity types cause runtime extraction failure** — `middleware/auth.rs:19-24` defines `Identity{user_id, provider, scopes}` while handlers extract `life_engine_types::identity::Identity{subject, issuer, claims}`. These are different types; Axum's type-keyed extension lookup will return `None` for every authenticated request.

### Functional: Plugin SDK Missing HTTP Methods

- **No PROPFIND or REPORT in `HttpMethod` enum** — `packages/plugin-sdk-rs/src/types.rs`. These are the two essential WebDAV/CalDAV/CardDAV methods. Without them, no DAV client can discover or sync resources. This blocks both API plugins.

### Functional: Data Corruption in Line Folding

- **UTF-8 characters split at byte boundaries** — `plugins/engine/api-caldav/src/serializer.rs:fold_line` and `plugins/engine/api-carddav/src/serializer.rs:fold_line`. Multi-byte characters straddling the 75-octet fold boundary are replaced via `from_utf8_lossy`, silently corrupting non-ASCII text in contact names, event titles, and descriptions.

---

## Major Findings

### Cryptography and Key Management

- **No key zeroization in `packages/crypto/` crate** — Derived keys returned as `[u8; 32]` on the stack are never zeroized. The crate does not depend on `zeroize`. (`packages/crypto/src/kdf.rs`)
- **Duplicated encryption implementations** — AES-256-GCM encrypt/decrypt exists in three separate locations: `packages/crypto/`, `apps/core/src/crypto.rs`, and `plugins/engine/backup/src/crypto.rs`.
- **Core crypto HMAC lacks constant-time verification** — `apps/core/src/crypto.rs:59` returns hex-encoded strings with no verify counterpart. Callers using `==` leak timing information.
- **Core crypto panics on invalid key length** — `apps/core/src/crypto.rs:31,51` uses `.expect()` instead of returning `Result`.

### Storage

- **Missing transaction boundaries on mutations** — `packages/storage-sqlite/src/backend.rs:259-421`. Update operations involving extension merging (read then write) are not wrapped in explicit transactions, creating TOCTOU races.
- **Backup does not handle encrypted databases** — `packages/storage-sqlite/src/migration/backup.rs:32-53`. Opens the database without the encryption key, causing integrity check failures.
- **PRAGMA key quoting inconsistency** — The crate and app-level code use different quoting formats for the SQLCipher PRAGMA key. Only one is correct per the SQLCipher specification.
- **Schema divergence between crate and app** — `packages/storage-sqlite` and `apps/core/src/sqlite_storage.rs` define different columns, nullability, and table sets.

### Authentication

- **API key validation is O(n) full scan** — `packages/auth/src/handlers/keys.rs:164`. Every validation retrieves all keys and iterates, creating a DoS vector as key count grows.
- **JWKS thundering herd on cache miss** — `packages/auth/src/handlers/validate.rs:225-236`. Multiple threads observing a stale cache all initiate concurrent JWKS fetches.
- **No passphrase length limits** — `apps/core/src/auth/types.rs:40-46`. Extremely long passphrases submitted to Argon2id enable CPU/memory DoS.
- **Duplicate rate limiter implementations** — `packages/auth/src/handlers/rate_limit.rs` and `apps/core/src/auth/middleware.rs` maintain separate, uncoordinated rate limiters.

### Plugin System

- **Blob host functions not injected** — `packages/plugin-system/src/injection.rs`. `build_host_functions()` does not build blob host functions despite `injected_function_names()` advertising them. Blob storage is non-functional via the plugin system.
- **Executor serializes all plugin execution** — `packages/plugin-system/src/execute.rs:79`. The handles mutex is held for the entire WASM call duration. Only one plugin can execute at a time system-wide.
- **Mutex poison in async executor** — `packages/plugin-system/src/execute.rs`. `std::sync::Mutex` with `.unwrap()` in async context. A single panic permanently disables the executor.
- **WASI enabled unconditionally for all plugins** — `packages/plugin-system/src/runtime.rs:182`. Third-party plugins get filesystem, environment, and clock access regardless of trust level.
- **HTTP response body downloaded fully before size check** — `packages/plugin-system/src/host_functions/http.rs:193-200`. A malicious server can cause OOM before the 10 MB limit is checked.
- **No blob size limit** — `packages/plugin-system/src/host_functions/blob.rs`. Plugins can store arbitrarily large blobs with no enforcement.
- **Blob key path traversal** — `packages/plugin-system/src/host_functions/blob.rs:98`. `../` sequences in user-provided keys could escape the plugin's namespace.

### Workflow Engine

- **Race condition in `spawn()` job registration** — `packages/workflow-engine/src/executor.rs:358-369`. Job is registered in a separate spawned task. Immediate `job_status()` queries can return `None`.
- **Event depth tracking defeated** — `packages/workflow-engine/src/event_bus.rs:222-234`. `WorkflowEventEmitter` always creates events at depth 0, defeating loop prevention for cascading workflows.
- **Unbounded job registry memory leak** — `packages/workflow-engine/src/executor.rs:278`. The in-memory job map grows without bound; `cleanup_expired_jobs()` is never called automatically.
- **Per-record WASM instantiation in migrations** — `packages/workflow-engine/src/migration/runner.rs`. Each record creates a fresh Extism plugin instance. Migrations of thousands of records will be extremely slow.
- **Quarantine failure silently discarded** — `packages/workflow-engine/src/migration/engine.rs:168`. If quarantining fails, the record is neither migrated nor quarantined — silent data loss.

### GraphQL

- **No batch query limit** — Multiple operations per HTTP POST are unbounded.
- **N+1 query problem in nested resolvers** — `CalendarEvent.attendeeContacts` and `Email.attachmentFiles` fire separate queries per parent. No DataLoader.
- **No subscription connection limits** — Attackers can open unlimited WebSocket subscriptions to exhaust memory.
- **No collection validation on mutations** — `createRecord`, `updateRecord`, `deleteRecord` accept arbitrary collection strings with no allowlist.

### Connector Plugins

- **All `execute()` methods are no-op passthroughs** — The WASM plugin interface is non-functional across all four connectors.
- **No retry/backoff on contacts, calendar, filesystem connectors** — Only email implements `RetryState`. Sync failures can trigger rapid-fire retries.
- **S3 credential stored directly in struct** — `plugins/engine/connector-filesystem/src/s3.rs:27` breaks the credential store pattern used everywhere else.
- **S3 `list_objects` does not paginate** — Buckets with >1000 objects return silently incomplete results.

### DAV Protocol

- **Missing well-known redirects, OPTIONS handlers, current-user-principal** — Both CalDAV and CardDAV discovery flows are non-functional for real clients.
- **ETag generation has second-level granularity** — Rapid updates within the same second produce identical ETags, violating RFC 7232.
- **ADR serialization does not escape semicolons** — Address components with semicolons corrupt the vCard field structure.

### Core Application

- **In-memory federation and household state lost on restart** — `apps/core/src/federation.rs`, `household.rs`. All peer registrations, memberships, and sync cursors are volatile.
- **Storage passphrase not redacted in config JSON** — `apps/core/src/config.rs:940-960`. The passphrase could appear in `GET /api/system/config` responses.
- **Search has no multi-tenancy isolation** — `apps/core/src/search.rs:144-227`. Any authenticated user can search all records across all plugins and users.
- **Per-document Tantivy commits cause write amplification** — `apps/core/src/search.rs:90-111`. Every indexing call does a full commit and reader reload.

### Build System

- **`apps/core/Dockerfile` is stale and will not build** — Only copies 4 of 28 workspace members.
- **Config format mismatch** — Docker Compose mounts `config.toml` but `config.rs` only parses YAML.
- **Backup plugin pins `cron = "0.13"` vs workspace `"0.15"`** — Genuine version mismatch.

---

## Cross-Cutting Patterns

Several themes recur across multiple reports:

### Orphaned and Stub Code

Empty `steps/mod.rs`, `transform/mod.rs`, and `tests/mod.rs` modules appear in every plugin. Three orphaned files in the traits crate import types that do not exist. Transport crates for CalDAV, CardDAV, and webhooks are complete scaffolding with zero implementation. The search-indexer plugin is a hollow shell. This pattern suggests the architecture redesign outpaced implementation.

### Duplicate Implementations

The same logic is implemented multiple times in different locations: encryption (3 copies), rate limiting (2 copies), `Capability` enums (2 divergent versions), `PluginManifest` structs (2 copies), `SchemaError` types (2 copies), `CANONICAL_COLLECTIONS` (2 copies), `AuthProvider` traits (2 versions), `Identity` types (2 incompatible shapes), `FileChange` types (2 copies), sync state types (2 copies). These duplications create maintenance hazards and silent divergence.

### X-Forwarded-For Trust

Three separate locations trust the `X-Forwarded-For` header without checking the `behind_proxy` configuration flag. This is the most frequently recurring security issue across the codebase.

### Blocking I/O in Async Context

Synchronous `std::fs` operations inside `async fn` appear in blob storage (`packages/storage-sqlite/src/blob_fs.rs`), TLS cert loading (`packages/transport-rest/src/listener.rs`), backup local backend (`plugins/engine/backup/src/backend/local.rs`), and migration engine (`packages/workflow-engine/src/migration/engine.rs`). These block the Tokio executor thread.

### Missing Resource Limits

No request body size limits on REST or GraphQL endpoints. No GraphQL query depth or complexity limits. No blob size limit in the plugin system. No subscription connection limit. No decompression bomb protection in backup restore. Unbounded job registry in the workflow engine. No pagination on audit query results. These collectively create a broad DoS attack surface.

### `let_chains` / Rust Edition Dependency

Multiple files use `if let ... && let ...` syntax (`schema.rs`, `connector-calendar/lib.rs`, `executor.rs`, `ical.rs`), which requires either nightly Rust or the 2024 edition (Rust 1.85+). The workspace declares `edition = "2024"`, so this is likely intentional, but the MSRV should be documented.

### Config Never Loaded in Plugins

Several plugins define config structs with defaults and deserialization support, but `CorePlugin::on_load()` never reads the configuration from the `PluginContext`. This affects backup, webhook-sender, and search-indexer plugins.

---

## Strengths

- **Test infrastructure is exceptional.** Across 15+ crates, test coverage is thorough, assertions are specific (checking error codes, severities, and message content, not just `is_err()`), mocks faithfully implement full trait surfaces, and real WASM modules are used for integration tests. The `packages/test-utils` crate provides comprehensive factories, assertion macros, Docker helpers, and mock adapters.

- **Security mindset is strong at the foundation.** Algorithm choices are uniformly correct (AES-256-GCM, Argon2id, HMAC-SHA256 with constant-time comparison, HKDF-SHA256). Credential values are redacted in Debug/Serialize output across every module. Plugin storage is scoped by `plugin_id` with forced overwrite to prevent impersonation. SQLCipher PRAGMA key input is validated against hex injection.

- **Capability enforcement is well-designed.** The two-layer model (CAP_001 at load time, CAP_002 at runtime) with per-host-function checks and forced `plugin_id` scoping in storage operations provides defence-in-depth. The crash isolation tests verify that one plugin failure does not affect others.

- **The CDM type system is clean and consistent.** All 7 CDM types follow uniform serde patterns, have round-trip tests, and use consistent naming conventions. The `PipelineMessage` envelope cleanly separates metadata from typed payloads.

- **Configuration system is well-layered.** Three-layer priority (YAML file, environment variables, CLI args), runtime hot-reload via `merge_partial`, sensitive field redaction in Debug output, and comprehensive validation that blocks insecure configurations (e.g., `local-token` auth on non-localhost).

- **Plugin manifest validation is thorough.** Format checks, semver validation, reserved name blocking, `deny_unknown_fields`, cross-section consistency, extension naming conventions, and duplicate detection are all present.

- **Connector normalizers are robust.** The email, contacts, and calendar normalizers handle edge cases well: missing fields, malformed data, encoding issues, incremental sync state tracking, and fallback defaults.

- **Integration tests use real services.** GreenMail for IMAP/SMTP, Radicale for CalDAV/CardDAV, MinIO for S3, and wiremock for OIDC — all properly guarded with `skip_unless_docker!` macros.

---

## Prioritized Remediation Roadmap

### Tier 1: Fix Before Any Deployment (Security Critical)

1. **Fix cryptographic salts** — Replace zero/fixed salts with random per-use salts in `rekey.rs`, `backup/crypto.rs`, and `sqlite_storage.rs`. Store salts alongside their encrypted outputs. Gate the zero-salt `derive_key()` behind `#[cfg(test)]`.
2. **Fix SQL injection vectors** — Validate field names and collection names in `migration/executor.rs` to allow only `[a-zA-Z0-9_]`. Parameterize or validate the sort field in `backend.rs`.
3. **Fix X-Forwarded-For trust** — Check `config.network.behind_proxy` before parsing the header in all three locations. Use `ConnectInfo<SocketAddr>` when not behind a proxy.
4. **Add GraphQL security limits** — Call `.limit_depth(10)`, `.limit_complexity(1000)`, and `.disable_introspection()` (for production) on the schema builder. Propagate authenticated identity into the GraphQL context.
5. **Block SSRF in plugin HTTP host function** — Add private IP range blocking (RFC 1918, link-local, loopback, cloud metadata) as a baseline.
6. **Strip sensitive fields from `list_keys`** — Create an `ApiKeyMetadata` response type without `key_hash` and `salt`.
7. **Implement credential re-encryption after rekey** — Iterate all credentials, decrypt with old-derived keys, re-encrypt with new-derived keys, within a transaction.

### Tier 2: Fix Before Beta Testing (Functional Correctness)

8. **Unify Identity types in REST transport** — Remove the middleware-local `Identity` and convert to `life_engine_types::identity::Identity`.
9. **Fix auth middleware parameterized route bypass** — Match against the Axum `MatchedPath` or use `.route_layer()` for per-route auth exemption.
10. **Wire blob host functions into plugin injection** — Add blob builders to `injection.rs` and pass `BlobBackend` through `InjectionDeps`.
11. **Fix executor locking** — Use per-plugin locks or extract handles before WASM calls to allow parallel plugin execution.
12. **Fix workflow spawn() race condition** — Register the job inline before spawning the execution task.
13. **Propagate event depth through workflow execution** — Increment depth when emitting events from event-triggered workflows.
14. **Add `Propfind` and `Report` to `HttpMethod` enum** — Unblocks both CalDAV and CardDAV plugins.
15. **Fix UTF-8 line folding** — Use `char_indices()` instead of byte offsets in both DAV serializers.
16. **Add request body size limits** — Apply `DefaultBodyLimit` to REST and GraphQL routes.
17. **Consolidate encryption to `packages/crypto/`** — Remove duplicate implementations in `apps/core/src/crypto.rs` and `plugins/engine/backup/src/crypto.rs`.
18. **Add key zeroization** — Add `zeroize` to `packages/crypto/` and use `Zeroizing<[u8; 32]>` for derived keys.

### Tier 3: Fix Before General Availability

19. **Implement actual webhook HTTP delivery** — Wire `reqwest` POST with HMAC-SHA256 signing, exponential backoff, and configurable timeouts.
20. **Implement CalDAV/CardDAV protocol handlers** — At minimum: PROPFIND (depth 0/1), GET, PUT, OPTIONS with DAV headers, and well-known redirects.
21. **Add search multi-tenancy filtering** — Filter by `user_id`/`household_id` in search queries.
22. **Implement batched Tantivy commits** — Use the existing `commit_threshold` config to buffer documents instead of per-document commits.
23. **Persist federation and household state** — Add SQLite tables for peer registrations, sync cursors, households, and invites.
24. **Add persistent search index** — Use `Index::create_in_dir()` instead of `create_in_ram()`.
25. **Unify Capability enums** — Either extend `traits::Capability` with the SDK's extra variants or provide explicit conversion functions.
26. **Document the two plugin models** — Clearly guide developers on `CorePlugin` vs `Plugin` usage and the migration path.
27. **Fix build infrastructure** — Delete stale `apps/core/Dockerfile`, fix Docker Compose config format mismatch, align version pins, fix Pocket ID port mapping.
28. **Add retry/backoff to all connectors** — Extract the email connector's `RetryState` pattern into a shared utility.
29. **Add resource limits everywhere** — Blob size limits, subscription connection limits, decompression bomb protection, job registry cleanup, audit query pagination.
30. **Fix Backup `retention_days` mismatch** — Either implement age-based retention or remove the config option.

---

## Risk Assessment

### What could go wrong in production

- **Data breach via SQL injection** — A malicious plugin manifest with crafted field names could execute arbitrary SQL through the migration executor, potentially exfiltrating or destroying all stored data.
- **Rate limit bypass enables brute force** — X-Forwarded-For spoofing combined with O(n) API key validation creates a viable path for credential brute-forcing on directly-exposed instances.
- **GraphQL abuse causes service disruption** — Without depth, complexity, or batch limits, a single malicious query can consume all server resources.
- **Backup encryption is weaker than expected** — The fixed salt means users with identical passphrases share encryption keys. If one user's passphrase is compromised, precomputed tables can be applied to all users.
- **Master key rotation breaks credential access** — Any attempt to rotate the database encryption key will silently break all per-credential derived keys, making stored credentials permanently inaccessible without the old key.
- **SSRF from untrusted plugins** — A malicious community plugin can scan the internal network, access cloud metadata endpoints, or attack co-located services through the HTTP host function.
- **Restart data loss** — Federation peers, household memberships, search indexes, webhook delivery logs, and plugin state are all in-memory. A process restart or crash resets these to empty.
- **Silent data corruption in DAV sync** — The UTF-8 line folding bug will corrupt non-ASCII characters in contact names and event descriptions during CalDAV/CardDAV sync, with no error or warning.

### Mitigating factors

- The local-first architecture means most deployments will be single-user on localhost, reducing the attack surface for network-based issues.
- The `config.rs` validator blocks several insecure configurations (e.g., `local-token` on non-localhost).
- The test suite provides high confidence that targeted fixes will not introduce regressions.
- The modular crate architecture means fixes can be applied to individual packages without system-wide changes.

---

## Report Index

Detailed findings for each area are in the individual reports:

Phase 1 (Foundation):

- [Types and Data Models](phase-1/types-and-data-models.md)
- [Traits and Interfaces](phase-1/traits-and-interfaces.md)
- [Cryptography](phase-1/cryptography.md)
- [Storage SQLite](phase-1/storage-sqlite.md)
- [Authentication](phase-1/authentication.md)
- [Build and Config](phase-1/build-and-config.md)

Phase 2 (Infrastructure):

- [Plugin System](phase-2/plugin-system.md)
- [Plugin SDK](phase-2/plugin-sdk.md)
- [Transport REST](phase-2/transport-rest.md)
- [Transport GraphQL](phase-2/transport-graphql.md)
- [Workflow Engine](phase-2/workflow-engine.md)
- [DAV and Webhooks](phase-2/dav-and-webhooks.md)

Phase 3 (Application and Plugins):

- [Core Application](phase-3/core-application.md)
- [Connector Plugins](phase-3/connector-plugins.md)
- [API Plugins](phase-3/api-plugins.md)
- [Backup and Webhooks](phase-3/backup-and-webhooks.md)
- [Search Indexer](phase-3/search-indexer.md)
- [Test Infrastructure](phase-3/test-infrastructure.md)
