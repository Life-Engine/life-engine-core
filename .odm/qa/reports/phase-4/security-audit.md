# Cross-Cutting Security Audit

Reviewed: 2026-03-28
Scope: Entire Life Engine Core codebase
Method: Analysis of Phase 1-3 review reports combined with targeted source code inspection

---

## Executive Summary

Life Engine Core demonstrates strong security *intent* throughout its architecture: encryption-at-rest via SQLCipher, WASM sandboxing for plugins, Ed25519 plugin signing, capability-based access control, credential redaction in debug output, and TLS enforcement for non-localhost deployments. The project uses well-audited Rust crates for cryptographic operations and avoids hand-rolled crypto primitives.

However, the audit identified 8 critical, 14 major, and 18 minor security issues spanning the full OWASP Top 10 surface. The most severe findings fall into three clusters:

- **Cryptographic salt failures** — Two critical paths use static/zero salts for Argon2id key derivation, enabling precomputed dictionary attacks against encrypted databases and backups.
- **Injection vectors** — SQL injection is possible through unsanitized field names in sort clauses and migration index creation. The plugin HTTP host function lacks SSRF protection for internal networks.
- **Authentication/rate-limit bypass** — The `X-Forwarded-For` header is trusted unconditionally across four independent code paths, allowing attackers to bypass all IP-based rate limiting on directly-exposed instances.

None of these issues are exploitable *together* as a single attack chain today, but several could combine with others (e.g., rate-limit bypass + passphrase brute-force) to escalate impact. The codebase needs targeted remediation before production deployment.

---

## OWASP Top 10 Assessment

### A01:2021 — Broken Access Control

Status: Issues found.

- Auth middleware public-route bypass is broken for parameterized routes (`packages/transport-rest/src/middleware/auth.rs`). Routes registered with `:collection` patterns never match concrete request URIs, so parameterized public routes still require auth.
- Dual `Identity` types (`middleware::auth::Identity` vs `life_engine_types::identity::Identity`) cause Axum extension extraction failure, producing 500 errors on authenticated requests.
- RBAC is scaffolded but not enforced. `HouseholdRole` (Admin/Member/Guest) exists in types but no middleware checks roles against operations.
- Scope strings in `AuthIdentity.scopes` are accepted without validation and never enforced by any middleware.
- `/api/storage/init` is exempt from auth with no alternative protection.
- Delete operations in `packages/storage-sqlite/src/backend.rs` do not filter by `collection`, allowing cross-collection deletion within a plugin's scope.

### A02:2021 — Cryptographic Failures

Status: Critical issues found.

- Zero salt in `apps/core/src/rekey.rs:88-93` — the production call path for SQLCipher key derivation uses `[0u8; SALT_LENGTH]`, defeating salt purpose entirely.
- Hardcoded salt `b"life-engine-salt"` in backup plugin (`plugins/engine/backup/src/crypto.rs:17`).
- `thread_rng()` used instead of `OsRng` for security-critical salt generation (`apps/core/src/rekey.rs:31`).
- No key zeroization in `packages/crypto/` crate or `apps/core/src/crypto.rs`.
- HMAC output returned as hex string with no constant-time verification function in `apps/core/src/crypto.rs`.
- HKDF without salt in `apps/core/src/crypto.rs:19`.
- Credential re-encryption missing after master key rotation — reading credentials after rekey fails silently.
- Three duplicated AES-256-GCM implementations create maintenance risk.

### A03:2021 — Injection

Status: Critical issues found.

- **SQL injection via sort field** — `packages/storage-sqlite/src/backend.rs:183` interpolates `s.field` directly into `ORDER BY json_extract(data, '$.{}')`. Same pattern in `apps/core/src/sqlite_storage.rs:473,605,620,630`.
- **SQL injection via migration DDL** — `packages/storage-sqlite/src/migration/executor.rs:77-103` interpolates plugin-provided field names directly into `CREATE INDEX` statements.
- **SQL injection via pg_storage sort** — `apps/core/src/pg_storage.rs:625` interpolates `opts.sort_by` into `ORDER BY data->>'{}' {dir}`. The `sort_by` is validated against `[a-zA-Z0-9_.]` which mitigates most attacks, but the approach is fragile.
- **Path traversal in blob keys** — `packages/plugin-system/src/host_functions/blob.rs:98` joins user-provided keys without rejecting `..` segments. `packages/storage-sqlite/src/blob_fs.rs:36` similarly joins keys to the root path without traversal validation.

