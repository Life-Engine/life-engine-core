# Dependency and Supply Chain Audit

## Summary

This audit examines all Cargo workspace dependencies, resolved versions in `Cargo.lock`, license compliance via `deny.toml`, Node.js dependencies in `package.json`/`pnpm-lock.yaml`, and feature flag correctness across 28 workspace members.

Key findings:

- **3 active security vulnerabilities** in wasmtime v37.0.3 (via the extism WASM runtime), all with available patches
- **2 unmaintained crate advisories** (fxhash, rustls-pemfile)
- **1 version conflict** where a plugin pins `cron = "0.13"` against the workspace's `"0.15"`
- **13 hardcoded dependency versions** that should use `{ workspace = true }` references
- **43 duplicate crate versions** in the resolved dependency tree (788 total crates)
- **1 feature flag divergence** on `tower` between workspace and `apps/core`
- **1 feature flag conflict** on `reqwest` in `connector-calendar` (different features for optional vs dev dependency)
- License compliance passes, all sources locked to crates.io

## Security Vulnerability Assessment

### Critical: Wasmtime v37.0.3 (3 CVEs)

All three vulnerabilities come through `extism v1.20.0` which pins `wasmtime 37.0.3`. This affects `life-engine-core`, `life-engine-plugin-system`, and `life-engine-workflow-engine`.

- **RUSTSEC-2026-0006** — Segfault or out-of-sandbox load via `f64.copysign` on x86-64. Solution: upgrade to wasmtime >=36.0.5 (within the 36.x series) or >=40.0.3.
- **RUSTSEC-2026-0020** — Guest-controlled resource exhaustion in WASI implementations. Solution: upgrade to wasmtime >=36.0.6 or >=40.0.4 or >=41.0.4.
- **RUSTSEC-2026-0021** — Panic from excessive fields in `wasi:http/types.fields`. Solution: same as RUSTSEC-2026-0020.

The fix path requires either:

1. Updating `extism` to a version that uses wasmtime >=36.0.6 (the v37 series does not have patches; only 36.0.x patch series does)
2. Or waiting for an extism release that bumps to wasmtime >=41.0.4

Since extism controls the wasmtime version, a `cargo update -p wasmtime` alone will not resolve this unless extism loosens its wasmtime version pin. Check for a newer extism release.

### Unmaintained Crates

- **RUSTSEC-2025-0057 (fxhash v0.2.1)** — No longer maintained. Transitive dependency via `wasmtime -> fxprof-processed-profile`. No safe upgrade available. This resolves when wasmtime is upgraded (newer wasmtime versions use `rustc-hash`).
- **RUSTSEC-2025-0134 (rustls-pemfile v2.2.0)** — Archived since August 2025. Direct dependency in `life-engine-core` and `life-engine-transport-rest`. Migration path: replace with `rustls-pki-types >= 1.9.0` which includes the same PEM parsing via the `PemObject` trait.

### Previously Acknowledged Advisories (in deny.toml)

- **RUSTSEC-2023-0071** (rsa timing sidechannel) — dev-dependency only, used for test JWT signing. Accepted risk.
- **RUSTSEC-2024-0384** (instant crate unmaintained) — transitive via tantivy/notify. Waiting for upstream migration to `web-time`.

## License Compliance Status

License checks pass. The `deny.toml` configuration is well-maintained.

- **Allowed licenses** — Apache-2.0, MIT, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, Zlib, 0BSD, BSL-1.0, MPL-2.0, CC0-1.0, and others
- **Special clarifications** — `ring` (MIT AND ISC AND OpenSSL) and `encoding_rs` (Apache-2.0/MIT AND BSD-3-Clause) are properly clarified with license file hashes
- **Source restrictions** — All dependencies must come from crates.io only; unknown registries and git sources are denied

One warning:

- **ical v0.11.0** — No license field in its manifest. Used by `api-caldav` and `connector-calendar`. The crate is published on crates.io and appears to be Apache-2.0/MIT licensed on GitHub, but the missing manifest field triggers a cargo-deny warning. This should be monitored.

