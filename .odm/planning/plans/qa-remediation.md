<!--
project: life-engine-core
specs: qa-executive-summary
updated: 2026-03-28
-->

# QA Remediation Plan

## Plan Overview

This plan addresses all findings from the Life Engine Core QA review (2026-03-28). The remediation roadmap is organized into three tiers matching the executive summary's prioritization: Tier 1 (security-critical, fix before any deployment), Tier 2 (functional correctness, fix before beta), and Tier 3 (fix before general availability). Work packages within each tier are ordered by dependency and impact.

**Source:** .odm/qa/reports/EXECUTIVE-SUMMARY.md

**Progress:** 15 / 30 work packages complete

---

## 1.1 — Fix cryptographic salts
> depends: none
> spec: .odm/qa/reports/phase-1/cryptography.md

- [x] Replace zero-salt `derive_key()` with random per-use salt in `rekey.rs` [security]
  <!-- file: apps/core/src/rekey.rs -->
  <!-- purpose: Eliminate all-zeros salt fallback that makes all same-passphrase databases share identical encryption keys -->
  <!-- requirements: C-1 from cryptography report -->
  <!-- leverage: packages/crypto/src/kdf.rs already uses OsRng correctly -->
- [x] Store generated salt alongside encrypted output in SQLite storage [security]
  <!-- file: apps/core/src/sqlite_storage.rs -->
  <!-- purpose: Persist the random salt so decryption can retrieve it -->
  <!-- requirements: C-1 from cryptography report -->
  <!-- leverage: none -->
- [x] Replace hardcoded `b"life-engine-salt"` with random per-backup salt in backup crypto [security]
  <!-- file: plugins/engine/backup/src/crypto.rs -->
  <!-- purpose: Each backup archive uses a unique salt stored in its header -->
  <!-- requirements: C-2 from cryptography report -->
  <!-- leverage: none -->
- [x] Replace `thread_rng()` with `OsRng` for salt generation in `rekey.rs` [security]
  <!-- file: apps/core/src/rekey.rs -->
  <!-- purpose: Use OS entropy source directly for security-critical material -->
  <!-- requirements: C-3 from cryptography report -->
  <!-- leverage: packages/crypto/src/kdf.rs already uses OsRng -->
- [x] Gate the zero-salt `derive_key()` behind `#[cfg(test)]` [security]
  <!-- file: apps/core/src/rekey.rs -->
  <!-- purpose: Prevent accidental production use of the zero-salt path -->
  <!-- requirements: C-1 from cryptography report -->
  <!-- leverage: none -->
- [x] Add tests verifying salts are unique across invocations [test]
  <!-- file: apps/core/src/rekey.rs -->
  <!-- purpose: Regression test for salt randomness -->
  <!-- requirements: C-1, C-2, C-3 from cryptography report -->
  <!-- leverage: existing test infrastructure -->

## 1.2 — Fix SQL injection vectors
> depends: none
> spec: .odm/qa/reports/phase-1/storage-sqlite.md

- [x] Validate field names in migration executor to allow only `[a-zA-Z0-9_]` [security]
  <!-- file: packages/storage-sqlite/src/migration/executor.rs -->
  <!-- purpose: Prevent plugin-provided field names from executing arbitrary SQL in CREATE INDEX statements -->
  <!-- requirements: Lines 77-83, 97-103 SQL injection finding -->
  <!-- leverage: none -->
- [x] Validate collection names in migration executor with the same allowlist [security]
  <!-- file: packages/storage-sqlite/src/migration/executor.rs -->
  <!-- purpose: Close injection vector in DDL statements using collection names -->
  <!-- requirements: Lines 77-83, 97-103 SQL injection finding -->
  <!-- leverage: none -->
- [x] Parameterize or validate the sort field in backend queries [security]
  <!-- file: packages/storage-sqlite/src/backend.rs -->
  <!-- purpose: Prevent ORDER BY clause injection via caller-provided field names -->
  <!-- requirements: Line 183 sort field injection finding -->
  <!-- leverage: none -->
- [x] Add tests with malicious field names to verify injection is blocked [test]
  <!-- file: packages/storage-sqlite/src/migration/executor.rs -->
  <!-- purpose: Regression tests for SQL injection prevention -->
  <!-- requirements: SQL injection findings -->
  <!-- leverage: existing test infrastructure -->

## 1.3 — Fix X-Forwarded-For trust
> depends: none
> spec: .odm/qa/reports/EXECUTIVE-SUMMARY.md

- [x] Check `config.network.behind_proxy` before parsing X-Forwarded-For in core auth middleware [security]
  <!-- file: apps/core/src/auth/middleware.rs -->
  <!-- purpose: Only trust X-Forwarded-For when explicitly configured as behind a proxy -->
  <!-- requirements: Lines 128-133 X-Forwarded-For finding -->
  <!-- leverage: behind_proxy config flag already exists -->
