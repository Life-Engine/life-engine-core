<!--
project: life-engine-core
phase: 10
specs: deployment-modes, migration-format
updated: 2026-03-23
-->

# Phase 10 — Deployment, Migration, and Plugin Conversion

## Plan Overview

This phase completes the migration by implementing the four deployment modes (Docker, standalone, Tauri sidecar, home server), the data migration system (WASM-based transforms for schema evolution), and converting existing first-party plugins from native Rust crates to WASM modules. This is the final phase — after completion, the system runs entirely on the new architecture.

This phase depends on Phase 9 (completed Core binary). The deployment modes share the same binary with config-driven behavior. The migration system handles schema evolution for both canonical and plugin-owned data. Plugin conversion is the last step — moving existing plugin logic into WASM modules that run through the workflow engine.

> spec: .odm/spec/deployment-modes/brief.md, .odm/spec/migration-format/brief.md

Progress: 4 / 24 work packages complete

---

## 10.1 — Docker Image
> spec: .odm/spec/deployment-modes/brief.md

- [x] Create multi-stage Dockerfile for Core based on Alpine Linux
  <!-- file: deploy/Dockerfile -->
  <!-- purpose: Define a multi-stage Dockerfile: Stage 1 (builder): FROM rust:1.85-alpine AS builder, install musl-dev and build dependencies, copy Cargo workspace files and source, run cargo build --release --bin life-engine-core targeting x86_64-unknown-linux-musl for a fully static binary. Stage 2 (runtime): FROM alpine:3.20, copy the release binary from builder, create /data, /plugins, /workflows directories with appropriate permissions, create a non-root user (life-engine:life-engine), set ENTRYPOINT to the binary. The final image should be under 50 MB. Include health check: HEALTHCHECK CMD wget --spider http://localhost:3000/health || exit 1. Expose default port 3000. Set working directory to /app. -->
  <!-- requirements: 3.1, 3.6 -->
  <!-- leverage: existing deploy/Dockerfile -->

---

## 10.2 — Docker Compose Configuration
> spec: .odm/spec/deployment-modes/brief.md

- [x] Create docker-compose.yml with volume mounts and environment configuration
  <!-- file: deploy/docker-compose.yml -->
  <!-- purpose: Define Core service: build from deploy/Dockerfile, volumes for persistent data (./data:/data for database, ./plugins:/plugins for WASM plugins, ./workflows:/workflows for YAML workflow files, ./config.toml:/app/config.toml for configuration), environment variables for config overrides (LIFE_ENGINE_STORAGE_PATH=/data/core.db, LIFE_ENGINE_STORAGE_PASSPHRASE from .env file, LIFE_ENGINE_TRANSPORTS_REST_PORT=3000), port mapping (3000:3000), restart policy (unless-stopped), resource limits (memory: 256M for default, adjustable), healthcheck referencing the container's health endpoint. Include a .env.example file documenting all available environment variables. -->
  <!-- requirements: 3.2, 3.3, 3.4, 3.5 -->
  <!-- leverage: existing deploy/docker-compose.yml -->

---

## 10.3 — Standalone Binary Configuration
> spec: .odm/spec/deployment-modes/brief.md

- [x] Implement platform-specific config file discovery for standalone mode
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Implement config file discovery for standalone binary mode: (1) check LIFE_ENGINE_CONFIG env var first, (2) check command-line --config argument, (3) check platform-specific default locations: Linux: $XDG_CONFIG_HOME/life-engine/config.toml or ~/.config/life-engine/config.toml, macOS: ~/Library/Application Support/life-engine/config.toml, Windows: %APPDATA%\life-engine\config.toml. (4) If no config found, create the default config directory and write a starter config.toml with commented-out sections explaining each option. Log the resolved config path at info level. Add support for --config CLI argument using clap or manual arg parsing. -->
  <!-- requirements: 2.1, 2.5, 6.1, 6.2, 7.1, 7.2, 7.3 -->
  <!-- leverage: existing apps/core/src/config.rs -->

