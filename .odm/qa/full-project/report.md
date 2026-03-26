# QA Inspection Report — Full Project

- **Date** — 2026-03-27
- **Scope** — Full codebase (158 files inspected across 9 inspection groups + supplementary scans)
- **Methodology** — Static analysis, manual code review, cross-group deduplication

## Summary

- **Critical** — 0
- **High** — 13
- **Medium** — 69 (70 IDs; F-076 merged into F-063)
- **Low** — 56
- **Info** — 7
- **Total** — 146 finding IDs (145 unique)

---

## High Severity

- **F-001** — `./apps/core/src/auth/middleware.rs:128-140` — Security — X-Forwarded-For header trusted unconditionally, allowing rate-limit bypass via IP spoofing. Only trust when `behind_proxy` is configured.

- **F-002** — `./apps/core/src/auth/webauthn_provider.rs:362-368` — Security — Each WebAuthn auth generates a random UUID passphrase for `local_token_provider.generate_token()`. First call sets master passphrase; subsequent calls fail since the random passphrase won't match. Pre-set the local provider passphrase or use a dedicated token generation method.

- **F-003** — `./apps/core/src/main.rs:697-703` — Bug Risk — WAL checkpoint runs after server stops but potentially before in-flight requests complete. Ensure shutdown waits for request drain before storage teardown.

- **F-004** — `./apps/core/src/pg_storage.rs:116` — Bug Risk — `root_store.add(cert).ok()` silently discards certificate loading errors. Log warning or propagate error.

- **F-005** — `./apps/core/src/rate_limit.rs:112-124` — Security — X-Forwarded-For header trusted unconditionally (same pattern as F-001 in a different rate limiter module). Same fix applies.

- **F-006** — `./apps/core/src/rekey.rs:87-92` — Security — `derive_key()` uses all-zero salt as fallback, eliminating salting benefit. Mark as `#[cfg(test)]` or remove, requiring callers to use `derive_key_with_salt`.

- **F-007** — `./apps/core/src/routes/credentials.rs:183` — Security — Any authenticated user can retrieve any plugin's credentials by guessing `plugin_id` and key. Add plugin-scoped access control.

- **F-008** — `./apps/core/src/routes/federation.rs:32,47,244-268` — Security / Bug Risk — Federation endpoints have no auth requirement (unauthenticated attacker could register rogue peers or exfiltrate data). Additionally, `serde_json::to_value().unwrap()` in handlers can panic on serialization failure. Protect routes with auth middleware and replace `unwrap()` with `?` or error response.

- **F-009** — `./apps/core/src/routes/storage.rs:59` — Security — All clients share a single rate-limit bucket using hardcoded `IpAddr::V4(Ipv4Addr::UNSPECIFIED)`, making per-IP limiting entirely ineffective. Extract actual client IP from request.

- **F-010** — `./apps/core/src/storage_migration.rs:99` — Bug Risk — `offset as u32` silently truncates, causing migration loop wrap-around on large collections. Change `Pagination.offset` to u64 or add bounds check.

- **F-011** — `./packages/storage-sqlite/src/backend.rs:183` — Security — Sort field string-interpolated into SQL ORDER BY clause. If field contains single quote, SQL injection is possible. Also subsumes inconsistent parameterization at line 135. Validate field names to alphanumeric + underscore + dots, or parameterize.

- **F-012** — `./packages/storage-sqlite/src/credentials.rs:44` — Bug Risk — JSON indexing on missing `"claims"` returns Null, which gets encrypted as `null`, silently corrupting credential data. Use `doc.get("claims").ok_or_else(...)` to fail fast.

- **F-013** — `./plugins/engine/backup/src/crypto.rs:16-17` — Security — Fixed salt `b"life-engine-salt"` means all installations with same passphrase derive same key. Generate random salt per backup/installation.

---

## Medium Severity