- [x] Check `config.network.behind_proxy` before parsing X-Forwarded-For in core rate limiter [security]
  <!-- file: apps/core/src/rate_limit.rs -->
  <!-- purpose: Prevent rate limit bypass via header spoofing -->
  <!-- requirements: Lines 113-118 X-Forwarded-For finding -->
  <!-- leverage: behind_proxy config flag already exists -->
- [x] Check `config.network.behind_proxy` before parsing X-Forwarded-For in REST auth middleware [security]
  <!-- file: packages/transport-rest/src/middleware/auth.rs -->
  <!-- purpose: Prevent rate limit bypass via header spoofing -->
  <!-- requirements: Lines 69-74 X-Forwarded-For finding -->
  <!-- leverage: behind_proxy config flag already exists -->
- [x] Use `ConnectInfo<SocketAddr>` as fallback when not behind proxy [security]
  <!-- file: apps/core/src/auth/middleware.rs -->
  <!-- purpose: Get real peer address from the TCP connection when not behind a proxy -->
  <!-- requirements: X-Forwarded-For finding -->
  <!-- leverage: Axum ConnectInfo extractor -->
- [x] Add tests for both proxy and direct-connection IP extraction [test]
  <!-- file: apps/core/src/auth/middleware.rs -->
  <!-- purpose: Verify correct IP is used in both configurations -->
  <!-- requirements: X-Forwarded-For finding -->
  <!-- leverage: existing test infrastructure -->

## 1.4 — Add GraphQL security limits
> depends: none
> spec: .odm/qa/reports/phase-2/transport-graphql.md

- [x] Add `.limit_depth(10)` to the GraphQL schema builder [security]
  <!-- file: apps/core/src/routes/graphql.rs -->
  <!-- purpose: Prevent arbitrarily nested queries that cause stack overflow or memory exhaustion -->
  <!-- requirements: No query depth limit finding -->
  <!-- leverage: async-graphql built-in limit_depth -->
- [x] Add `.limit_complexity(1000)` to the GraphQL schema builder [security]
  <!-- file: apps/core/src/routes/graphql.rs -->
  <!-- purpose: Prevent single queries from requesting excessive data -->
  <!-- requirements: No query complexity limit finding -->
  <!-- leverage: async-graphql built-in limit_complexity -->
- [x] Disable introspection in production with `.disable_introspection()` [security]
  <!-- file: apps/core/src/routes/graphql.rs -->
  <!-- purpose: Prevent unauthenticated clients from discovering the full schema in production -->
  <!-- requirements: Introspection enabled in production finding -->
  <!-- leverage: async-graphql built-in disable_introspection -->
- [x] Propagate authenticated identity into the async-graphql context [security]
  <!-- file: apps/core/src/routes/graphql.rs -->
  <!-- purpose: Enable authorization checks at the resolver level -->
  <!-- requirements: No authentication at resolver level finding -->
  <!-- leverage: async-graphql context data injection -->
- [x] Add tests for depth and complexity limit enforcement [test]
  <!-- file: apps/core/src/routes/graphql.rs -->
  <!-- purpose: Verify deeply nested and overly complex queries are rejected -->
  <!-- requirements: GraphQL security findings -->
  <!-- leverage: existing test infrastructure -->

## 1.5 — Block SSRF in plugin HTTP host function
> depends: none
> spec: .odm/qa/reports/phase-2/plugin-system.md

- [x] Add private IP range blocking (RFC 1918, link-local, loopback, cloud metadata) [security]
  <!-- file: packages/plugin-system/src/host_functions/http.rs -->
  <!-- purpose: Prevent plugins from probing internal services and cloud metadata endpoints -->
  <!-- requirements: SSRF finding when allowed_domains is None -->
  <!-- leverage: none -->
- [x] Block requests to 169.254.169.254 (cloud metadata endpoint) explicitly [security]
  <!-- file: packages/plugin-system/src/host_functions/http.rs -->
  <!-- purpose: Prevent cloud metadata credential theft -->
  <!-- requirements: SSRF finding -->
  <!-- leverage: none -->
- [x] Add tests verifying private IP ranges and metadata endpoints are blocked [test]
  <!-- file: packages/plugin-system/src/host_functions/http.rs -->
  <!-- purpose: Regression tests for SSRF prevention -->
  <!-- requirements: SSRF finding -->
  <!-- leverage: existing test infrastructure -->

## 1.6 — Strip sensitive fields from list_keys
> depends: none
> spec: .odm/qa/reports/phase-1/authentication.md

- [x] Create `ApiKeyMetadata` response type without `key_hash` and `salt` [security]
  <!-- file: packages/auth/src/handlers/keys.rs -->
  <!-- purpose: Prevent offline brute-force attacks by not exposing cryptographic material -->
  <!-- requirements: Lines 103-127 key hash exposure finding -->
  <!-- leverage: existing ApiKeyRecord type as basis -->
- [x] Update `list_keys` to return `ApiKeyMetadata` instead of full `ApiKeyRecord` [security]
  <!-- file: packages/auth/src/handlers/keys.rs -->
  <!-- purpose: Strip key_hash and salt from API responses -->
  <!-- requirements: Lines 103-127 key hash exposure finding -->
  <!-- leverage: none -->