### A04:2021 — Insecure Design

Status: Issues found.

- Federation and household state is entirely in-memory — lost on restart.
- API key validation requires O(n) full table scan for every request.
- WASI enabled unconditionally for all WASM plugins regardless of trust level.
- Plugin execution serialized behind a single global mutex.
- Event name validation does not verify plugin ID prefix — a plugin can declare events in another plugin's namespace.

### A05:2021 — Security Misconfiguration

Status: Issues found.

- Docker compose config (`deploy/docker-compose.yml:15`) sets `local-token` auth with `0.0.0.0` binding, which fails the config validator for non-localhost addresses.
- `to_redacted_json()` in `apps/core/src/config.rs:940-960` does not redact `storage.passphrase` — this plaintext passphrase can appear in the `/api/system/config` response.
- `OidcConfig.client_secret` derives `Serialize` without `#[serde(skip_serializing)]`.
- TLS `ServerConfig` uses `.with_no_client_auth()` for the main server, which is correct for a general API but means federation peers connecting to the API are not mutually authenticated at the TLS level.
- `PgSslMode::Prefer` maps to the same code path as `Require` — no fallback to plaintext.
- API key hashes and salts exposed via `list_keys` response.

### A06:2021 — Vulnerable and Outdated Components

Status: Low risk.

- `rand` version 0.8 is used (0.9 available) — not a security issue.
- `sha2` and `mime_guess` are used transitively in `packages/storage-sqlite/src/blob_fs.rs` without explicit Cargo.toml dependencies — fragile but not insecure.
- All cryptographic crates (`aes-gcm`, `argon2`, `hmac`, `sha2`, `hkdf`, `ed25519-dalek`) are well-maintained and widely audited.

### A07:2021 — Identification and Authentication Failures

Status: Issues found.

- `X-Forwarded-For` trusted without proxy validation in four locations: `apps/core/src/auth/middleware.rs`, `apps/core/src/rate_limit.rs`, `packages/auth/src/handlers/rate_limit.rs`, `packages/transport-rest/src/middleware/auth.rs`.
- No passphrase length limits — arbitrarily long passphrases to Argon2id cause DoS.
- JWKS refresh has a thundering herd condition — all concurrent threads can trigger parallel fetches.
- OIDC HTTP client has no timeout in `apps/core/src/auth/oidc.rs`.
- Expired tokens never cleaned from storage — unbounded growth.
- `jwks_refresh_interval` of zero not validated — triggers fetch on every request.
- `KeyRevoked` returned for expired keys — semantic confusion.

### A08:2021 — Software and Data Integrity Failures

Status: Well-addressed.

- Ed25519 plugin signing binds WASM binary to manifest hash, preventing post-signing capability tampering.
- Revocation list with normalized hex comparison.
- Unsigned plugins blocked by default (opt-in required).
- Manifest validation is thorough (ID format, semver, reserved names, capability cross-checks).

Gaps:

- `thread_rng()` used for Ed25519 key generation in tests (`apps/core/src/plugin_signing.rs:299`). Test-only, but `OsRng` would be more consistent.
- No maximum length for plugin IDs or action names — could cause issues with path construction.

### A09:2021 — Security Logging and Monitoring Failures

Status: Moderate coverage.

- Audit logging exists for storage mutations, credential access, and plugin lifecycle.
- Disclosure events are logged with credential ID, claim names, recipient, and timestamp.
- Rate-limited auth failures are tracked.

Gaps:

- TLS handshake failures logged at `debug` level — too quiet for attack detection.
- Logging middleware omits request correlation IDs.
- No audit trail for federation sync operations beyond in-memory history.
- `Lagged` broadcast events (dropped audit entries) logged as warning but no recovery attempted.

### A10:2021 — Server-Side Request Forgery (SSRF)

Status: Critical issue found.

- `packages/plugin-system/src/host_functions/http.rs` blocks non-HTTP schemes but does **not** block requests to internal network addresses (127.0.0.1, 169.254.169.254, 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, fd00::/8) when `allowed_domains` is `None`.
- `allowed_domains` is hardcoded to `None` in injection (`packages/plugin-system/src/injection.rs:324`), meaning domain restrictions are **never enforced** in practice.
- A malicious plugin can probe internal services, access cloud metadata endpoints (AWS/GCP/Azure at 169.254.169.254), and attack services on the local network.
- Response body size check happens **after** full download (`response.bytes().await`), allowing a malicious server to send multi-gigabyte responses causing OOM before the 10 MB check triggers.

---

## Consolidated Vulnerability List

### Critical (8)

- **SEC-C01: Zero salt for Argon2id in production SQLCipher path** — `apps/core/src/rekey.rs:88-93`, `apps/core/src/sqlite_storage.rs:111`. All databases derived from the same passphrase get identical encryption keys.
- **SEC-C02: Hardcoded salt in backup encryption** — `plugins/engine/backup/src/crypto.rs:17`. Fixed `b"life-engine-salt"` enables precomputed dictionary attacks.
- **SEC-C03: X-Forwarded-For spoofing bypasses all rate limiting** — `apps/core/src/auth/middleware.rs`, `apps/core/src/rate_limit.rs`, `packages/auth/src/handlers/rate_limit.rs`, `packages/transport-rest/src/middleware/auth.rs`. Four independent rate limiters all trust this header unconditionally.
- **SEC-C04: SQL injection via sort field interpolation** — `packages/storage-sqlite/src/backend.rs:183`, `apps/core/src/sqlite_storage.rs:473,605,620,630`. Caller-controlled `s.field` is interpolated directly into SQL.
- **SEC-C05: SQL injection via migration index DDL** — `packages/storage-sqlite/src/migration/executor.rs:77-103`. Plugin manifest field names interpolated into `CREATE INDEX` statements.
- **SEC-C06: SSRF via plugin HTTP host function** — `packages/plugin-system/src/host_functions/http.rs`. No private IP blocking, and domain allowlist is permanently disabled via hardcoded `None`.
- **SEC-C07: Auth middleware public-route bypass broken for parameterized routes** — `packages/transport-rest/src/middleware/auth.rs:56-60`. Pattern vs concrete path mismatch means parameterized public routes always require auth.
- **SEC-C08: Dual Identity types cause runtime 500 on all authenticated requests** — `packages/transport-rest/src/middleware/auth.rs:19-24` vs `packages/transport-rest/src/handlers/mod.rs:17`. Different struct types keyed in Axum's extension map.

### Major (14)