- [x] Add startup logging for deployment mode and active configuration
  <!-- file: apps/core/src/main.rs -->
  <!-- purpose: At the start of the startup sequence (after config loading), log a summary: deployment mode (bundled/standalone/docker — detected from environment), bind address and port for each active transport, TLS status (enabled/disabled), auth provider, database path, plugins directory, workflows directory, number of loaded plugins, and any warnings (e.g., "No transports configured", "Running without TLS on non-localhost address"). -->
  <!-- requirements: 6.3 -->
  <!-- leverage: existing apps/core/src/main.rs -->

---

## 10.4 — Systemd Service Unit
> spec: .odm/spec/deployment-modes/brief.md

- [x] Create systemd service unit for Linux
  <!-- file: deploy/systemd/life-engine-core.service -->
  <!-- purpose: Define a systemd unit file: [Unit] Description=Life Engine Core, After=network.target. [Service] Type=exec, User=life-engine, Group=life-engine, ExecStart=/usr/local/bin/life-engine-core --config /etc/life-engine/config.toml, Restart=on-failure, RestartSec=5, Environment=LIFE_ENGINE_STORAGE_PASSPHRASE= (from systemd credential or separate env file), WorkingDirectory=/var/lib/life-engine, StandardOutput=journal, StandardError=journal, ProtectSystem=strict, ProtectHome=true, ReadWritePaths=/var/lib/life-engine, NoNewPrivileges=true, PrivateTmp=true. [Install] WantedBy=multi-user.target. Include security hardening directives. -->
  <!-- requirements: 2.2 -->
  <!-- leverage: existing deploy/systemd/ -->

---

## 10.5 — Launchd Plist
> spec: .odm/spec/deployment-modes/brief.md

- [x] Create launchd plist for macOS
  <!-- file: deploy/launchd/com.life-engine.core.plist -->
  <!-- purpose: Define a launchd plist: Label=com.life-engine.core, ProgramArguments=[/usr/local/bin/life-engine-core, --config, ~/Library/Application Support/life-engine/config.toml], RunAtLoad=true, KeepAlive=true, StandardOutPath=~/Library/Logs/life-engine/core.log, StandardErrorPath=~/Library/Logs/life-engine/core-error.log, WorkingDirectory=~/Library/Application Support/life-engine, EnvironmentVariables with LIFE_ENGINE_STORAGE_PASSPHRASE placeholder. -->
  <!-- requirements: 2.3 -->
  <!-- leverage: existing deploy/launchd/ -->

---

## 10.6 — Install-Service CLI Subcommand
> spec: .odm/spec/deployment-modes/brief.md

- [x] Implement install-service CLI subcommand
  <!-- file: apps/core/src/main.rs -->
  <!-- purpose: Add a CLI subcommand: life-engine-core install-service. On Linux: (1) copy deploy/systemd/life-engine-core.service to /etc/systemd/system/, (2) create /var/lib/life-engine directory with correct ownership, (3) create life-engine user/group if they don't exist, (4) run systemctl daemon-reload, (5) run systemctl enable life-engine-core, (6) print instructions for setting the passphrase and starting the service. On macOS: (1) copy deploy/launchd/com.life-engine.core.plist to ~/Library/LaunchAgents/, (2) create ~/Library/Application Support/life-engine/ directory, (3) run launchctl load the plist, (4) print instructions for configuring. Detect platform at runtime. Print clear messages for each step. Require root/sudo on Linux. -->
  <!-- requirements: 2.2, 2.3, 2.4 -->
  <!-- leverage: service unit files from WPs 10.4, 10.5 -->

---

## 10.7 — Caddy Reverse Proxy Configuration
> spec: .odm/spec/deployment-modes/brief.md