- [x] Add test verifying list_keys response does not contain hash or salt [test]
  <!-- file: packages/auth/src/handlers/keys.rs -->
  <!-- purpose: Regression test for key material exposure -->
  <!-- requirements: Key material exposure finding -->
  <!-- leverage: existing test infrastructure -->

## 1.7 — Implement credential re-encryption after rekey
> depends: 1.1
> spec: .odm/qa/reports/phase-1/storage-sqlite.md

- [x] Add `re_encrypt_credentials()` function that iterates all credentials [security]
  <!-- file: packages/storage-sqlite/src/credentials.rs -->
  <!-- purpose: Decrypt with old-derived keys and re-encrypt with new-derived keys after master key rotation -->
  <!-- requirements: Credential re-encryption missing after rekey finding -->
  <!-- leverage: existing credential encryption/decryption functions -->
- [x] Call `re_encrypt_credentials()` within `rekey()` inside a transaction [security]
  <!-- file: packages/storage-sqlite/src/lib.rs -->
  <!-- purpose: Ensure all credentials are atomically re-encrypted during key rotation -->
  <!-- requirements: Credential re-encryption missing after rekey finding -->
  <!-- leverage: existing rekey workflow -->
- [x] Add test verifying credentials remain readable after a rekey operation [test]
  <!-- file: packages/storage-sqlite/src/credentials.rs -->
  <!-- purpose: Verify the re-encryption flow works end-to-end -->
  <!-- requirements: Credential re-encryption finding -->
  <!-- leverage: existing test infrastructure -->

## 2.1 — Unify Identity types in REST transport
> depends: none
> spec: .odm/qa/reports/phase-2/transport-rest.md

- [x] Remove middleware-local `Identity` struct from `middleware/auth.rs` [refactor]
  <!-- file: packages/transport-rest/src/middleware/auth.rs -->
  <!-- purpose: Eliminate duplicate Identity type that causes runtime extraction failure -->
  <!-- requirements: Dual Identity types finding, lines 19-24 -->
  <!-- leverage: life_engine_types::identity::Identity -->
- [x] Convert auth middleware to insert `life_engine_types::identity::Identity` into extensions [refactor]
  <!-- file: packages/transport-rest/src/middleware/auth.rs -->
  <!-- purpose: Ensure handlers can extract the correct Identity type from Axum extensions -->
  <!-- requirements: Dual Identity types finding -->
  <!-- leverage: life_engine_types::identity::Identity -->
- [x] Update handler extraction to match the unified Identity type [refactor]
  <!-- file: packages/transport-rest/src/handlers/mod.rs -->
  <!-- purpose: Ensure handlers extract the same type the middleware inserts -->
  <!-- requirements: Dual Identity types finding -->
  <!-- leverage: none -->
- [x] Add integration test verifying identity flows from middleware to handler [test]
  <!-- file: packages/transport-rest/src/middleware/auth.rs -->
  <!-- purpose: Verify identity extraction works end-to-end -->
  <!-- requirements: Dual Identity types finding -->
  <!-- leverage: existing test infrastructure -->

## 2.2 — Fix auth middleware parameterized route bypass
> depends: 2.1
> spec: .odm/qa/reports/phase-2/transport-rest.md

- [x] Replace concrete path matching with Axum `MatchedPath` or `.route_layer()` for public route exemption [bugfix]
  <!-- file: packages/transport-rest/src/middleware/auth.rs -->
  <!-- purpose: Fix public route bypass that fails for any parameterized route -->
  <!-- requirements: Lines 56-60 parameterized route bypass finding -->
  <!-- leverage: Axum MatchedPath extractor -->
- [x] Add tests for parameterized public routes like `/api/v1/data/:collection` [test]
  <!-- file: packages/transport-rest/src/middleware/auth.rs -->
  <!-- purpose: Verify parameterized routes can be marked as public -->
  <!-- requirements: Parameterized route bypass finding -->
  <!-- leverage: existing test infrastructure -->

## 2.3 — Wire blob host functions into plugin injection
> depends: none
> spec: .odm/qa/reports/phase-2/plugin-system.md

- [x] Implement `build_blob_store_function()`, `build_blob_retrieve_function()`, `build_blob_delete_function()` builders [feature]
  <!-- file: packages/plugin-system/src/injection.rs -->
  <!-- purpose: Create the blob host function builders that injection.rs is missing -->
  <!-- requirements: Blob host functions not injected finding -->
  <!-- leverage: existing host function builder pattern in injection.rs -->
- [x] Add blob builders to `build_host_functions()` for StorageBlobRead/Write/Delete capabilities [feature]
  <!-- file: packages/plugin-system/src/injection.rs -->
  <!-- purpose: Wire blob host functions so plugins with blob capabilities can actually use them -->
  <!-- requirements: Blob host functions not injected finding -->
  <!-- leverage: existing capability-to-function mapping pattern -->