- **F-014** — `./apps/core/src/auth/local_token.rs:67-70` — Performance — SQLite mutex serialization bottleneck in token validation.

- **F-015** — `./apps/core/src/auth/local_token.rs:252-265` — Security — No passphrase complexity enforcement on first set.

- **F-016** — `./apps/core/src/auth/mod.rs:61-69` — Bug Risk — `MultiAuthProvider` returns last error, discarding more relevant earlier errors.

- **F-017** — `./apps/core/src/auth/oidc.rs:18-33` — Security — `OidcConfig` `client_secret` exposed via Serialize/Debug derives.

- **F-018** — `./apps/core/src/auth/oidc.rs:227-234` — Bug Risk — Missing `exp` claim defaults to now, producing immediately-expired tokens.

- **F-019** — `./apps/core/src/auth/routes.rs:693-717` — Maintainability — String-based error classification for WebAuthn errors.

- **F-020** — `./apps/core/src/auth/routes.rs:722-761` — Security — WebAuthn authenticate-start allows user enumeration.

- **F-021** — `./apps/core/src/config.rs:940-960` — Security — `storage.passphrase` not redacted in config output.

- **F-022** — `./apps/core/src/config.rs:1350-1373` — Bug Risk — Env override underscore-to-key mapping is ambiguous.

- **F-023** — `./apps/core/src/crypto.rs:18-24` — Security — HKDF without salt.

- **F-024** — `./apps/core/src/crypto.rs:30-31` — Bug Risk — `encrypt` panics on non-32-byte key.

- **F-025** — `./apps/core/src/pg_storage.rs:115-116` — Bug Risk — `expect()` on `load_native_certs` panics in minimal containers.

- **F-026** — `./apps/core/src/pg_storage.rs:605-608` — Security — `sort_by` interpolated in PG query (same pattern as F-011 in sqlite, but validated differently).

- **F-027** — `./apps/core/src/pg_storage.rs:747` — Bug Risk — Dots in field names produce unexpected JSONB query paths.

- **F-028** — `./apps/core/src/routes/credentials.rs:183` — Security — Credential retrieval logged at INFO with key name.

- **F-029** — `./apps/core/src/routes/data.rs:170-205` — Security — Optional identity allows unauthenticated record creation.

- **F-030** — `./apps/core/src/routes/data.rs:261-266` — Security — `update_record` lacks ownership check.

- **F-031** — `./apps/core/src/routes/data.rs:318-363` — Security — `delete_record` lacks ownership check.

- **F-032** — `./apps/core/src/routes/events.rs:32-35` — Bug Risk — `BroadcastStream` errors silently dropped.

- **F-033** — `./apps/core/src/routes/federation.rs:157-172` — Performance — `serve_changes` loads up to 1000 records then filters in memory.

- **F-034** — `./apps/core/src/routes/graphql.rs:585-888` — Bug Risk — `record_to_*` silently defaults missing fields.

- **F-035** — `./apps/core/src/routes/graphql.rs:1373-1390` — Performance — Schema rebuilt per request.

- **F-036** — `./apps/core/src/routes/graphql.rs:1396-1409` — Maintainability — Playground guarded by runtime check, not compile-time feature gate.

- **F-037** — `./apps/core/src/routes/household.rs:19-21` — Maintainability — `HouseholdState` inconsistent with `AppState` pattern.

- **F-038** — `./apps/core/src/routes/household.rs:237-255` — Bug Risk — TOCTOU race on last-admin check.

- **F-039** — `./apps/core/src/routes/identity.rs:337-363` — Maintainability — String matching for error classification.

- **F-040** — `./apps/core/src/routes/storage.rs:109-118` — Security — DB path exposed in response.

- **F-041** — `./apps/core/src/search.rs:90-111` — Performance — Commit per record in `index_record`.

- **F-042** — `./apps/core/src/shutdown.rs:41-44` — Bug Risk — `ShutdownHandles` only holds SQLite, not `PgStorage`.

