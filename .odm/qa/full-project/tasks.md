# QA Remediation Tasks

- **Date** — 2026-03-27
- **Source** — `.odm/qa/full-project/report.md`
- **Total tasks** — 36

---

## High Severity Tasks (1 task per finding)

### T-01: Fix unconditional X-Forwarded-For trust in auth middleware

- **Findings** — F-001
- **File** — `./apps/core/src/auth/middleware.rs`
- **Action** — Only trust X-Forwarded-For when `behind_proxy` config is enabled. Fall back to peer address otherwise.

### T-02: Fix WebAuthn session token passphrase design bug

- **Findings** — F-002
- **File** — `./apps/core/src/auth/webauthn_provider.rs`
- **Action** — Pre-set the local provider passphrase before token generation, or use a dedicated method that bypasses passphrase verification.

### T-03: Fix graceful shutdown race with in-flight requests

- **Findings** — F-003
- **File** — `./apps/core/src/main.rs`
- **Action** — Ensure shutdown sequence waits for request drain before running WAL checkpoint and storage teardown.

### T-04: Handle TLS cert loading errors in pg_storage

- **Findings** — F-004
- **File** — `./apps/core/src/pg_storage.rs`
- **Action** — Replace `.ok()` with logging or error propagation on certificate load failure.

### T-05: Fix unconditional X-Forwarded-For trust in rate_limit.rs

- **Findings** — F-005
- **File** — `./apps/core/src/rate_limit.rs`
- **Action** — Same fix as T-01, applied to the rate limiter module.

### T-06: Remove zero-salt fallback in derive_key

- **Findings** — F-006
- **File** — `./apps/core/src/rekey.rs`
- **Action** — Mark `derive_key()` as `#[cfg(test)]` or remove entirely, requiring callers to use `derive_key_with_salt`.

### T-07: Add plugin-scoped access control to credential retrieval

- **Findings** — F-007
- **File** — `./apps/core/src/routes/credentials.rs`
- **Action** — Verify the requesting user/plugin has access to the target `plugin_id` before returning credentials.

### T-08: Secure federation routes and remove unwraps

- **Findings** — F-008
- **File** — `./apps/core/src/routes/federation.rs`
- **Action** — Add auth middleware to federation endpoints. Replace `serde_json::to_value().unwrap()` with `?` operator or error response.

### T-09: Fix hardcoded rate-limit IP in storage routes

- **Findings** — F-009
- **File** — `./apps/core/src/routes/storage.rs`
- **Action** — Extract actual client IP from the request instead of using `Ipv4Addr::UNSPECIFIED`.

### T-10: Fix u64-to-u32 truncation in migration pagination

- **Findings** — F-010
- **File** — `./apps/core/src/storage_migration.rs`
- **Action** — Change `Pagination.offset` to `u64` or add bounds check before cast.

### T-11: Fix SQL injection in sort field interpolation

- **Findings** — F-011
- **File** — `./packages/storage-sqlite/src/backend.rs`
- **Action** — Validate field names against an allowlist (alphanumeric, underscore, dots) or parameterize the ORDER BY clause.

### T-12: Handle missing claims in credential encryption

- **Findings** — F-012
- **File** — `./packages/storage-sqlite/src/credentials.rs`
- **Action** — Replace JSON null-index with `doc.get("claims").ok_or_else(...)` to fail fast on missing claims.

### T-13: Replace hardcoded Argon2 salt in backup crypto

- **Findings** — F-013
- **File** — `./plugins/engine/backup/src/crypto.rs`
- **Action** — Generate and store a random salt per backup or per installation.

---

## Medium Severity Tasks (grouped by file)

### T-14: Harden auth/local_token.rs

- **Findings** — F-014, F-015
- **File** — `./apps/core/src/auth/local_token.rs`
- **Action** — Address SQLite mutex bottleneck (F-014) and add passphrase complexity enforcement (F-015).

### T-15: Fix MultiAuthProvider error handling

- **Findings** — F-016
- **File** — `./apps/core/src/auth/mod.rs`
- **Action** — Return the most relevant error rather than the last one.

### T-16: Harden OIDC config and token handling

- **Findings** — F-017, F-018
- **File** — `./apps/core/src/auth/oidc.rs`
- **Action** — Remove Serialize/Debug from `client_secret` (F-017). Require `exp` claim or use a sensible default (F-018).

### T-17: Improve auth route error handling

- **Findings** — F-019, F-020
- **File** — `./apps/core/src/auth/routes.rs`
- **Action** — Replace string-based error classification (F-019). Prevent user enumeration on WebAuthn start (F-020).

### T-18: Redact sensitive config and fix env mapping

- **Findings** — F-021, F-022
- **File** — `./apps/core/src/config.rs`
- **Action** — Redact `storage.passphrase` in config output (F-021). Disambiguate underscore-to-key env mapping (F-022).

