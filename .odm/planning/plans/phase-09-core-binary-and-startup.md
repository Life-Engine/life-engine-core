<!--
project: life-engine-core
phase: 9
specs: binary-and-startup
updated: 2026-03-23
-->

# Phase 9 — Core Binary and Startup

## Plan Overview

This phase slims `apps/core/` down to the thin orchestrator described in ARCHITECTURE.md: three files (`main.rs`, `config.rs`, `shutdown.rs`) that wire together all the modules built in Phases 2-8. The current monolithic Core (50+ source files) is replaced with a minimal binary that loads config, initializes modules in dependency order via a 10-step startup sequence, and coordinates graceful shutdown in reverse order.

This phase depends on all previous phases (types, traits, crypto, SDK, storage, auth, workflow engine, plugin system). Phase 10 (deployment) depends on the completed Core binary.

> spec: .odm/spec/binary-and-startup/brief.md

Progress: 9 / 11 work packages complete

---

## 9.1 — Top-Level Configuration Struct
> spec: .odm/spec/binary-and-startup/brief.md

- [x] Define top-level CoreConfig struct with TOML deserialization
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Define CoreConfig struct with serde Deserialize. Sections: storage (toml::Value — passed to StorageBackend for module-specific parsing), auth (toml::Value — passed to auth module), transports (HashMap<String, toml::Value> — keyed by transport name "rest", "graphql", "caldav", "carddav", "webhook", each value passed to the corresponding Transport), workflows (WorkflowsConfig: path String — directory containing YAML workflow files), plugins (PluginConfig from Phase 8: path String, per-plugin config), logging (LoggingConfig: level String default "info", format String default "json"). Each section is a raw toml::Value that gets handed to the owning module — Core does not parse module internals. Define DEFAULT_CONFIG_PATH constant for platform-specific config location. Derive Debug, Clone for CoreConfig. -->
  <!-- requirements: 1.1, 1.3 -->
  <!-- leverage: existing apps/core/src/config.rs -->

---

## 9.2 — Config Loading with Environment Variable Overrides
> spec: .odm/spec/binary-and-startup/brief.md

- [x] Implement config.toml loading with LIFE_ENGINE_* env var overrides
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Implement pub fn load_config(path: Option<&str>) -> Result<CoreConfig, ConfigError>. Logic: (1) determine config file path: use provided path, or LIFE_ENGINE_CONFIG env var, or DEFAULT_CONFIG_PATH, (2) read and parse config.toml using toml crate, (3) scan environment variables for LIFE_ENGINE_* prefix, (4) map env var names to config keys using underscore separation: LIFE_ENGINE_STORAGE_PATH -> storage.path, LIFE_ENGINE_AUTH_PROVIDER -> auth.provider, LIFE_ENGINE_TRANSPORTS_REST_PORT -> transports.rest.port, (5) apply env var overrides on top of TOML values — env vars always win (precedence: env > TOML > defaults), (6) return the merged CoreConfig. Handle errors: missing config file (create default), parse errors (show line/column), env var type conversion failures. Log the loaded config at info level excluding any values containing "key", "secret", "password", or "token" (redact sensitive values). -->
  <!-- requirements: 1.2, 1.4 -->
  <!-- leverage: existing apps/core/src/config.rs -->

---

## 9.3 — Config Validation and Section Delegation
> spec: .odm/spec/binary-and-startup/brief.md

- [x] Implement top-level validation and module-level delegation
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Implement pub fn validate_config(config: &CoreConfig) -> Result<(), ConfigError>. Logic: (1) validate top-level: storage section must exist, (2) validate at least one transport is configured (warn if zero — Core with no transports is valid but useless), (3) delegate to each module's validation: pass storage section to StorageBackend for validation (e.g., path exists), pass auth section to auth module for validation (e.g., issuer URL is valid for pocket-id provider), pass each transport section to the corresponding Transport for validation, (4) pass plugins section to plugin config validation, (5) collect all validation errors and report them together (don't stop at first error). Define ConfigError enum with variants: MissingSection { name: String }, InvalidValue { section: String, field: String, message: String }, ModuleValidationFailed { module: String, errors: Vec<String> }. Implement std::error::Error and Display. -->
  <!-- requirements: 2.1, 2.2, 2.4, 2.5 -->
  <!-- leverage: none -->

---

## 9.4 — Config Loading and Validation Tests
> spec: .odm/spec/binary-and-startup/brief.md