Two license allowances are currently unused (OpenSSL, Unicode-DFS-2016). These are harmless but could be cleaned up.

## Workspace Dependency Consistency

### Crates Using Hardcoded Versions Instead of Workspace References

These crates specify versions directly instead of using `{ workspace = true }`, even though the workspace defines the same dependency at a compatible version.

Packages:

- **packages/traits** — `toml = "0.8"` (workspace defines `"0.8"`)
- **packages/dav-utils** — `base64 = "0.22"` (workspace defines `"0.22"`)
- **packages/test-utils** — `base64 = "0.22"` (workspace defines `"0.22"`)

Plugins:

- **plugins/engine/webhook-receiver** — `hmac = "0.12"` (workspace defines `"0.12"`)
- **plugins/engine/connector-calendar** — `base64 = "0.22"`, `url = "2"` (both in workspace)
- **plugins/engine/connector-contacts** — `base64 = "0.22"`, `url = "2"` (both in workspace)
- **plugins/engine/connector-filesystem** — `mime_guess = "2"` (workspace defines `"2"`)
- **plugins/engine/backup** — `flate2 = "1"` (workspace defines `"1"`)

Dev-dependencies:

- **packages/types** — `tempfile = "3"` (workspace defines `"3"`)
- **apps/core** — `tempfile = "3"` (workspace defines `"3"`)
- **plugins/engine/backup** — `tempfile = "3"` (workspace defines `"3"`)
- **plugins/engine/connector-filesystem** — `tempfile = "3"` (workspace defines `"3"`)

### Version Conflict: cron

- **plugins/engine/backup** pins `cron = "0.13"`
- Workspace defines `cron = "0.15"`
- Both versions resolve in `Cargo.lock` (0.13.0 and 0.15.0), meaning two copies of the cron crate are compiled

This is a genuine version conflict. The backup plugin will use cron 0.13 semantics while the workflow-engine uses cron 0.15. If the cron API changed between these versions, this could cause subtle behavior differences in schedule parsing.

### Feature Flag Issues

- **tower** — The workspace defines `tower = "0.5"` (no features). `apps/core` overrides this with `tower = { version = "0.5", features = ["util"] }` in both `[dependencies]` and `[dev-dependencies]`. `packages/transport-rest` uses `tower = { workspace = true }` (no `util` feature). This means `transport-rest` cannot use `tower::ServiceExt` and other util types unless feature unification provides them at build time. This works in practice because Cargo unifies features for a single resolved version, but it is fragile — if `transport-rest` were compiled independently, it would lack `util`.
- **reqwest in connector-calendar** — The plugin declares `reqwest = { version = "0.12", features = ["json"], optional = true }` as an optional dependency and `reqwest = { workspace = true }` as a dev-dependency. The workspace reqwest uses `rustls-tls` and disables default features. The optional dep does not disable default features and does not include `rustls-tls`. If the `integration` feature is enabled, the resolved reqwest will use the union of both feature sets, but the intent appears inconsistent.

### Dependencies Not in Workspace

Several crates are used by individual packages or plugins but are not centralized in the workspace:

- **chrono-tz** — used by `dav-utils` only
- **ical** — used by `api-caldav` and `connector-calendar`
- **lettre** — used by `connector-email` only
- **mail-parser** — used by `connector-email` only
- **notify** — used by `connector-filesystem` only
- **glob** — used by `connector-filesystem` only
- **quick-xml** — used by `backup` only
- **aws-sdk-s3** / **aws-config** — used by `backup` and `connector-filesystem` (optional)
- **mockito** — used by `connector-contacts` dev-deps only
- **rcgen** / **rsa** — used by `apps/core` dev-deps only
- **wat** — used by `workflow-engine` and `plugin-system` dev-deps

Of these, `ical`, `aws-sdk-s3`, `aws-config`, and `wat` are used by more than one crate and would benefit from workspace centralization.

## Version Conflict Inventory

