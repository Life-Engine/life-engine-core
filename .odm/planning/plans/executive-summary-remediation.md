<!--
project: life-engine-core
specs: executive-summary-qa
updated: 2026-03-28
-->

# QA Remediation Plan

## Plan Overview

This plan addresses all findings from the QA executive summary report across three tiers of priority. Tier 1 covers security-critical issues that must be resolved before any deployment. Tier 2 covers functional correctness issues required before beta testing. Tier 3 covers improvements needed before general availability.

Work packages are organized by domain area within each tier. Tier 2 WPs depend on their related Tier 1 WPs being complete first (where applicable). Tier 3 WPs depend on related Tier 2 WPs.

**Progress:** 8 / 13 work packages complete

---

## 1.1 — Fix Cryptographic Salt Failures
> depends: none
> spec: .odm/qa/reports/EXECUTIVE-SUMMARY.md

- [x] Replace zero-salt fallback in `derive_key()` with random per-use salt generation [security-critical]
  <!-- file: apps/core/src/rekey.rs -->
  <!-- purpose: Eliminate all-zeros salt in production SQLCipher key derivation -->
  <!-- requirements: Tier 1, item 1 -->
  <!-- leverage: existing derive_key function at rekey.rs:88-93 -->
- [x] Replace hardcoded fixed salt in backup crypto with random per-backup salt [security-critical]
  <!-- file: plugins/engine/backup/src/crypto.rs -->
  <!-- purpose: Eliminate fixed b"life-engine-salt" constant -->
  <!-- requirements: Tier 1, item 1 -->
  <!-- leverage: existing crypto.rs:17 -->
- [x] Store generated salts alongside their encrypted outputs in both locations [security-critical]
  <!-- file: apps/core/src/rekey.rs -->
  <!-- purpose: Enable decryption with per-use random salts -->
  <!-- requirements: Tier 1, item 1 -->
  <!-- leverage: none -->
- [x] Switch salt generation from `thread_rng()` to `OsRng` [security-critical]
  <!-- file: apps/core/src/rekey.rs -->
  <!-- purpose: Use OS entropy source for security-critical material -->
  <!-- requirements: Tier 1, item 1 -->
  <!-- leverage: existing rekey.rs:31 -->
- [x] Gate the zero-salt `derive_key()` path behind `#[cfg(test)]` [security-critical]
  <!-- file: apps/core/src/rekey.rs -->
  <!-- purpose: Prevent test-only fallback from being used in production -->
  <!-- requirements: Tier 1, item 1 -->
  <!-- leverage: none -->

## 1.2 — Fix SQL Injection Vectors
> depends: none
> spec: .odm/qa/reports/EXECUTIVE-SUMMARY.md

- [x] Validate field names in migration executor to allow only `[a-zA-Z0-9_]` [security-critical]
  <!-- file: packages/storage-sqlite/src/migration/executor.rs -->
  <!-- purpose: Prevent arbitrary SQL execution via plugin manifest field names -->
  <!-- requirements: Tier 1, item 2 -->
  <!-- leverage: existing executor.rs:77-83, 97-103 -->
- [x] Parameterize or validate the sort field in backend queries [security-critical]
  <!-- file: packages/storage-sqlite/src/backend.rs -->
  <!-- purpose: Prevent SQL injection via ORDER BY clause -->
  <!-- requirements: Tier 1, item 2 -->
  <!-- leverage: existing backend.rs:183 -->

## 1.3 — Fix Network Security (X-Forwarded-For, SSRF, Rate Limiting)
> depends: none
> spec: .odm/qa/reports/EXECUTIVE-SUMMARY.md

- [x] Check `config.network.behind_proxy` before parsing X-Forwarded-For in auth middleware [security-critical]
  <!-- file: apps/core/src/auth/middleware.rs -->
  <!-- purpose: Prevent IP spoofing to bypass rate limiting -->
  <!-- requirements: Tier 1, item 3 -->
  <!-- leverage: existing middleware.rs:128-133 -->
- [x] Check `config.network.behind_proxy` before parsing X-Forwarded-For in rate limiter [security-critical]
  <!-- file: apps/core/src/rate_limit.rs -->
  <!-- purpose: Prevent IP spoofing to bypass rate limiting -->
  <!-- requirements: Tier 1, item 3 -->
  <!-- leverage: existing rate_limit.rs:113-118 -->