- [x] Pass `BlobBackend` through `InjectionDeps` [feature]
  <!-- file: packages/plugin-system/src/injection.rs -->
  <!-- purpose: Provide blob storage backend to host function implementations -->
  <!-- requirements: Blob host functions not injected finding -->
  <!-- leverage: existing InjectionDeps pattern -->
- [x] Add integration test verifying blob operations work through plugin system [test]
  <!-- file: packages/plugin-system/src/injection.rs -->
  <!-- purpose: End-to-end test for blob host functions -->
  <!-- requirements: Blob host functions finding -->
  <!-- leverage: existing WASM test fixtures -->

## 2.4 — Fix executor locking for parallel plugin execution
> depends: none
> spec: .odm/qa/reports/phase-2/plugin-system.md

- [x] Replace system-wide `std::sync::Mutex` with per-plugin locks or extract handles before WASM calls [performance]
  <!-- file: packages/plugin-system/src/execute.rs -->
  <!-- purpose: Allow multiple plugins to execute concurrently instead of serializing all execution -->
  <!-- requirements: Executor serializes all plugin execution finding, line 79 -->
  <!-- leverage: none -->
- [x] Replace `std::sync::Mutex` with `tokio::sync::Mutex` or handle poison explicitly [bugfix]
  <!-- file: packages/plugin-system/src/execute.rs -->
  <!-- purpose: Prevent a single panic from permanently disabling the executor -->
  <!-- requirements: Mutex poison in async executor finding -->
  <!-- leverage: tokio::sync::Mutex -->
- [x] Add test verifying concurrent plugin execution is possible [test]
  <!-- file: packages/plugin-system/src/execute.rs -->
  <!-- purpose: Verify the locking fix allows parallel execution -->
  <!-- requirements: Executor locking finding -->
  <!-- leverage: existing test infrastructure -->

## 2.5 — Fix workflow spawn() race condition
> depends: none
> spec: .odm/qa/reports/phase-2/workflow-engine.md

- [x] Register the job inline before spawning the execution task [bugfix]
  <!-- file: packages/workflow-engine/src/executor.rs -->
  <!-- purpose: Ensure job_status() returns valid state immediately after spawn() -->
  <!-- requirements: Lines 358-369 race condition finding -->
  <!-- leverage: none -->
- [x] Add test verifying job_status() returns valid state immediately after spawn() [test]
  <!-- file: packages/workflow-engine/src/executor.rs -->
  <!-- purpose: Regression test for the spawn race condition -->
  <!-- requirements: Race condition finding -->
  <!-- leverage: existing test infrastructure -->

## 2.6 — Propagate event depth through workflow execution
> depends: none
> spec: .odm/qa/reports/phase-2/workflow-engine.md

- [x] Fix `WorkflowEventEmitter` to increment depth when emitting events from event-triggered workflows [bugfix]
  <!-- file: packages/workflow-engine/src/event_bus.rs -->
  <!-- purpose: Restore loop prevention for cascading workflows -->
  <!-- requirements: Lines 222-234 event depth tracking defeated finding -->
  <!-- leverage: existing depth field on events -->
- [x] Add test verifying depth increments on cascading workflow events [test]
  <!-- file: packages/workflow-engine/src/event_bus.rs -->
  <!-- purpose: Regression test for event depth tracking -->
  <!-- requirements: Event depth tracking finding -->
  <!-- leverage: existing test infrastructure -->

## 2.7 — Add PROPFIND and REPORT to HttpMethod enum
> depends: none
> spec: .odm/qa/reports/phase-3/api-plugins.md

- [x] Add `Propfind` and `Report` variants to `HttpMethod` enum [feature]
  <!-- file: packages/plugin-sdk-rs/src/types.rs -->
  <!-- purpose: Unblock CalDAV and CardDAV plugins that require these essential WebDAV methods -->
  <!-- requirements: Missing HTTP methods finding -->
  <!-- leverage: existing HttpMethod enum -->
- [x] Add serialization and deserialization support for the new variants [feature]
  <!-- file: packages/plugin-sdk-rs/src/types.rs -->
  <!-- purpose: Ensure new methods work through the WASM boundary -->
  <!-- requirements: Missing HTTP methods finding -->
  <!-- leverage: existing serde patterns on HttpMethod -->
- [x] Add tests for the new HttpMethod variants [test]
  <!-- file: packages/plugin-sdk-rs/src/types.rs -->
  <!-- purpose: Verify round-trip serialization of new methods -->
  <!-- requirements: Missing HTTP methods finding -->
  <!-- leverage: existing test patterns -->

## 2.8 — Fix UTF-8 line folding in DAV serializers
> depends: none
> spec: .odm/qa/reports/phase-3/api-plugins.md

- [x] Replace byte-offset folding with `char_indices()` in CalDAV serializer [bugfix]
  <!-- file: plugins/engine/api-caldav/src/serializer.rs -->
  <!-- purpose: Prevent multi-byte UTF-8 characters from being split and corrupted at fold boundaries -->
  <!-- requirements: UTF-8 line folding corruption finding -->
  <!-- leverage: none -->
