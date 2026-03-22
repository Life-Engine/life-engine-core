<!--
qa-name: full-project
target: entire life-engine-core repository
status: complete
updated: 2026-03-22
files-inspected: 170
-->

# QA Report — Full Project

## Summary

- **Target:** Entire life-engine-core repository
- **Scope:** 170 files inspected across 30 directories
- **Date:** 2026-03-22

### Findings by Severity

- **Critical:** 8
- **High:** 27
- **Medium:** 68
- **Low:** 75
- **Info:** 13

### Findings by Dimension

- **Bug Risk:** 58
- **Security:** 68
- **Performance:** 27
- **Code Quality:** 24
- **Testing:** 16
- **Consistency:** 11
- **Documentation:** 5

## Files Inspected

### apps/core/src/

- `main.rs` — 6 findings
- `config.rs` — 5 findings
- `crypto.rs` — 3 findings
- `sqlite_storage.rs` — 4 findings
- `pg_storage.rs` — 4 findings
- `storage_migration.rs` — 2 findings
- `credential_store.rs` — 1 finding
- `credential_bridge.rs` — 0 findings
- `error.rs` — 1 finding
- `storage.rs` — 0 findings
- `tls.rs` — 1 finding
- `rekey.rs` — 2 findings
- `conflict.rs` — 3 findings
- `connector.rs` — 1 finding
- `federation.rs` — 5 findings
- `household.rs` — 2 findings
- `identity.rs` — 3 findings
- `manifest.rs` — 1 finding
- `message_bus.rs` — 1 finding
- `rate_limit.rs` — 2 findings
- `schema_registry.rs` — 1 finding
- `shutdown.rs` — 1 finding
- `plugin_loader.rs` — 1 finding
- `plugin_signing.rs` — 2 findings
- `search.rs` — 5 findings
- `search_processor.rs` — 3 findings
- `sync_primitives.rs` — 2 findings
- `test_helpers.rs` — 1 finding
- `wasm_adapter.rs` — 2 findings
- `wasm_runtime.rs` — 5 findings

### apps/core/src/auth/

- `jwt.rs` — 2 findings
- `local_token.rs` — 3 findings
- `middleware.rs` — 2 findings
- `mod.rs` — 1 finding
- `oidc.rs` — 1 finding
- `routes.rs` — 5 findings
- `types.rs` — 0 findings
- `webauthn_provider.rs` — 2 findings
- `webauthn_store.rs` — 1 finding

### apps/core/src/routes/

- `conflicts.rs` — 1 finding
- `connectors.rs` — 2 findings
- `credentials.rs` — 1 finding
- `data.rs` — 2 findings
- `events.rs` — 2 findings
- `federation.rs` — 4 findings
- `graphql.rs` — 7 findings
- `health.rs` — 0 findings
- `household.rs` — 3 findings
- `identity.rs` — 2 findings
- `mod.rs` — 1 finding
- `plugins.rs` — 0 findings
- `quarantine.rs` — 0 findings
- `search.rs` — 1 finding
- `storage.rs` — 4 findings
- `system.rs` — 0 findings

### packages/dav-utils/src/

- `dav_xml.rs` — 2 findings
- `ical.rs` — 2 findings
- `text.rs` — 2 findings
- `url.rs` — 1 finding
- `vcard.rs` — 1 finding
- `etag.rs` — 0 findings
- `lib.rs` — 0 findings
- `sync_state.rs` — 0 findings
- `auth.rs` — 0 findings

### packages/plugin-sdk-rs/src/

- `credential_store.rs` — 1 finding
- `wasm_guest.rs` — 2 findings
- `types.rs` — 1 finding
- `lib.rs` — 0 findings
- `retry.rs` — 0 findings
- `traits.rs` — 0 findings

### packages/types/src/

- `credentials.rs` — 2 findings
- `events.rs` — 1 finding
- `file_helpers.rs` — 1 finding
- `tasks.rs` — 1 finding
- `contacts.rs` — 0 findings
- `emails.rs` — 0 findings
- `files.rs` — 0 findings
- `lib.rs` — 0 findings
- `notes.rs` — 0 findings

### packages/test-utils/src/

- `connectors.rs` — 3 findings
- `docker.rs` — 1 finding
- `lib.rs` — 0 findings
- `assert_macros.rs` — 0 findings
- `plugin_test_helpers.rs` — 0 findings

### packages/test-fixtures/src/

- `lib.rs` — 1 finding

### plugins/engine/backup/src/

- `crypto.rs` — 4 findings
- `engine.rs` — 3 findings
- `backend.rs` — 0 findings
- `backend/local.rs` — 1 finding
- `backend/s3.rs` — 2 findings
- `backend/webdav.rs` — 3 findings
- `lib.rs` — 1 finding
- `retention.rs` — 1 finding
- `schedule.rs` — 2 findings
- `types.rs` — 1 finding

### plugins/engine/api-caldav/src/

- `protocol.rs` — 1 finding
- `lib.rs` — 1 finding
- `serializer.rs` — 1 finding
- `discovery.rs` — 0 findings

### plugins/engine/api-carddav/src/

- `protocol.rs` — 1 finding
- `lib.rs` — 1 finding
- `serializer.rs` — 2 findings

### plugins/engine/connector-calendar/src/

- `caldav.rs` — 1 finding
- `google.rs` — 1 finding
- `lib.rs` — 1 finding
- `normalizer.rs` — 0 findings

### plugins/engine/connector-contacts/src/

- `normalizer.rs` — 1 finding
- `carddav.rs` — 0 findings
- `google.rs` — 0 findings
- `lib.rs` — 0 findings

### plugins/engine/connector-email/src/

- `imap.rs` — 0 findings
- `lib.rs` — 0 findings
- `normalizer.rs` — 0 findings
- `smtp.rs` — 0 findings

### plugins/engine/connector-filesystem/src/

- `local.rs` — 1 finding
- `s3.rs` — 1 finding
- `lib.rs` — 0 findings
- `normalizer.rs` — 0 findings

### plugins/engine/webhook-receiver/src/

- `models.rs` — 1 finding
- `lib.rs` — 0 findings
- `mapping.rs` — 0 findings
- `signature.rs` — 0 findings

### plugins/engine/webhook-sender/src/

- `models.rs` — 1 finding
- `delivery.rs` — 1 finding
- `lib.rs` — 0 findings

### Config, deploy, tests, scripts

- `Cargo.toml` (root) — 1 finding
- `apps/core/Cargo.toml` — 2 findings
- `apps/core/Dockerfile` — 3 findings
- `apps/core/openapi.yaml` — 3 findings
- `docker-compose.yml` — 1 finding
- `deploy/docker-compose.full.yml` — 2 findings
- `deploy/install.sh` — 1 finding
- `tools/scripts/scaffold-plugin.sh` — 1 finding
- `tools/scripts/configure-branch-protection.sh` — 1 finding
- `tools/templates/engine-plugin/Cargo.toml` — 1 finding
- `tools/templates/engine-plugin/src/lib.rs` — 1 finding
- `apps/core/tests/build_verification_test.rs` — 1 finding
- `apps/core/tests/ci_check_test.rs` — 1 finding
- `apps/core/tests/schema_validation_test.rs` — 1 finding
- `apps/core/tests/dev_environment_test.rs` — 1 finding
- `deny.toml` — 0 findings

---

## Findings

### Critical

#### F-001 — XOR encryption is cryptographically broken

- **File:** `./apps/core/src/crypto.rs`
- **Line(s):** 25-30
- **Dimension:** Security
- **Severity:** Critical
- **Description:** `xor_encrypt` implements a repeating-key XOR cipher (`byte ^ key[i % key.len()]`). This is trivially broken via known-plaintext attacks and frequency analysis. It is used by `credential_store.rs` to encrypt stored API keys, tokens, and passwords at rest.
- **Recommendation:** Replace with AES-256-GCM via the `aes-gcm` crate with a unique nonce per encryption.

---

#### F-002 — XOR encryption on identity documents with no integrity