- **F-043** — `./apps/core/src/sqlite_storage.rs:463-466` — Security — `sort_by` string interpolated into SQL (validated but fragile).

- **F-044** — `./apps/core/src/sqlite_storage.rs:594-597` — Bug Risk — Invalid filter fields silently skipped.

- **F-045** — `./apps/core/src/storage_migration.rs:113-136` — Performance — Records inserted one-at-a-time instead of batch.

- **F-046** — `./apps/core/src/storage_migration.rs:115-117` — Bug Risk — ON CONFLICT DO NOTHING preserves stale data.

- **F-047** — `./apps/core/src/wasm_runtime.rs:426-429` — Bug Risk — UTF-8 panic on log message truncation.

- **F-048** — `./apps/core/src/wasm_runtime.rs:455-476` — Security — Hostless URLs bypass domain allowlist.

- **F-049** — `./packages/auth/src/handlers/keys.rs:145` — Bug Risk — Hardcoded `expected_version: 1` in `revoke_key`.

- **F-050** — `./packages/auth/src/handlers/keys.rs:164` — Performance — O(n) key scan on every auth attempt.

- **F-051** — `./packages/auth/src/handlers/rate_limit.rs:17-24` — Bug Risk — Unbounded rate limiter HashMap.

- **F-052** — `./packages/auth/src/handlers/validate.rs:419-427` — Bug Risk — Case-sensitive auth scheme comparison.

- **F-053** — `./packages/auth/src/types.rs:37` — Security — `ApiKeyRecord` serializes `key_hash` and `salt`.

- **F-054** — `./packages/crypto/src/kdf.rs:15` — Security — `derive_key` has no minimum salt length enforcement.

- **F-055** — `./packages/dav-utils/src/dav_xml.rs:122` — Bug Risk — XML namespace spacing bug.

- **F-056** — `./packages/plugin-sdk-rs/src/test/mock_storage.rs:66-94` — Bug Risk — `MockStorageContext::update` and `delete` match by `correlation_id` not domain id.

- **F-057** — `./packages/plugin-sdk-rs/src/wasm_guest.rs:30-85` — Bug Risk — Missing credential `HostRequest` variants.

- **F-058** — `./packages/storage-sqlite/src/backend.rs:24-36` — Maintainability — Duplicate `CANONICAL_COLLECTIONS` definition.

- **F-059** — `./packages/traits/src/capability.rs:13-26` — Bug Risk — Capability enum divergence between traits and plugin-sdk.

- **F-060** — `./packages/types/src/pipeline.rs:72-74` — Bug Risk — `SchemaValidated` deserialization bypasses validation.

- **F-061** — `./plugins/engine/api-caldav/src/serializer.rs:68-93` — Bug Risk — UTF-8 `fold_line` corruption in CalDAV serializer.

- **F-062** — `./plugins/engine/api-carddav/src/serializer.rs:138-161` — Bug Risk — UTF-8 `fold_line` corruption in CardDAV serializer (same pattern as F-061).

- **F-063** — Systemic pattern across 7 plugins — Bug Risk — `Plugin::execute` is a no-op pass-through in `connector-email`, `connector-calendar`, `connector-contacts`, `connector-filesystem`, `webhook-sender`, `backup`, and `search-indexer`. These stubs silently accept and ignore commands.

- **F-064** — `./plugins/engine/backup/src/backend/local.rs:25` — Security — Path traversal string check is fragile.

- **F-065** — `./plugins/engine/backup/src/backend/webdav.rs:167-207` — Bug Risk — PROPFIND incomplete parsing (size=0).

- **F-066** — `./plugins/engine/backup/src/backend/webdav.rs:184` — Bug Risk — Prefix matching uses `contains` instead of `starts_with`.

- **F-067** — `./plugins/engine/backup/src/config.rs:1-25` — Maintainability — Duplicate config struct.

