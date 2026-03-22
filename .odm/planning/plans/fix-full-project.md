<!--
project: life-engine-core
source: .odm/qa/full-project/tasks.md
updated: 2026-03-22
-->

# QA Fix Plan — Full Project

## Plan Overview

This plan addresses 196 QA findings across 170 files discovered in the full-project audit. It is organized into four work packages by severity: critical blockers first (crypto, SQL injection, Dockerfile), then high-priority auth/data bugs, medium-priority hardening and test gaps, and finally low-priority cleanup. WPs 1.2–1.4 all depend on 1.1 completing first, but can run in parallel with each other once critical fixes land.

Source: `.odm/qa/full-project/report.md`

Progress: 0 / 4 work packages complete (76 tasks)

---

## 1.1 — Critical Fixes
> qa-report: .odm/qa/full-project/report.md

- [x] Replace XOR encryption with AES-256-GCM in core crypto [BLOCKER]
  <!-- file: apps/core/src/crypto.rs -->
  <!-- purpose: Replace xor_encrypt with aes-gcm crate, unique nonce per encryption -->
  <!-- findings: F-001 -->
  <!-- severity: critical -->
  <!-- impact: Credentials at rest are trivially recoverable with current XOR cipher -->

- [x] Replace XOR encryption in identity module [BLOCKER]
  <!-- file: apps/core/src/identity.rs -->
  <!-- purpose: Use authenticated encryption for identity documents (passports, licences) -->
  <!-- findings: F-002 -->
  <!-- severity: critical -->
  <!-- impact: Identity documents have no confidentiality or integrity protection -->

- [x] Fix SQL injection via sort_by in storage backends [BLOCKER]
  <!-- file: apps/core/src/sqlite_storage.rs, apps/core/src/pg_storage.rs -->
  <!-- purpose: Validate sort_by against allowlist or restrict to [a-zA-Z0-9_] -->
  <!-- findings: F-003 -->
  <!-- severity: critical -->
  <!-- impact: Attacker can execute arbitrary SQL via crafted sort field name -->

- [x] Replace fake AES-256-GCM in backup crypto [BLOCKER]
  <!-- file: plugins/engine/backup/src/crypto.rs -->
  <!-- purpose: Implement real AES-256-GCM using aes-gcm crate, remove homebrew XOR cipher -->
  <!-- findings: F-004, F-005, F-006, F-031 -->
  <!-- severity: critical -->
  <!-- impact: Backup encryption provides no meaningful security; nonce reuse and fixed salt compound the issue -->

- [x] Fix panic in production GraphQL handler [BLOCKER]
  <!-- file: apps/core/src/routes/graphql.rs -->
  <!-- purpose: Replace expect() with 503 error response matching other handlers -->
  <!-- findings: F-007 -->
  <!-- severity: critical -->
  <!-- impact: Server crashes if storage is not initialized when GraphQL request arrives -->

- [x] Fix Dockerfile Rust version for edition 2024 [BLOCKER]
  <!-- file: apps/core/Dockerfile -->
  <!-- purpose: Update FROM rust:1.83-alpine to rust:1.85-alpine or later -->
  <!-- findings: F-008 -->
  <!-- severity: critical -->
  <!-- impact: All Docker builds fail; blocks CI/CD and deployment -->

## 1.2 — High Priority Fixes
> depends: 1.1
> qa-report: .odm/qa/full-project/report.md

- [x] Replace fake HMAC with proper HMAC construction
  <!-- file: apps/core/src/crypto.rs -->
  <!-- purpose: Use hmac crate instead of SHA256(key || data) to prevent length-extension attacks -->
  <!-- findings: F-009 -->
  <!-- severity: high -->
  <!-- impact: Identity token signing vulnerable to forgery -->

- [x] Fix derive_key domain separation collisions
  <!-- file: apps/core/src/crypto.rs -->
  <!-- purpose: Use HKDF or add separator between secret and domain -->
  <!-- findings: F-010 -->
  <!-- severity: high -->
  <!-- impact: Key isolation between subsystems is broken -->

- [x] Fix SQL injection via filter field names
  <!-- file: apps/core/src/sqlite_storage.rs, apps/core/src/pg_storage.rs -->
  <!-- purpose: Validate filter field names to [a-zA-Z0-9_.] characters -->
  <!-- findings: F-011 -->
  <!-- severity: high -->
  <!-- impact: Filter field names from HTTP API can inject arbitrary SQL -->