- [x] Add comprehensive config tests
  <!-- file: apps/core/tests/config_test.rs -->
  <!-- purpose: Test cases: (1) valid config.toml loads and parses correctly, (2) LIFE_ENGINE_STORAGE_PATH env var overrides storage.path from TOML, (3) env vars take precedence over TOML values, (4) missing config file creates default config, (5) missing required field (storage) returns ConfigError::MissingSection, (6) invalid TOML syntax returns parse error with line/column, (7) sensitive values are redacted in log output (test by capturing tracing output), (8) multiple validation errors are collected and reported together, (9) zero configured transports produces a warning but not an error. Use tempdir for test config files and std::env::set_var for env var tests (with cleanup). -->
  <!-- requirements: 1.1, 1.2, 1.4, 2.1, 2.2, 2.5 -->
  <!-- leverage: none -->

---

## 9.5 — Key Derivation Integration
> spec: .odm/spec/binary-and-startup/brief.md

- [x] Wire Argon2id key derivation into Core startup
  <!-- file: apps/core/src/main.rs -->
  <!-- purpose: During startup step 3 (derive database key), call life_engine_crypto::derive_key() with the master passphrase from config (storage.passphrase or LIFE_ENGINE_STORAGE_PASSPHRASE env var) and a salt (stored alongside the database or generated on first run). Store the derived 32-byte key in memory for passing to StorageBackend::init(). The passphrase is never stored — only the derived key is kept. The salt is stored in a file next to the database (e.g., data/salt.bin) or in the database header. On first run (no existing database): generate a random salt using life_engine_crypto::generate_salt(), derive the key, save the salt. On subsequent runs: read the existing salt, derive the key. Log "Database key derived" at info level (never log the key or passphrase). -->
  <!-- requirements: 3.1, 3.2 -->
  <!-- leverage: packages/crypto from Phase 3 -->

---

## 9.6 — Storage Backend Initialization
> spec: .odm/spec/binary-and-startup/brief.md