- [x] Replace byte-offset folding with `char_indices()` in CardDAV serializer [bugfix]
  <!-- file: plugins/engine/api-carddav/src/serializer.rs -->
  <!-- purpose: Prevent multi-byte UTF-8 characters from being split and corrupted at fold boundaries -->
  <!-- requirements: UTF-8 line folding corruption finding -->
  <!-- leverage: none -->
- [x] Add tests with multi-byte characters (CJK, emoji, accented) at fold boundaries [test]
  <!-- file: plugins/engine/api-caldav/src/serializer.rs -->
  <!-- purpose: Verify non-ASCII text survives folding without corruption -->
  <!-- requirements: UTF-8 line folding finding -->
  <!-- leverage: existing test infrastructure -->

## 2.9 — Add request body size limits
> depends: none
> spec: .odm/qa/reports/phase-2/transport-rest.md

- [ ] Apply `DefaultBodyLimit` to REST routes [security]
  <!-- file: packages/transport-rest/src/router/mod.rs -->
  <!-- purpose: Prevent unbounded request bodies from causing memory exhaustion -->
  <!-- requirements: No request body size limits finding -->
  <!-- leverage: tower-http DefaultBodyLimit -->
- [ ] Apply body size limit to GraphQL endpoint [security]
  <!-- file: apps/core/src/routes/graphql.rs -->
  <!-- purpose: Prevent oversized GraphQL queries from consuming excessive memory -->
  <!-- requirements: No request body size limits finding -->
  <!-- leverage: tower-http DefaultBodyLimit -->
- [ ] Add test verifying oversized requests are rejected [test]
  <!-- file: packages/transport-rest/src/router/mod.rs -->
  <!-- purpose: Regression test for body size limits -->
  <!-- requirements: Body size limits finding -->
  <!-- leverage: existing test infrastructure -->

## 2.10 — Consolidate encryption to packages/crypto
> depends: none
> spec: .odm/qa/reports/phase-1/cryptography.md

- [ ] Migrate `apps/core/src/crypto.rs` callers to use `packages/crypto/` functions [refactor]
  <!-- file: apps/core/src/crypto.rs -->
  <!-- purpose: Remove duplicate AES-256-GCM implementation in favor of the centralized crate -->
  <!-- requirements: M-5 duplicated encryption implementations finding -->
  <!-- leverage: packages/crypto/ already has the canonical implementation -->
- [ ] Migrate `plugins/engine/backup/src/crypto.rs` callers to use `packages/crypto/` functions [refactor]
  <!-- file: plugins/engine/backup/src/crypto.rs -->
  <!-- purpose: Remove duplicate AES-256-GCM implementation in the backup plugin -->
  <!-- requirements: M-5 duplicated encryption implementations finding -->
  <!-- leverage: packages/crypto/ already has the canonical implementation -->
- [ ] Delete or deprecate the duplicate implementations [refactor]
  <!-- file: apps/core/src/crypto.rs -->
  <!-- purpose: Eliminate maintenance hazard of three divergent copies -->
  <!-- requirements: M-5 finding -->
  <!-- leverage: none -->
- [ ] Fix HMAC constant-time verification gap in core crypto [security]
  <!-- file: apps/core/src/crypto.rs -->
  <!-- purpose: Prevent timing attacks on HMAC verification -->
  <!-- requirements: M-6 HMAC constant-time verification finding -->
  <!-- leverage: packages/crypto/ hmac_verify already does this correctly -->
- [ ] Fix panic-on-invalid-key-length in core crypto [bugfix]
  <!-- file: apps/core/src/crypto.rs -->
  <!-- purpose: Return Result instead of panicking on wrong key length -->
  <!-- requirements: M-4 panic on invalid key length finding -->
  <!-- leverage: packages/crypto/ already handles this correctly -->

## 2.11 — Add key zeroization to packages/crypto
> depends: none
> spec: .odm/qa/reports/phase-1/cryptography.md

- [ ] Add `zeroize` dependency to `packages/crypto/Cargo.toml` [security]
  <!-- file: packages/crypto/Cargo.toml -->
  <!-- purpose: Enable memory zeroization for key material -->
  <!-- requirements: M-1 no key zeroization finding -->
  <!-- leverage: none -->
- [ ] Use `Zeroizing<[u8; 32]>` for derived keys in `kdf.rs` [security]
  <!-- file: packages/crypto/src/kdf.rs -->
  <!-- purpose: Ensure derived keys are cleared from memory when dropped -->
  <!-- requirements: M-1 no key zeroization finding -->
  <!-- leverage: zeroize crate Zeroizing wrapper -->
- [ ] Update callers to handle `Zeroizing` return type [security]
  <!-- file: packages/crypto/src/encryption.rs -->
  <!-- purpose: Propagate zeroization through the encryption API -->
  <!-- requirements: M-1, M-2 zeroization findings -->
  <!-- leverage: none -->

## 3.1 — Implement webhook HTTP delivery
> depends: none
> spec: .odm/qa/reports/phase-2/dav-and-webhooks.md

