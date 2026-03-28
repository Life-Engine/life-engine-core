<!--
project: core-application
source: .odm/qa/reports/phase-3/core-application.md
updated: 2026-03-28
-->

# Core Application — QA Remediation Plan

## Plan Overview

This plan addresses the issues identified in the phase-3 QA review of the `apps/core` crate — the main binary for Life Engine Core. Work packages are sequenced by priority: critical security fixes first (X-Forwarded-For trust, Docker config), then major data persistence and correctness issues, then minor improvements.

**Source:** .odm/qa/reports/phase-3/core-application.md

**Progress:** 0 / 8 work packages complete

---

## 1.1 — Fix X-Forwarded-For Trust and Rate Limiting
> depends: none
> spec: .odm/qa/reports/phase-3/core-application.md

- [ ] Check `config.network.behind_proxy` before parsing `X-Forwarded-For` in auth middleware [critical]
  <!-- file: apps/core/src/auth/middleware.rs -->
  <!-- purpose: Attacker on directly-exposed instance can spoof IP to bypass rate limiting -->
  <!-- requirements: 1 -->
  <!-- leverage: existing behind_proxy config flag -->
- [ ] Check `config.network.behind_proxy` before parsing `X-Forwarded-For` in rate limit middleware [critical]
  <!-- file: apps/core/src/rate_limit.rs -->
  <!-- purpose: Same IP spoofing issue in general rate limiter -->
  <!-- requirements: 1 -->
  <!-- leverage: existing behind_proxy config flag -->
- [ ] When not behind a proxy, always use `ConnectInfo<SocketAddr>` for client IP [critical]
  <!-- file: apps/core/src/auth/middleware.rs, apps/core/src/rate_limit.rs -->
  <!-- purpose: Ensure direct connections use the real socket address -->
  <!-- requirements: 1 -->
  <!-- leverage: axum ConnectInfo extractor -->
- [ ] Add tests verifying X-Forwarded-For is ignored when behind_proxy is false [critical]
  <!-- file: apps/core/src/auth/middleware.rs, apps/core/src/rate_limit.rs -->
  <!-- purpose: Regression prevention -->
  <!-- requirements: 1 -->
  <!-- leverage: none -->

## 1.2 — Fix Docker Compose Configuration
> depends: none
> spec: .odm/qa/reports/phase-3/core-application.md

- [x] Add `LIFE_ENGINE_BEHIND_PROXY=true` to docker-compose.yml or change auth provider [critical]
  <!-- file: deploy/docker-compose.yml -->
  <!-- purpose: Config validator rejects local-token on 0.0.0.0 without behind_proxy flag -->
  <!-- requirements: 2 -->
  <!-- leverage: existing config validation -->

## 1.3 — Redact Storage Passphrase and Fix Credential Store Docs
> depends: none
> spec: .odm/qa/reports/phase-3/core-application.md

- [ ] Add `storage.passphrase` to redaction logic in `to_redacted_json()` [major]
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Plaintext passphrase could appear in GET /api/system/config response -->
  <!-- requirements: 3 -->
  <!-- leverage: existing redaction pattern for oidc client_secret and pg password -->
- [ ] Update credential store module doc comment from "XOR-based" to describe actual AES-256-GCM implementation [major]
  <!-- file: apps/core/src/credential_store.rs -->
  <!-- purpose: Misleading doc could cause confusion during security audits -->
  <!-- requirements: 7 -->
  <!-- leverage: none -->

## 1.4 — Persist Federation and Household State
> depends: none
> spec: .odm/qa/reports/phase-3/core-application.md

- [ ] Add SQLite tables for federation peers and sync cursors [major]
  <!-- file: apps/core/src/federation.rs -->
  <!-- purpose: All federation state is in-memory; lost on restart -->
  <!-- requirements: 4 -->
  <!-- leverage: existing SQLite storage patterns in the codebase -->
- [ ] Load federation state from SQLite on startup, write on mutation [major]
  <!-- file: apps/core/src/federation.rs -->
  <!-- purpose: Persistence for peer registrations and sync history -->
  <!-- requirements: 4 -->
  <!-- leverage: existing FederationStore struct -->
- [ ] Add SQLite tables for households, memberships, and invites [major]
  <!-- file: apps/core/src/household.rs -->
  <!-- purpose: All household state is in-memory; lost on restart -->
  <!-- requirements: 4 -->
  <!-- leverage: existing SQLite storage patterns -->
- [ ] Load household state from SQLite on startup, write on mutation [major]
  <!-- file: apps/core/src/household.rs -->
  <!-- purpose: Persistence for household memberships -->
  <!-- requirements: 4 -->
  <!-- leverage: existing HouseholdStore struct -->
- [ ] Validate federation peer TLS certificate paths at peer creation time, not sync time [minor]
  <!-- file: apps/core/src/federation.rs -->
  <!-- purpose: Invalid paths currently only fail at sync time -->
  <!-- requirements: 4 -->
  <!-- leverage: none -->

