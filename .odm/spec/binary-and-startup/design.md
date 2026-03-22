<!--
domain: binary-and-startup
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Core Binary and Startup

Reference: ARCHITECTURE.md

## Purpose

This spec defines the Core binary entry point and startup sequence. Core is a thin orchestrator — a self-hosted Rust backend that wires together independent modules but contains no business logic, route handlers, database queries, or protocol implementations. After all modules are extracted, `apps/core/src/` contains three files: `main.rs`, `config.rs`, and `shutdown.rs`. Everything else lives in independent crates under `packages/`.

## Tech Stack

- **Language** — Rust (edition 2024)
- **Async runtime** — `tokio`
- **WASM runtime** — Extism
- **Storage** — `StorageBackend` trait; SQLite/SQLCipher is the current implementation (`packages/storage-sqlite`)
- **Auth** — Pocket ID (OIDC) via `packages/auth`
- **TLS** — `rustls`
- **Config** — TOML (app settings) + YAML (workflow definitions, separate files)
- **Logging** — `tracing` crate with JSON output

## Binary Structure

The Core binary lives at `apps/core/`. It contains exactly three source files:

- `main.rs` — Startup wiring. Calls into modules in dependency order. No logic beyond sequencing.
- `config.rs` — Config loading and top-level validation. Reads `config.toml`, applies env var overrides, and hands sections to modules.
- `shutdown.rs` — Graceful shutdown coordination. Tears down resources in reverse startup order.

Core depends on every module crate but only calls their public `init` functions and passes config sections. Core never reaches into module internals.

## Startup Sequence

Core performs the following steps in order on launch:

1. **Load config** — Read `config.toml`. Apply environment variable overrides (prefixed `LIFE_ENGINE_*`). Later sources override earlier ones.
2. **Validate config** — Reject insecure or invalid settings with clear error messages. Each config section is handed to its owning module for module-specific validation.
3. **Derive database key** — Derive the encryption key from the master passphrase using Argon2id with these parameters: 64 MB memory, 3 iterations, 4 parallelism, 32-byte output.
4. **Initialize storage backend** — Call `StorageBackend::init()` with the derived key and the `[storage]` config section. The current implementation uses SQLite/SQLCipher. The storage backend handles database creation on first launch and schema migrations.
5. **Initialize auth module** — Call `auth::init()` with the `[auth]` config section. The auth module handles provider setup (Pocket ID/OIDC, WebAuthn).
6. **Create workflow engine** — Instantiate the workflow engine with references to the storage backend and auth module.
7. **Load workflow definitions** — Read YAML files from the configured `workflows/` directory. The workflow engine validates that referenced plugins exist and step types are compatible.
8. **Discover and load plugins** — Scan the configured plugins directory. Each plugin directory contains a `plugin.wasm` and a `manifest.toml`. Load each WASM module into the Extism runtime, grant approved capabilities, and reject plugins with unapproved capability requests.
9. **Start active transports** — Read the `[transports]` config section. For each enabled transport, call its `start()` function, passing a reference to the workflow engine. Only configured transports are started — there is no default transport.
10. **Wait for shutdown signal** — Block on `SIGTERM` or `SIGINT`. When received, begin graceful shutdown.

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::load()?;
    config.validate()?;
    let db_key = crypto::derive_key(&config.storage.passphrase)?;
    let storage = storage_sqlite::init(&config.storage, &db_key)?;
    let auth = auth::init(&config.auth)?;
    let engine = WorkflowEngine::new(storage.clone(), auth.clone());
    engine.load_workflows(&config.workflows.path)?;
    engine.load_plugins(&config.plugins)?;
    let transports = transports::start(&config.transports, &engine)?;
    shutdown::wait(transports, engine, storage).await?;
}
```

## Config Structure

Config is TOML for application settings. Workflow definitions are separate YAML files in a configured directory.

```toml
[storage]
backend = "sqlite"
path = "./data/core.db"

[auth]
provider = "pocket-id"
issuer = "https://auth.local"

[transports.rest]
host = "127.0.0.1"
port = 3000

[transports.graphql]
host = "127.0.0.1"
port = 3001

[workflows]
path = "./workflows/"

[plugins]
path = "./plugins/"

[logging]
level = "info"
format = "json"
```

Each module declares its own config struct. Core reads top-level section keys to determine which modules are active, then hands the raw TOML section to the relevant module for parsing:

- `[storage]` is handed to `packages/storage-sqlite`
- `[auth]` is handed to `packages/auth`
- `[transports.rest]` is handed to `packages/transport-rest`
- `[transports.graphql]` is handed to `packages/transport-graphql`
- `[plugins]` is used by Core for plugin discovery; individual plugin configs like `[plugins.connector-email]` are handed to each plugin
- `[workflows]` is used by the workflow engine for definition loading

Environment variables override any TOML value. The naming convention maps the TOML hierarchy to underscore-separated uppercase keys. For example, `storage.path` becomes `LIFE_ENGINE_STORAGE_PATH`.

## Defaults

- **Localhost-only** — Transports bind to `127.0.0.1` by default. No network exposure without explicit config change.
- **Encrypted storage** — SQLCipher encryption at rest is always enabled.
- **Deny-by-default plugins** — Plugins receive no capabilities unless explicitly granted in their manifest and approved in config.
- **Offline-capable** — Everything works without internet access except syncing from external services.
- **No default transport** — If no transports are configured, Core starts but accepts no connections. This is valid for headless/scheduler-only deployments.

## Deployment Modes

- **Bundled with App** — Core runs as a subprocess of the Tauri desktop app. One install, zero server setup. The App manages Core's lifecycle.
- **Standalone binary** — Run on any machine directly. No runtime dependencies beyond the binary itself.
- **Docker container** — Single `docker run` command. Config mounted as a volume.
- **Home server** — Raspberry Pi, old laptop, NAS. Runs on 128 MB RAM minimum.

## System Requirements

- 64-bit processor (ARM or x86)
- 128 MB RAM (Core alone)
- 100 MB disk (application binary and initial data)

## Graceful Shutdown

When Core receives `SIGTERM` or `SIGINT`, it shuts down in reverse startup order:

1. Stop all active transports — cease accepting new connections, drain in-flight requests
2. Unload all WASM plugins
3. Stop the workflow engine — stop the scheduler, wait for running tasks to complete
4. Shut down the auth module
5. Close the storage backend
6. If any step exceeds the configurable shutdown timeout, force shutdown and log a warning

## Structured Logging

Core uses the `tracing` crate for structured logging with JSON output. Every log entry includes a timestamp, level, module path, and structured fields. Startup steps are logged with step number, name, and duration.

Request-level logging (method, path, status, duration) is the responsibility of each transport crate, not Core.

Log levels are configurable via the `logging.level` config key. Default is `info`.

## Acceptance Criteria

- `cargo run -p core` starts Core, loads zero plugins, and waits for shutdown
- If the REST transport is configured, `GET /api/system/health` returns a valid health response
- Core shuts down cleanly on `SIGTERM` with no error output
- Config loads correctly from TOML with env var overrides applied in the correct priority order
- Invalid or insecure config values produce clear error messages and prevent startup
- Database encryption key is derived correctly and the storage backend opens on subsequent launches with the same passphrase
- Core starts with no transports configured (headless/scheduler-only mode)