- [x] Create Caddy reverse proxy configuration for internet-facing deployment
  <!-- file: deploy/caddy/Caddyfile -->
  <!-- purpose: Define a Caddyfile: site block with the user's domain, automatic HTTPS via Let's Encrypt (Caddy's default), reverse_proxy directive pointing to localhost:3000 (Core's REST transport), header directives for security headers (Strict-Transport-Security, X-Content-Type-Options, X-Frame-Options), request_body max size limit (10 MB default), encode gzip for response compression, log directive for access logging. Include comments explaining how to configure the domain name and any required DNS setup. Add a separate section for CalDAV/CardDAV paths (/.well-known/caldav, /.well-known/carddav) that proxy to the appropriate transport ports. -->
  <!-- requirements: 4.3 -->
  <!-- leverage: existing deploy/caddy/ -->

---

## 10.8 — Behind-Proxy Support
> spec: .odm/spec/deployment-modes/brief.md

- [x] Add LE_BEHIND_PROXY flag support to Core startup
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Add behind_proxy (bool, default false) field to CoreConfig. When behind_proxy is true or LIFE_ENGINE_BEHIND_PROXY=true env var is set: (1) skip the TLS requirement for non-localhost bind addresses (the reverse proxy handles TLS), (2) trust X-Forwarded-For headers for client IP extraction (used by rate limiter), (3) trust X-Forwarded-Proto for protocol detection. When behind_proxy is false and bind address is not localhost: enforce TLS configuration — refuse to start without [transports.rest.tls] config containing cert_path and key_path. Log whether behind_proxy mode is active at startup. -->
  <!-- requirements: 4.4, 5.2 -->
  <!-- leverage: existing apps/core/src/config.rs -->

---

## 10.9 — ARM64 Build Verification
> spec: .odm/spec/deployment-modes/brief.md

- [x] Verify ARM64 binary builds and runs correctly
  <!-- file: Cargo.toml -->
  <!-- purpose: Cross-compile Core for aarch64-unknown-linux-gnu using cross or cargo with the appropriate target. Verify: (1) the build completes without errors, (2) the resulting binary runs on an ARM64 system (or QEMU emulation), (3) Core starts and responds to a health check request, (4) memory usage stays under 128 MB at idle with no plugins loaded, (5) SQLCipher works correctly on ARM64 (encryption/decryption round-trip). Document the cross-compilation command and any required toolchain setup. If cross-compilation requires additional linker configuration, add it to .cargo/config.toml with a target-specific section. -->
  <!-- requirements: 4.1, 4.2 -->
  <!-- leverage: existing Cargo workspace -->

---

## 10.10 — Tauri Sidecar Integration
> spec: .odm/spec/deployment-modes/brief.md

- [x] Configure Tauri sidecar to spawn and manage Core process lifecycle
  <!-- file: apps/app/src-tauri/tauri.conf.json -->
  <!-- file: apps/app/src-tauri/src/main.rs -->
  <!-- purpose: In tauri.conf.json: add Core binary as a sidecar in the bundle.externalBin array, configure the sidecar path for each platform (x86_64 and aarch64). In the Tauri main.rs: (1) spawn Core as a sidecar process on App launch using tauri::api::process::Command::new_sidecar(), (2) pass a bundled-mode config: storage in platform App data directory (dirs::data_dir()/life-engine/), plugins from bundled resources, auto-generated passphrase stored in platform keychain, (3) wait for Core's health endpoint to respond before showing the App UI, (4) on App close, send SIGTERM to the sidecar process, wait up to 5 seconds for graceful shutdown, then SIGKILL if still running. Include the plugins/ directory in the Tauri resource bundle so first-party plugins are available in bundled mode. -->
  <!-- requirements: 1.1, 1.2, 1.4, 1.6 -->
  <!-- leverage: existing apps/app/ Tauri configuration -->