- **SEC-M01: `thread_rng()` for security-critical salt generation** — `apps/core/src/rekey.rs:31`. Should use `OsRng`.
- **SEC-M02: No key zeroization in crypto crate** — `packages/crypto/src/kdf.rs` returns `[u8; 32]` with no zeroize. Derived keys persist in memory indefinitely.
- **SEC-M03: Credential re-encryption missing after rekey** — `packages/storage-sqlite/src/credentials.rs` + `src/lib.rs`. After master key rotation, all credential reads fail because ciphertext was encrypted under old derived keys.
- **SEC-M04: `storage.passphrase` not redacted in config API** — `apps/core/src/config.rs:940-960`. Plaintext passphrase can appear in `GET /api/system/config` response.
- **SEC-M05: No request body size limits** — Neither `packages/transport-rest` nor `apps/core` apply `DefaultBodyLimit`. Clients can send arbitrarily large bodies.
- **SEC-M06: HTTP response fully downloaded before size check** — `packages/plugin-system/src/host_functions/http.rs:193-200`. Multi-GB response causes OOM before the 10 MB cap triggers.
- **SEC-M07: No blob size limit in plugin host function** — `packages/plugin-system/src/host_functions/blob.rs`. Arbitrary-size blobs can cause OOM during base64 decode.
- **SEC-M08: Path traversal in blob storage keys** — `packages/plugin-system/src/host_functions/blob.rs:98`, `packages/storage-sqlite/src/blob_fs.rs:36`. `../` sequences can escape plugin namespace.
- **SEC-M09: WASI enabled unconditionally for all plugins** — `packages/plugin-system/src/runtime.rs:182`. Third-party plugins get clock, env, and random access.
- **SEC-M10: API key hashes and salts exposed via list_keys** — `packages/auth/src/handlers/keys.rs:103-127`. Gives attackers material for offline brute-force.
- **SEC-M11: No passphrase length validation** — `apps/core/src/auth/types.rs:40-46`. Multi-MB passphrases to Argon2id cause DoS.
- **SEC-M12: OIDC HTTP client has no timeout** — `apps/core/src/auth/oidc.rs:101`. Slow OIDC provider hangs the server indefinitely.
- **SEC-M13: TLS connection spawning unbounded** — `packages/transport-rest/src/listener.rs:54-97`. No concurrency limit on incoming TLS connections.
- **SEC-M14: `std::sync::Mutex` with `unwrap()` in async executor** — `packages/plugin-system/src/execute.rs`. Any panic poisons the mutex permanently.

### Minor (18)

- **SEC-m01: HMAC without constant-time verification** — `apps/core/src/crypto.rs:59`. `hmac_sha256` returns hex string; callers may compare with `==`.
- **SEC-m02: HKDF without salt** — `apps/core/src/crypto.rs:19`. `Hkdf::<Sha256>::new(None, ...)`.
- **SEC-m03: Panics on invalid key length** — `apps/core/src/crypto.rs:31,51`. `.expect()` instead of `Result`.
- **SEC-m04: `declared_emit_events` never wired** — `packages/plugin-system/src/injection.rs:365`. Manifest event restrictions dead in practice.
- **SEC-m05: `execution_depth` always 0** — `packages/plugin-system/src/injection.rs:366`. Cascading event detection disabled.
- **SEC-m06: `allowed_domains` never populated** — `packages/plugin-system/src/injection.rs:324`. HTTP domain restrictions dead in practice.
- **SEC-m07: Inconsistent Argon2id parameters** — `packages/crypto/src/kdf.rs` vs `apps/core/src/config.rs`. Two KDF configurations can diverge silently.
- **SEC-m08: Error messages leak implementation details** — `CryptoError::EncryptionFailed(String)` and `CryptoError::DecryptionFailed(String)` include library error strings.
- **SEC-m09: JWKS thundering herd** — `packages/auth/src/handlers/validate.rs:225-236`. All threads refresh simultaneously under TOCTOU race.
- **SEC-m10: Duplicate rate limiter implementations** — `packages/auth/src/handlers/rate_limit.rs` vs `apps/core/src/auth/middleware.rs`. Two separate, uncoordinated limiters.
- **SEC-m11: TLS handshake failures logged at debug** — `packages/transport-rest/src/listener.rs`. Too quiet for attack detection.
- **SEC-m12: No `#[deny(unsafe_code)]`** — `packages/crypto/src/lib.rs`. No compile-time guarantee against future unsafe additions.
- **SEC-m13: Expired tokens never cleaned** — `apps/core/src/auth/local_token.rs`. Unbounded memory and storage growth.
- **SEC-m14: `/api/auth/register` exempt from auth with no separate rate limiting** — `apps/core/src/auth/middleware.rs`. Registration endpoint could be abused.
- **SEC-m15: Event naming does not verify plugin ID prefix** — `packages/plugin-system/src/manifest.rs:517`. Plugins can declare events in another plugin's namespace.
- **SEC-m16: No maximum length for plugin IDs** — `packages/plugin-system/src/manifest.rs`. Could cause path and logging issues.
- **SEC-m17: `OidcConfig.client_secret` derives Serialize** — `apps/core/src/auth/oidc.rs:26`. Could be exposed in debug or API output.
- **SEC-m18: PRAGMA key quoting inconsistency** — `packages/storage-sqlite/src/lib.rs:77` uses `'x"..."'` while `apps/core/src/sqlite_storage.rs:127` uses `"x'...'"`. Only one is correct per SQLCipher docs.