- [x] Implement storage backend initialization call in Core
  <!-- file: apps/core/src/main.rs -->
  <!-- purpose: During startup step 4 (initialize storage), call StorageBackend::init() with the derived key and the [storage] config section. Core does not know about SQLCipher internals — it uses the StorageBackend trait. Handle initialization errors with clear messages: if the key is wrong (SQLCipher can't decrypt), suggest checking the passphrase; if the database file doesn't exist, log that a new database is being created; if permissions are insufficient, report the path and required permissions. Wrap the StorageBackend in Arc for sharing across modules. Log "Storage initialized" with database path (but not key) at info level. -->
  <!-- requirements: 4.1, 4.4 -->
  <!-- leverage: packages/storage-sqlite from Phase 5 -->

- [x] Implement SQLCipher storage backend init in the storage crate
  <!-- file: packages/storage-sqlite/src/lib.rs -->
  <!-- purpose: Implement the StorageBackend::init() associated function for SqliteStorage: (1) extract path from the config section, (2) open the SQLCipher database with the derived key, (3) run PRAGMA key with the hex-encoded key, (4) enable WAL mode, (5) create tables if they don't exist using schema DDL, (6) run any pending schema migrations (compare current schema version with expected), (7) return the initialized SqliteStorage. If the database file doesn't exist, create it. If the key is wrong, return a clear error (SQLCipher will fail to read the database header). -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4 -->
  <!-- leverage: packages/storage-sqlite from Phase 5 -->

---

## 9.7 — 10-Step Startup Orchestrator
> spec: .odm/spec/binary-and-startup/brief.md

- [x] Implement the complete 10-step startup sequence in main
  <!-- file: apps/core/src/main.rs -->
  <!-- purpose: Implement the #[tokio::main] async fn main() with the 10 startup steps in dependency order: (1) Load config — call load_config() and validate_config(), (2) Initialize logging — set up tracing subscriber with JSON formatter and configured log level, (3) Derive database key — call derive_key() with passphrase and salt, (4) Initialize storage — call StorageBackend::init() with key and config, (5) Initialize auth — call create_auth_provider() with [auth] config section, (6) Create workflow engine — call WorkflowEngine::new() with [workflows] config and plugin executor, (7) Load workflows — the engine loads and validates YAML workflow files, builds trigger registry, (8) Discover and load plugins — call load_plugins() with config, plugins directory, storage, and event bus, (9) Start active transports — for each configured transport, instantiate and start it with the workflow engine and auth provider, (10) Wait for shutdown signal — call shutdown::wait_for_signal(). Each step logs: step number, step name, and duration (e.g., "Step 4/10: Initialize storage... done (23ms)"). If any step fails, log the error and exit with non-zero code — do not continue with partially initialized system. Log total startup duration after step 10 begins. -->
  <!-- requirements: 5.1, 5.2, 5.3, 5.4 -->
  <!-- leverage: all previous phases -->

---

## 9.8 — Transport Instantiation
> depends: 9.7
> spec: .odm/spec/binary-and-startup/brief.md

- [x] Instantiate and start configured transports during startup
  <!-- file: apps/core/src/main.rs -->
  <!-- purpose: During startup step 9, iterate the transports section of CoreConfig. For each configured transport (e.g., "rest", "graphql", "caldav", "carddav", "webhook"): (1) match the transport name to the corresponding crate (transport-rest, transport-graphql, etc.), (2) instantiate the Transport implementation with the workflow engine reference, auth provider reference, and transport-specific config, (3) call transport.start() which binds to the configured address and port, (4) store the transport handle for shutdown. Only configured transports are started — if [transports.graphql] is not in config.toml, the GraphQL transport is not started. Log each transport: "Transport rest started on 127.0.0.1:3000". If a transport fails to start (e.g., port already in use), log the error but continue starting other transports — one failed transport should not prevent others from starting. -->
  <!-- requirements: from binary-and-startup spec -->
  <!-- leverage: transport crates from Phase 1 scaffolding -->

---

## 9.9 — Graceful Shutdown Handler
> spec: .odm/spec/binary-and-startup/brief.md

- [x] Implement shutdown handler with reverse-order teardown
  <!-- file: apps/core/src/shutdown.rs -->
  <!-- purpose: Implement pub async fn wait_for_signal() that listens for SIGTERM and SIGINT using tokio::signal. On signal receipt, log "Shutdown signal received, beginning graceful shutdown..." and execute teardown in reverse startup order: (1) Stop transports — call transport.stop() on each active transport, stop accepting new connections, finish in-flight requests with a configurable timeout (default 30 seconds), (2) Unload plugins — call plugin_manager.stop_all() which stops and unloads all plugins, (3) Stop workflow engine — drain the event bus, cancel scheduled tasks, wait for running workflows to complete (with timeout), (4) Shut down auth — clear cached JWKS keys, flush rate limiter state, (5) Close storage — flush WAL, close SQLCipher connection, ensure all pending writes are committed. Each teardown step has its own timeout (configurable, default 10 seconds each). If a step exceeds its timeout, log a warning and force-proceed to the next step. Log each step: "Shutdown step 1/5: Stopping transports... done". After all steps, exit with code 0 if clean, code 1 if any step timed out. -->
  <!-- requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7 -->
  <!-- leverage: none -->

---

## 9.10 — Shutdown Integration Test
> spec: .odm/spec/binary-and-startup/brief.md

- [x] Add shutdown integration test
  <!-- file: apps/core/tests/shutdown_test.rs -->
  <!-- purpose: Start Core with a minimal config (SQLite in-memory, no transports). Send SIGTERM to the process. Verify: (1) process exits with code 0, (2) no error output on stderr, (3) shutdown log messages appear in order (transports, plugins, workflow engine, auth, storage), (4) the database connection is properly closed (no WAL file remaining or properly checkpointed). This test requires spawning Core as a subprocess using std::process::Command and sending signals via nix::sys::signal or libc. Use a timeout to catch hangs. -->
  <!-- requirements: 6.1, 6.7 -->
  <!-- leverage: none -->

---

## 9.11 — Structured Logging
> spec: .odm/spec/binary-and-startup/brief.md

- [ ] Configure tracing with JSON output and configurable log levels
  <!-- file: apps/core/src/main.rs -->
  <!-- purpose: During startup step 2, initialize the tracing subscriber: (1) create a tracing_subscriber::fmt subscriber with JSON formatter (tracing_subscriber::fmt::json()), (2) set the global log level from config.logging.level (default "info"), (3) add common fields to all log entries: version (from Cargo.toml), pid (process ID), (4) configure per-module log levels if specified in config (e.g., logging.modules.storage = "debug"), (5) set as the global default subscriber. The JSON format ensures log entries are machine-parseable for log aggregation. Each startup step already logs its number, name, and duration — this WP ensures the logging infrastructure is properly configured before those logs are emitted. Request-level logging (correlation IDs, request paths, response times) is delegated to transport crates — Core only logs startup, shutdown, and system-level events. -->
  <!-- requirements: 8.1, 8.2, 8.3, 8.4 -->
  <!-- leverage: tracing, tracing-subscriber -->