- [x] Configure platform-standard data directory for bundled mode
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Detect bundled mode: check for LIFE_ENGINE_BUNDLED=true env var (set by Tauri sidecar). When in bundled mode: (1) use platform App data directory for database storage: macOS ~/Library/Application Support/life-engine/, Linux $XDG_DATA_HOME/life-engine/ or ~/.local/share/life-engine/, Windows %APPDATA%/life-engine/, (2) use bundled plugins directory from the app resources, (3) generate a passphrase on first run and store it in the platform keychain (macOS Keychain, Linux secret-tool, Windows Credential Manager) — the user never needs to manage the passphrase in bundled mode, (4) default to REST transport on localhost:0 (random port) with no TLS (localhost-only). -->
  <!-- requirements: 1.3, 1.5 -->
  <!-- leverage: existing config.rs -->

---

## 10.11 — Network Security Enforcement
> spec: .odm/spec/deployment-modes/brief.md

- [x] Implement non-localhost startup validation
  <!-- file: apps/core/src/main.rs -->
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: During startup, after loading config and before starting transports: (1) check if any transport binds to a non-localhost address (0.0.0.0, ::, or a specific non-127.0.0.1 IP), (2) if non-localhost AND behind_proxy is false: require TLS config (cert_path and key_path) in the transport config — refuse to start without it, log error "Refusing to start: non-localhost bind address requires TLS configuration or LE_BEHIND_PROXY=true", (3) if non-localhost: require auth config (auth section must be present and valid) — refuse to start without authentication on a network-facing instance, (4) if non-localhost: enable rate limiting in the auth module (it's optional for localhost), (5) log security posture at startup: "Security: TLS=enabled, Auth=pocket-id, RateLimit=enabled, BindAddress=0.0.0.0:3000". This ensures the "pit of success" — it's impossible to accidentally expose an unauthenticated, unencrypted instance to the network. -->
  <!-- requirements: 5.1, 5.2, 5.3, 5.4, 5.5 -->
  <!-- leverage: packages/auth for auth enforcement -->

---

## 10.12 — Migration Manifest Validation
> spec: .odm/spec/migration-format/brief.md

- [x] Implement manifest.toml migration entry parsing and validation
  <!-- file: packages/workflow-engine/src/migration/manifest.rs -->
  <!-- purpose: Parse the [[migrations]] array from a plugin's manifest.toml. Each migration entry has fields: from (String — semver range like "1.0.x" or "1.x"), to (String — exact semver like "2.0.0"), transform (String — name of the WASM export function that performs the transform), description (String — human-readable description of what the migration does), collection (String — which collection this migration applies to). Validation rules: (1) from must be a valid simplified semver range (major.x, major.minor.x, or exact major.minor.patch), (2) to must be an exact semver version, (3) to version must be greater than any version matching the from range, (4) transform name must be a valid Rust identifier (matches [a-zA-Z_][a-zA-Z0-9_]*), (5) the migration chain must be contiguous — if migrations go 1.x->2.0.0 and 2.x->3.0.0, there must be no gap, (6) collection must be a valid collection name. Return a Vec<MigrationEntry> or a clear validation error. -->
  <!-- requirements: 1.1, 1.2, 1.3, 1.5, 1.6 -->
  <!-- leverage: none -->

---

## 10.13 — Migration Entry Overlap Detection
> depends: 10.12
> spec: .odm/spec/migration-format/brief.md

- [x] Add overlap detection for migration from ranges
  <!-- file: packages/workflow-engine/src/migration/manifest.rs -->
  <!-- purpose: After parsing all migration entries for a plugin, check for overlapping from ranges within the same collection. Two entries overlap if any concrete version could match both from ranges (e.g., "1.x" and "1.0.x" overlap because 1.0.5 matches both). If overlap is detected, reject the plugin update with a MigrationError including both conflicting entries and an example version that matches both. This prevents ambiguous migration paths where Core can't determine which transform to apply. Add tests: (1) non-overlapping ranges pass (1.x->2.0.0, 2.x->3.0.0), (2) overlapping ranges fail (1.x->2.0.0, 1.0.x->1.1.0), (3) same from range with different collections is allowed, (4) exact version ranges never overlap with each other. -->
  <!-- requirements: 1.4 -->
  <!-- leverage: manifest parsing from WP 10.12 -->

---

## 10.14 — WASM Export Validation
> depends: 10.12
> spec: .odm/spec/migration-format/brief.md

- [x] Validate transform export names exist in plugin.wasm
  <!-- file: packages/workflow-engine/src/migration/validate.rs -->
  <!-- purpose: Implement pub fn validate_wasm_exports(wasm_path: &Path, entries: &[MigrationEntry]) -> Result<(), MigrationError>. Logic: (1) load the plugin.wasm binary, (2) parse the WASM module's export section (use wasmparser or Extism's introspection), (3) for each MigrationEntry, check that the transform function name exists as an exported function, (4) verify the exported function has the correct signature: takes one input (JSON bytes) and returns one output (JSON bytes), (5) if any transform name is missing, return an error listing all missing exports with their expected names. This validation runs at plugin load time — a plugin with missing migration exports is rejected before it can run. -->
  <!-- requirements: 8.1, 8.2, 8.3 -->
  <!-- leverage: Extism WASM loading -->