The resolved `Cargo.lock` contains 788 unique crate entries with 43 duplicate-version warnings. The significant ones (excluding Windows platform crates) are:

Caused by ecosystem transitions (unavoidable, transitive):

- **rand** — 0.8.5 and 0.9.2 (workspace pins 0.8, but newer deps pull 0.9)
- **getrandom** — 0.2.17, 0.3.4, 0.4.2 (three versions across the rand ecosystem split)
- **hashbrown** — 0.14.5, 0.15.5, 0.16.1 (used by different indexmap/hashbrown consumers)
- **rustls** — 0.21.12 and 0.23.37 (old rustls from webauthn-rs transitive deps)
- **hyper** — 0.14.32 and 1.8.1 (legacy hyper from webauthn/rustls-0.21 chain)
- **http** / **http-body** — 0.2.x and 1.x (same hyper migration)
- **thiserror** — 1.0.69 and 2.0.18 (workspace uses 2.x but transitive deps still on 1.x)
- **toml** — 0.8.23 and 0.9.12 (workspace defines 0.8 but cargo-internal may pull 0.9)

Caused by direct dependency choices (potentially avoidable):

- **cron** — 0.13.0 and 0.15.0 (backup plugin pins old version)
- **base64** — 0.21.7 and 0.22.1 (webauthn-rs pulls 0.21; workspace uses 0.22)

Caused by WASM toolchain version spread:

- **wasmparser** — 0.239.0, 0.244.0, 0.245.1 (three versions)
- **wasm-encoder** — 0.239.0, 0.244.0, 0.245.1 (three versions)
- **wast** — 35.0.2 and 245.0.1
- **wit-parser** — 0.239.0 and 0.244.0

## Node.js Dependency Assessment

`package.json` declares 4 dev-dependencies:

- `nx` — ^20.4.0 (resolved to 20.8.4)
- `@nx/js` — ^20.4.0 (resolved to 20.8.4)
- `@nx/vite` — ^20.4.0 (resolved to 20.8.4)
- `@nx/react` — ^20.4.0 (resolved to 20.8.4)

All Nx packages are at the same minor version, which is correct. The lockfile version is 9.0 (pnpm v9 format).

The `apps/admin` directory has additional dependencies (React 19, react-router-dom, Vite) defined in the pnpm lockfile but is not listed in `pnpm-workspace.yaml` packages. This is a configuration inconsistency but not a security issue.

pnpm@9.15.4 is specified as the package manager. This is a stable release with no known vulnerabilities.

## Plugin Template Assessment

Two plugin templates exist with different approaches:

- **tools/templates/engine-plugin/Cargo.toml** — Hardcodes versions (`serde = { version = "1", features = ["derive"] }`, `serde_json = "1"`, `async-trait = "0.1"`, `anyhow = "1"`). Does NOT use `version.workspace`, `edition.workspace`, or `license.workspace`. Plugins scaffolded from this template will immediately drift from workspace version pins and lack workspace metadata inheritance.
- **tools/templates/plugin/Cargo.toml** — Correctly uses `{ workspace = true }` references and inherits workspace metadata. This is the correct template.

The `justfile` `new-plugin` command references the first (broken) template; `tools/scripts/scaffold-plugin.sh` references the second (correct) template.

## cargo-deny Configuration Review

The `deny.toml` configuration is well-structured:

- **Advisories** — Two advisories are explicitly ignored with documented rationale (RUSTSEC-2023-0071, RUSTSEC-2024-0384). However, the three new wasmtime advisories (RUSTSEC-2026-0006, 0020, 0021) and two unmaintained advisories (RUSTSEC-2025-0057, 0134) are not acknowledged and cause `cargo deny check advisories` to fail.
- **Bans** — Multiple versions are set to `warn` (not `deny`), which is appropriate for a project with many transitive dependencies. Wildcards are allowed.
- **Sources** — Locked to crates.io only. No git dependencies allowed. This is a strong supply chain control.
- **Licenses** — Comprehensive allow-list with proper clarifications for non-standard license files (ring, encoding_rs).