- **F-068** — `./plugins/engine/backup/src/crypto.rs:57-65` — Security — Decompression bomb risk (no size limit on decompression).

- **F-069** — `./plugins/engine/backup/src/types.rs:70-102` — Security — `BackupTarget` secrets not marked `skip_serializing`.

- **F-070** — `./plugins/engine/connector-calendar/src/caldav.rs:215-248` — Bug Risk — iCal special chars not escaped.

- **F-071** — `./plugins/engine/connector-calendar/src/google.rs:256-258` — Bug Risk — Empty `refresh_token` fallback.

- **F-072** — `./plugins/engine/connector-contacts/src/google.rs:253-267` — Maintainability — Error string matching for 410 Gone.

- **F-073** — `./plugins/engine/connector-email/src/imap.rs:160-227` — Security — `connect_plain()` is pub and gated only by feature flag.

- **F-074** — `./plugins/engine/connector-email/src/normalizer.rs:54` — Bug Risk — Non-idempotent `updated_at` in email normalization.

- **F-075** — `./plugins/engine/connector-email/src/smtp.rs:56-110` — Performance — New SMTP transport created per send.

- **F-076** — `./plugins/engine/connector-filesystem/src/lib.rs:128-139` — Bug Risk — (Merged into F-063.)

- **F-077** — `./plugins/engine/connector-filesystem/src/s3.rs:15-30` — Security — `S3Config` stores `secret_access_key` as plain `String`, inconsistent with credential store pattern.

- **F-078** — `./plugins/engine/connector-filesystem/src/s3.rs:148-164` — Performance — SDK client rebuilt per operation.

- **F-079** — `./plugins/engine/connector-filesystem/src/s3.rs:170-213` — Bug Risk — S3 `list_objects` has no pagination.

- **F-080** — `./plugins/engine/connector-filesystem/src/local.rs:182-211` — Security — Symlink following without bounds.

- **F-081** — `./plugins/engine/webhook-receiver/src/lib.rs:44-46` — Bug Risk — No duplicate endpoint ID check.

- **F-082** — `./plugins/engine/webhook-sender/src/lib.rs:163-197` — Maintainability — Duplicated Plugin/CorePlugin identity.

- **F-083** — `./plugins/engine/webhook-sender/src/lib.rs:261-271` — Bug Risk — `handle_event` doesn't dispatch webhooks.

---

## Low Severity

- **F-084** — `./apps/core/src/auth/jwt.rs:291-325` — Maintainability — Dead `validate_jwt` function duplicates logic.

- **F-085** — `./apps/core/src/auth/jwt.rs:340-348` — Security — No issuer validation when `issuer=None`.

- **F-086** — `./apps/core/src/auth/local_token.rs:122-128` — Bug Risk — Silent date parse fallback to now.

- **F-087** — `./apps/core/src/auth/webauthn_provider.rs:145-151` — Maintainability — Non-conformant UUID generation.

- **F-088** — `./apps/core/src/config.rs:924-929` — Bug Risk — Rate limiter panics on zero; config validates but no defense-in-depth. Also applies to `./apps/core/src/rate_limit.rs:46-48`.

- **F-089** — `./apps/core/src/credential_store.rs:7-11` — Maintainability — Stale doc comment says XOR but implementation uses AES-256-GCM.

- **F-090** — `./apps/core/src/federation.rs:31-35` — Security — `FederationPeer` serializes `client_key_path`.

- **F-091** — `./apps/core/src/federation.rs:286-320` — Security — Private key material not zeroized after use.

- **F-092** — `./apps/core/src/identity.rs:960-971` — Maintainability — Test uses `HashMap` instead of `BTreeMap`, causing non-determinism.

- **F-093** — `./apps/core/src/install_service.rs:167-168` — Bug Risk — `plist_dest.to_str().unwrap()` panic risk on non-UTF-8 paths.