- [x] Check `config.network.behind_proxy` before parsing X-Forwarded-For in REST transport [security-critical]
  <!-- file: packages/transport-rest/src/middleware/auth.rs -->
  <!-- purpose: Prevent IP spoofing to bypass rate limiting -->
  <!-- requirements: Tier 1, item 3 -->
  <!-- leverage: existing auth.rs:69-74 -->
- [x] Use `ConnectInfo<SocketAddr>` when not behind a proxy in all three locations [security-critical]
  <!-- file: apps/core/src/auth/middleware.rs -->
  <!-- purpose: Use actual client IP when proxy header is not trusted -->
  <!-- requirements: Tier 1, item 3 -->
  <!-- leverage: none -->
- [x] Add private IP range blocking (RFC 1918, link-local, loopback, cloud metadata) to HTTP host function [security-critical]
  <!-- file: packages/plugin-system/src/host_functions/http.rs -->
  <!-- purpose: Prevent SSRF when allowed_domains is None -->
  <!-- requirements: Tier 1, item 5 -->
  <!-- leverage: existing http.rs host function -->

## 1.4 — Fix GraphQL Security
> depends: none
> spec: .odm/qa/reports/EXECUTIVE-SUMMARY.md

- [x] Add `.limit_depth(10)` to the GraphQL schema builder [security-critical]
  <!-- file: apps/core/src/routes/graphql.rs -->
  <!-- purpose: Prevent stack overflow via deeply nested queries -->
  <!-- requirements: Tier 1, item 4 -->
  <!-- leverage: existing graphql.rs:1366 -->
- [x] Add `.limit_complexity(1000)` to the GraphQL schema builder [security-critical]
  <!-- file: apps/core/src/routes/graphql.rs -->
  <!-- purpose: Prevent resource exhaustion via complex queries -->
  <!-- requirements: Tier 1, item 4 -->
  <!-- leverage: existing graphql.rs schema builder -->
- [x] Add `.disable_introspection()` for production builds [security-critical]
  <!-- file: apps/core/src/routes/graphql.rs -->
  <!-- purpose: Prevent schema exposure to unauthenticated clients -->
  <!-- requirements: Tier 1, item 4 -->
  <!-- leverage: none -->
- [x] Propagate authenticated identity into the async-graphql context [security-critical]
  <!-- file: apps/core/src/routes/graphql.rs -->
  <!-- purpose: Enable authorization checks at resolver level -->
  <!-- requirements: Tier 1, item 4 -->
  <!-- leverage: none -->

## 1.5 — Fix API Key Material Exposure and Credential Re-encryption
> depends: 1.1
> spec: .odm/qa/reports/EXECUTIVE-SUMMARY.md

- [x] Create `ApiKeyMetadata` response type without `key_hash` and `salt` fields [security-critical]
  <!-- file: packages/auth/src/handlers/keys.rs -->
  <!-- purpose: Prevent offline brute-force attacks via exposed key material -->
  <!-- requirements: Tier 1, item 6 -->
  <!-- leverage: existing keys.rs:103-127 -->
- [x] Return `ApiKeyMetadata` instead of full `ApiKeyRecord` from `list_keys` [security-critical]
  <!-- file: packages/auth/src/handlers/keys.rs -->
  <!-- purpose: Strip sensitive fields from API response -->
  <!-- requirements: Tier 1, item 6 -->
  <!-- leverage: existing keys.rs:103-127 -->
- [x] Implement credential re-encryption after rekey within a transaction [security-critical]
  <!-- file: packages/storage-sqlite/src/credentials.rs -->
  <!-- purpose: Prevent all credential reads from failing after master key rotation -->
  <!-- requirements: Tier 1, item 7 -->
  <!-- leverage: existing credentials.rs and lib.rs -->

## 2.1 — Fix REST Transport Auth and Identity
> depends: 1.3
> spec: .odm/qa/reports/EXECUTIVE-SUMMARY.md

- [x] Remove middleware-local `Identity` type and use `life_engine_types::identity::Identity` [functional]
  <!-- file: packages/transport-rest/src/middleware/auth.rs -->
  <!-- purpose: Fix runtime extraction failure caused by dual Identity types -->
  <!-- requirements: Tier 2, item 8 -->
  <!-- leverage: existing auth.rs:19-24 -->