### T-19: Fix crypto module issues

- **Findings** — F-023, F-024
- **File** — `./apps/core/src/crypto.rs`
- **Action** — Add salt to HKDF (F-023). Return error instead of panicking on non-32-byte key (F-024).

### T-20: Harden pg_storage.rs

- **Findings** — F-025, F-026, F-027
- **File** — `./apps/core/src/pg_storage.rs`
- **Action** — Replace `expect()` with graceful error on cert load (F-025). Parameterize or validate `sort_by` (F-026). Handle dots in field names for JSONB (F-027).

### T-21: Fix credential route logging

- **Findings** — F-028
- **File** — `./apps/core/src/routes/credentials.rs`
- **Action** — Reduce credential retrieval log level or redact key names.

### T-22: Add ownership checks to data routes

- **Findings** — F-029, F-030, F-031
- **File** — `./apps/core/src/routes/data.rs`
- **Action** — Require identity for record creation (F-029). Add ownership checks to update (F-030) and delete (F-031).

### T-23: Fix event stream error handling

- **Findings** — F-032
- **File** — `./apps/core/src/routes/events.rs`
- **Action** — Handle `BroadcastStream` errors instead of silently dropping them.

### T-24: Optimize federation serve_changes

- **Findings** — F-033
- **File** — `./apps/core/src/routes/federation.rs`
- **Action** — Push filtering into the database query instead of loading 1000 records and filtering in memory.

### T-25: Fix GraphQL schema and record handling

- **Findings** — F-034, F-035, F-036
- **File** — `./apps/core/src/routes/graphql.rs`
- **Action** — Fail on missing fields instead of defaulting (F-034). Cache schema (F-035). Use compile-time feature gate for playground (F-036).

### T-26: Fix household route issues

- **Findings** — F-037, F-038
- **File** — `./apps/core/src/routes/household.rs`
- **Action** — Align `HouseholdState` with `AppState` pattern (F-037). Fix TOCTOU race on last-admin check (F-038).

### T-27: Fix identity route error handling

- **Findings** — F-039
- **File** — `./apps/core/src/routes/identity.rs`
- **Action** — Replace string matching with typed error variants.

### T-28: Remove DB path from storage response

- **Findings** — F-040
- **File** — `./apps/core/src/routes/storage.rs`
- **Action** — Remove or redact database path from API response.

### T-29: Batch search indexing commits

- **Findings** — F-041
- **File** — `./apps/core/src/search.rs`
- **Action** — Batch commits instead of committing per record.

### T-30: Fix shutdown handles for PgStorage

- **Findings** — F-042
- **File** — `./apps/core/src/shutdown.rs`
- **Action** — Include `PgStorage` in `ShutdownHandles` for proper teardown.

### T-31: Harden sqlite_storage sort and filter

- **Findings** — F-043, F-044
- **File** — `./apps/core/src/sqlite_storage.rs`
- **Action** — Validate `sort_by` field names (F-043). Return errors for invalid filter fields (F-044).

### T-32: Fix storage migration batch and conflict handling

- **Findings** — F-045, F-046
- **File** — `./apps/core/src/storage_migration.rs`
- **Action** — Batch record inserts (F-045). Replace DO NOTHING with upsert or error (F-046).

### T-33: Fix WASM runtime UTF-8 and URL handling

- **Findings** — F-047, F-048
- **File** — `./apps/core/src/wasm_runtime.rs`
- **Action** — Use safe UTF-8 truncation (F-047). Block hostless URLs in domain allowlist (F-048).

### T-34: Harden packages/auth handlers

- **Findings** — F-049, F-050, F-051, F-052, F-053
- **Files** — `./packages/auth/src/handlers/keys.rs`, `./packages/auth/src/handlers/rate_limit.rs`, `./packages/auth/src/handlers/validate.rs`, `./packages/auth/src/types.rs`
- **Action** — Remove hardcoded version (F-049). Index keys for O(1) lookup (F-050). Bound the rate limiter map (F-051). Use case-insensitive scheme comparison (F-052). Skip-serialize `key_hash` and `salt` (F-053).

### T-35: Fix packages-level medium findings

- **Findings** — F-054, F-055, F-056, F-057, F-058, F-059, F-060
- **Files** — `./packages/crypto/src/kdf.rs`, `./packages/dav-utils/src/dav_xml.rs`, `./packages/plugin-sdk-rs/src/test/mock_storage.rs`, `./packages/plugin-sdk-rs/src/wasm_guest.rs`, `./packages/storage-sqlite/src/backend.rs`, `./packages/traits/src/capability.rs`, `./packages/types/src/pipeline.rs`
- **Action** — Enforce minimum salt length (F-054). Fix XML namespace spacing (F-055). Fix mock storage matching (F-056). Add credential HostRequest variants (F-057). Remove duplicate CANONICAL_COLLECTIONS (F-058). Reconcile Capability enums (F-059). Validate on deserialization (F-060).