- **F-094** — `./apps/core/src/main.rs:546` — Bug Risk — Workflows path not resolved relative to `data_dir`.

- **F-095** — `./apps/core/src/main.rs:1136-1137` — Maintainability — `mem::forget` on span guard.

- **F-096** — `./apps/core/src/plugin_loader.rs:134-148` — Bug Risk — Loose semver validation allows non-conformant versions.

- **F-097** — `./apps/core/src/rekey.rs:199` — Security — Same salt reused on rekey.

- **F-098** — `./apps/core/src/routes/credentials.rs:65` — Bug Risk — Empty credential value accepted.

- **F-099** — `./apps/core/src/routes/events.rs:49-56` — Bug Risk — Collection filter only applies to `NewRecords` events.

- **F-100** — `./apps/core/src/routes/graphql.rs:992-994` — Bug Risk — Missing and/or compound filter check.

- **F-101** — `./apps/core/src/routes/health.rs:98` — Security — Plugin failure messages exposed to clients.

- **F-102** — `./apps/core/src/routes/identity.rs:317-329` — Security — No max TTL on disclosure tokens.

- **F-103** — `./apps/core/src/routes/search.rs:63-71` — Bug Risk — All search errors return 400.

- **F-104** — `./apps/core/src/routes/system.rs:64-77` — Performance — Blocking fs I/O in async handler.

- **F-105** — `./apps/core/src/sqlite_storage.rs:333-343,413-425` and `./apps/core/src/pg_storage.rs:550-560` — Bug Risk — `user_id` / `household_id` always None in create and update across both storage backends.

- **F-106** — `./apps/core/src/storage_migration.rs:115` — Bug Risk — `user_id` / `household_id` not migrated.

- **F-107** — `./apps/core/src/wasm_runtime.rs:506` — Security — Unbounded HTTP response body from WASM guest.

- **F-108** — `./apps/core/tests/dev_environment_test.rs:22-37` — Maintainability — Docker test not `#[ignore]`d.

- **F-109** — `./packages/auth/src/config.rs:44-49` — Bug Risk — No unknown provider validation.

- **F-110** — `./packages/plugin-sdk-rs/src/macros.rs:169` — Performance — Plugin instantiated per call.

- **F-111** — `./packages/plugin-system/src/host_functions/logging.rs:28` — Maintainability — `from_str` shadows `FromStr` trait.

- **F-112** — `./packages/storage-sqlite/src/audit.rs:78-81` — Bug Risk — Fallback to `Utc::now` could delete all audit entries.

- **F-113** — `./packages/storage-sqlite/src/credentials.rs:132-133` — Bug Risk — Silent map result discard.

- **F-114** — `./packages/storage-sqlite/src/validation.rs:162-165` — Performance — String allocation in `has_schema`.

- **F-115** — `./packages/types/src/events.rs:122-123` — Maintainability — Inconsistent serde rename.

- **F-116** — `./packages/types/src/file_helpers.rs:95` — Bug Risk — `u64 as i64` cast risks sign overflow.

- **F-117** — `./packages/types/src/pipeline.rs:193` — Maintainability — `matches!` without `assert!` in test.

- **F-118** — `./packages/types/src/storage.rs:24` — Bug Risk — Limit cap not enforced in type.

- **F-119** — `./packages/workflow-engine/src/loader.rs:48-49` — Bug Risk — `read_dir` errors silently filtered.

- **F-120** — `./plugins/engine/api-caldav/src/serializer.rs:20` — Bug Risk — Zero-duration event default.

- **F-121** — `./plugins/engine/api-carddav/src/serializer.rs:113` — Bug Risk — ADR fields not escaped.

- **F-122** — `./plugins/engine/backup/src/backend/local.rs:56` — Bug Risk — TOCTOU in delete.

- **F-123** — `./plugins/engine/backup/src/backend/local.rs:119-121` — Performance — Blocking `exists()` in async context.