- [x] Fix public route bypass to match against `MatchedPath` or use `.route_layer()` [functional]
  <!-- file: packages/transport-rest/src/middleware/auth.rs -->
  <!-- purpose: Fix parameterized routes incorrectly requiring authentication -->
  <!-- requirements: Tier 2, item 9 -->
  <!-- leverage: existing auth.rs:56-60 -->
- [x] Apply `DefaultBodyLimit` to REST and GraphQL routes [functional]
  <!-- file: packages/transport-rest/src/lib.rs -->
  <!-- purpose: Prevent unbounded request body consumption -->
  <!-- requirements: Tier 2, item 16 -->
  <!-- leverage: none -->

## 2.2 — Fix Plugin System Execution and Host Functions
> depends: 1.3
> spec: .odm/qa/reports/EXECUTIVE-SUMMARY.md

- [x] Add blob builders to `injection.rs` and pass `BlobBackend` through `InjectionDeps` [functional]
  <!-- file: packages/plugin-system/src/injection.rs -->
  <!-- purpose: Wire blob host functions so blob storage works via plugin system -->
  <!-- requirements: Tier 2, item 10 -->
  <!-- leverage: existing injection.rs build_host_functions -->
- [x] Replace single global mutex with per-plugin locks or extract handles before WASM calls [functional]
  <!-- file: packages/plugin-system/src/execute.rs -->
  <!-- purpose: Allow parallel plugin execution system-wide -->
  <!-- requirements: Tier 2, item 11 -->
  <!-- leverage: existing execute.rs:79 -->
- [x] Add `Propfind` and `Report` variants to `HttpMethod` enum in plugin SDK [functional]
  <!-- file: packages/plugin-sdk-rs/src/types.rs -->
  <!-- purpose: Unblock CalDAV and CardDAV plugin development -->
  <!-- requirements: Tier 2, item 14 -->
  <!-- leverage: existing HttpMethod enum -->

## 2.3 — Fix Workflow Engine Race Conditions and Depth Tracking
> depends: none
> spec: .odm/qa/reports/EXECUTIVE-SUMMARY.md

- [x] Register job inline in `spawn()` before spawning the execution task [functional]
  <!-- file: packages/workflow-engine/src/executor.rs -->
  <!-- purpose: Fix race condition where immediate job_status() returns None -->
  <!-- requirements: Tier 2, item 12 -->
  <!-- leverage: existing executor.rs:358-369 -->
- [x] Increment depth when emitting events from event-triggered workflows [functional]
  <!-- file: packages/workflow-engine/src/event_bus.rs -->
  <!-- purpose: Fix event depth tracking so loop prevention works for cascading workflows -->
  <!-- requirements: Tier 2, item 13 -->
  <!-- leverage: existing event_bus.rs:222-234 -->

## 2.4 — Consolidate Cryptography and Fix DAV Serialization
> depends: 1.1
> spec: .odm/qa/reports/EXECUTIVE-SUMMARY.md

- [x] Remove duplicate AES-256-GCM implementation from `apps/core/src/crypto.rs` and redirect to `packages/crypto/` [functional]
  <!-- file: apps/core/src/crypto.rs -->
  <!-- purpose: Eliminate one of three duplicate encryption implementations -->
  <!-- requirements: Tier 2, item 17 -->
  <!-- leverage: packages/crypto/ as canonical implementation -->
- [x] Remove duplicate AES-256-GCM implementation from `plugins/engine/backup/src/crypto.rs` and redirect to `packages/crypto/` [functional]
  <!-- file: plugins/engine/backup/src/crypto.rs -->
  <!-- purpose: Eliminate second duplicate encryption implementation -->
  <!-- requirements: Tier 2, item 17 -->
  <!-- leverage: packages/crypto/ as canonical implementation -->
- [x] Add `zeroize` dependency to `packages/crypto/` and use `Zeroizing<[u8; 32]>` for derived keys [functional]
  <!-- file: packages/crypto/src/kdf.rs -->
  <!-- purpose: Prevent derived key material from persisting in memory -->
  <!-- requirements: Tier 2, item 18 -->
  <!-- leverage: existing kdf.rs -->
- [x] Fix UTF-8 line folding in CalDAV serializer to use `char_indices()` instead of byte offsets [functional]
  <!-- file: plugins/engine/api-caldav/src/serializer.rs -->
  <!-- purpose: Prevent silent corruption of non-ASCII characters during line folding -->
  <!-- requirements: Tier 2, item 15 -->
  <!-- leverage: existing fold_line function -->
