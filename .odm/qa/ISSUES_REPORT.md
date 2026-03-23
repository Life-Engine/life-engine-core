# Life Engine Core — Issues Report

Generated: 2026-03-23

## Summary

- **Test failures** — 1 failing test
- **Security vulnerabilities** — 3 (all in `wasmtime` v37.0.3)
- **Unmaintained dependencies** — 2
- **Clippy warnings** — 104 across the core binary, plus 4 in plugins/packages
- **Duplicate dependencies** — 43 crate version duplicates

---

## 1. Test Failures

One test is failing in `life-engine-test-fixtures`:

- **Test** — `tests::credential_fixture_validates_against_schema`
- **Location** — `packages/test-fixtures/src/lib.rs:96`
- **Cause** — The credential fixture uses `"2026-04-15"` and `"2026-01-15"` (date-only strings) where the JSON schema expects `"date-time"` format (e.g. `"2026-04-15T00:00:00Z"`)
- **Fix** — Update the credential fixture dates to include a time component, or change the schema to accept `"date"` format

All other tests pass: 1,382 passed, 1 failed, 6 ignored.

---

## 2. Security Vulnerabilities (cargo-deny)

All three vulnerabilities come from `wasmtime` v37.0.3, pulled in via `extism` v1.20.0:

- **RUSTSEC-2026-0006** — Wasmtime segfault or out-of-sandbox load with `f64.copysign` on x86-64. Fix: upgrade wasmtime to >=24.0.6/>=36.0.6/>=40.0.4/>=41.0.4
- **RUSTSEC-2026-0020** — Guest-controlled resource exhaustion in WASI implementations. Fix: same wasmtime upgrade
- **RUSTSEC-2026-0021** — Panic adding excessive fields to `wasi:http/types.fields`. Fix: same wasmtime upgrade

**Root cause** — The `extism` crate pins `wasmtime` v37.0.3. Updating `extism` to a version that uses a patched wasmtime (>=37.0.6) would resolve all three.

---

## 3. Unmaintained Dependencies (cargo-deny)

- **fxhash** v0.2.1 — RUSTSEC-2025-0057. No longer maintained. Consider migrating to `rustc-hash` or `foldhash`.
- **rustls-pemfile** v2.2.0 — RUSTSEC-2025-0134. Repository archived. Functionality merged into `rustls` crate; use `rustls::pki_types` instead.

---

## 4. Clippy Warnings (104 in core + 4 in workspace)

### 4.1 Dead Code — 82 warnings

Large portions of code are compiled but never used. This is the biggest category.

**Entire unused modules/subsystems:**

- `wasm_runtime.rs` — `WasmRuntime`, `WasmHostBridge`, `HostRequest`, `HostResponse`, `WasmPluginConfig`, plus all methods (19+ items)
- `household.rs` — `HouseholdStore` fields, all methods (12+ items), route handlers in `routes/household.rs` (10+ items)
- `federation.rs` — `SyncRequest`, `sync_history_for_peer`, `build_mtls_server_config`
- `identity.rs` — `IdentityStore::new`, `init`, `update`, `verify_token`
- `plugin_signing.rs` — `verify_plugin`, `sign_plugin`, `compute_signing_payload`, `compute_manifest_hash`, `check_unsigned_policy`, `SignatureVerifierConfig`, `PluginSignature`, `RevocationList`
- `plugin_loader.rs` — `PluginManifest`, `ManifestCollection`, `discover_plugins`, `register`, `registered_count`, etc.
- `conflict.rs` — `resolve_last_write_wins`, `resolve_field_merge`, `resolve_manual`, `detect_conflict`, `strategy_for_collection`, `field_merge_needs_manual`
- `storage_migration.rs` — `migrate_sqlite_to_pg`, `MigrationResult`, `MigrationProgress`
- `rekey.rs` — `run_rekey`, `rekey_database`, `open_encrypted`, `load_or_create_salt`, `generate_salt`, `derive_key`
- `connector.rs` — `Connector` trait, `ConnectorCredentials`
- `crypto.rs` — `DOMAIN_IDENTITY_SIGN`, `DOMAIN_IDENTITY_ENCRYPT`, `DOMAIN_CREDENTIAL_STORE`
- `pg_storage.rs` — `PgConfig`, `PgStorage::open`, `from_pool`, `create_tables`, `fulltext_search`, `pool`
- `credential_store.rs` — `CredentialStore` associated items
- `search.rs` — `index_records_bulk`
- `message_bus.rs` — `subscriber_count`
- `tls.rs` — `make_rustls_config`
- `wasm_adapter.rs` — `WasmPluginAdapter`