- [x] Fix LIKE meta-character injection in text search
  <!-- file: apps/core/src/pg_storage.rs, apps/core/src/sqlite_storage.rs -->
  <!-- purpose: Escape %, _, \ in text search contains values -->
  <!-- findings: F-012 -->
  <!-- severity: high -->
  <!-- impact: Unintended pattern matching through unescaped LIKE wildcards -->

- [x] Generate random Argon2 salt per database in rekey
  <!-- file: apps/core/src/rekey.rs -->
  <!-- purpose: Replace hardcoded b"life-engine-salt" with random salt stored alongside DB -->
  <!-- findings: F-013 -->
  <!-- severity: high -->
  <!-- impact: Identical passphrases on different databases produce identical keys -->

- [x] Wire up file-backed storage instead of in-memory
  <!-- file: apps/core/src/main.rs -->
  <!-- purpose: Use configured data_dir and storage.backend to open persistent storage -->
  <!-- findings: F-014 -->
  <!-- severity: high -->
  <!-- impact: All data is lost on every server restart -->

- [x] Fix storage init router auth ordering
  <!-- file: apps/core/src/main.rs -->
  <!-- purpose: Verify and fix auth middleware coverage for /api/storage/init -->
  <!-- findings: F-015 -->
  <!-- severity: high -->
  <!-- impact: Endpoint auth behavior contradicts documented design -->

- [x] Fix predictable WebAuthn session passphrase
  <!-- file: apps/core/src/auth/webauthn_provider.rs -->
  <!-- purpose: Create dedicated internal token-minting bypassing passphrase verification -->
  <!-- findings: F-016 -->
  <!-- severity: high -->
  <!-- impact: Anyone knowing a user_id can mint arbitrary session tokens -->

- [x] Add /api/auth/register to middleware bypass list
  <!-- file: apps/core/src/auth/middleware.rs -->
  <!-- purpose: Add register endpoint to skip list or update documentation -->
  <!-- findings: F-017 -->
  <!-- severity: high -->
  <!-- impact: Registration endpoint returns 401 before reaching handler -->

- [x] Fix conflict resolver KeepLocal placeholder
  <!-- file: apps/core/src/conflict.rs -->
  <!-- purpose: Return RequiresManual variant or Err when auto-merge fails -->
  <!-- findings: F-018 -->
  <!-- severity: high -->
  <!-- impact: Remote changes silently discarded on merge failure -->

- [x] Replace std::sync::RwLock with tokio::sync in federation
  <!-- file: apps/core/src/federation.rs -->
  <!-- purpose: Use async-safe locks to prevent deadlocks in tokio runtime -->
  <!-- findings: F-019 -->
  <!-- severity: high -->
  <!-- impact: Potential runtime deadlock under concurrent federation sync -->

- [x] URL-encode federation sync parameters
  <!-- file: apps/core/src/federation.rs -->
  <!-- purpose: Use urlencoding::encode() for collection and cursor in URLs -->
  <!-- findings: F-020 -->
  <!-- severity: high -->
  <!-- impact: Malicious peer cursor values can alter sync request URLs -->

- [x] Fix rate limiter 0.0.0.0 fallback
  <!-- file: apps/core/src/rate_limit.rs -->
  <!-- purpose: Fail closed or extract IP from X-Forwarded-For when ConnectInfo absent -->
  <!-- findings: F-021 -->
  <!-- severity: high -->
  <!-- impact: All requests behind a proxy share one rate-limit bucket -->

- [x] Apply WASM HTTP request headers and body
  <!-- file: apps/core/src/wasm_runtime.rs -->
  <!-- purpose: Stop ignoring _headers and _body parameters in handle_http_request -->
  <!-- findings: F-022 -->
  <!-- severity: high -->
  <!-- impact: Plugin HTTP POST/PUT requests are sent without payload -->

- [x] Fix HTTP domain allowlist suffix bypass
  <!-- file: apps/core/src/wasm_runtime.rs -->
  <!-- purpose: Require exact or dot-prefixed match instead of ends_with -->
  <!-- findings: F-023 -->
  <!-- severity: high -->
  <!-- impact: evilexample.com passes when example.com is allowed -->

