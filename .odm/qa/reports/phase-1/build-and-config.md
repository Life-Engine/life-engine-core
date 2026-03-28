# Build System and Configuration Review

## Summary

The life-engine-core project uses a polyglot monorepo with Cargo workspaces (Rust) and Nx/pnpm (Node/TypeScript). The build infrastructure is generally well-structured: workspace dependency management is mostly consistent, Docker images use multi-stage builds with Alpine for small size, cargo-deny is configured for license and advisory auditing, and cross-compilation targets are documented.

However, the review uncovered several issues across dependency consistency, Dockerfile staleness, Docker Compose configuration mismatches, and template maintenance. The most impactful problems are version mismatches where crates use hardcoded versions instead of workspace references, two divergent Dockerfiles that have drifted apart, and a Docker image size check script pointing at the wrong Dockerfile.

## File-by-File Analysis

### Root Cargo.toml (workspace definition)

The workspace defines 28 members across apps, packages, and plugins. It uses `resolver = "2"` and `edition = "2024"` (Rust 2024 edition, requires nightly or Rust 1.85+). Workspace-level dependencies are centralized for the major crates.

Issues found:

- The `tools/templates/engine-plugin/` crate is not listed as a workspace member but has its own `Cargo.toml`. This is intentional (it is a template), but the template hardcodes dependency versions (e.g., `serde = { version = "1", features = ["derive"] }`) instead of using workspace references, so new plugins scaffolded from it will not inherit workspace version pins.
- The `tools/templates/plugin/` crate is similarly excluded from the workspace, also with hardcoded versions.

### Cargo.lock

Uses lockfile version 4 (Cargo 1.78+). The lock file is committed to the repository, which is correct for a binary/application project.

### deny.toml (cargo-deny)

Well-configured. Two advisories are ignored with documented rationale:

- `RUSTSEC-2023-0071` (rsa timing sidechannel) -- dev-dependency only
- `RUSTSEC-2024-0384` (instant crate unmaintained) -- transitive via tantivy

License allow-list is comprehensive. Sources are locked to crates.io only (no unknown registries or git sources).

No issues found.

### .cargo/config.toml

Provides cross-compilation linker configuration for `aarch64-unknown-linux-gnu` and `aarch64-unknown-linux-musl`. Documentation in comments is clear.

No issues found.

### justfile

Three development commands (`dev-core`, `dev-app`, `dev-all`), a test command, a lint command, and a `new-plugin` scaffolding command. The `new-plugin` command uses `sed -i ''` which is macOS-specific (GNU sed uses `sed -i` without the empty string argument). This will fail on Linux.

There is also a separate `tools/scripts/scaffold-plugin.sh` that does the same job but with better cross-platform handling (uses `awk` for Cargo.toml manipulation). The duplication is confusing.

### package.json

Defines `pnpm@9.15.4` as the package manager. Dev dependencies include `nx`, `@nx/js`, `@nx/vite`, and `@nx/react` all at `^20.4.0`. Scripts proxy through Nx.

No issues found.

### nx.json

Defines named inputs for `rust` and `typescript`, target defaults with caching, and `defaultBase: "main"`. The `sharedGlobals` input references `.github/**/*` which includes the disabled workflow files.

No issues found.

### pnpm-workspace.yaml

Declares pnpm workspace packages at `apps/core`, `packages/*`, and `plugins/**`. Note that `apps/core` is a Rust crate with no `package.json`, so pnpm will silently skip it. The `apps/admin` directory (which has a `project.json`) is not listed here but would be matched by a `apps/*` glob if it existed.

### docker-compose.yml (root)

Development services: Pocket ID, GreenMail (email), Radicale (CalDAV/CardDAV), and MinIO (S3). MinIO uses default credentials (`minioadmin/minioadmin`) with env var overrides.

No issues found for development use.

### docker-compose.test.yml

Test-specific services with different port mappings to avoid conflicts with development. MinIO test credentials are hardcoded (acceptable for test-only).

No issues found.

### deploy/docker-compose.yml

Production-like single-service deployment. References `deploy/Dockerfile`. Uses `LIFE_ENGINE_STORAGE_PASSPHRASE` from env var (good). Sets `auth_provider: "local-token"`.

Issues found:

- Uses `wget` for healthcheck but `wget` is available in the Alpine runtime image. This is fine.
- The `config.toml` mount (`./config.toml:/app/config.toml:ro`) but the config loading code in `config.rs` reads YAML (`config.yaml`), not TOML. The `CoreConfig::load_from_yaml` function uses `serde_yaml::from_str`. The mount path suggests TOML, but the config parser expects YAML. This is a mismatch -- either the mount should reference a `.yaml` file, or the config parser should support TOML.

### deploy/docker-compose.full.yml

Full stack with Core and Pocket ID. Core depends on `pocket-id` with `condition: service_healthy`. Auth provider is set to `"oidc"`.

Issues found:

- Pocket ID port mapping is `3751:3751` but the Pocket ID image typically serves on port 80 internally. The root `docker-compose.yml` correctly maps `3751:80`. The full compose maps `3751:3751`, which would only work if Pocket ID is configured to listen on 3751 internally, but no such environment variable is set.
- Same `config.toml` vs YAML mismatch as `deploy/docker-compose.yml`.
- Uses `./data:/data` bind mount (not a named volume like the standalone compose), which creates a `data` directory in `deploy/`.

### deploy/Dockerfile

Multi-stage build using `rust:1.85-alpine` and `alpine:3.20`. Copies all workspace member Cargo.toml files for dependency caching, creates stubs, then copies full source.

Issues found:

- Installs `perl` and `make` in the builder stage (needed for OpenSSL/SQLCipher compilation). The `apps/core/Dockerfile` does not install these, which could cause build failures.
- Uses `ENTRYPOINT` (vs `CMD` in the other Dockerfile). This is a minor inconsistency.

### apps/core/Dockerfile

An older/simpler Dockerfile that only copies a subset of workspace members (types, plugin-sdk-rs, test-utils). This is stale -- it does not include many packages that `apps/core/Cargo.toml` depends on (traits, crypto, storage-sqlite, auth, workflow-engine, plugin-system, transport-rest, transport-graphql, transport-caldav, transport-carddav, transport-webhook, dav-utils).

Issues found:

- This Dockerfile will fail to build because it does not copy all required workspace members. The workspace Cargo.toml references 28 members, but this Dockerfile only provides stubs for 4 of them (types, plugin-sdk-rs, test-utils, and the core app itself).
- Missing `perl` and `make` packages needed for SQLCipher compilation (the `bundled-sqlcipher` feature in rusqlite requires these).
- `tools/verify-docker-image-size.sh` references `apps/core/Dockerfile` (the broken one) instead of `deploy/Dockerfile` (the working one).

### .devcontainer/devcontainer.json

Uses `mcr.microsoft.com/devcontainers/rust:latest` with Node 20 and Docker-in-Docker. Includes VS Code extensions for Rust Analyzer, Tauri, ESLint, and Prettier. Post-create runs `cargo check --workspace && pnpm install`.

Issues found:

- Includes `tauri-apps.tauri-vscode` extension, but there is no Tauri dependency or configuration in the project. This appears to be leftover from an earlier architecture.

### .gitignore

Comprehensive. Covers Rust, Node, IDE, OS, environment, test/coverage, Nx, Astro, logs, Claude, Stitch, and secrets.

No issues found.

### apps/core/src/config.rs

Well-structured configuration with layered loading (YAML file, env vars, CLI args). Sensitive fields are redacted in Debug output. Argon2 parameters are configurable.

Issues found:

- Config files are loaded as YAML (`serde_yaml::from_str`) but Docker Compose mounts a `config.toml` file. The config struct derives both `Serialize` and `Deserialize` for serde, and `toml` is in the core's dependencies, but the loading code only supports YAML.
- The `apply_env_overrides` method has significant code duplication for OIDC and WebAuthn field construction (the same `get_or_insert` pattern with identical default structs repeated 6+ times).

### Package Cargo.toml Files

All packages correctly inherit `version.workspace`, `edition.workspace`, and `license.workspace`. Most use `{ workspace = true }` for dependencies.

Issues found across packages:

- `packages/traits/Cargo.toml` uses `toml = "0.8"` (hardcoded) instead of `toml = { workspace = true }` (workspace version is `"0.8"` so they match, but not using the workspace reference is inconsistent).
- `packages/dav-utils/Cargo.toml` uses `base64 = "0.22"` (hardcoded) instead of workspace. Same for `chrono-tz = "0.10"` which is not in the workspace dependencies at all.
- `packages/test-utils/Cargo.toml` uses `base64 = "0.22"` (hardcoded) instead of workspace.

### Plugin Cargo.toml Files

Plugins correctly inherit workspace version/edition/license. Most use workspace dependencies.