### 4.2 Collapsible If Statements — 12 warnings

Nested `if let` / `if` chains that can be collapsed using `let-chain` syntax:

- `apps/core/src/routes/data.rs:196–197`
- `apps/core/src/routes/events.rs:50–51, 59`
- `apps/core/src/routes/graphql.rs:1242, 1255, 1268`
- `apps/core/src/wasm_runtime.rs:491`
- `apps/core/src/household.rs:155`
- `apps/core/src/sync_primitives.rs:137`

### 4.3 Useless Conversions — 2 warnings

Redundant `.into()` calls where the type is already `anyhow::Error`:

- `apps/core/src/sqlite_storage.rs:358`
- `apps/core/src/pg_storage.rs:542`

### 4.4 Derivable Impl — 1 warning

- `apps/core/src/pg_storage.rs:40` — Manual `Default` impl for `PgSslMode` can be replaced with `#[derive(Default)]` and `#[default]` attribute

### 4.5 Too Many Arguments — 1 warning

- `apps/core/src/household.rs:334` — `check_record_access` takes 8 arguments (max recommended: 7). Consider introducing a parameter struct.

### 4.6 Manual is_multiple_of — 1 warning

- `apps/core/src/auth/middleware.rs:73` — `count % CLEANUP_INTERVAL != 0` can be `.is_multiple_of()`

### 4.7 Other Warnings (plugins/packages)

- `plugins/engine/backup/src/engine.rs:257-258` — Unnecessary `if let` (only `Ok` variant used)
- `plugins/engine/connector-filesystem/src/local.rs:175` — Immediate dereference of created reference
- `packages/dav-utils/src/vcard.rs:25` — Very complex type; should be factored into a type alias
- `plugins/engine/api-caldav/src/serializer.rs:110` — Collapsible if

---

## 5. Duplicate Dependencies (cargo-deny)

43 crate version duplicates detected. Notable ones:

- **windows-sys** — 4 versions
- **toml_datetime**, **hashbrown**, **getrandom** — 3 versions each
- **thiserror**, **rand**, **rustix**, **bitflags**, **base64**, **itertools**, **nom**, **socket2** — 2 versions each

These increase compile time and binary size. Most are caused by transitive dependency version conflicts. Running `cargo update` may reduce some; others require coordinating upstream crate updates.

---

## 6. License Warnings (cargo-deny)

- 2 licenses listed in `deny.toml` were not encountered in any dependency (informational only)
- `ical` v0.11.0 has no license field in its manifest

---

## 7. Disabled CI

Both CI workflows are disabled:

- `.github/workflows/ci.yml.disabled`
- `.github/workflows/release.yml.disabled`

This means none of the above issues are being caught automatically on push/PR.

---

## Recommended Priority

1. **Security** — Update `extism`/`wasmtime` to resolve 3 CVEs
2. **Test failure** — Fix credential fixture date format (trivial)
3. **Unmaintained deps** — Replace `fxhash` and `rustls-pemfile`
4. **Dead code** — Remove or wire up ~82 unused items (significant codebase hygiene issue)
5. **Clippy lints** — Fix 22 non-dead-code warnings (mostly mechanical)
6. **CI** — Re-enable CI pipelines to catch regressions
7. **Duplicate deps** — Run `cargo update` and audit transitive version conflicts