- [x] Fix XML injection in dav-utils responses
  <!-- file: packages/dav-utils/src/dav_xml.rs -->
  <!-- purpose: XML-escape all interpolated values in write_response_entry and open_multistatus -->
  <!-- findings: F-024 -->
  <!-- severity: high -->
  <!-- impact: Malformed XML or injection via user-controlled calendar/contact data -->

- [x] Fix iCal timezone handling for TZID datetimes
  <!-- file: packages/dav-utils/src/ical.rs -->
  <!-- purpose: Integrate chrono-tz for correct TZID-to-UTC conversion -->
  <!-- findings: F-025 -->
  <!-- severity: high -->
  <!-- impact: All non-UTC events stored at wrong time -->

- [x] Add auth/rate-limiting to storage init endpoint
  <!-- file: apps/core/src/routes/storage.rs -->
  <!-- purpose: Prevent unlimited passphrase brute-force retries -->
  <!-- findings: F-026 -->
  <!-- severity: high -->
  <!-- impact: Unlimited passphrase guessing on encryption init endpoint -->

- [x] Add pagination and filter pushdown to federation changes
  <!-- file: apps/core/src/routes/federation.rs -->
  <!-- purpose: Push since filter into storage query, add cursor-based pagination -->
  <!-- findings: F-027 -->
  <!-- severity: high -->
  <!-- impact: Changes beyond 1000 records silently dropped -->

- [x] Fix N+1 query in GraphQL attendee resolver
  <!-- file: apps/core/src/routes/graphql.rs -->
  <!-- purpose: Batch attendee email lookups into single query -->
  <!-- findings: F-028 -->
  <!-- severity: high -->
  <!-- impact: Sequential DB round-trips per attendee in calendar events -->

- [x] Secure credential retrieval endpoint
  <!-- file: apps/core/src/routes/credentials.rs -->
  <!-- purpose: Add Cache-Control: no-store, audit logging middleware, ensure HTTPS -->
  <!-- findings: F-029 -->
  <!-- severity: high -->
  <!-- impact: Plaintext secrets (API keys, passwords) returned in API response body -->

- [x] Persist household role changes
  <!-- file: apps/core/src/routes/household.rs -->
  <!-- purpose: Call store.update_member_role() before returning success -->
  <!-- findings: F-030 -->
  <!-- severity: high -->
  <!-- impact: Role changes return success but are never saved -->

- [x] Fix XML injection in CalDAV/CardDAV protocol responses
  <!-- file: plugins/engine/api-caldav/src/protocol.rs, plugins/engine/api-carddav/src/protocol.rs -->
  <!-- purpose: XML-escape all interpolated values in build_propfind_xml and build_report_xml -->
  <!-- findings: F-032 -->
  <!-- severity: high -->
  <!-- impact: User-controlled data injected into XML responses -->

- [x] Fix path traversal in local backup backend
  <!-- file: plugins/engine/backup/src/backend/local.rs -->
  <!-- purpose: Validate resolved path stays within base_dir; reject keys with .. -->
  <!-- findings: F-033 -->
  <!-- severity: high -->
  <!-- impact: Attacker can read/write files outside backup directory -->

- [x] Fix scaffold plugin template dependency path
  <!-- file: tools/templates/engine-plugin/Cargo.toml -->
  <!-- purpose: Change to ../../../packages/plugin-sdk-rs -->
  <!-- findings: F-034 -->
  <!-- severity: high -->
  <!-- impact: Every scaffolded plugin fails cargo check -->

- [x] Update build verification test workspace members
  <!-- file: apps/core/tests/build_verification_test.rs -->
  <!-- purpose: Add 7 missing workspace members to expected set -->
  <!-- findings: F-035 -->
  <!-- severity: high -->
  <!-- impact: False sense of workspace completeness -->

## 1.3 — Medium Priority Fixes
> depends: 1.1
> qa-report: .odm/qa/full-project/report.md

- [ ] Harden auth middleware and token validation
  <!-- file: apps/core/src/auth/middleware.rs, apps/core/src/auth/local_token.rs -->
  <!-- purpose: Add periodic RateLimiter cleanup; add token hash index for O(1) lookup -->
  <!-- findings: F-036, F-037 -->
  <!-- severity: medium -->
  <!-- impact: Unbounded memory growth and O(n) per-request cost -->