## 1.5 — Fix PostgreSQL SSL and Rekey Security
> depends: none
> spec: .odm/qa/reports/phase-3/core-application.md

- [ ] Implement actual fallback behavior for `PgSslMode::Prefer` (try TLS, catch error, retry plaintext) [major]
  <!-- file: apps/core/src/pg_storage.rs -->
  <!-- purpose: Prefer variant currently behaves identically to Require; no fallback to plaintext -->
  <!-- requirements: 5 -->
  <!-- leverage: existing PgSslMode enum -->
- [ ] Mark `rekey::derive_key()` zeroed-salt fallback as `#[cfg(test)]` [major]
  <!-- file: apps/core/src/rekey.rs -->
  <!-- purpose: Public function with zeroed salt could be accidentally used in production -->
  <!-- requirements: 6 -->
  <!-- leverage: existing function at rekey.rs:91-92 -->

## 1.6 — Unify Plugin Manifest Types and Search Index
> depends: none
> spec: .odm/qa/reports/phase-3/core-application.md

- [ ] Unify or rename the two `PluginManifest` structs to avoid confusion [major]
  <!-- file: apps/core/src/manifest.rs, apps/core/src/plugin_loader.rs -->
  <!-- purpose: Two structs with same name but different fields creates confusion about which is authoritative -->
  <!-- requirements: 8 -->
  <!-- leverage: existing structs -->
- [ ] Consider using `Index::create_in_dir()` for persistent search index in production [major]
  <!-- file: apps/core/src/search.rs -->
  <!-- purpose: In-memory index lost on restart; re-indexing could be slow with large datasets -->
  <!-- requirements: 9 -->
  <!-- leverage: existing tantivy setup at search.rs:70 -->

## 1.7 — Extract Migration Logic from main()
> depends: none
> spec: .odm/qa/reports/phase-3/core-application.md

- [ ] Extract Step 4b canonical schema migration logic (~170 lines) into a dedicated `canonical_migrations` module [minor]
  <!-- file: apps/core/src/main.rs -->
  <!-- purpose: Business logic embedded in main() is a cohesion concern; limits testability -->
  <!-- requirements: 10 -->
  <!-- leverage: existing migration logic at main.rs:329-499 -->
- [ ] Add unit tests for the extracted migration module [minor]
  <!-- file: apps/core/src/canonical_migrations.rs -->
  <!-- purpose: Currently untestable due to being embedded in main -->
  <!-- requirements: 10 -->
  <!-- leverage: none -->

## 1.8 — Minor Hardening and Cleanup
> depends: none
> spec: .odm/qa/reports/phase-3/core-application.md

- [ ] Add HKDF salt parameter to `crypto::derive_key()` (even a static application-level salt) [minor]
  <!-- file: apps/core/src/crypto.rs -->
  <!-- purpose: Strengthen HKDF extraction step against related-key attacks -->
  <!-- requirements: 11 -->
  <!-- leverage: existing derive_key at crypto.rs:19 -->
- [ ] Log a warning when PEM file contains multiple private keys in `tls.rs` [minor]
  <!-- file: apps/core/src/tls.rs -->
  <!-- purpose: Multiple keys silently ignored; could cause confusion when wrong key selected -->
  <!-- requirements: 12 -->
  <!-- leverage: existing read_private_key function -->
- [ ] Replace sleep-based synchronization in audit subscriber tests with channel-based approach [minor]
  <!-- file: apps/core/src/audit.rs -->
  <!-- purpose: tokio::time::sleep(50ms) is fragile under CI load -->
  <!-- requirements: 13 -->
  <!-- leverage: none -->
- [ ] Remove unnecessary `Result` wrapper from `IdentityStore::new()` [minor]
  <!-- file: apps/core/src/identity.rs -->
  <!-- purpose: Constructor always returns Ok; Result wrapper is misleading -->
  <!-- requirements: 14 -->
  <!-- leverage: existing constructor at identity.rs:178-186 -->
- [ ] Remove redundant inner `Arc` from `ConflictStore` and `HouseholdStore` fields [minor]
  <!-- file: apps/core/src/household.rs -->
  <!-- purpose: Arc<RwLock<HashMap>> inside Arc is double-wrapped -->
  <!-- requirements: 15 -->
  <!-- leverage: none -->
- [ ] Consider caching plugin count in health endpoint to avoid Mutex contention [minor]
  <!-- file: apps/core/src/routes/health.rs -->
  <!-- purpose: Health check acquires plugin_loader Mutex; could be delayed under contention -->
  <!-- requirements: 16 -->
  <!-- leverage: none -->
- [ ] Prevent user from being added to multiple households in `create_household()` [minor]
  <!-- file: apps/core/src/household.rs -->
  <!-- purpose: No check if admin user already belongs to a household; creates ambiguous state -->
  <!-- requirements: 4 -->
  <!-- leverage: existing user_household_map -->