---

## Attack Surface Analysis

### External entry points

- **REST API** (`packages/transport-rest/`, `apps/core/src/routes/`) — Primary attack surface. JSON request bodies, URL path parameters, query parameters, and HTTP headers all flow into the application.
- **GraphQL API** (`apps/core/src/routes/graphql.rs`, `packages/transport-graphql/`) — Query complexity and depth are not limited. Deeply nested or aliased queries could cause resource exhaustion.
- **SSE event stream** (`apps/core/src/routes/events.rs`) — Authenticated endpoint; keep-alive prevents idle timeout. Filtering is server-side.
- **Federation sync endpoint** (`apps/core/src/federation.rs`) — Accepts pull requests from peer instances. mTLS provides mutual authentication, but federation data flows into storage without schema validation.
- **WebAuthn endpoints** (`apps/core/src/auth/webauthn_provider.rs`) — Challenge state stored in memory with TTL. Registration endpoints exempt from auth.

### Internal (plugin-to-host) boundary

- **WASM host functions** — Plugins communicate with Core via JSON serialization/deserialization. The host function interface is the second-most critical attack boundary after the REST API.
- **Storage host functions** — Plugin ID scoping is enforced at the host level. Cross-plugin impersonation is prevented.
- **HTTP outbound** — Plugins can make HTTP requests to any domain (SSRF risk). No streaming body limit.
- **Blob storage** — Path traversal risk in scoped key construction.
- **Event system** — Event source is host-injected (not plugin-controlled), but depth tracking and event name scoping are not enforced.

### Data at rest

- **SQLCipher database** — Encrypted, but key derivation uses zero/fixed salts in production paths.
- **Blob filesystem** — Unencrypted. No integrity verification beyond SHA-256 checksums.
- **Configuration files** — May contain plaintext passphrases and client secrets.
- **Backup archives** — Encrypted with hardcoded salt, undermining key derivation.

### Network boundary

- **TLS termination** — Implemented via `rustls` with no client auth. Strong cipher suite defaults from rustls.
- **mTLS for federation** — Properly implemented with CA verification and client certificates.
- **PostgreSQL TLS** — Supported but `Prefer` mode does not actually fall back. Root cert loading silently ignores individual failures.
- **OIDC provider communication** — No timeout on HTTP client in one of two implementations.

---

## Remediation Priorities

### Immediate (before any production deployment)

1. **Fix cryptographic salts** (SEC-C01, SEC-C02) — Replace zero/hardcoded salts with per-database and per-backup random salts. Gate the `derive_key` zero-salt function behind `#[cfg(test)]`.

2. **Fix SQL injection vectors** (SEC-C04, SEC-C05) — Validate field names against `[a-zA-Z0-9_]` allowlist before interpolation. For sort fields, use parameterized JSON path binding or validate against declared schema fields. For migration indexes, sanitize collection and field names from plugin manifests.

3. **Fix X-Forwarded-For trust** (SEC-C03) — Check `config.network.behind_proxy` before parsing `X-Forwarded-For` in all four locations. When not behind a proxy, use `ConnectInfo<SocketAddr>` for the real peer address.

4. **Add SSRF protection** (SEC-C06) — Block RFC 1918, link-local (169.254.0.0/16), loopback (127.0.0.0/8), and cloud metadata (169.254.169.254) addresses in the HTTP host function. Wire `allowed_domains` from plugin manifests.