---

## 10.15 — WASM Transform Runner
> depends: 10.12
> spec: .odm/spec/migration-format/brief.md

- [x] Implement the WASM migration transform executor
  <!-- file: packages/workflow-engine/src/migration/runner.rs -->
  <!-- purpose: Implement pub async fn run_transform(wasm_path: &Path, function_name: &str, input_record: serde_json::Value) -> Result<serde_json::Value, MigrationError>. Logic: (1) load the plugin WASM module into a fresh Extism instance with NO host functions — migration transforms run in a pure sandbox with no storage, HTTP, or event access, (2) serialize the input record as JSON bytes, (3) call the named export function with the serialized input, (4) deserialize the output bytes as JSON, (5) return the transformed record. Error handling: if the WASM function returns an error (non-zero exit), capture the error message and return MigrationError::TransformFailed. If the function traps (panic), return MigrationError::TransformCrashed. Enforce a 10-second timeout per record transform — migrations should be fast. The transform function is pure: JSON in, JSON out, no side effects. -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.4, 2.5 -->
  <!-- leverage: Extism WASM runtime -->

---

## 10.16 — Quarantine Table and Operations
> spec: .odm/spec/migration-format/brief.md

- [ ] Create quarantine table schema and CRUD operations
  <!-- file: packages/storage-sqlite/src/migration/quarantine.rs -->
  <!-- purpose: Define CREATE TABLE quarantine with columns: id (TEXT PRIMARY KEY — UUID), record_data (TEXT NOT NULL — the original JSON record that failed to migrate), plugin_id (TEXT NOT NULL), collection (TEXT NOT NULL), from_version (TEXT NOT NULL — version the record was at), to_version (TEXT NOT NULL — version the migration was targeting), error_message (TEXT NOT NULL — why the transform failed), timestamp (TEXT NOT NULL — ISO 8601 when quarantined). Implement pub async fn quarantine_record(db, record, plugin_id, collection, from_version, to_version, error) -> Result<Uuid> that inserts a record into quarantine and returns its ID. Implement pub async fn list_quarantined(db, plugin_id, collection) -> Result<Vec<QuarantinedRecord>> for admin review. Implement pub async fn retry_quarantined(db, quarantine_id) -> Result<serde_json::Value> that retrieves a quarantined record for re-migration. Records stay in quarantine until explicitly retried or deleted by an admin. -->
  <!-- requirements: 5.1, 5.2, 5.3 -->
  <!-- leverage: packages/storage-sqlite -->

---

## 10.17 — Migration Log Table and Operations
> spec: .odm/spec/migration-format/brief.md