## Dependency Bloat Assessment

The project pulls 788 resolved crates. The largest contributors are:

- **wasmtime/extism ecosystem** — Pulls in the entire Cranelift compiler, WASM runtime, WASI implementation, profiling tools, and related crates. This is the single biggest dependency subtree and is responsible for the fxhash unmaintained advisory, the WASM toolchain version spread (three versions of wasmparser), and the wasmtime CVEs. This is inherent to the plugin architecture choice of using WASM sandboxing.
- **webauthn-rs** — Pulls legacy rustls 0.21, hyper 0.14, and base64 0.21. This creates the most significant duplicate chain in the project.
- **tantivy** — Full-text search engine. Pulls many dependencies including the unmaintained `instant` crate (acknowledged in deny.toml).
- **async-graphql** — GraphQL framework. Moderate dependency footprint.
- **deadpool-postgres / tokio-postgres** — PostgreSQL client. Used only in `apps/core` for federation features.

No obviously unnecessary dependencies were found. All direct dependencies align with documented features.

## Recommendations

### Critical (address before next release)

1. **Update extism to resolve wasmtime CVEs** — Check for an extism release that uses wasmtime >=36.0.6 or >=41.0.4. If no such release exists, pin wasmtime to the 36.0.6 patch series using a `[patch.crates-io]` section, or open an issue upstream. These are sandbox-escape-adjacent vulnerabilities in a WASM runtime used to execute third-party plugin code.

2. **Fix cron version conflict in backup plugin** — Change `plugins/engine/backup/Cargo.toml` from `cron = "0.13"` to `cron = { workspace = true }` and update any API call sites for the 0.13-to-0.15 migration.

### High Priority

3. **Migrate off rustls-pemfile** — Replace direct usage of `rustls-pemfile` in `apps/core` and `packages/transport-rest` with the `PemObject` trait from `rustls-pki-types >= 1.9.0`. The crate is archived and will not receive updates.

4. **Add new advisories to deny.toml** — Either fix the underlying issues or add explicit `ignore` entries with rationale for the 5 new advisories so that `cargo deny check` passes in CI-like contexts. Currently `cargo deny check advisories` fails.

5. **Convert all hardcoded versions to workspace references** — The 13 hardcoded version specifications listed above should use `{ workspace = true }`. This is a mechanical change that prevents version drift.

### Medium Priority

6. **Fix tower feature flag** — Add `features = ["util"]` to the workspace tower definition, or ensure crates that need `tower::util` declare the feature explicitly. The current setup relies on Cargo feature unification which is fragile.

7. **Reconcile reqwest features in connector-calendar** — The optional `reqwest` dep and the dev-dependency `reqwest` have inconsistent feature sets. Align them, or use `{ workspace = true }` for the optional dep with additional features.

8. **Centralize shared non-workspace dependencies** — Add `ical`, `wat`, `aws-sdk-s3`, and `aws-config` to the workspace `[workspace.dependencies]` section since they are used by multiple crates.

9. **Fix or remove the engine-plugin template** — Either update `tools/templates/engine-plugin/Cargo.toml` to use workspace references and workspace metadata, or delete it and standardize on `tools/templates/plugin/`.

### Low Priority

10. **Clean up unused license allowances** — Remove `OpenSSL` and `Unicode-DFS-2016` from `deny.toml` license allow-list if no longer needed.

11. **Monitor ical crate license** — The `ical` crate has no license field in its manifest. Verify it is properly licensed before relying on it in production.

12. **Consider rand version unification** — The workspace pins `rand = "0.8"` but some transitive dependencies pull `rand 0.9`. Evaluate whether upgrading to `rand = "0.9"` is feasible to eliminate the duplicate.

13. **Consider webauthn-rs upgrade** — The `webauthn-rs = "0.5.4"` dependency is responsible for pulling legacy `rustls 0.21`, `hyper 0.14`, `http 0.2`, and `base64 0.21`. If a newer version exists that uses the current ecosystem versions, upgrading would eliminate several duplicate chains.