- **File:** `./apps/core/src/identity.rs`
- **Line(s):** 570-581
- **Dimension:** Security
- **Severity:** Critical
- **Description:** `encrypt_claims` and `decrypt_claims` use `crypto::xor_encrypt` to protect identity documents (passports, driver's licences). XOR provides no authenticated encryption; tampered ciphertext silently produces garbage or attacker-controlled JSON. No integrity check exists.
- **Recommendation:** Use AES-256-GCM or ChaCha20-Poly1305 which rejects tampered ciphertext before returning plaintext.

---

#### F-003 — SQL injection via sort_by field

- **File:** `./apps/core/src/sqlite_storage.rs`, `./apps/core/src/pg_storage.rs`
- **Line(s):** sqlite:364, pg:490
- **Dimension:** Security
- **Severity:** Critical
- **Description:** The `query` method interpolates the `sort_by` field name directly into SQL: `format!("ORDER BY json_extract(data, '$.{}') {dir}", opts.sort_by)`. This field originates from HTTP API input. An attacker can inject arbitrary SQL via a crafted sort_by value.
- **Recommendation:** Validate `sort_by` against an allowlist of known field names, or restrict to `[a-zA-Z0-9_]` characters.

---

#### F-004 — Backup encryption is fake AES-256-GCM

- **File:** `./plugins/engine/backup/src/crypto.rs`
- **Line(s):** 68-91
- **Dimension:** Security
- **Severity:** Critical
- **Description:** The `encrypt` function's doc comment claims AES-256-GCM, but the implementation is a homebrew XOR stream cipher using SHA-256 keystream blocks with a SHA-256-based MAC. This provides no meaningful security for encrypted backups.
- **Recommendation:** Replace with the `aes-gcm` or `chacha20poly1305` crate from the RustCrypto project. Remove the homebrew cipher entirely.

---

#### F-005 — Timestamp-based nonce enables nonce reuse

- **File:** `./plugins/engine/backup/src/crypto.rs`
- **Line(s):** 146-158
- **Dimension:** Security
- **Severity:** Critical
- **Description:** `rand_nonce` derives the 12-byte nonce from `SystemTime::now().as_nanos()`, not a CSPRNG. Two encryptions within the same nanosecond reuse the nonce, which is catastrophic for stream ciphers (reveals XOR of plaintexts).
- **Recommendation:** Use `rand::thread_rng().fill()` or `getrandom` for cryptographically random nonces.

---

#### F-006 — Hardcoded Argon2 salt in backup crypto

- **File:** `./plugins/engine/backup/src/crypto.rs`
- **Line(s):** 14-15
- **Dimension:** Security
- **Severity:** Critical
- **Description:** `ARGON2_SALT` is the fixed string `b"life-engine-salt"`. Users with the same passphrase derive identical encryption keys, enabling rainbow table attacks.
- **Recommendation:** Generate a random 16-byte salt per backup and store it alongside the encrypted data.

---

#### F-007 — Panic in production GraphQL handler

- **File:** `./apps/core/src/routes/graphql.rs`
- **Line(s):** 1237
- **Dimension:** Bug Risk
- **Severity:** Critical
- **Description:** `graphql_handler` calls `state.storage.expect("storage must be initialized")` which panics if storage is `None`. Every other route handler returns a 503 JSON error.
- **Recommendation:** Return 503 Service Unavailable instead of panicking.

---

#### F-008 — Dockerfile Rust version incompatible with edition 2024

- **File:** `./apps/core/Dockerfile`
- **Line(s):** 2
- **Dimension:** Bug Risk
- **Severity:** Critical
- **Description:** Builder stage uses `rust:1.83-alpine` but `Cargo.toml` specifies `edition = "2024"`, which requires Rust 1.85+. Docker builds will fail.
- **Recommendation:** Update to `FROM rust:1.85-alpine AS builder` or later.

---

### High

#### F-009 — hmac_sha256 vulnerable to length-extension attacks

- **File:** `./apps/core/src/crypto.rs`
- **Line(s):** 35-39
- **Dimension:** Security
- **Severity:** High
- **Description:** Computes `SHA256(key || data)` instead of proper HMAC. Vulnerable to length-extension attacks. Used for identity token signing.
- **Recommendation:** Use the `hmac` crate with `sha2` for standard HMAC construction.

---

#### F-010 — derive_key has domain separation collisions

- **File:** `./apps/core/src/crypto.rs`
- **Line(s):** 14-18
- **Dimension:** Security
- **Severity:** High
- **Description:** `derive_key("secretA", "BC")` and `derive_key("secretAB", "C")` produce the same output because it computes `SHA256(secret || domain)` with no separator. Breaks claimed key isolation.
- **Recommendation:** Use HKDF from the `hkdf` crate, or insert a length prefix between secret and domain.

---

#### F-011 — SQL injection via filter field names

- **File:** `./apps/core/src/sqlite_storage.rs`, `./apps/core/src/pg_storage.rs`
- **Line(s):** sqlite:475,487,493 / pg:611,627,636
- **Dimension:** Security
- **Severity:** High
- **Description:** Filter field names from `FieldFilter`, `ComparisonFilter`, and `TextFilter` are interpolated directly into SQL without sanitization. Values are parameterized, but field names are not.
- **Recommendation:** Validate all field names to contain only `[a-zA-Z0-9_.]` characters.

---

#### F-012 — LIKE meta-character injection

- **File:** `./apps/core/src/pg_storage.rs`, `./apps/core/src/sqlite_storage.rs`
- **Line(s):** pg:637, sqlite:494
- **Dimension:** Security
- **Severity:** High
- **Description:** Text search `contains` value is wrapped with `%` without escaping LIKE meta-characters (`%`, `_`), allowing unintended pattern matching.
- **Recommendation:** Escape `%`, `_`, and `\` in the contains value before wrapping.

---

#### F-013 — Hardcoded Argon2 salt in core rekey

- **File:** `./apps/core/src/rekey.rs`
- **Line(s):** 29
- **Dimension:** Security
- **Severity:** High
- **Description:** Fixed salt `b"life-engine-salt"` for Argon2 KDF. Two databases with the same passphrase derive the same encryption key.
- **Recommendation:** Generate and store a random salt per database.

---

#### F-014 — Storage unconditionally in-memory

- **File:** `./apps/core/src/main.rs`
- **Line(s):** 122
- **Dimension:** Bug Risk
- **Severity:** High
- **Description:** `SqliteStorage::open_in_memory()` is called unconditionally. All data is lost on every restart. The configured `data_dir` is used for storage init state only.
- **Recommendation:** Use configured `data_dir` and `storage.backend` to open file-backed or PostgreSQL storage. Reserve in-memory for testing.

---

#### F-015 — Storage init router auth ordering confusion

- **File:** `./apps/core/src/main.rs`
- **Line(s):** 400-411
- **Dimension:** Security
- **Severity:** High
- **Description:** The storage init router is merged after auth middleware layers, but comments claim it needs no auth. Due to axum's layer ordering, the endpoint IS subject to auth, contradicting the design.
- **Recommendation:** Verify intended auth behavior and adjust merge order or comments accordingly.

---

#### F-016 — Predictable WebAuthn session passphrase

- **File:** `./apps/core/src/auth/webauthn_provider.rs`
- **Line(s):** 362-365
- **Dimension:** Security
- **Severity:** High
- **Description:** WebAuthn uses `format!("webauthn:{user_id}")` as the passphrase for token generation. This is deterministic from user_id, allowing anyone who knows a user_id to mint arbitrary session tokens.
- **Recommendation:** Create a dedicated internal token-minting method that bypasses passphrase verification for WebAuthn sessions.

---

#### F-017 — /api/auth/register not in middleware bypass

- **File:** `./apps/core/src/auth/middleware.rs`
- **Line(s):** 85-93
- **Dimension:** Security
- **Severity:** High
- **Description:** The register endpoint is documented as unauthenticated but is not in the middleware skip list. Requests will be rejected with 401 before reaching the handler.
- **Recommendation:** Add `/api/auth/register` to the bypass list, or update documentation.

---

#### F-018 — Conflict resolver silently drops remote changes

- **File:** `./apps/core/src/conflict.rs`
- **Line(s):** 204-206
- **Dimension:** Bug Risk
- **Severity:** High
- **Description:** `resolve_field_merge` returns `KeepLocal` when merge fails, with comment "Placeholder -- caller should set ManualResolution". Remote changes are silently discarded.
- **Recommendation:** Return a distinct `RequiresManual` variant or `Err` to signal merge failure.

---

#### F-019 — std::sync::RwLock in async context

- **File:** `./apps/core/src/federation.rs`
- **Line(s):** 141-143
- **Dimension:** Bug Risk
- **Severity:** High
- **Description:** `FederationStore` uses `std::sync::RwLock` for peers, sync_history, and cursors. Holding these across `.await` points can deadlock the tokio runtime.
- **Recommendation:** Replace with `tokio::sync::RwLock` or ensure locks are never held across await points.

---

#### F-020 — URL parameter injection in federation sync

- **File:** `./apps/core/src/federation.rs`
- **Line(s):** 376-378
- **Dimension:** Security
- **Severity:** High
- **Description:** `collection` and `cursor` are interpolated into URLs without URL-encoding. A malicious peer could return a crafted cursor altering subsequent requests.
- **Recommendation:** URL-encode parameters using `urlencoding::encode()` or `Url::parse_with_params`.

---

#### F-021 — Rate limiter fallback to 0.0.0.0

- **File:** `./apps/core/src/rate_limit.rs`
- **Line(s):** 90-94
- **Dimension:** Security
- **Severity:** High
- **Description:** When `ConnectInfo<SocketAddr>` is absent (e.g., behind a reverse proxy), all requests share the 0.0.0.0 rate-limit bucket, enabling DoS against all unauthenticated-IP requests.
- **Recommendation:** Fail closed (500) if ConnectInfo absent, or extract IP from `X-Forwarded-For` with a trusted proxy list.

---

#### F-022 — WASM HTTP request headers/body silently dropped

- **File:** `./apps/core/src/wasm_runtime.rs`
- **Line(s):** 429-434
- **Dimension:** Bug Risk
- **Severity:** High
- **Description:** `handle_http_request` accepts `_headers` and `_body` parameters but ignores them (underscore prefix). Plugin HTTP POST/PUT requests are sent without their payload.
- **Recommendation:** Apply headers and body to the request builder before sending.

---

#### F-023 — HTTP domain allowlist suffix bypass

- **File:** `./apps/core/src/wasm_runtime.rs`
- **Line(s):** 441-451
- **Dimension:** Security
- **Severity:** High
- **Description:** Domain check uses `host.ends_with(d)`. If allowlist contains `example.com`, then `evilexample.com` also passes.
- **Recommendation:** Require exact match or match with leading dot: `host == d || host.ends_with(&format!(".{}", d))`.

---

#### F-024 — XML injection in DAV responses

- **File:** `./packages/dav-utils/src/dav_xml.rs`
- **Line(s):** 29-44
- **Dimension:** Security
- **Severity:** High
- **Description:** `write_response_entry` interpolates `href`, `etag`, `content_type`, and `custom` directly into XML via `format!()` without escaping. Characters like `<`, `>`, `&` break XML structure.
- **Recommendation:** XML-escape all interpolated values (`&` → `&amp;`, `<` → `&lt;`, `>` → `&gt;`, `"` → `&quot;`).

---

#### F-025 — iCal TZID timezone silently treated as UTC

- **File:** `./packages/dav-utils/src/ical.rs`
- **Line(s):** 75-89
- **Dimension:** Bug Risk
- **Severity:** High
- **Description:** All TZID-annotated datetimes are returned as UTC without conversion. An event at 10:00 AM Eastern is stored as 10:00 UTC, causing systematic time offsets.
- **Recommendation:** Integrate `chrono-tz` for correct timezone conversion, or indicate in the return type that conversion was not applied.

---

#### F-026 — No auth or rate limiting on storage init

- **File:** `./apps/core/src/routes/storage.rs`
- **Line(s):** 43-100
- **Dimension:** Security
- **Severity:** High
- **Description:** `POST /api/storage/init` has no authentication and no rate limiting. Failed attempts reset the `AtomicBool` guard, allowing unlimited passphrase brute-force retries.
- **Recommendation:** Add rate limiting with exponential backoff on failures. Consider lockout after N failed attempts.

---

#### F-027 — Federation changes endpoint with hard-coded limits

- **File:** `./apps/core/src/routes/federation.rs`
- **Line(s):** 155-161
- **Dimension:** Performance
- **Severity:** High
- **Description:** Fetches up to 1000 records and filters in-memory by `updated_at`. No pagination for the caller. Changes beyond 1000 records are silently dropped.
- **Recommendation:** Push the `since` filter into the storage query and add cursor-based pagination.

---

#### F-028 — N+1 query pattern in GraphQL attendee resolver

- **File:** `./apps/core/src/routes/graphql.rs`
- **Line(s):** 191-221
- **Dimension:** Performance
- **Severity:** High
- **Description:** Each attendee email triggers a separate storage query. Events with many attendees cause N sequential database round-trips.
- **Recommendation:** Use a dataloader pattern or batch lookups into a single query with an OR/IN filter.

---

#### F-029 — Plaintext credential returned in API response

- **File:** `./apps/core/src/routes/credentials.rs`
- **Line(s):** 182-188
- **Dimension:** Security
- **Severity:** High
- **Description:** `GET /api/credentials/{plugin_id}/{key}` returns the raw credential value (password, token, API key) in the JSON response body. Combined with logging middleware or response caching, this is a secret exposure risk.
- **Recommendation:** Ensure HTTPS enforcement, add `Cache-Control: no-store` headers, audit response-logging middleware.

---

#### F-030 — Household role change never persisted

- **File:** `./apps/core/src/routes/household.rs`
- **Line(s):** 256-262
- **Dimension:** Bug Risk
- **Severity:** High
- **Description:** `update_member_role` checks authorization and last-admin guard but never calls a store method to save the role change. Returns success with the new role in JSON but the change is lost.
- **Recommendation:** Call `store.update_member_role()` before returning the success response.

---

#### F-031 — Misleading AES-256-GCM documentation

- **File:** `./plugins/engine/backup/src/crypto.rs`
- **Line(s):** 72-74
- **Dimension:** Documentation
- **Severity:** High
- **Description:** Module doc says "AES-256-GCM for authenticated encryption" but code comment says "In production this would use a proper AES-256-GCM crate." Callers relying on module docs believe they have real AES-GCM protection.
- **Recommendation:** Implement real AES-256-GCM or correct the module documentation to state actual algorithm.

---

#### F-032 — XML injection in CalDAV/CardDAV protocol responses

- **File:** `./plugins/engine/api-caldav/src/protocol.rs`, `./plugins/engine/api-carddav/src/protocol.rs`
- **Line(s):** caldav:48-74, carddav:42-95
- **Dimension:** Security
- **Severity:** High
- **Description:** `build_propfind_xml` and `build_report_xml` interpolate user-controlled values (display_name, etag, content, href) into XML via `format!()` without escaping.
- **Recommendation:** Apply XML escaping to all interpolated values or use an XML builder library.

---

#### F-033 — Path traversal in backup local backend

- **File:** `./plugins/engine/backup/src/backend/local.rs`
- **Line(s):** 22-24
- **Dimension:** Security
- **Severity:** High
- **Description:** `full_path` joins `base_dir` with user-supplied `key` without sanitization. Keys containing `../../etc/passwd` allow reading/writing outside the backup directory.
- **Recommendation:** Validate resolved path stays within `base_dir` using `canonicalize()` and `starts_with()`. Reject keys containing `..`.

---

#### F-034 — Scaffold plugin template has wrong dependency path

- **File:** `./tools/templates/engine-plugin/Cargo.toml`
- **Line(s):** 7
- **Dimension:** Bug Risk
- **Severity:** High
- **Description:** Path `"../../packages/plugin-sdk-rs"` is one level too shallow. Scaffolded plugins at `plugins/engine/<name>/` need `"../../../packages/plugin-sdk-rs"`. Every scaffolded plugin fails `cargo check`.
- **Recommendation:** Change to `path = "../../../packages/plugin-sdk-rs"`.

---

#### F-035 — Build verification test has outdated workspace members

- **File:** `./apps/core/tests/build_verification_test.rs`
- **Line(s):** 96-105
- **Dimension:** Bug Risk
- **Severity:** High
- **Description:** The `expected_members` array is missing 7 workspace members including `packages/test-fixtures`, `packages/dav-utils`, and 5 plugin crates. Gives a false sense of completeness.
- **Recommendation:** Update to match all members in root `Cargo.toml`, or dynamically compare.

---

### Medium

#### F-036 — RateLimiter unbounded memory growth

- **File:** `./apps/core/src/auth/middleware.rs`
- **Line(s):** 28-57
- **Dimension:** Performance
- **Severity:** Medium
- **Description:** `HashMap<IpAddr, Vec<Instant>>` only prunes during `is_rate_limited` for a specific IP. IPs that trigger failures but are never checked again accumulate indefinitely.
- **Recommendation:** Add periodic background cleanup or cap map size.

---

#### F-037 — O(n) linear scan for token validation

- **File:** `./apps/core/src/auth/local_token.rs`
- **Line(s):** 209-217
- **Dimension:** Performance
- **Severity:** Medium
- **Description:** `validate_token` scans all stored tokens to match a hash. Every authenticated request pays O(n) cost.
- **Recommendation:** Add a secondary index mapping `token_hash` to `token_id` for O(1) lookup.

---

#### F-038 — IDOR in passkey deletion

- **File:** `./apps/core/src/auth/routes.rs`
- **Line(s):** 824-873
- **Dimension:** Security
- **Severity:** Medium
- **Description:** `webauthn_delete_passkey` verifies authentication but not that the passkey belongs to the caller. Any authenticated user can delete another user's passkey.
- **Recommendation:** Verify passkey's `user_id` matches the authenticated user before deletion.

---

#### F-039 — WebAuthn user ID uses ephemeral token_id

- **File:** `./apps/core/src/auth/routes.rs`
- **Line(s):** 580
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** WebAuthn user ID is set to `identity.token_id`, a random UUID per session. Users registering passkeys from different sessions get different WebAuthn user IDs, breaking cross-session lookups.
- **Recommendation:** Use a stable `identity.user_id` as the WebAuthn user ID.

---

#### F-040 — Passkey label not preserved in registration

- **File:** `./apps/core/src/auth/routes.rs`
- **Line(s):** 650
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** The user-provided label from `RegisterStartRequest` is not forwarded to `finish_registration`. All passkeys are stored with the token UUID as their label.
- **Recommendation:** Persist the label in challenge state during start, or accept it in finish request.

---

#### F-041 — OIDC/PG secrets in Debug and Serialize

- **File:** `./apps/core/src/config.rs`
- **Line(s):** 128-129, 186
- **Dimension:** Security
- **Severity:** Medium
- **Description:** `OidcSettings.client_secret` and `PostgresSettings.password` are plain `String` with `Debug` and `Serialize` derived. Secrets appear in debug output and config dumps.
- **Recommendation:** Use a `Secret<String>` wrapper that redacts on Debug, or add `#[serde(skip_serializing)]`.

---

#### F-042 — create_dir_all error silently ignored

- **File:** `./apps/core/src/main.rs`
- **Line(s):** 279
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** `let _ = std::fs::create_dir_all(&data_dir)` discards errors. Permission-denied failures cause confusing downstream errors.
- **Recommendation:** Propagate the error or log a warning.

---

#### F-043 — Schema directory path baked at compile time

- **File:** `./apps/core/src/main.rs`
- **Line(s):** 126
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** Schema path uses `env!("CARGO_MANIFEST_DIR")` which is baked into the binary. Won't work in production deployments.
- **Recommendation:** Make configurable at runtime or embed schemas with `include_str!`.

---

#### F-044 — Blanket #[allow(dead_code)] on all modules

- **File:** `./apps/core/src/main.rs`
- **Line(s):** 6-53
- **Dimension:** Code Quality
- **Severity:** Medium
- **Description:** Nearly every module declaration is annotated with `#[allow(dead_code)]`, suppressing warnings for ~20 modules. Genuinely unused code cannot be detected.
- **Recommendation:** Remove blanket annotations; apply to specific items only.

---

#### F-045 — PRAGMA key hex not validated

- **File:** `./apps/core/src/sqlite_storage.rs`
- **Line(s):** 103
- **Dimension:** Security
- **Severity:** Medium
- **Description:** The hex key from Argon2 is interpolated into `PRAGMA key` without asserting it contains only hex characters. A bug in derivation could break the SQL statement.
- **Recommendation:** Assert hex-only characters before interpolation.

---

#### F-046 — Single mutex serializes all SQLite operations

- **File:** `./apps/core/src/sqlite_storage.rs`
- **Line(s):** 64-66
- **Dimension:** Performance
- **Severity:** Medium
- **Description:** `Arc<Mutex<Connection>>` serializes all reads and writes despite WAL mode enabling concurrent readers.
- **Recommendation:** Use connection pooling (`r2d2`) or `RwLock` to allow concurrent reads.

---

#### F-047 — PostgreSQL connections use NoTLS

- **File:** `./apps/core/src/pg_storage.rs`
- **Line(s):** 16
- **Dimension:** Security
- **Severity:** Medium
- **Description:** Database connections always use `NoTls`. Credentials and data are transmitted in plaintext.
- **Recommendation:** Make TLS mode configurable; default to `require` in production.

---

#### F-048 — Migration record count verification unreliable

- **File:** `./apps/core/src/storage_migration.rs`
- **Line(s):** 158
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** Count compares `pg_count` vs `total_records`, but `ON CONFLICT DO NOTHING` skips duplicates from previous attempts, causing false mismatches.
- **Recommendation:** Truncate target table before migration or count per-collection.

---

#### F-049 — No auth provider or storage backend validation

- **File:** `./apps/core/src/config.rs`
- **Line(s):** 592-657
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** `validate()` does not check that OIDC settings are present when `provider: "oidc"`, or that `storage.backend` is a known value. Server starts with invalid config and fails at runtime.
- **Recommendation:** Validate provider-specific and backend-specific settings during config validation.

---

#### F-050 — Unbounded TLS connections

- **File:** `./apps/core/src/main.rs`
- **Line(s):** 443-495
- **Dimension:** Performance
- **Severity:** Medium
- **Description:** Each TLS connection spawns a task with no concurrency limit. A connection flood consumes all memory.
- **Recommendation:** Add a `tokio::sync::Semaphore` to cap concurrent connections.

---

#### F-051 — OIDC env var overrides silently ignored

- **File:** `./apps/core/src/config.rs`
- **Line(s):** 478-486
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** `client_id` and `client_secret` env var overrides only apply if `self.auth.oidc` is already `Some`. Setting only `LIFE_ENGINE_OIDC_CLIENT_ID` without `ISSUER_URL` is silently ignored.
- **Recommendation:** Use `get_or_insert_default` for all OIDC env vars, or document the dependency.

---

#### F-052 — No read/write distinction in household access check

- **File:** `./apps/core/src/household.rs`
- **Line(s):** 302-340
- **Dimension:** Security
- **Severity:** Medium
- **Description:** `check_record_access` has no read/write parameter. Guest users may be allowed to modify shared-collection data if no higher-level write guard exists.
- **Recommendation:** Add a read/write permission parameter to the function.

---

#### F-053 — RwLock unwrap on poisoning causes cascading panics

- **File:** `./apps/core/src/federation.rs`
- **Line(s):** 172-262
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** All `RwLock` accesses use `.unwrap()`. A thread panic while holding a lock poisons it, crashing all subsequent access.
- **Recommendation:** Use `.unwrap_or_else(|e| e.into_inner())` to recover from poisoning.

---

#### F-054 — Non-deterministic JSON for signature verification

- **File:** `./apps/core/src/identity.rs`
- **Line(s):** 429-448
- **Dimension:** Security
- **Severity:** Medium
- **Description:** `verify_token` rebuilds JSON payload for signature check. HashMap key order is non-deterministic; signature verification can fail across processes/platforms.
- **Recommendation:** Use canonical JSON serialization (sorted keys) or `BTreeMap`.

---

#### F-055 — Unbounded sync_history vector

- **File:** `./apps/core/src/federation.rs`
- **Line(s):** 142
- **Dimension:** Performance
- **Severity:** Medium
- **Description:** `sync_history` is an unbounded `Vec<SyncResult>` growing with every sync. `sync_history_for_peer` iterates the entire vector.
- **Recommendation:** Cap to a ring buffer or use a HashMap keyed by peer_id with bounded deques.

---

#### F-056 — No Connector trait lifecycle tests

- **File:** `./apps/core/src/connector.rs`
- **Line(s):** 47
- **Dimension:** Testing
- **Severity:** Medium
- **Description:** The `Connector` trait has no integration tests for the full lifecycle (authenticate → sync → disconnect) or error handling during sync failures.
- **Recommendation:** Add a mock `Connector` and test the full lifecycle including error paths.

---

#### F-057 — New reqwest::Client created per HTTP request

- **File:** `./apps/core/src/wasm_runtime.rs`
- **Line(s):** 457
- **Dimension:** Performance
- **Severity:** Medium
- **Description:** Each WASM plugin HTTP request creates a new `reqwest::Client`, losing connection pooling and repeating TLS handshakes.
- **Recommendation:** Store a `reqwest::Client` in `WasmHostBridge` and reuse it.

---

#### F-058 — Search collection field tokenized as TEXT

- **File:** `./apps/core/src/search.rs`
- **Line(s):** 64
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** The `collection` field uses `TEXT | STORED`, which tokenizes collection names. `"custom_data"` becomes tokens `["custom", "data"]`, causing unexpected search matches.
- **Recommendation:** Use `STRING | STORED` for non-tokenized single-value fields.

---

#### F-059 — Tantivy commit on every single record index

- **File:** `./apps/core/src/search.rs`
- **Line(s):** 90-111
- **Dimension:** Performance
- **Severity:** Medium
- **Description:** `index_record` acquires the writer, adds a document, commits, and reloads the reader on every call. Commits are expensive; bulk imports pay this per record.
- **Recommendation:** Provide a bulk indexing method or allow callers to control commit timing.

---

#### F-060 — Heuristic fetch_count may miss sparse collections

- **File:** `./apps/core/src/search.rs`
- **Line(s):** 135
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** `fetch_count = (offset + limit) * 2 + 100` may be too small for highly selective collection filters, causing incomplete pagination.
- **Recommendation:** Use tantivy BooleanQuery combining text query with term query on collection.

---

#### F-061 — Search processor has no graceful shutdown

- **File:** `./apps/core/src/search_processor.rs`
- **Line(s):** 22-52
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** Background task relies on bus channel closing for shutdown. No cancellation token or abort handle.
- **Recommendation:** Accept a `CancellationToken` and use `tokio::select!` with a shutdown signal.

---

#### F-062 — Federated create ignores remote record ID

- **File:** `./apps/core/src/sync_primitives.rs`
- **Line(s):** 100-106
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** `apply_change` for `Create` calls `storage.create()` which generates a new UUID, ignoring the remote `change.id`. Subsequent updates referencing the original ID fail.
- **Recommendation:** Accept an optional ID in `create()` to preserve remote record IDs for federated sync.

---

#### F-063 — Unbounded plugin log messages

- **File:** `./apps/core/src/wasm_runtime.rs`
- **Line(s):** 414-423
- **Dimension:** Security
- **Severity:** Medium
- **Description:** Log messages from untrusted WASM plugins are unbounded in size. A malicious plugin could flood logs causing disk exhaustion.
- **Recommendation:** Truncate log messages to a maximum length (e.g., 4096 bytes).

---

#### F-064 — Duplicated bridge call boilerplate in wasm_adapter

- **File:** `./apps/core/src/wasm_adapter.rs`
- **Line(s):** 71-209
- **Dimension:** Code Quality
- **Severity:** Medium
- **Description:** Five bridge methods follow identical call/check/convert patterns with ~15 lines of identical boilerplate each.
- **Recommendation:** Extract a private `bridge_call` helper method.

---

#### F-065 — MockStorage triplicated across test modules

- **File:** `./apps/core/src/wasm_adapter.rs`, `wasm_runtime.rs`, `sync_primitives.rs`
- **Line(s):** ~130 lines duplicated 3 times
- **Dimension:** Code Quality
- **Severity:** Medium
- **Description:** Three test modules contain near-identical ~130-line MockStorage implementations.
- **Recommendation:** Extract shared mock into `test_helpers.rs`.

---

#### F-066 — Empty HTTP allowlist permits all domains

- **File:** `./apps/core/src/wasm_runtime.rs`
- **Line(s):** 441-453
- **Dimension:** Security
- **Severity:** Medium
- **Description:** When `allowed_http_domains` is empty, the domain check is skipped entirely. Default config has empty allowlist, so any plugin with `HttpOutbound` can reach any domain.
- **Recommendation:** Block all domains when allowlist is empty (deny by default).

---

#### F-067 — XML namespace injection in DAV multistatus

- **File:** `./packages/dav-utils/src/dav_xml.rs`
- **Line(s):** 58-60
- **Dimension:** Security
- **Severity:** Medium
- **Description:** `extra_namespaces` is interpolated into the XML opening tag without validation or escaping.
- **Recommendation:** Validate that only well-formed namespace declarations are accepted.

---

#### F-068 — Escape sequence ordering bug

- **File:** `./packages/dav-utils/src/text.rs`
- **Line(s):** 49-55
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** `decode_escaped_value` replaces `\\n` before `\\\\`, causing `\\\\n` (literal backslash + n) to be incorrectly decoded as backslash + newline.
- **Recommendation:** Reorder so `\\\\` → `\\` is applied first, or use a state-machine parser.

---

#### F-069 — vCard CRLF not properly escaped

- **File:** `./packages/dav-utils/src/vcard.rs`
- **Line(s):** 14-19
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** `escape_value` escapes `\n` but not `\r\n` sequences. Bare `\r` remains in output, violating RFC 6350.
- **Recommendation:** Add `.replace("\r\n", "\\n")` before `.replace('\n', "\\n")`.

---

#### F-070 — Debug trait exposes credential values

- **File:** `./packages/plugin-sdk-rs/src/credential_store.rs`
- **Line(s):** 17-18
- **Dimension:** Security
- **Severity:** Medium
- **Description:** `StoredCredential` derives `Debug`, printing the `value` field in logs/panics. Module doc says "MUST NEVER log credential values."
- **Recommendation:** Implement manual `Debug` that redacts `value`.

---

#### F-071 — Raw String for HTTP method at WASM boundary

- **File:** `./packages/plugin-sdk-rs/src/wasm_guest.rs`
- **Line(s):** 77-83
- **Dimension:** Security
- **Severity:** Medium
- **Description:** `HostRequest::HttpRequest` uses `method: String` instead of the `HttpMethod` enum, allowing arbitrary method strings through the WASM boundary.
- **Recommendation:** Use the `HttpMethod` enum from `types.rs`.

---

#### F-072 — Credential dates stored as unvalidated strings

- **File:** `./packages/types/src/credentials.rs`
- **Line(s):** 24, 26
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** `issued_date` and `expiry_date` are `String` type. Any arbitrary string passes deserialization without validation.
- **Recommendation:** Use `chrono::NaiveDate` with serde validation.

---

#### F-073 — Fragile SMTP EHLO response parsing

- **File:** `./packages/test-utils/src/connectors.rs`
- **Line(s):** 355-470
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** Uses `set_nonblocking` toggling to drain EHLO responses. Buffered data and nonblocking state can desync.
- **Recommendation:** Parse properly by reading until `250 ` (space, not dash) per RFC 5321.

---

#### F-074 — Entire file read into memory for SHA-256

- **File:** `./packages/types/src/file_helpers.rs`
- **Line(s):** 56-57
- **Dimension:** Performance
- **Severity:** Medium
- **Description:** `compute_sha256` uses `fs::read(path)` loading the entire file. Large files cause high memory usage.
- **Recommendation:** Stream through `BufReader` in 8KB chunks.

---

#### F-075 — Raw errors leaked to API clients

- **File:** `./apps/core/src/routes/connectors.rs`, `./apps/core/src/routes/storage.rs`
- **Line(s):** connectors:117, storage:94
- **Dimension:** Security
- **Severity:** Medium
- **Description:** Error responses include raw error strings that could leak internal paths, connection strings, or stack traces.
- **Recommendation:** Return generic error messages; keep details in server-side logs.

---

#### F-076 — Weak passphrase minimum (8 characters)

- **File:** `./apps/core/src/routes/storage.rs`
- **Line(s):** 61
- **Dimension:** Security
- **Severity:** Medium
- **Description:** 8-character minimum for master encryption passphrase is weak per NIST guidelines for high-value secrets.
- **Recommendation:** Increase to 12+ characters or add entropy-based validation.

---

#### F-077 — expect() panic after conflict resolve

- **File:** `./apps/core/src/routes/conflicts.rs`
- **Line(s):** 141
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** `store.get(&id).expect("just resolved")` panics if a race condition removes the conflict between resolve and get.
- **Recommendation:** Return 500 error instead of panicking.

---

#### F-078 — Brittle string matching for error classification

- **File:** `./apps/core/src/routes/data.rs`, `./apps/core/src/routes/graphql.rs`
- **Line(s):** data:274-284, graphql:1086-1094
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** Error handling uses `msg.contains("version mismatch")` and `msg.contains("not found")` to determine HTTP status codes. Breaks silently if messages change.
- **Recommendation:** Use typed error variants from the storage layer.

---

#### F-079 — Invalid `since` timestamp silently treated as None

- **File:** `./apps/core/src/routes/federation.rs`
- **Line(s):** 170-176
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** When `since` is non-empty but not valid RFC 3339, it is silently treated as None, returning all records.
- **Recommendation:** Return 400 Bad Request for invalid `since` values.

---

#### F-080 — N+1 attachment file queries in GraphQL

- **File:** `./apps/core/src/routes/graphql.rs`
- **Line(s):** 307-322
- **Dimension:** Performance
- **Severity:** Medium
- **Description:** `attachment_files` resolver performs a separate `storage.get()` for each attachment.
- **Recommendation:** Batch file lookups into a single query.

---

#### F-081 — user_id injection when identity absent

- **File:** `./apps/core/src/routes/data.rs`
- **Line(s):** 190-200
- **Dimension:** Security
- **Severity:** Medium
- **Description:** The handler injects `_user_id` from identity, but if `identity` is `None`, a client-provided `_user_id` passes through unmodified, allowing impersonation.
- **Recommendation:** Always strip `_user_id` and `_household_id` from incoming body regardless of identity presence.

---

#### F-082 — GraphQL Playground exposed unconditionally

- **File:** `./apps/core/src/routes/graphql.rs`
- **Line(s):** 1243-1247
- **Dimension:** Security
- **Severity:** Medium
- **Description:** The GraphQL Playground is always available, providing an interactive query interface to explore the schema and execute arbitrary queries in production.
- **Recommendation:** Disable in production or gate behind auth middleware and an environment variable.

---

#### F-083 — Seven duplicated record_to_* conversion functions

- **File:** `./apps/core/src/routes/graphql.rs`
- **Line(s):** 494-763
- **Dimension:** Code Quality
- **Severity:** Medium
- **Description:** `record_to_task`, `record_to_contact`, `record_to_event`, etc. follow identical patterns with duplicated timestamp parsing and extensions handling.
- **Recommendation:** Extract common field-mapping logic into a shared helper.

---

#### F-084 — No HTTP-level federation route tests

- **File:** `./apps/core/src/routes/federation.rs`
- **Line(s):** 220-272
- **Dimension:** Testing
- **Severity:** Medium
- **Description:** Federation tests only test the store directly, not HTTP handlers. Auth middleware and request/response serialization are untested.
- **Recommendation:** Add HTTP-level integration tests matching the pattern in other route modules.

---

#### F-085 — Minimal household route test coverage

- **File:** `./apps/core/src/routes/household.rs`
- **Line(s):** 369-440
- **Dimension:** Testing
- **Severity:** Medium
- **Description:** Tests only verify auth is required. No tests for create, invite, accept, role management, shared collections, or last-admin guard.
- **Recommendation:** Add integration tests for the complete household lifecycle.

---

#### F-086 — Mutex held during slow plugin sync IO

- **File:** `./apps/core/src/routes/connectors.rs`
- **Line(s):** 18
- **Dimension:** Performance
- **Severity:** Medium
- **Description:** `state.plugin_loader.lock().await` is held while calling `plugin.handle_route().await`, which may perform slow network I/O. Blocks all other plugin access.
- **Recommendation:** Clone the needed plugin reference, drop the lock, then call handle_route.

---

#### F-087 — URL path traversal in WebDAV backup backend

- **File:** `./plugins/engine/backup/src/backend/webdav.rs`
- **Line(s):** 31-34
- **Dimension:** Security
- **Severity:** Medium
- **Description:** URL constructed via string concatenation without encoding the key parameter.
- **Recommendation:** URL-encode the key or validate it does not contain traversal sequences.

---

#### F-088 — Secrets in Debug/Serialize across webhook and backup plugins

- **File:** `./plugins/engine/backup/src/types.rs`, `./plugins/engine/webhook-receiver/src/models.rs`, `./plugins/engine/webhook-sender/src/models.rs`
- **Line(s):** backup:12, receiver:19, sender:16
- **Dimension:** Security
- **Severity:** Medium
- **Description:** Passphrase and secret fields derive `Debug` and `Serialize`, exposing secrets in logs and API responses.
- **Recommendation:** Add `#[serde(skip_serializing)]` on secret fields and implement custom `Debug`.

---

#### F-089 — Backup manifest stats inconsistency

- **File:** `./plugins/engine/backup/src/engine.rs`
- **Line(s):** 70-78
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** The manifest embedded in `BackupArchive` has `compressed_size: 0` and empty `checksum`, differing from the sidecar `.manifest.json`.
- **Recommendation:** Document that sidecar manifest is authoritative, or update archive manifest before serialization.

---

#### F-090 — No hour/day range validation in backup schedule

- **File:** `./plugins/engine/backup/src/schedule.rs`
- **Line(s):** 13-14
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** `hour` (u32) and `day` (u32) are not validated against 0-23 and 0-6 respectively. Invalid values produce invalid cron expressions that silently return None.
- **Recommendation:** Validate ranges at construction time and return an error.

---

#### F-091 — Naive WebDAV XML parser

- **File:** `./plugins/engine/backup/src/backend/webdav.rs`
- **Line(s):** 142-164
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** Line-by-line XML parsing for `<href>` tags fails on multi-line elements, attributes, or varied namespace prefixes. Size and last_modified are hardcoded to defaults.
- **Recommendation:** Use a proper XML parser like `quick-xml` or `roxmltree`.

---

#### F-092 — Missing PROPFIND/REPORT routes in CalDAV/CardDAV plugins

- **File:** `./plugins/engine/api-caldav/src/lib.rs`, `./plugins/engine/api-carddav/src/lib.rs`
- **Line(s):** caldav:75-100, carddav:75-98
- **Dimension:** Consistency
- **Severity:** Medium
- **Description:** Only GET/PUT/DELETE routes are registered. PROPFIND and REPORT are the core WebDAV/CalDAV/CardDAV methods that clients need.
- **Recommendation:** Verify SDK supports these methods and register appropriate routes.

---

#### F-093 — N+1 manifest fetches in backup listing

- **File:** `./plugins/engine/backup/src/engine.rs`
- **Line(s):** 223-238
- **Dimension:** Performance
- **Severity:** Medium
- **Description:** `list_backups` calls `backend.get()` individually for each `.manifest.json`. On remote backends (S3, WebDAV), each is an HTTP request.
- **Recommendation:** Batch-fetch manifest data or filter before individual gets.

---

#### F-094 — No BackupPlugin unit tests

- **File:** `./plugins/engine/backup/src/lib.rs`
- **Line(s):** 20-103
- **Dimension:** Testing
- **Severity:** Medium
- **Description:** No test module for plugin metadata, capabilities, routes, or lifecycle.
- **Recommendation:** Add tests matching patterns in other plugin files.

---

#### F-095 — CI check test uses invalid --rust-only flag

- **File:** `./apps/core/tests/ci_check_test.rs`
- **Line(s):** 93
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** Script only recognizes `--quick` and `--help`. `--rust-only` causes `exit 1`.
- **Recommendation:** Change to `--quick` or add `--rust-only` to the script.

---

#### F-096 — Pocket ID image mismatch across compose files

- **File:** `./deploy/docker-compose.full.yml`
- **Line(s):** 27
- **Dimension:** Consistency
- **Severity:** Medium
- **Description:** `stonith404/pocket-id:latest` vs `ghcr.io/pocket-id/pocket-id:latest` in dev compose. Different registries could diverge.
- **Recommendation:** Standardize on one image reference.

---

#### F-097 — OIDC issuer URL port mismatch in deploy compose

- **File:** `./deploy/docker-compose.full.yml`
- **Line(s):** 11
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** `LIFE_ENGINE_OIDC_ISSUER_URL=http://pocket-id:3751` may not match the container's internal port.
- **Recommendation:** Verify pocket-id internal port and adjust accordingly.

---

#### F-098 — Hardcoded MinIO dev credentials

- **File:** `./docker-compose.yml`
- **Line(s):** 34-36
- **Dimension:** Security
- **Severity:** Medium
- **Description:** `minioadmin/minioadmin` with no mechanism preventing accidental deployment.
- **Recommendation:** Use env var references with defaults: `${MINIO_ROOT_USER:-minioadmin}`.

---

#### F-099 — Dockerfile missing HEALTHCHECK

- **File:** `./apps/core/Dockerfile`
- **Line(s):** 1-36
- **Dimension:** Security
- **Severity:** Medium
- **Description:** No HEALTHCHECK instruction for standalone container use.
- **Recommendation:** Add `HEALTHCHECK CMD wget --spider -q http://localhost:3750/api/system/health || exit 1`.

---

#### F-100 — Dockerfile has no dependency caching layer

- **File:** `./apps/core/Dockerfile`
- **Line(s):** 1
- **Dimension:** Performance
- **Severity:** Medium
- **Description:** Any source change triggers full `cargo build --release` including all dependencies.
- **Recommendation:** Add a dummy build step that caches dependency compilation.

---

#### F-101 — No upper bound on list records limit parameter

- **File:** `./apps/core/openapi.yaml`
- **Line(s):** 48-49
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** A client can request `limit=999999999`, potentially loading unbounded records into memory.
- **Recommendation:** Add `maximum: 1000` to the limit parameter schema.

---

#### F-102 — Deprecated launchctl load in install script

- **File:** `./deploy/install.sh`
- **Line(s):** 78
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** `launchctl load` is deprecated since macOS 10.10. Modern equivalent is `launchctl bootstrap`.
- **Recommendation:** Use `launchctl bootstrap "gui/$(id -u)"` with fallback.

---

#### F-103 — Fragile scaffold plugin script

- **File:** `./tools/scripts/scaffold-plugin.sh`
- **Line(s):** 47-64
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** Awk workspace member injection could misfire if other arrays have `]` on own line. Sed substitution breaks if plugin name contains special chars.
- **Recommendation:** Add input validation (`[a-z][a-z0-9-]*`) and reset awk state after insertion.

---

#### F-104 — Branch protection allows admin bypass

- **File:** `./tools/scripts/configure-branch-protection.sh`
- **Line(s):** 70
- **Dimension:** Security
- **Severity:** Medium
- **Description:** `enforce_admins: false` lets repository admins bypass all protections.
- **Recommendation:** Set to `true` unless documented otherwise.

---

#### F-105 — Meaningless schema validation test

- **File:** `./apps/core/tests/schema_validation_test.rs`
- **Line(s):** 330-350
- **Dimension:** Testing
- **Severity:** Medium
- **Description:** Test catches known violations but only prints them. Passes whether schema validates or not.
- **Recommendation:** Assert `is_err()` so the test fails if violations are fixed.

---

#### F-106 — Conditional required not enforced in OpenAPI

- **File:** `./apps/core/openapi.yaml`
- **Line(s):** 1004-1013
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** `merged_data` is described as "Required when resolution is merge" but is always optional in the schema.
- **Recommendation:** Use `oneOf`/`if-then` constructs, or document server-side enforcement.

---

#### F-107 — O(n^2) key collection in conflict merge

- **File:** `./apps/core/src/conflict.rs`
- **Line(s):** 155-159
- **Dimension:** Performance
- **Severity:** Medium
- **Description:** Uses `Vec::contains` for deduplication during field merge, which is O(n) per lookup.
- **Recommendation:** Use `HashSet` for unique key collection.

---

#### F-108 — Dockerfile missing COPY for workspace packages

- **File:** `./apps/core/Dockerfile`
- **Line(s):** 14
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** Does not copy `packages/dav-utils/` or `packages/test-fixtures/`, which are workspace members and dependencies. Build may fail.
- **Recommendation:** Add COPY instructions for all required workspace packages.

---

### Low

The following low-severity findings are grouped by dimension. These should be addressed when touching the affected files.

#### Bug Risk (23 findings)

- **F-109** `./apps/core/src/auth/oidc.rs:158-161` — Uses let-chains (edition 2024 or nightly feature). Verify MSRV compatibility.
- **F-110** `./apps/core/src/auth/local_token.rs:263` — `expires_in_days: Some(0)` creates immediately-expired token. No minimum validation.
- **F-111** `./apps/core/src/storage_migration.rs:141` — `offset` is `u32`; overflows if records exceed 4.3 billion.
- **F-112** `./apps/core/src/config.rs:336-337` — Default `data_dir` `"~/.life-engine/data"` uses tilde which Rust doesn't auto-expand.
- **F-113** `./apps/core/src/main.rs:253-256` — Unparseable CORS origins silently dropped.
- **F-114** `./apps/core/src/manifest.rs:234-265` — `validate_reverse_domain_id` only checks first char; allows invalid chars in segments.
- **F-115** `./apps/core/src/rate_limit.rs:78-81` — Depends on `into_make_service_with_connect_info` at startup; not enforced.
- **F-116** `./apps/core/src/search.rs:65` — `plugin_id` is `STORED` only, cannot be searched/filtered.
- **F-117** `./apps/core/src/search_processor.rs:42-43` — Lagged broadcast messages permanently lost; records never indexed until next update.
- **F-118** `./apps/core/src/plugin_signing.rs:243` — `expect()` on hex decode could panic if validation has a bug.
- **F-119** `./apps/core/src/plugin_loader.rs:134-140` — Strict semver rejects valid pre-release versions.
- **F-120** `./apps/core/src/sync_primitives.rs:134-138` — Delete doesn't check remote version against local; stale deletes bypass LWW.
- **F-121** `./apps/core/src/routes/graphql.rs:1186-1194` — `RecordDeleted` uses `collection: "unknown"`.
- **F-122** `./apps/core/src/routes/identity.rs:317` — `ttl_hours: Option<i64>` allows negative values creating past-expiry tokens.
- **F-123** `./apps/core/src/routes/storage.rs:48` — TOCTOU window in `AtomicBool` swap for concurrent init requests.
- **F-124** `./apps/core/src/routes/federation.rs:93-96` — `store.update_peer_status()` and `store.record_sync()` return values ignored.
- **F-125** `./packages/dav-utils/src/ical.rs:61-65` — Confusing error when `VALUE=DATE` with datetime-format value.
- **F-126** `./packages/dav-utils/src/text.rs:24` — `&line[1..]` relies on implicit single-byte assumption for continuation markers.
- **F-127** `./packages/dav-utils/src/url.rs:21-25` — `join_dav_url` only normalizes boundary, not internal double slashes.
- **F-128** `./packages/types/src/events.rs:9-31` — No validation that `start` < `end` on `CalendarEvent`.
- **F-129** `./plugins/engine/backup/src/schedule.rs:18-19` — Malformed cron expression silently returns None; backups never run.
- **F-130** `./plugins/engine/backup/src/backend/webdav.rs:115` — Non-207 success code parsed as multistatus XML, producing empty results.
- **F-131** `./plugins/engine/backup/src/retention.rs:19` — Assumes manifests pre-sorted newest-first; not enforced.

#### Security (10 findings)

- **F-132** `./apps/core/src/config.rs:930-932` — Wildcard CORS (`"*"`) validates without warning in config.
- **F-133** `./apps/core/src/plugin_signing.rs:102-103` — Revocation list case-sensitive; uppercase hex won't match lowercase from `hex::encode`.
- **F-134** `./apps/core/src/test_helpers.rs:37` — Module not `#[cfg(test)]` gated; weak test passphrase could leak to non-test builds.
- **F-135** `./packages/test-utils/src/connectors.rs:435-448` — SMTP responses not validated; rejected recipients pass silently.
- **F-136** `./plugins/engine/backup/src/backend/s3.rs:10-17` — `secret_access_key` in `Serialize`/`Debug`.
- **F-137** `./plugins/engine/connector-filesystem/src/s3.rs:15-29` — `secret_access_key` in `Serialize`/`Debug`.
- **F-138** `./plugins/engine/backup/src/backend/webdav.rs:10-17` — `password` in `Serialize`/`Debug`.
- **F-139** `./apps/core/openapi.yaml:700-701` — No pattern constraint on `collection` path parameter.
- **F-140** `./docker-compose.test.yml:22-23` — Hardcoded MinIO test credentials; should be documented as test-only.
- **F-141** `./deny.toml:13` — RUSTSEC-2023-0071 (RSA timing sidechannel) suppressed; dev-only.

#### Code Quality (14 findings)

- **F-142** `./apps/core/src/auth/mod.rs:7-18` — Blanket `#[allow(dead_code, unused_imports)]` on auth submodules.
- **F-143** `./apps/core/src/auth/jwt.rs:96-98` — `len()` without `is_empty()` on `SyncJwksCache`.
- **F-144** `./apps/core/src/auth/jwt.rs:186-251` — Duplicate JWKS cache implementations; `SyncJwksCache` unused outside tests.
- **F-145** `./apps/core/src/error.rs:10` — Blanket `#[allow(dead_code)]` on entire `CoreError` enum.
- **F-146** `./apps/core/src/credential_store.rs:188` — `.filter_map(|r| r.ok())` silently drops row deserialization errors.
- **F-147** `./apps/core/src/household.rs:328-331` — `user_hid` bound but never read; should use `_` prefix.
- **F-148** `./apps/core/src/conflict.rs:11` — Uses `std::sync::Mutex` while rest of codebase uses `tokio::sync`.
- **F-149** `./apps/core/src/shutdown.rs:15` — `SHUTDOWN_TIMEOUT_SECS` hardcoded to 5 with no config option.
- **F-150** `./apps/core/src/schema_registry.rs:293-340` — Quarantine logic duplicated between `validated_create` and `validated_update`.
- **F-151** `./apps/core/src/routes/household.rs:407-428` — Local test helpers duplicate `crate::test_helpers`.
- **F-152** `./apps/core/src/routes/identity.rs:500-511` — Dead `setup_state` function with `unreachable!()`.
- **F-153** `./apps/core/src/routes/events.rs:69` — `serde_json::to_string().unwrap_or_default()` silently produces empty SSE data.
- **F-154** `./apps/core/src/routes/events.rs:46-78` — Redundant event type filtering across 3 branches.
- **F-155** `./plugins/engine/connector-calendar/src/caldav.rs:193-211` — Public stub methods silently no-op.

#### Performance (4 findings)

- **F-156** `./apps/core/src/search.rs:291-306` — `flatten_strings` recursively allocates; no depth limit.
- **F-157** `./plugins/engine/connector-filesystem/src/local.rs:165` — `watch_paths.clone()` on every `scan()`.
- **F-158** `./plugins/engine/backup/src/engine.rs:32-39` — `Vec::contains` for collection dedup.
- **F-159** `./plugins/engine/webhook-sender/src/delivery.rs:15` — `DeliveryLog` unbounded in-memory vector.

#### Testing (10 findings)

- **F-160** `./apps/core/src/routes/search.rs:89-100` — Search tests skip auth middleware.
- **F-161** `./apps/core/src/search_processor.rs:88+` — Tests use `sleep(50ms)` for timing; flaky on slow CI.
- **F-162** `./packages/test-utils/src/docker.rs:191-203` — `skip_unless_docker!` only checks GreenMail, not other services.
- **F-163** `./packages/test-fixtures/src/lib.rs:97-98` — Schema validation uses brittle `include_str!` with `../../../` traversal.
- **F-164** `./apps/core/tests/dev_environment_test.rs:23` — Checks for `node`/`pnpm` in a predominantly Rust project.
- **F-165** `./apps/core/tests/dev_environment_test.rs:100-114` — Tests for nx targets that may be stale after scaffold simplification.
- **F-166** `./plugins/engine/connector-email/tests/greenmail_integration.rs:80-81` — Mixes async-std and tokio runtimes.
- **F-167** `./plugins/engine/connector-filesystem/tests/s3_integration.rs:152-178` — `Drop` spawns OS thread for async cleanup; `block_on` could deadlock.
- **F-168** `./registry/plugin-registry.json` — No validation test for registry format.
- **F-169** `./apps/core/tests/schema_validation_test.rs:319-327` — References `plugins/life/` and `tools/templates/life-plugin-*` paths that may not exist.

#### Consistency (8 findings)

- **F-170** `./apps/core/src/sqlite_storage.rs:148-174` — Missing `user_id`/`household_id` columns in both SQLite and PG schemas.
- **F-171** `./packages/plugin-sdk-rs/src/wasm_guest.rs:78` — `method: String` vs `HttpMethod` enum inconsistency.
- **F-172** `./packages/types/src/tasks.rs:39` — `labels` missing `skip_serializing_if = "Vec::is_empty"` unlike all other CDM types.
- **F-173** `./packages/types/src/credentials.rs:18-32` — Only CDM type missing `extensions` field.
- **F-174** `./apps/core/src/routes/mod.rs:1-18` — Module declarations not alphabetically sorted.
- **F-175** `./Cargo.toml:27` vs `openapi.yaml:10` — License mismatch: Apache-2.0 vs AGPL-3.0.
- **F-176** `./apps/core/Cargo.toml:29,41` — `directories` and `base64` specified directly instead of via workspace.
- **F-177** `./apps/core/tests/dev_environment_test.rs:88` — pocket-id asymmetry between dev and test compose.

#### Documentation (6 findings)

- **F-178** `./apps/core/src/identity.rs:563-566` — `generate_did` claims `did:key` method but implementation is non-compliant.
- **F-179** `./apps/core/src/auth/routes.rs:668-692` — Error classification relies on string matching instead of structured errors.
- **F-180** `./plugins/engine/api-caldav/src/serializer.rs:15-53` — No iCalendar RFC 5545 line folding on output.
- **F-181** `./plugins/engine/api-carddav/src/serializer.rs:15-64` — No vCard RFC 6350 line folding on output.
- **F-182** `./plugins/engine/connector-contacts/src/normalizer.rs:60-63` — `TYPE=pref` not parsed from comma-separated TYPE values.
- **F-183** `./plugins/engine/api-carddav/src/serializer.rs:120-123` — PREF parameter handling inconsistent between vCard 3.0 and 4.0.

---

### Info / Observations

#### F-184 — ROPC grant deprecated in OAuth 2.1

- **File:** `./apps/core/src/auth/routes.rs`
- **Line(s):** 198-264
- **Dimension:** Security
- **Observation:** OIDC login uses Resource Owner Password Credentials grant, deprecated in OAuth 2.1. Consider migrating to Authorization Code flow with PKCE.

#### F-185 — WebAuthn challenges cleanup not wired

- **File:** `./apps/core/src/auth/webauthn_provider.rs`
- **Line(s):** 66-77
- **Dimension:** Performance
- **Observation:** `challenges` map grows without bound. Doc says "call cleanup periodically" but no code invokes it.

#### F-186 — No persistent WebAuthn credential store

- **File:** `./apps/core/src/auth/webauthn_store.rs`
- **Line(s):** 120-242
- **Dimension:** Testing
- **Observation:** Only `InMemoryWebAuthnStore` exists. Passkey registrations are lost on restart.

#### F-187 — Sync SQLite operations on async runtime

- **File:** `./apps/core/src/auth/local_token.rs`
- **Line(s):** 92-147
- **Dimension:** Bug Risk
- **Observation:** Blocking rusqlite operations under `tokio::sync::Mutex` can stall the async runtime under load.

#### F-188 — Message bus capacity documentation gap

- **File:** `./apps/core/src/message_bus.rs`
- **Line(s):** 10
- **Dimension:** Code Quality
- **Observation:** `DEFAULT_CAPACITY = 256`. No guidance for subscribers on handling `RecvError::Lagged`.

#### F-189 — No shared MockStorage in test_helpers

- **File:** `./apps/core/src/test_helpers.rs`
- **Line(s):** 1-80
- **Dimension:** Testing
- **Observation:** Three test modules implement independent ~130-line MockStorage implementations. A shared mock would reduce duplication.

#### F-190 — PluginRoute handler placeholder

- **File:** `./packages/plugin-sdk-rs/src/types.rs`
- **Line(s):** 66-68
- **Dimension:** Documentation
- **Observation:** Comment indicates handler definition deferred to "Phase 1 when WASM host function interface is finalised."

#### F-191 — Nightly let_chains in dav-utils

- **File:** `./packages/dav-utils/src/ical.rs`
- **Line(s):** 77-78
- **Dimension:** Code Quality
- **Observation:** Uses `if let ... && let ...` syntax requiring Rust 2024 edition or nightly `let_chains` feature.

#### F-192 — TLS supports single certificate only

- **File:** `./apps/core/src/tls.rs`
- **Line(s):** 56-58
- **Dimension:** Security
- **Observation:** No multi-domain SNI or ECDSA+RSA dual-cert support. Fine for initial implementation.

#### F-193 — Passphrases not zeroed from memory

- **File:** `./apps/core/src/rekey.rs`
- **Line(s):** 155-162
- **Dimension:** Security
- **Observation:** `drop()` calls don't guarantee memory zeroing. Use `zeroize` crate for secure erasure.

#### F-194 — Template plugin has hardcoded ID and display_name

- **File:** `./tools/templates/engine-plugin/src/lib.rs`
- **Line(s):** 14, 26
- **Dimension:** Code Quality
- **Observation:** Scaffold script replaces `MyPlugin` but not `"com.example.my-plugin"` or `"My Plugin"`. All scaffolded plugins share these defaults.

#### F-195 — OpenAPI spec doesn't document rate limiting

- **File:** `./apps/core/openapi.yaml`
- **Line(s):** 231-260
- **Dimension:** Security
- **Observation:** No documentation of rate limiting behavior on `POST /api/auth/token`, despite code using `governor` crate.

#### F-196 — rand crate at 0.8, 0.9 available

- **File:** `./Cargo.toml`
- **Line(s):** 47
- **Dimension:** Security
- **Observation:** Consider upgrading to `rand = "0.9"` for security-critical crate currency.

---

## Technical Debt Markers

- `./apps/core/tests/schema_validation_test.rs:334` — `// TODO: Update calendar plugin.json or manifest schema to align`

## Suppressed Lint Rules

- `./apps/core/src/main.rs:8-52` — 20x `#[allow(dead_code)]` on module declarations
- `./apps/core/src/error.rs:10` — `#[allow(dead_code)]` on entire `CoreError` enum
- `./apps/core/src/auth/mod.rs:7-18` — `#[allow(dead_code)]` and `#[allow(dead_code, unused_imports)]` on auth submodules
- `./apps/core/src/auth/oidc.rs:108,288` — `#[allow(dead_code)]` on fields and types
- `./apps/core/src/auth/types.rs:12,74` — `#[allow(dead_code)]` on types
- `./apps/core/src/auth/webauthn_provider.rs:424` — `#[allow(dead_code)]` on test struct
- `./apps/core/src/auth/routes.rs:521` — `#[allow(clippy::result_large_err)]`
- `./apps/core/src/pg_storage.rs:95` — `#[allow(dead_code)]`
- `./apps/core/src/sqlite_storage.rs:71,93,116` — `#[allow(dead_code)]` on fields
- `./apps/core/src/schema_registry.rs:198,215,283,292,343` — `#[allow(dead_code)]` on test structs
- `./plugins/engine/connector-calendar/src/caldav.rs:193,200,207` — `#[allow(dead_code)]` on stub methods
- `./plugins/engine/connector-calendar/src/normalizer.rs:155` — `#[allow(clippy::type_complexity)]`
- `./plugins/engine/connector-contacts/src/google.rs:96` — `#[allow(dead_code)]`
- `./plugins/engine/backup/src/lib.rs:22` — `#[allow(dead_code)]`
- `./plugins/engine/backup/src/backend/s3.rs:29` — `#[cfg_attr(not(feature = "integration"), allow(dead_code))]`