- [ ] Fix auth route WebAuthn and passkey issues
  <!-- file: apps/core/src/auth/routes.rs -->
  <!-- purpose: Fix IDOR in passkey deletion, use stable user_id for WebAuthn, preserve passkey label -->
  <!-- findings: F-038, F-039, F-040 -->
  <!-- severity: medium -->
  <!-- impact: Authorization bypass, broken cross-session passkeys, wrong labels -->

- [ ] Harden config validation and secret handling
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Redact secrets in Debug/Serialize, validate auth provider and storage backend settings, fix env var override ordering -->
  <!-- findings: F-041, F-049, F-051 -->
  <!-- severity: medium -->
  <!-- impact: Secrets leaked in logs; invalid config starts server that fails at runtime -->

- [ ] Fix main.rs startup issues
  <!-- file: apps/core/src/main.rs -->
  <!-- purpose: Propagate create_dir_all error, make schema dir configurable, remove blanket #[allow(dead_code)], cap TLS connections -->
  <!-- findings: F-042, F-043, F-044, F-050 -->
  <!-- severity: medium -->
  <!-- impact: Silent failures, compile-time paths in prod, dead code hidden, connection floods -->

- [ ] Harden SQLite storage
  <!-- file: apps/core/src/sqlite_storage.rs -->
  <!-- purpose: Assert hex-only chars for PRAGMA key; consider connection pooling or RwLock -->
  <!-- findings: F-045, F-046 -->
  <!-- severity: medium -->
  <!-- impact: Potential PRAGMA breakage; single-mutex bottleneck under concurrent load -->

- [ ] Enable PostgreSQL TLS
  <!-- file: apps/core/src/pg_storage.rs -->
  <!-- purpose: Make TLS mode configurable; default to require in production -->
  <!-- findings: F-047 -->
  <!-- severity: medium -->
  <!-- impact: Database credentials transmitted in plaintext -->

- [ ] Fix storage migration count verification
  <!-- file: apps/core/src/storage_migration.rs -->
  <!-- purpose: Truncate target table or count per-collection to avoid false mismatches -->
  <!-- findings: F-048 -->
  <!-- severity: medium -->
  <!-- impact: Migration incorrectly reports failure on re-run -->

- [ ] Fix household access check and federation store issues
  <!-- file: apps/core/src/household.rs, apps/core/src/federation.rs -->
  <!-- purpose: Add read/write distinction; handle poisoned locks; cap sync_history -->
  <!-- findings: F-052, F-053, F-055 -->
  <!-- severity: medium -->
  <!-- impact: Guest write access gap; cascading panics; unbounded memory -->

- [ ] Fix identity signature verification determinism
  <!-- file: apps/core/src/identity.rs -->
  <!-- purpose: Use canonical JSON (sorted keys) or BTreeMap for signature payload -->
  <!-- findings: F-054 -->
  <!-- severity: medium -->
  <!-- impact: Signature verification can fail across processes/platforms -->

- [ ] Fix conflict merge O(n^2) key collection
  <!-- file: apps/core/src/conflict.rs -->
  <!-- purpose: Use HashSet for unique key collection -->
  <!-- findings: F-107 -->
  <!-- severity: medium -->
  <!-- impact: Quadratic complexity in field merge for large records -->

- [ ] Add Connector trait lifecycle tests
  <!-- file: apps/core/src/connector.rs -->
  <!-- purpose: Add mock Connector and test authenticate-sync-disconnect including error paths -->
  <!-- findings: F-056 -->
  <!-- severity: medium -->
  <!-- impact: Core plugin interface untested -->

- [ ] Fix WASM runtime HTTP handling and security
  <!-- file: apps/core/src/wasm_runtime.rs -->
  <!-- purpose: Reuse reqwest::Client, truncate plugin logs, default-deny empty allowlist -->
  <!-- findings: F-057, F-063, F-066 -->
  <!-- severity: medium -->
  <!-- impact: Connection churn, log flooding, unrestricted outbound HTTP -->