### T-36: Fix plugin-level medium findings

- **Findings** — F-061, F-062, F-063, F-064, F-065, F-066, F-067, F-068, F-069, F-070, F-071, F-072, F-073, F-074, F-075, F-077, F-078, F-079, F-080, F-081, F-082, F-083
- **Files** — Multiple files across `./plugins/engine/`
- **Action** — Fix UTF-8 fold_line in CalDAV/CardDAV serializers (F-061, F-062). Implement or remove `Plugin::execute` stubs (F-063). Harden backup path traversal check (F-064). Fix WebDAV PROPFIND parsing (F-065, F-066). Deduplicate backup config (F-067). Add decompression size limit (F-068). Add skip_serializing to BackupTarget secrets (F-069). Escape iCal special chars (F-070). Handle empty refresh_token (F-071). Replace error string matching (F-072). Restrict connect_plain visibility (F-073). Fix non-idempotent updated_at (F-074). Pool SMTP transport (F-075). Align S3 secret with credential store (F-077). Cache S3 SDK client (F-078). Add S3 list pagination (F-079). Bound symlink following (F-080). Check duplicate endpoint IDs (F-081). Deduplicate plugin identity (F-082). Implement handle_event dispatch (F-083).

---

## Low Severity Tasks (grouped by dimension)

### T-37: Low — Security findings

- **Findings** — F-085, F-088, F-090, F-091, F-097, F-101, F-102, F-107, F-130
- **Action** — Validate issuer when present (F-085). Add defense-in-depth for zero rate-limit config (F-088). Exclude key paths from serialization (F-090). Zeroize private key material (F-091). Don't reuse salt on rekey (F-097). Redact plugin failure messages (F-101). Enforce max TTL on disclosure tokens (F-102). Bound WASM HTTP response body (F-107). Hide `http_client` field (F-130).

### T-38: Low — Bug Risk findings

- **Findings** — F-086, F-093, F-094, F-096, F-098, F-099, F-100, F-103, F-105, F-106, F-109, F-112, F-113, F-116, F-118, F-119, F-120, F-121, F-122, F-124, F-125, F-127, F-129, F-131, F-136
- **Action** — Fix silent date parse fallback (F-086). Handle non-UTF-8 plist paths (F-093). Resolve workflows path relative to data_dir (F-094). Tighten semver validation (F-096). Reject empty credential values (F-098). Apply collection filter to all event types (F-099). Add compound filter check (F-100). Return appropriate HTTP status for search errors (F-103). Populate user_id/household_id in storage backends (F-105). Migrate user_id/household_id (F-106). Validate unknown auth providers (F-109). Fix audit timestamp fallback (F-112). Handle map result in credentials (F-113). Guard u64-to-i64 cast (F-116). Enforce limit cap in type (F-118). Propagate read_dir errors (F-119). Set non-zero event duration default (F-120). Escape ADR fields (F-121). Fix TOCTOU in backup delete (F-122). Guard i64-to-u64 cast (F-124). Refresh stale backup manifest (F-125). Return error from CalDAV stubs (F-127). Populate all_day field (F-129). Add TEL pref detection (F-131). Guard i64-to-u64 cast in S3 (F-136).

### T-39: Low — Performance findings

- **Findings** — F-104, F-110, F-123, F-126, F-135, F-138, F-139
- **Action** — Use async fs I/O in system handler (F-104). Cache plugin instances (F-110). Use async exists check (F-123). Use set-based dedup for incremental backup (F-126). Remove unnecessary watch_paths clone (F-135). Remove unnecessary body clone (F-138). Use indexed eviction strategy (F-139).

### T-40: Low — Maintainability findings

- **Findings** — F-084, F-087, F-089, F-092, F-095, F-108, F-111, F-114, F-115, F-117, F-128, F-132, F-133, F-134, F-137
- **Action** — Remove dead validate_jwt (F-084). Fix UUID generation (F-087). Update stale doc comment (F-089). Use BTreeMap in test (F-092). Remove mem::forget on span guard (F-095). Add #[ignore] to Docker test (F-108). Rename from_str to avoid trait shadow (F-111). Avoid allocation in has_schema (F-114). Fix serde rename inconsistency (F-115). Add assert to test (F-117). Use url crate for encoding (F-128). Remove dead use_tls field (F-132). Deduplicate message building (F-133). Replace hardcoded sleep (F-134). Consolidate FileChange types (F-137).

---

## Info (no tasks)

Findings F-140 through F-146 are informational and do not require remediation tasks.