- [ ] Implement `reqwest` POST dispatch in webhook sender `handle_event` [feature]
  <!-- file: plugins/engine/webhook-sender/src/lib.rs -->
  <!-- purpose: Actually send HTTP requests instead of just logging matches -->
  <!-- requirements: Lines 261-271 webhook sender does not send finding -->
  <!-- leverage: reqwest is already a dependency -->
- [ ] Add HMAC-SHA256 request signing for webhook payloads [feature]
  <!-- file: plugins/engine/webhook-sender/src/lib.rs -->
  <!-- purpose: Allow receivers to verify webhook authenticity -->
  <!-- requirements: Webhook delivery finding -->
  <!-- leverage: packages/crypto hmac_sign -->
- [ ] Add exponential backoff and configurable timeouts for delivery [feature]
  <!-- file: plugins/engine/webhook-sender/src/lib.rs -->
  <!-- purpose: Handle transient failures gracefully -->
  <!-- requirements: Webhook delivery finding -->
  <!-- leverage: none -->
- [ ] Add tests for webhook delivery including retry behavior [test]
  <!-- file: plugins/engine/webhook-sender/src/lib.rs -->
  <!-- purpose: Verify end-to-end webhook delivery flow -->
  <!-- requirements: Webhook delivery finding -->
  <!-- leverage: wiremock for HTTP mocking -->

## 3.2 — Implement CalDAV/CardDAV protocol handlers
> depends: 2.7
> spec: .odm/qa/reports/phase-2/dav-and-webhooks.md

- [ ] Implement PROPFIND handler (depth 0 and 1) for CalDAV transport [feature]
  <!-- file: packages/transport-caldav/ -->
  <!-- purpose: Enable calendar discovery and collection listing -->
  <!-- requirements: Transport stubs finding, zero protocol logic -->
  <!-- leverage: none -->
- [ ] Implement PROPFIND handler (depth 0 and 1) for CardDAV transport [feature]
  <!-- file: packages/transport-carddav/ -->
  <!-- purpose: Enable contact discovery and collection listing -->
  <!-- requirements: Transport stubs finding, zero protocol logic -->
  <!-- leverage: CalDAV PROPFIND implementation as reference -->
- [ ] Implement GET and PUT handlers for both transports [feature]
  <!-- file: packages/transport-caldav/ -->
  <!-- purpose: Enable reading and writing individual resources -->
  <!-- requirements: Transport stubs finding -->
  <!-- leverage: none -->
- [ ] Implement OPTIONS handler with DAV headers for both transports [feature]
  <!-- file: packages/transport-caldav/ -->
  <!-- purpose: Advertise DAV compliance level to clients -->
  <!-- requirements: Missing OPTIONS handler finding -->
  <!-- leverage: none -->
- [ ] Add well-known URI redirects (/.well-known/caldav, /.well-known/carddav) [feature]
  <!-- file: packages/transport-caldav/ -->
  <!-- purpose: Enable standard DAV client auto-discovery -->
  <!-- requirements: Missing well-known redirects finding -->
  <!-- leverage: none -->

## 3.3 — Add search multi-tenancy filtering
> depends: none
> spec: .odm/qa/reports/phase-3/core-application.md

- [ ] Filter search queries by `user_id` and `household_id` [security]
  <!-- file: apps/core/src/search.rs -->
  <!-- purpose: Prevent authenticated users from searching records belonging to other users -->
  <!-- requirements: Lines 144-227 no multi-tenancy isolation finding -->
  <!-- leverage: none -->
- [ ] Add user/household fields to Tantivy index schema [feature]
  <!-- file: apps/core/src/search.rs -->
  <!-- purpose: Enable efficient filtering by ownership in the search index -->
  <!-- requirements: Search multi-tenancy finding -->
  <!-- leverage: existing Tantivy schema setup -->
- [ ] Add tests verifying cross-user search isolation [test]
  <!-- file: apps/core/src/search.rs -->
  <!-- purpose: Regression test for multi-tenancy enforcement -->
  <!-- requirements: Search multi-tenancy finding -->
  <!-- leverage: existing test infrastructure -->

## 3.4 — Implement batched Tantivy commits
> depends: none
> spec: .odm/qa/reports/phase-3/core-application.md

- [ ] Buffer documents and use `commit_threshold` config for batch commits [performance]
  <!-- file: apps/core/src/search.rs -->
  <!-- purpose: Eliminate per-document commits that cause write amplification -->
  <!-- requirements: Lines 90-111 per-document commits finding -->
  <!-- leverage: existing commit_threshold config field -->
- [ ] Add periodic commit flush for buffered documents [performance]
  <!-- file: apps/core/src/search.rs -->
  <!-- purpose: Ensure buffered documents are committed within a reasonable time window -->
  <!-- requirements: Per-document commits finding -->
  <!-- leverage: none -->

## 3.5 — Persist federation and household state
> depends: none
> spec: .odm/qa/reports/phase-3/core-application.md