- [ ] Fix search indexing and query issues
  <!-- file: apps/core/src/search.rs, apps/core/src/search_processor.rs -->
  <!-- purpose: Use STRING for collection field, add bulk indexing, use BooleanQuery for filtering, add graceful shutdown -->
  <!-- findings: F-058, F-059, F-060, F-061 -->
  <!-- severity: medium -->
  <!-- impact: False matches, slow bulk import, incomplete pagination, no shutdown -->

- [ ] Fix federated create ID preservation
  <!-- file: apps/core/src/sync_primitives.rs -->
  <!-- purpose: Accept optional ID in storage.create() for federated sync -->
  <!-- findings: F-062 -->
  <!-- severity: medium -->
  <!-- impact: Federated records get new IDs; subsequent updates fail -->

- [ ] Reduce wasm_adapter code duplication
  <!-- file: apps/core/src/wasm_adapter.rs, apps/core/src/test_helpers.rs -->
  <!-- purpose: Extract bridge_call helper; consolidate MockStorage into test_helpers -->
  <!-- findings: F-064, F-065 -->
  <!-- severity: medium -->
  <!-- impact: ~400 lines of duplicated boilerplate across 3-4 files -->

- [ ] Fix dav-utils parsing and escaping bugs
  <!-- file: packages/dav-utils/src/dav_xml.rs, packages/dav-utils/src/text.rs, packages/dav-utils/src/vcard.rs -->
  <!-- purpose: Validate XML namespaces, fix escape ordering, handle CRLF in vCard -->
  <!-- findings: F-067, F-068, F-069 -->
  <!-- severity: medium -->
  <!-- impact: Malformed DAV XML, incorrect escape decoding, RFC 6350 violations -->

- [ ] Redact credentials in plugin SDK Debug output
  <!-- file: packages/plugin-sdk-rs/src/credential_store.rs, packages/plugin-sdk-rs/src/wasm_guest.rs -->
  <!-- purpose: Custom Debug for StoredCredential; use HttpMethod enum for method field -->
  <!-- findings: F-070, F-071 -->
  <!-- severity: medium -->
  <!-- impact: Plaintext credentials in panic messages and logs -->

- [ ] Fix credential type date validation
  <!-- file: packages/types/src/credentials.rs -->
  <!-- purpose: Use chrono::NaiveDate for issued_date and expiry_date -->
  <!-- findings: F-072 -->
  <!-- severity: medium -->
  <!-- impact: Arbitrary strings accepted as dates -->

- [ ] Fix test utility SMTP parsing
  <!-- file: packages/test-utils/src/connectors.rs -->
  <!-- purpose: Parse EHLO properly per RFC 5321 multi-line response format -->
  <!-- findings: F-073 -->
  <!-- severity: medium -->
  <!-- impact: Fragile integration test infrastructure -->

- [ ] Stream file content for SHA-256 computation
  <!-- file: packages/types/src/file_helpers.rs -->
  <!-- purpose: Use BufReader with 8KB chunks instead of fs::read -->
  <!-- findings: F-074 -->
  <!-- severity: medium -->
  <!-- impact: High memory usage for large files -->

- [ ] Sanitize error responses and strengthen passphrase requirements
  <!-- file: apps/core/src/routes/connectors.rs, apps/core/src/routes/storage.rs -->
  <!-- purpose: Return generic errors to clients; increase passphrase minimum to 12+ chars -->
  <!-- findings: F-075, F-076 -->
  <!-- severity: medium -->
  <!-- impact: Internal details leaked; weak master passphrase for encrypted storage -->

- [ ] Replace expect() panics in route handlers
  <!-- file: apps/core/src/routes/conflicts.rs -->
  <!-- purpose: Return 500 error instead of panicking on race condition -->
  <!-- findings: F-077 -->
  <!-- severity: medium -->
  <!-- impact: Server crash on concurrent conflict operations -->

- [ ] Replace brittle string matching with typed errors
  <!-- file: apps/core/src/routes/data.rs, apps/core/src/routes/graphql.rs -->
  <!-- purpose: Use storage layer error enum instead of msg.contains() -->
  <!-- findings: F-078 -->
  <!-- severity: medium -->
  <!-- impact: Silent breakage if error messages change -->

- [ ] Fix federation route validation and testing
  <!-- file: apps/core/src/routes/federation.rs -->
  <!-- purpose: Return 400 for invalid since param; add HTTP-level integration tests -->
  <!-- findings: F-079, F-084 -->
  <!-- severity: medium -->
  <!-- impact: Invalid timestamps return all data; auth middleware untested -->