- [ ] Create migration log table schema and logging operations
  <!-- file: packages/storage-sqlite/src/migration/log.rs -->
  <!-- purpose: Define CREATE TABLE migration_log with columns: id (TEXT PRIMARY KEY), plugin_id (TEXT NOT NULL), collection (TEXT NOT NULL), from_version (TEXT NOT NULL), to_version (TEXT NOT NULL), records_migrated (INTEGER NOT NULL — count of successfully transformed records), records_quarantined (INTEGER NOT NULL — count of failed records sent to quarantine), duration_ms (INTEGER NOT NULL — total migration time in milliseconds), backup_path (TEXT — path to pre-migration backup file), timestamp (TEXT NOT NULL — ISO 8601). Implement pub async fn log_migration(db, entry: MigrationLogEntry) -> Result<()> that inserts a log entry. Implement pub async fn log_failure(db, plugin_id, collection, from_version, to_version, error) -> Result<()> for recording migration failures that prevented execution entirely (as opposed to per-record failures which go to quarantine). Implement pub async fn get_migration_history(db, plugin_id, collection) -> Result<Vec<MigrationLogEntry>> for admin review. -->
  <!-- requirements: 6.1, 6.2 -->
  <!-- leverage: packages/storage-sqlite -->

---

## 10.18 — Pre-Migration Backup
> spec: .odm/spec/migration-format/brief.md