- [ ] Add SQLite tables for peer registrations and sync cursors [feature]
  <!-- file: apps/core/src/federation.rs -->
  <!-- purpose: Survive process restarts without losing federation peer state -->
  <!-- requirements: In-memory state lost on restart finding -->
  <!-- leverage: existing SQLite storage patterns -->
- [ ] Add SQLite tables for households and invites [feature]
  <!-- file: apps/core/src/household.rs -->
  <!-- purpose: Survive process restarts without losing household membership state -->
  <!-- requirements: In-memory state lost on restart finding -->
  <!-- leverage: existing SQLite storage patterns -->
- [ ] Migrate federation and household modules to load from and persist to SQLite [feature]
  <!-- file: apps/core/src/federation.rs -->
  <!-- purpose: Replace volatile in-memory state with durable storage -->
  <!-- requirements: In-memory state lost on restart finding -->
  <!-- leverage: packages/storage-sqlite -->

## 3.6 — Add persistent search index
> depends: none
> spec: .odm/qa/reports/phase-3/search-indexer.md

- [ ] Replace `Index::create_in_ram()` with `Index::create_in_dir()` [feature]
  <!-- file: apps/core/src/search.rs -->
  <!-- purpose: Persist search index to disk so it survives restarts -->
  <!-- requirements: Volatile in-memory index finding -->
  <!-- leverage: Tantivy create_in_dir API -->
- [ ] Add configurable index directory path [feature]
  <!-- file: apps/core/src/search.rs -->
  <!-- purpose: Allow users to configure where the search index is stored -->
  <!-- requirements: Volatile in-memory index finding -->
  <!-- leverage: existing config system -->

## 3.7 — Unify Capability enums
> depends: none
> spec: .odm/qa/reports/phase-2/plugin-sdk.md

- [ ] Reconcile divergent `Capability` enums between traits crate and SDK [refactor]
  <!-- file: packages/plugin-sdk-rs/src/types.rs -->
  <!-- purpose: Eliminate silent divergence between two Capability enum definitions -->
  <!-- requirements: Divergent Capability enums finding -->
  <!-- leverage: none -->
- [ ] Add explicit conversion functions or extend `traits::Capability` with SDK variants [refactor]
  <!-- file: packages/traits/src/lib.rs -->
  <!-- purpose: Provide a single source of truth for capability definitions -->
  <!-- requirements: Divergent Capability enums finding -->
  <!-- leverage: none -->

## 3.8 — Document the two plugin models
> depends: none
> spec: .odm/qa/reports/phase-2/plugin-sdk.md

- [ ] Document `CorePlugin` vs `Plugin` usage and migration path [docs]
  <!-- file: packages/plugin-sdk-rs/src/lib.rs -->
  <!-- purpose: Guide developers on which plugin model to use and when -->
  <!-- requirements: Two undocumented plugin models finding -->
  <!-- leverage: none -->

## 3.9 — Fix build infrastructure
> depends: none
> spec: .odm/qa/reports/phase-1/build-and-config.md

- [ ] Delete stale `apps/core/Dockerfile` that only copies 4 of 28 workspace members [cleanup]
  <!-- file: apps/core/Dockerfile -->
  <!-- purpose: Remove broken Dockerfile that cannot build successfully -->
  <!-- requirements: Stale Dockerfile finding -->
  <!-- leverage: none -->
- [ ] Fix Docker Compose config format mismatch (TOML mount vs YAML parser) [bugfix]
  <!-- file: docker-compose.yml -->
  <!-- purpose: Align mounted config file format with what config.rs actually parses -->
  <!-- requirements: Config format mismatch finding -->
  <!-- leverage: none -->
- [ ] Align `cron` version pin in backup plugin with workspace version [bugfix]
  <!-- file: plugins/engine/backup/Cargo.toml -->
  <!-- purpose: Fix genuine version mismatch between backup plugin (0.13) and workspace (0.15) -->
  <!-- requirements: Version pinning inconsistency finding -->
  <!-- leverage: none -->

## 3.10 — Add retry/backoff to all connectors
> depends: none
> spec: .odm/qa/reports/phase-3/connector-plugins.md

- [ ] Extract email connector's `RetryState` pattern into a shared utility [refactor]
  <!-- file: plugins/engine/connector-email/src/lib.rs -->
  <!-- purpose: Create reusable retry/backoff logic for all connectors -->
  <!-- requirements: No retry/backoff on most connectors finding -->
  <!-- leverage: existing RetryState in email connector -->
- [ ] Add retry/backoff to contacts connector [feature]
  <!-- file: plugins/engine/connector-contacts/src/lib.rs -->
  <!-- purpose: Handle transient sync failures gracefully -->
  <!-- requirements: No retry/backoff finding -->
  <!-- leverage: shared RetryState utility -->
- [ ] Add retry/backoff to calendar connector [feature]
  <!-- file: plugins/engine/connector-calendar/src/lib.rs -->
  <!-- purpose: Handle transient sync failures gracefully -->
  <!-- requirements: No retry/backoff finding -->
  <!-- leverage: shared RetryState utility -->