Issues found:

- `plugins/engine/backup/Cargo.toml` has `cron = "0.13"` but the workspace defines `cron = "0.15"`. This is a version mismatch that could cause two versions of the cron crate to be compiled.
- `plugins/engine/backup/Cargo.toml` has `flate2 = "1"` hardcoded instead of using workspace.
- `plugins/engine/webhook-receiver/Cargo.toml` has `hmac = "0.12"` hardcoded instead of using workspace (workspace also defines `"0.12"`, but the reference should be `{ workspace = true }`).
- `plugins/engine/connector-calendar/Cargo.toml` has `base64 = "0.22"` and `url = "2"` hardcoded instead of workspace.
- `plugins/engine/connector-contacts/Cargo.toml` has `base64 = "0.22"` and `url = "2"` hardcoded instead of workspace.
- Several plugin dev-dependencies use hardcoded `tempfile = "3"` instead of `tempfile = { workspace = true }` (types, core, backup, connector-filesystem).

### project.json Files (Nx)

Each Rust crate has a `project.json` for Nx integration. Plugin build targets use `--target wasm32-wasip1` for WASM compilation.

Issues found:

- `plugins/engine/connector-email/project.json` and likely other plugins set WASM build target, but the plugins' `crate-type` includes `"cdylib"` which is needed for WASM. However, some plugins (webhook-receiver, api-caldav, api-carddav) do not set `crate-type = ["cdylib", "rlib"]` -- they only have a `[lib]` section without crate-type, meaning they will only produce `rlib` and cannot be compiled to WASM.
- `packages/plugin-system` does not have a `project.json` file, so Nx will not manage it. Nx will still discover it through the pnpm workspace, but it will lack build/test/lint targets.
- `apps/admin/project.json` exists but there is no `apps/admin/Cargo.toml` -- it is a TypeScript app. The pnpm-workspace does not include `apps/admin` (only `apps/core`), so pnpm will not discover it. It is not clear how the admin app is integrated.

### scripts/ Directory

Three scripts for plugin registry validation and schema compatibility checking:

- `validate-plugin-submission.js` -- ESM module, validates plugin registry entries
- `validate-registry-index.js` -- ESM module, validates the full registry index
- `check-schema-compat.sh` -- Bash script for schema backward compatibility checking

No build-related issues found.

### tools/ Directory

Contains CI check script, scaffold scripts, ADR validation, Docker image size check, and two plugin templates.

Issues found:

- `tools/verify-docker-image-size.sh` references `apps/core/Dockerfile` (line 30) but the working Dockerfile is at `deploy/Dockerfile`. The script will build from a broken Dockerfile.
- Two plugin templates exist (`tools/templates/plugin/` and `tools/templates/engine-plugin/`). The `justfile` `new-plugin` command references `tools/templates/plugin` while `tools/scripts/scaffold-plugin.sh` references `tools/templates/engine-plugin`. They serve the same purpose but use different templates.
- `tools/templates/engine-plugin/Cargo.toml` hardcodes versions instead of using workspace references, so scaffolded plugins will drift from workspace versions.

### .github/ Directory

All CI/CD workflows and Dependabot are correctly `.disabled` (suffixed) per the project rules (no CI/CD enabled). Issue templates and PR template exist.

Issues found:

- The disabled `ci.yml` references jobs for `sdk-docs`, `web`, `docker`, and `e2e` that depend on paths/projects (`apps/web/`, Playwright) that do not exist in this repository. If CI were re-enabled, these jobs would fail.
- The `docker` job in disabled CI references `--test docker_test` which likely doesn't exist (no such test file found in the codebase).

## Problems Found

### Critical

1. `apps/core/Dockerfile` is stale and will fail to build -- it only copies 4 of 28 workspace members, missing all the packages added since the initial architecture (traits, crypto, storage-sqlite, auth, workflow-engine, plugin-system, all transport packages, dav-utils). It also lacks `perl` and `make` packages needed for SQLCipher.

2. `plugins/engine/backup/Cargo.toml` pins `cron = "0.13"` while the workspace defines `cron = "0.15"`. This is a genuine version mismatch that may cause compile errors or unexpected behavior if the API changed between 0.13 and 0.15.

### Major

3. `tools/verify-docker-image-size.sh` references `apps/core/Dockerfile` (the broken one) instead of `deploy/Dockerfile`. The image size verification script cannot produce a valid build.