- [x] Fix UTF-8 line folding in CardDAV serializer to use `char_indices()` instead of byte offsets [functional]
  <!-- file: plugins/engine/api-carddav/src/serializer.rs -->
  <!-- purpose: Prevent silent corruption of non-ASCII characters during line folding -->
  <!-- requirements: Tier 2, item 15 -->
  <!-- leverage: existing fold_line function -->

## 3.1 — Implement Webhook Delivery and DAV Protocol Handlers
> depends: 2.2
> spec: .odm/qa/reports/EXECUTIVE-SUMMARY.md

- [ ] Implement actual HTTP POST delivery with HMAC-SHA256 signing in webhook sender [ga-readiness]
  <!-- file: plugins/engine/webhook-sender/src/lib.rs -->
  <!-- purpose: Make webhook delivery functional instead of log-only -->
  <!-- requirements: Tier 3, item 19 -->
  <!-- leverage: existing lib.rs:261-271, reqwest already in deps -->
- [ ] Add exponential backoff and configurable timeouts to webhook delivery [ga-readiness]
  <!-- file: plugins/engine/webhook-sender/src/lib.rs -->
  <!-- purpose: Handle transient delivery failures gracefully -->
  <!-- requirements: Tier 3, item 19 -->
  <!-- leverage: none -->
- [ ] Implement minimum CalDAV protocol handlers: PROPFIND (depth 0/1), GET, PUT, OPTIONS with DAV headers [ga-readiness]
  <!-- file: packages/transport-caldav/src/lib.rs -->
  <!-- purpose: Enable basic CalDAV client compatibility -->
  <!-- requirements: Tier 3, item 20 -->
  <!-- leverage: existing transport crate scaffolding -->
- [ ] Implement minimum CardDAV protocol handlers: PROPFIND (depth 0/1), GET, PUT, OPTIONS with DAV headers [ga-readiness]
  <!-- file: packages/transport-carddav/src/lib.rs -->
  <!-- purpose: Enable basic CardDAV client compatibility -->
  <!-- requirements: Tier 3, item 20 -->
  <!-- leverage: existing transport crate scaffolding -->
- [ ] Add well-known redirects and current-user-principal for CalDAV and CardDAV discovery [ga-readiness]
  <!-- file: packages/transport-caldav/src/lib.rs -->
  <!-- purpose: Enable standard DAV client discovery flows -->
  <!-- requirements: Tier 3, item 20 -->
  <!-- leverage: none -->

## 3.2 — Fix Search, Persistence, and State Management
> depends: 2.3
> spec: .odm/qa/reports/EXECUTIVE-SUMMARY.md

- [ ] Add `user_id`/`household_id` filtering to search queries [ga-readiness]
  <!-- file: apps/core/src/search.rs -->
  <!-- purpose: Prevent cross-user/cross-tenant search result leakage -->
  <!-- requirements: Tier 3, item 21 -->
  <!-- leverage: existing search.rs:144-227 -->
- [ ] Implement batched Tantivy commits using existing `commit_threshold` config [ga-readiness]
  <!-- file: apps/core/src/search.rs -->
  <!-- purpose: Eliminate per-document write amplification -->
  <!-- requirements: Tier 3, item 22 -->
  <!-- leverage: existing commit_threshold config -->
- [ ] Add SQLite tables for federation peer registrations and sync cursors [ga-readiness]
  <!-- file: apps/core/src/federation.rs -->
  <!-- purpose: Persist federation state across restarts -->
  <!-- requirements: Tier 3, item 23 -->
  <!-- leverage: existing in-memory federation.rs -->
- [ ] Add SQLite tables for households and invites [ga-readiness]
  <!-- file: apps/core/src/household.rs -->
  <!-- purpose: Persist household state across restarts -->
  <!-- requirements: Tier 3, item 23 -->
  <!-- leverage: existing in-memory household.rs -->
- [ ] Switch search index from `create_in_ram()` to `Index::create_in_dir()` [ga-readiness]
  <!-- file: apps/core/src/search.rs -->
  <!-- purpose: Persist search index across restarts -->
  <!-- requirements: Tier 3, item 24 -->
  <!-- leverage: existing search.rs Tantivy setup -->