- [ ] Fix GraphQL performance and security issues
  <!-- file: apps/core/src/routes/graphql.rs -->
  <!-- purpose: Batch N+1 attachment queries, strip user_id from input, gate Playground, deduplicate record_to_* functions -->
  <!-- findings: F-080, F-082, F-083 -->
  <!-- severity: medium -->
  <!-- impact: N+1 queries, exposed introspection, ~270 lines of duplicate code -->

- [ ] Fix data route user_id injection
  <!-- file: apps/core/src/routes/data.rs -->
  <!-- purpose: Always strip _user_id and _household_id from incoming body -->
  <!-- findings: F-081 -->
  <!-- severity: medium -->
  <!-- impact: Impersonation possible when identity extension absent -->

- [ ] Add household route integration tests
  <!-- file: apps/core/src/routes/household.rs -->
  <!-- purpose: Test create, invite, accept, role change, shared collections, last-admin guard -->
  <!-- findings: F-085 -->
  <!-- severity: medium -->
  <!-- impact: Least-tested route module in the codebase -->

- [ ] Fix connector route mutex holding during IO
  <!-- file: apps/core/src/routes/connectors.rs -->
  <!-- purpose: Clone plugin reference and drop lock before calling handle_route -->
  <!-- findings: F-086 -->
  <!-- severity: medium -->
  <!-- impact: All plugin access blocked during slow network sync -->

- [ ] Harden backup plugin backends
  <!-- file: plugins/engine/backup/src/backend/webdav.rs, plugins/engine/backup/src/types.rs, plugins/engine/webhook-receiver/src/models.rs, plugins/engine/webhook-sender/src/models.rs -->
  <!-- purpose: URL-encode backup keys, use proper XML parser, redact secrets in Debug/Serialize -->
  <!-- findings: F-087, F-088, F-091 -->
  <!-- severity: medium -->
  <!-- impact: URL traversal, naive XML parsing, secrets in logs -->

- [ ] Fix backup engine manifest and schedule issues
  <!-- file: plugins/engine/backup/src/engine.rs, plugins/engine/backup/src/schedule.rs -->
  <!-- purpose: Fix manifest stats inconsistency, validate hour/day ranges, batch manifest fetches -->
  <!-- findings: F-089, F-090, F-093 -->
  <!-- severity: medium -->
  <!-- impact: Wrong manifest data, silent backup schedule failures, N+1 fetches -->

- [ ] Register PROPFIND/REPORT routes in CalDAV/CardDAV plugins
  <!-- file: plugins/engine/api-caldav/src/lib.rs, plugins/engine/api-carddav/src/lib.rs -->
  <!-- purpose: Verify SDK supports WebDAV methods and register core CalDAV/CardDAV routes -->
  <!-- findings: F-092 -->
  <!-- severity: medium -->
  <!-- impact: Calendar/contact clients cannot perform discovery or sync -->

- [ ] Add BackupPlugin tests
  <!-- file: plugins/engine/backup/src/lib.rs -->
  <!-- purpose: Add unit tests for metadata, capabilities, routes, lifecycle -->
  <!-- findings: F-094 -->
  <!-- severity: medium -->
  <!-- impact: Plugin behaviour untested -->

- [ ] Fix CI check test flag and schema test
  <!-- file: apps/core/tests/ci_check_test.rs, apps/core/tests/schema_validation_test.rs -->
  <!-- purpose: Change --rust-only to --quick; make schema test assert on known violations -->
  <!-- findings: F-095, F-105 -->
  <!-- severity: medium -->
  <!-- impact: Test always fails; meaningless test that passes regardless -->

- [ ] Fix deploy compose configuration issues
  <!-- file: deploy/docker-compose.full.yml, docker-compose.yml -->
  <!-- purpose: Standardize Pocket ID image, verify issuer URL port, use env var refs for MinIO creds -->
  <!-- findings: F-096, F-097, F-098 -->
  <!-- severity: medium -->
  <!-- impact: Deployment inconsistencies and accidental credential exposure -->