- **F-124** — `./plugins/engine/backup/src/backend/s3.rs:131` — Bug Risk — `i64 as u64` cast.

- **F-125** — `./plugins/engine/backup/src/engine.rs:44-57` — Bug Risk — Stale manifest in `BackupArchive`.

- **F-126** — `./plugins/engine/backup/src/engine.rs:100-109` — Performance — O(n) dedup in incremental backup.

- **F-127** — `./plugins/engine/connector-calendar/src/caldav.rs:194-211` — Bug Risk — Stubs return `Ok` instead of `Err`.

- **F-128** — `./plugins/engine/connector-calendar/src/google.rs:322-338` — Maintainability — Hand-rolled URL encoding.

- **F-129** — `./plugins/engine/connector-calendar/src/normalizer.rs:71` — Bug Risk — `all_day` always None.

- **F-130** — `./plugins/engine/connector-contacts/src/google.rs:113` — Security — `pub http_client` field exposes internal client.

- **F-131** — `./plugins/engine/connector-contacts/src/normalizer.rs:98-105` — Bug Risk — TEL missing pref detection.

- **F-132** — `./plugins/engine/connector-email/src/imap.rs:29-30` — Maintainability — Dead `use_tls` config field.

- **F-133** — `./plugins/engine/connector-email/src/smtp.rs:56-138` — Maintainability — Duplicated message building logic.

- **F-134** — `./plugins/engine/connector-email/tests/greenmail_integration.rs:159+` — Maintainability — Hardcoded sleep in tests.

- **F-135** — `./plugins/engine/connector-filesystem/src/local.rs:162-179` — Performance — Unnecessary clone of `watch_paths`.

- **F-136** — `./plugins/engine/connector-filesystem/src/s3.rs:187` — Bug Risk — `i64 as u64` cast on size.

- **F-137** — `./plugins/engine/connector-filesystem/src/types.rs:1-18` — Maintainability — Two competing `FileChange` types.

- **F-138** — `./plugins/engine/webhook-receiver/src/lib.rs:86` — Performance — Unnecessary `body.clone()`.

- **F-139** — `./plugins/engine/webhook-sender/src/delivery.rs:50-55` — Performance — O(n) drain eviction strategy.

---

## Info

- **F-140** — `./apps/core/src/pg_storage.rs:155-159` — `PgSslMode::Prefer` behaves like `Require` in practice.

- **F-141** — `./apps/core/tests/shutdown_test.rs:120-122` — Unsafe `libc::kill` usage (correct).

- **F-142** — `./apps/core/tests/config_test.rs:349+` — Unsafe `env::set_var` usage (correct with mutex guard).

- **F-143** — `./packages/dav-utils/src/ical.rs:85-103` — let-chains feature usage (nightly).

- **F-144** — Codebase-wide — 30 `#[allow(dead_code)]` suppressions concentrated in `auth/` and `schema_registry.rs`.

- **F-145** — Codebase-wide — 4 `TODO(F-092)` markers in CalDAV/CardDAV for unimplemented routes.

- **F-146** — Codebase-wide — Multiple empty `steps`/`transform`/`tests` modules across all plugins.

---

## Notes on Deduplication

- F-001 and F-005 are the same X-Forwarded-For trust pattern in different files (`middleware.rs` and `rate_limit.rs`). Both retained as separate findings.
- F-011 subsumes the inconsistent parameterization finding (G8-M3) since it covers the same code path.
- F-063 groups 7 `Plugin::execute` no-op stubs across all connector, backup, webhook-sender, and search-indexer plugins into a single systemic finding.
- F-008 merges the federation auth gap (G2-H2) with the federation unwrap panic (SUPP-H1 / G2-L3) into a single finding.
- F-088 groups the rate-limiter zero-panic from both `config.rs` and `rate_limit.rs`.
- F-105 groups the `user_id`/`household_id` always-None pattern across sqlite create, sqlite update, and pg update.