- [ ] Add retry/backoff to filesystem connector [feature]
  <!-- file: plugins/engine/connector-filesystem/src/lib.rs -->
  <!-- purpose: Handle transient sync failures gracefully -->
  <!-- requirements: No retry/backoff finding -->
  <!-- leverage: shared RetryState utility -->

## 3.11 — Add resource limits everywhere
> depends: none
> spec: .odm/qa/reports/EXECUTIVE-SUMMARY.md

- [ ] Add blob size limit enforcement in plugin blob host functions [security]
  <!-- file: packages/plugin-system/src/host_functions/blob.rs -->
  <!-- purpose: Prevent plugins from storing arbitrarily large blobs -->
  <!-- requirements: No blob size limit finding -->
  <!-- leverage: none -->
- [ ] Add blob key path traversal prevention [security]
  <!-- file: packages/plugin-system/src/host_functions/blob.rs -->
  <!-- purpose: Prevent ../ sequences from escaping plugin namespace -->
  <!-- requirements: Blob key path traversal finding, line 98 -->
  <!-- leverage: none -->
- [ ] Add WebSocket subscription connection limits for GraphQL [security]
  <!-- file: apps/core/src/routes/graphql.rs -->
  <!-- purpose: Prevent attackers from exhausting memory with unlimited subscriptions -->
  <!-- requirements: No subscription connection limits finding -->
  <!-- leverage: none -->
- [ ] Add decompression bomb protection in backup restore [security]
  <!-- file: plugins/engine/backup/src/lib.rs -->
  <!-- purpose: Prevent malicious backup archives from consuming excessive resources -->
  <!-- requirements: Missing resource limits finding -->
  <!-- leverage: none -->
- [ ] Add automatic job registry cleanup in workflow engine [bugfix]
  <!-- file: packages/workflow-engine/src/executor.rs -->
  <!-- purpose: Prevent unbounded memory growth from completed jobs -->
  <!-- requirements: Unbounded job registry finding, line 278 -->
  <!-- leverage: existing cleanup_expired_jobs function -->
- [ ] Add pagination to audit query results [feature]
  <!-- file: packages/storage-sqlite/src/audit.rs -->
  <!-- purpose: Prevent unbounded result sets from audit queries -->
  <!-- requirements: No pagination on audit query finding -->
  <!-- leverage: none -->
- [ ] Add batch query limit for GraphQL [security]
  <!-- file: apps/core/src/routes/graphql.rs -->
  <!-- purpose: Prevent multiple unbounded operations per HTTP POST -->
  <!-- requirements: No batch query limit finding -->
  <!-- leverage: none -->
- [ ] Check HTTP response body size before full download in plugin HTTP host function [security]
  <!-- file: packages/plugin-system/src/host_functions/http.rs -->
  <!-- purpose: Prevent OOM from malicious servers before the 10 MB limit is checked -->
  <!-- requirements: HTTP response body downloaded fully before size check finding, lines 193-200 -->
  <!-- leverage: none -->
- [ ] Restrict WASI access based on plugin trust level [security]
  <!-- file: packages/plugin-system/src/runtime.rs -->
  <!-- purpose: Prevent third-party plugins from getting filesystem, environment, and clock access -->
  <!-- requirements: WASI enabled unconditionally finding, line 182 -->
  <!-- leverage: existing trust level concept -->

## 3.12 — Fix backup retention_days mismatch
> depends: none
> spec: .odm/qa/reports/phase-3/backup-and-webhooks.md

- [ ] Implement age-based retention or remove the `retention_days` config option [bugfix]
  <!-- file: plugins/engine/backup/src/lib.rs -->
  <!-- purpose: Align implementation with configuration surface -->
  <!-- requirements: retention_days mismatch finding -->
  <!-- leverage: none -->
- [ ] Fix unencrypted backup manifests that leak metadata [security]
  <!-- file: plugins/engine/backup/src/lib.rs -->
  <!-- purpose: Encrypt manifests to prevent metadata leakage -->
  <!-- requirements: Unencrypted manifests finding -->
  <!-- leverage: packages/crypto encryption functions -->

## 3.13 — Fix S3 connector issues
> depends: none
> spec: .odm/qa/reports/phase-3/connector-plugins.md

- [ ] Move S3 credentials to the credential store instead of direct struct storage [security]
  <!-- file: plugins/engine/connector-filesystem/src/s3.rs -->
  <!-- purpose: Follow the credential store pattern used everywhere else -->
  <!-- requirements: S3 credential stored directly finding, line 27 -->
  <!-- leverage: existing credential store pattern -->
- [ ] Implement pagination for `list_objects` to handle buckets with more than 1000 objects [bugfix]
  <!-- file: plugins/engine/connector-filesystem/src/s3.rs -->
  <!-- purpose: Prevent silently incomplete results from large buckets -->
  <!-- requirements: S3 list_objects does not paginate finding -->
  <!-- leverage: S3 API continuation tokens -->