5. **Unify Identity types** (SEC-C07, SEC-C08) — Remove `middleware::auth::Identity` and map to `life_engine_types::identity::Identity`. Fix parameterized route matching by using `MatchedPath` or route-level middleware.

6. **Redact storage passphrase** (SEC-M04) — Add `storage.passphrase` to `to_redacted_json()`.

### Short-term (before beta/early access)

7. **Add key zeroization** (SEC-M02) — Add `zeroize` to `packages/crypto/` and use `Zeroizing<[u8; 32]>` for derived keys.

8. **Implement credential re-encryption on rekey** (SEC-M03) — After master key rotation, re-encrypt all credentials within a transaction.

9. **Add request body size limits** (SEC-M05) — Apply `DefaultBodyLimit` to the middleware stack.

10. **Fix HTTP response streaming** (SEC-M06) — Use `content_length()` pre-check or incremental streaming with size limit.

11. **Add blob size limit** (SEC-M07) — Enforce a `MAX_BLOB_SIZE` constant.

12. **Validate blob keys** (SEC-M08) — Reject keys containing `..`, absolute paths, or null bytes.

13. **Add passphrase length limit** (SEC-M11) — Cap at 1024 bytes before passing to Argon2id.

14. **Strip hashes from API key listing** (SEC-M10) — Return metadata-only records.

15. **Set OIDC HTTP timeout** (SEC-M12) — Match the 10-second timeout from the PocketIdProvider.

16. **Make WASI configurable** (SEC-M09) — Default to disabled for third-party plugins.

### Medium-term (ongoing hardening)

17. **Consolidate crypto implementations** — Migrate `apps/core/src/crypto.rs` and `plugins/engine/backup/src/crypto.rs` to use `packages/crypto/`.

18. **Add `#[deny(unsafe_code)]`** to `packages/crypto/src/lib.rs`.

19. **Consolidate rate limiters** — Remove duplicates and use a single implementation.

20. **Wire up manifest-declared restrictions** — Connect `declared_emit_events`, `allowed_domains`, and `execution_depth`.

21. **Add GraphQL query depth/complexity limits**.

22. **Add TLS connection concurrency limits** (SEC-M13).

23. **Replace `std::sync::Mutex` with `tokio::sync::Mutex` in async executor** (SEC-M14).

24. **Add expired token cleanup** (SEC-m13).

---

## Recommendations

### Architecture

- **Consolidate cryptographic code into `packages/crypto/`** — Having three separate AES-256-GCM implementations with different salt handling and different zeroization practices is the root cause of multiple critical findings. A single, well-tested crypto module with mandatory zeroization and proper salt handling would eliminate SEC-C01, SEC-C02, SEC-M01, SEC-M02, SEC-m01, SEC-m02, SEC-m03, and SEC-m07.

- **Establish a clear input validation boundary** — All user-controlled values that reach SQL statements or filesystem paths should pass through a validation layer. Currently, some paths validate (filter values use bind parameters) while others do not (sort fields interpolated directly). A `validate_field_name()` function that enforces `[a-zA-Z0-9_]` would close all SQL injection vectors.

- **Wire the plugin capability system end-to-end** — Several security features exist in code but are disconnected from the runtime: `allowed_domains`, `declared_emit_events`, `execution_depth`, and blob host function injection. These represent significant engineering effort that is currently providing zero security value.

### Process

- **Add security-focused integration tests** — Tests for SSRF (internal IP rejection), SQL injection (special characters in sort fields), path traversal (blob keys with `../`), and rate-limit bypass (spoofed X-Forwarded-For) would prevent regressions.

- **Run `cargo audit` in CI** — Check for known vulnerabilities in dependencies.

- **Document the trust model** — Explicitly document which components trust which inputs. The current codebase has implicit trust boundaries that are easy to miss (e.g., plugin manifests are trusted for DDL generation but not for direct SQL queries).

### Operations

- **Ensure TLS handshake failures are logged at warn or error level** for production deployments.

- **Add request correlation IDs** to enable tracing across log entries.

- **Persist federation and household state** before relying on these features.