4. Config format mismatch: Docker Compose files mount `config.toml` but `config.rs` only parses YAML via `serde_yaml`. The application will fail to read a TOML config file.

5. `deploy/docker-compose.full.yml` maps Pocket ID port as `3751:3751` but the container likely serves on port 80. The root `docker-compose.yml` correctly uses `3751:80`. The healthcheck URL in the full compose hits port 3751 internally, which may not be listening.

6. Three plugins (webhook-receiver, api-caldav, api-carddav) lack `crate-type = ["cdylib", "rlib"]` in their lib section, so they cannot be compiled as WASM modules despite being in the plugin directory.

### Minor

7. Multiple crates use hardcoded dependency versions instead of workspace references, even when the workspace version matches:
   - `packages/traits` -- `toml = "0.8"` (should be `{ workspace = true }`)
   - `packages/dav-utils` -- `base64 = "0.22"` (should be `{ workspace = true }`)
   - `packages/test-utils` -- `base64 = "0.22"` (should be `{ workspace = true }`)
   - `plugins/engine/webhook-receiver` -- `hmac = "0.12"` (should be `{ workspace = true }`)
   - `plugins/engine/connector-calendar` -- `base64 = "0.22"`, `url = "2"` (should be `{ workspace = true }`)
   - `plugins/engine/connector-contacts` -- `base64 = "0.22"`, `url = "2"` (should be `{ workspace = true }`)
   - `plugins/engine/backup` -- `flate2 = "1"` (should be `{ workspace = true }`)

8. Several dev-dependencies use hardcoded `tempfile = "3"` instead of `tempfile = { workspace = true }` (packages/types, apps/core, plugins/engine/backup, plugins/engine/connector-filesystem).

9. Duplicate plugin scaffolding: `justfile` `new-plugin` command and `tools/scripts/scaffold-plugin.sh` do the same thing but reference different templates and use different substitution approaches.

10. `justfile` `new-plugin` uses `sed -i ''` which is macOS-only. The separate `scaffold-plugin.sh` handles this more portably.

11. `.devcontainer/devcontainer.json` includes the Tauri VS Code extension, which is no longer relevant to this project.

12. `pnpm-workspace.yaml` lists `apps/core` (a Rust crate without `package.json`) and does not list `apps/admin` (which has a `project.json`).

13. `tools/templates/engine-plugin/Cargo.toml` hardcodes dependency versions instead of using workspace references, meaning scaffolded plugins will not track workspace version updates.

14. `packages/plugin-system` has no `project.json` file for Nx, so it lacks Nx-managed build/test/lint targets.

15. Dependencies not in workspace that could be (`chrono-tz`, `mockito`, `ical`, `lettre`, `mail-parser`, `notify`, `glob`, `quick-xml`, `aws-sdk-s3`, `aws-config`) -- these are specific to individual crates so not necessarily an issue, but `chrono-tz`, `ical`, and others used by multiple crates could benefit from workspace centralization.

## Recommendations

1. Delete or completely rebuild `apps/core/Dockerfile` to match `deploy/Dockerfile`. Until then, update `tools/verify-docker-image-size.sh` to reference `deploy/Dockerfile`.

2. Fix the backup plugin's `cron` version: change `cron = "0.13"` to `cron = { workspace = true }` in `plugins/engine/backup/Cargo.toml`.

3. Add TOML config file support to `config.rs` (it already has `toml` as a dependency) or change the Docker Compose mounts to reference `config.yaml` instead of `config.toml`.

4. Fix the Pocket ID port mapping in `deploy/docker-compose.full.yml` to `3751:80` (matching the root compose).

5. Add `crate-type = ["cdylib", "rlib"]` to the `[lib]` sections of webhook-receiver, api-caldav, and api-carddav if they are intended to be compiled as WASM plugins.

6. Convert all hardcoded dependency versions to `{ workspace = true }` references where the workspace already defines the dependency at the same version.

7. Consolidate the two plugin scaffold mechanisms: remove the `justfile` `new-plugin` command and standardize on `tools/scripts/scaffold-plugin.sh`, or vice versa. Update the chosen template to use workspace dependency references.

8. Add a `project.json` for `packages/plugin-system` so Nx can manage its build/test/lint targets.

9. Remove the Tauri VS Code extension from `.devcontainer/devcontainer.json`.

10. If the disabled CI workflows are ever re-enabled, they will need significant updates to remove references to non-existent paths (`apps/web/`, Playwright tests, `docker_test`).