- [ ] Fix Dockerfile HEALTHCHECK and dependency caching
  <!-- file: apps/core/Dockerfile -->
  <!-- purpose: Add HEALTHCHECK instruction, add dep caching layer, add missing COPY for packages -->
  <!-- findings: F-099, F-100, F-108 -->
  <!-- severity: medium -->
  <!-- impact: No health monitoring, slow builds, build failures from missing packages -->

- [ ] Fix OpenAPI spec validation gaps
  <!-- file: apps/core/openapi.yaml -->
  <!-- purpose: Add maximum on limit param, enforce conditional required for merged_data -->
  <!-- findings: F-101, F-106 -->
  <!-- severity: medium -->
  <!-- impact: Unbounded responses, schema doesn't reflect server validation -->

- [ ] Fix install script and scaffold script issues
  <!-- file: deploy/install.sh, tools/scripts/scaffold-plugin.sh, tools/scripts/configure-branch-protection.sh -->
  <!-- purpose: Use modern launchctl, validate plugin name input, consider enforce_admins: true -->
  <!-- findings: F-102, F-103, F-104 -->
  <!-- severity: medium -->
  <!-- impact: Deprecated macOS API, sed injection, weakened branch protection -->

## 1.4 — Low Priority Improvements
> depends: 1.1
> qa-report: .odm/qa/full-project/report.md

- [ ] Fix low-severity Bug Risk findings
  <!-- file: multiple files (23 findings) -->
  <!-- purpose: Fix edge cases: zero expiry, tilde expansion, parse errors, version checks, sort assumptions -->
  <!-- findings: F-109, F-110, F-111, F-112, F-113, F-114, F-115, F-116, F-117, F-118, F-119, F-120, F-121, F-122, F-123, F-124, F-125, F-126, F-127, F-128, F-129, F-130, F-131 -->
  <!-- severity: low -->
  <!-- impact: Various edge cases and silent failures that could confuse users -->

- [ ] Fix low-severity Security findings
  <!-- file: multiple files (10 findings) -->
  <!-- purpose: Redact secrets in Debug output, validate collection params, document test-only creds -->
  <!-- findings: F-132, F-133, F-134, F-135, F-136, F-137, F-138, F-139, F-140, F-141 -->
  <!-- severity: low -->
  <!-- impact: Secret exposure in logs, minor input validation gaps -->

- [ ] Fix low-severity Code Quality findings
  <!-- file: multiple files (14 findings) -->
  <!-- purpose: Remove blanket allow(dead_code), fix dead code, reduce duplication, handle errors -->
  <!-- findings: F-142, F-143, F-144, F-145, F-146, F-147, F-148, F-149, F-150, F-151, F-152, F-153, F-154, F-155 -->
  <!-- severity: low -->
  <!-- impact: Suppressed compiler warnings, dead code, duplicated test helpers -->

- [ ] Fix low-severity Performance findings
  <!-- file: multiple files (4 findings) -->
  <!-- purpose: Add depth limits, avoid unnecessary clones, use HashSet, bound delivery log -->
  <!-- findings: F-156, F-157, F-158, F-159 -->
  <!-- severity: low -->
  <!-- impact: Minor inefficiencies in search indexing, backup, and webhook delivery -->

- [ ] Fix low-severity Testing findings
  <!-- file: multiple files (10 findings) -->
  <!-- purpose: Add auth to test setup, fix timing-dependent tests, update stale test assertions -->
  <!-- findings: F-160, F-161, F-162, F-163, F-164, F-165, F-166, F-167, F-168, F-169 -->
  <!-- severity: low -->
  <!-- impact: Flaky tests, missing validation, stale assertions -->

- [ ] Fix low-severity Consistency findings
  <!-- file: multiple files (8 findings) -->
  <!-- purpose: Align schemas, types, imports, license declarations, dependency management -->
  <!-- findings: F-170, F-171, F-172, F-173, F-174, F-175, F-176, F-177 -->
  <!-- severity: low -->
  <!-- impact: Inconsistent patterns across codebase; Apache vs AGPL license confusion -->

- [ ] Fix low-severity Documentation findings
  <!-- file: multiple files (6 findings) -->
  <!-- purpose: Fix did:key compliance, add RFC line folding, fix vCard PREF parsing -->
  <!-- findings: F-178, F-179, F-180, F-181, F-182, F-183 -->
  <!-- severity: low -->
  <!-- impact: Non-compliant DID generation, RFC violations in iCal/vCard output -->