- [ ] Implement pre-migration SQLite backup mechanism
  <!-- file: packages/storage-sqlite/src/migration/backup.rs -->
  <!-- purpose: Implement pub async fn create_backup(db_path: &Path, data_dir: &Path) -> Result<PathBuf> that: (1) creates a backups/ subdirectory in data_dir if it doesn't exist, (2) generates a timestamped backup filename: pre-migration-{YYYY-MM-DD-HHmmss}.db, (3) uses SQLite's backup API (rusqlite's backup_to_file or VACUUM INTO for an atomic copy) to create a consistent backup without blocking reads, (4) verifies the backup by opening it and running PRAGMA integrity_check, (5) returns the path to the backup file. Implement pub async fn restore_backup(backup_path: &Path, db_path: &Path) -> Result<()> that replaces the current database with a backup (forward-only migrations — restore is a manual admin action, not automatic rollback). This backup is created before every migration run and its path is recorded in the migration log. -->
  <!-- requirements: 7.1, 7.2, 7.3 -->
  <!-- leverage: packages/storage-sqlite, rusqlite backup API -->

---

## 10.19 — Migration Execution Engine
> depends: 10.15, 10.16, 10.17, 10.18
> spec: .odm/spec/migration-format/brief.md

- [ ] Implement the core migration execution loop
  <!-- file: packages/workflow-engine/src/migration/engine.rs -->
  <!-- purpose: Implement pub async fn run_migrations(storage: &dyn StorageBackend, wasm_path: &Path, entries: &[MigrationEntry], plugin_id: &str, db_path: &Path, data_dir: &Path) -> Result<MigrationResult>. Logic: (1) create a pre-migration backup via create_backup(), (2) for each MigrationEntry in ascending version order: (a) query all records in the target collection with version matching the from range, (b) begin a SQLite transaction, (c) for each matching record: call run_transform() with the record's JSON data, if transform succeeds: validate the output against the collection's schema (canonical or private), update the record's data and version column in plugin_data, increment migrated count; if transform fails: insert the record into quarantine with the error message, increment quarantined count, (d) commit the transaction (all-or-nothing per migration entry), (3) log the migration result (migrated count, quarantined count, duration) to migration_log, (4) return MigrationResult with counts and backup path. Define MigrationResult struct: migrated (u64), quarantined (u64), duration_ms (u64), backup_path (PathBuf), entries_applied (Vec<String>). If a migration entry has zero matching records, skip it silently. -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4, 9.1, 9.2 -->
  <!-- leverage: runner, quarantine, log, backup from previous WPs -->

---

## 10.20 — Canonical Migration Path
> depends: 10.19
> spec: .odm/spec/migration-format/brief.md

- [ ] Set up canonical migration file structure and startup trigger
  <!-- file: packages/types/src/migrations/mod.rs -->
  <!-- purpose: Create a packages/types/migrations/ directory structure for bundling canonical schema migrations with the types crate. Structure: migrations/events/ (event schema transforms), migrations/tasks/, migrations/contacts/, etc. — one subdirectory per canonical collection. Each subdirectory contains compiled WASM transform binaries. During Core startup, after storage initialization (step 4) and before loading plugins (step 8): compare each canonical collection's current schema version (stored in a schema_versions table or metadata) against the types crate's declared version. If the stored version is behind, run the canonical migration transforms through the same migration execution engine used for plugin migrations. This ensures canonical schema evolution is handled identically to plugin schema evolution — same WASM sandbox, same quarantine, same backup, same logging. Add schema_versions table DDL to storage-sqlite schema.rs. -->
  <!-- requirements: 3.1, 3.2, 3.3 -->
  <!-- leverage: migration engine from WP 10.19 -->

---

## 10.21 — Version Column Update
> depends: 10.19
> spec: .odm/spec/migration-format/brief.md

- [ ] Implement post-transform version stamping
  <!-- file: packages/storage-sqlite/src/migration/version.rs -->
  <!-- purpose: After a successful transform, update the record's version column in the plugin_data table to the migration entry's to version. This must happen within the same SQLite transaction as the data update to ensure atomicity. Implement pub fn stamp_version(tx: &Transaction, record_id: &str, new_version: &str) -> Result<()> that runs UPDATE plugin_data SET version = ? WHERE id = ?. After stamping, the record will not match the from range of the same migration entry on subsequent startups, preventing re-migration. Verify idempotency: running migrations twice produces the same result (already-migrated records are skipped because their version no longer matches the from range). Add tests: (1) version is updated after successful transform, (2) version update is atomic with data update, (3) re-running migration skips already-migrated records. -->
  <!-- requirements: 4.4 -->
  <!-- leverage: migration engine from WP 10.19 -->

---

## 10.22 — Migration Integration Tests
> depends: 10.20, 10.21
> spec: .odm/spec/migration-format/brief.md

- [ ] Verify end-to-end migration behavior
  <!-- file: packages/workflow-engine/tests/migration_test.rs -->
  <!-- purpose: Create test WASM transforms and run full migration scenarios. Test cases: (1) Simple migration: WASM transform that renames a field (e.g., "title" -> "name") and adds a default value for a new field. Insert test records at v1, run migration, verify records are at v2 with renamed field and new default. (2) Quarantine: WASM transform that fails on records where a field exceeds a length limit. Insert valid and invalid records, run migration, verify valid records migrated and invalid records quarantined with error messages. (3) Chain migration: three migrations in sequence (v1->v2, v2->v3, v3->v4). Insert records at v1, run all migrations in a single invocation, verify records end up at v4 with all transforms applied. (4) Schema validation: WASM transform that produces output violating the target schema. Verify the record is quarantined (not stored with invalid data). (5) Backup: verify backup is created before migration and can be restored. (6) Idempotency: run the same migration twice, verify no duplicate transforms. -->
  <!-- requirements: 2.1, 2.2, 2.3, 4.1, 4.3, 5.1, 5.2, 9.1 -->
  <!-- leverage: packages/test-utils -->

---

## 10.23 — First-Party Plugin WASM Conversion
> depends: 10.19
> spec: .odm/spec/plugin-system/brief.md

- [ ] Convert connector-email plugin from native Rust to WASM module
  <!-- file: plugins/connector-email/src/lib.rs -->
  <!-- file: plugins/connector-email/manifest.toml -->
  <!-- purpose: Refactor the connector-email plugin to compile as a WASM module. Steps: (1) update Cargo.toml to use life-engine-plugin-sdk as the only dependency (remove direct tokio, axum, etc.), set crate-type = ["cdylib"], (2) implement the Plugin trait: id() returns "connector-email", actions() returns ["fetch", "send"], execute() routes to fetch or send step handlers, (3) add register_plugin!(ConnectorEmail) macro call, (4) replace direct HTTP calls with the http:outbound host function (IMAP/SMTP via HTTP bridge or host function extension), (5) replace direct database access with StorageContext calls via storage host functions, (6) update manifest.toml with capabilities: ["storage:read", "storage:write", "http:outbound", "config:read"], (7) verify cargo build --target wasm32-wasi produces a valid plugin.wasm, (8) test the WASM plugin loads correctly in Core and can execute its actions. Repeat this pattern for all 6 remaining first-party plugins (connector-calendar, connector-contacts, connector-filesystem, webhook-sender, search-indexer, backup) — each as a sub-task. -->
  <!-- requirements: from plugin-system spec, ARCHITECTURE.md migration path step 11 -->
  <!-- leverage: existing plugin source code -->

- [ ] Convert remaining 6 first-party plugins to WASM modules
  <!-- file: plugins/connector-calendar/src/lib.rs -->
  <!-- file: plugins/connector-contacts/src/lib.rs -->
  <!-- file: plugins/connector-filesystem/src/lib.rs -->
  <!-- file: plugins/webhook-sender/src/lib.rs -->
  <!-- file: plugins/search-indexer/src/lib.rs -->
  <!-- file: plugins/backup/src/lib.rs -->
  <!-- purpose: Apply the same conversion pattern as connector-email to each remaining plugin: update Cargo.toml (plugin-sdk only, cdylib), implement Plugin trait, add register_plugin! macro, replace direct I/O with host function calls, update manifest.toml with required capabilities, verify WASM compilation, test loading and execution. connector-calendar: capabilities [storage:read, storage:write, http:outbound, config:read], actions [sync-caldav, sync-google]. connector-contacts: capabilities [storage:read, storage:write, http:outbound, config:read], actions [sync-carddav, sync-google]. connector-filesystem: capabilities [storage:read, storage:write, config:read], actions [scan, watch]. webhook-sender: capabilities [http:outbound, events:subscribe, config:read], actions [send]. search-indexer: capabilities [storage:read, storage:write], actions [index, search]. backup: capabilities [storage:read, http:outbound, config:read], actions [backup, restore]. -->
  <!-- requirements: from ARCHITECTURE.md migration path step 11 -->
  <!-- leverage: existing plugin source code -->

---

## 10.24 — Private Collection Support Registration
> depends: 10.23
> spec: .odm/spec/canonical-data-models/brief.md

- [ ] Implement private collection registration from plugin manifests
  <!-- file: packages/workflow-engine/src/schema_registry.rs -->
  <!-- purpose: During plugin loading (Phase 8), after parsing each plugin's manifest.toml, check for [collections.private] sections declaring plugin-owned collections. Each private collection declaration includes: name (String — the collection name, automatically namespaced as "plugin-id:collection-name"), schema (serde_json::Value — JSON Schema for validation). Register these schemas in a SchemaRegistry that the storage validation layer (Phase 5) uses. Implement SchemaRegistry struct with methods: register(plugin_id: &str, collection: &str, schema: serde_json::Value), get_schema(plugin_id: &str, collection: &str) -> Option<&serde_json::Value>, is_registered(plugin_id: &str, collection: &str) -> bool. The schema registry is shared with the storage backend. On write to a private collection, the storage layer looks up the schema and validates before persisting. Private collections are fully isolated by plugin_id — no cross-plugin access. Add tests: registration succeeds, schema lookup works, unregistered collection write is rejected, cross-plugin access is denied. -->
  <!-- requirements: from canonical-data-models spec 6.1, 6.2, 6.3 -->
  <!-- leverage: validation from Phase 5 -->