## 3.3 — Unify Plugin Types and Fix Build Infrastructure
> depends: 2.2
> spec: .odm/qa/reports/EXECUTIVE-SUMMARY.md

- [ ] Unify `Capability` enums between traits crate and SDK or add explicit conversion functions [ga-readiness]
  <!-- file: packages/plugin-sdk-rs/src/types.rs -->
  <!-- purpose: Eliminate divergent Capability definitions -->
  <!-- requirements: Tier 3, item 25 -->
  <!-- leverage: existing traits::Capability and SDK Capability -->
- [ ] Document `CorePlugin` vs `Plugin` usage and migration path [ga-readiness]
  <!-- file: packages/plugin-sdk-rs/src/lib.rs -->
  <!-- purpose: Guide developers on which plugin model to use -->
  <!-- requirements: Tier 3, item 26 -->
  <!-- leverage: none -->
- [ ] Delete stale `apps/core/Dockerfile` [ga-readiness]
  <!-- file: apps/core/Dockerfile -->
  <!-- purpose: Remove non-functional Dockerfile that copies only 4 of 28 workspace members -->
  <!-- requirements: Tier 3, item 27 -->
  <!-- leverage: none -->
- [ ] Fix Docker Compose config format mismatch (TOML mount vs YAML parser) [ga-readiness]
  <!-- file: docker-compose.yml -->
  <!-- purpose: Align mounted config format with what config.rs expects -->
  <!-- requirements: Tier 3, item 27 -->
  <!-- leverage: none -->
- [ ] Align backup plugin cron version pin (`0.13`) with workspace (`0.15`) [ga-readiness]
  <!-- file: plugins/engine/backup/Cargo.toml -->
  <!-- purpose: Fix genuine version mismatch -->
  <!-- requirements: Tier 3, item 27 -->
  <!-- leverage: none -->

## 3.4 — Add Resource Limits and Connector Resilience
> depends: 2.1, 2.2
> spec: .odm/qa/reports/EXECUTIVE-SUMMARY.md

- [ ] Extract email connector `RetryState` into shared utility and apply to contacts, calendar, filesystem connectors [ga-readiness]
  <!-- file: plugins/engine/connector-email/src/lib.rs -->
  <!-- purpose: Add retry/backoff to all connectors, not just email -->
  <!-- requirements: Tier 3, item 28 -->
  <!-- leverage: existing RetryState in email connector -->
- [ ] Add blob size limit enforcement in plugin system [ga-readiness]
  <!-- file: packages/plugin-system/src/host_functions/blob.rs -->
  <!-- purpose: Prevent plugins from storing arbitrarily large blobs -->
  <!-- requirements: Tier 3, item 29 -->
  <!-- leverage: existing blob.rs -->
- [ ] Add WebSocket subscription connection limits to GraphQL [ga-readiness]
  <!-- file: apps/core/src/routes/graphql.rs -->
  <!-- purpose: Prevent memory exhaustion via unlimited subscriptions -->
  <!-- requirements: Tier 3, item 29 -->
  <!-- leverage: none -->
- [ ] Add automatic job registry cleanup to workflow engine [ga-readiness]
  <!-- file: packages/workflow-engine/src/executor.rs -->
  <!-- purpose: Prevent unbounded memory growth from completed jobs -->
  <!-- requirements: Tier 3, item 29 -->
  <!-- leverage: existing cleanup_expired_jobs() function -->
- [ ] Add decompression bomb protection to backup restore [ga-readiness]
  <!-- file: plugins/engine/backup/src/lib.rs -->
  <!-- purpose: Prevent memory exhaustion from malicious backup archives -->
  <!-- requirements: Tier 3, item 29 -->
  <!-- leverage: none -->
- [ ] Add audit query result pagination [ga-readiness]
  <!-- file: apps/core/src/routes/graphql.rs -->
  <!-- purpose: Prevent unbounded audit log query results -->
  <!-- requirements: Tier 3, item 29 -->
  <!-- leverage: none -->
- [ ] Fix backup `retention_days` mismatch — implement age-based retention or remove config option [ga-readiness]
  <!-- file: plugins/engine/backup/src/lib.rs -->
  <!-- purpose: Align backup behavior with documented configuration -->
  <!-- requirements: Tier 3, item 30 -->
  <!-- leverage: none -->
