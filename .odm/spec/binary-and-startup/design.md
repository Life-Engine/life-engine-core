<!--
domain: binary-and-startup
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Core Binary and Startup

Reference: [[03 - Projects/Life Engine/Design/Core/Overview]]

## Purpose

This spec defines the Core binary entry point and startup sequence. Core is a self-hosted Rust backend that aggregates personal data, stores it locally in an encrypted SQLite database, and exposes a REST API. It contains no business logic — all features are provided by plugins.

## Tech Stack

- **Language** — Rust
- **HTTP** — `axum`
- **Async runtime** — `tokio`
- **Storage** — `rusqlite` with SQLCipher
- **Plugin isolation** — Extism (WASM)
- **Auth** — Pocket ID sidecar (Go binary, OIDC)
- **TLS** — `rustls`
- **Config** — YAML file + environment variable overrides + CLI argument overrides
- **Logging** — `tracing` crate with JSON output

## Startup Sequence

Core performs the following steps in order on launch:

1. **Load config** — Read YAML file at `~/.life-engine/config.yaml`. Apply environment variable overrides (prefixed `LIFE_ENGINE_*`). Apply CLI argument overrides. Later sources override earlier ones.
2. **Validate config** — Reject insecure settings with clear error messages. Refuse to start if critical values are missing or invalid.
3. **Derive database key** — Derive the encryption key from the master passphrase using Argon2id with these parameters: 64 MB memory, 3 iterations, 4 parallelism, 32-byte output.
4. **Open encrypted database** — Open the SQLite database via SQLCipher using the derived key. Create the database on first launch.
5. **Spawn Pocket ID sidecar** — Start the bundled Pocket ID Go binary as a managed subprocess. Core owns its lifecycle.
6. **Discover and load plugins** — Read plugin paths from config. Load each WASM module into the Extism runtime.
7. **Register plugin routes and run on_load** — Each loaded plugin registers its HTTP routes and executes its `on_load` initialisation.
8. **Load workflow definitions** — Read stored workflow definitions from the database. Validate that referenced plugins are loaded and step types are compatible.
9. **Start background scheduler** — Initialise the cron-like scheduler for connector syncs, token rotation, and cleanup tasks.
10. **Bind HTTP server** — Start the `axum` server on `127.0.0.1:3750`. Core is now ready to accept requests.

## Config Structure

```yaml
core:
  host: "127.0.0.1"
  port: 3750
  log_level: "info"
  log_format: "json"
  data_dir: "~/.life-engine/data"

auth:
  provider: "local-token"  # "local-token" (Phase 1) or "pocket-id" (Phase 2)
  pocket_id:
    binary_path: "/usr/local/bin/pocket-id"
    port: 3751

storage:
  encryption: true
  argon2:
    memory_mb: 64
    iterations: 3
    parallelism: 4

plugins:
  paths:
    - /usr/local/lib/life-engine/plugins/
  auto_enable: false

network:
  tls:
    enabled: false  # auto-enabled when host is not 127.0.0.1
    cert_path: ""
    key_path: ""
  cors:
    allowed_origins:
      - "http://localhost:1420"
  rate_limit:
    requests_per_minute: 60
```

Environment variables override any YAML value. The naming convention maps the YAML hierarchy to underscore-separated uppercase keys. For example, `core.port` becomes `LIFE_ENGINE_CORE_PORT`.

CLI arguments follow the same naming with dot notation: `--core.port=3750`.

## Defaults

- **Localhost-only** — Binds to `127.0.0.1` by default. No network exposure without explicit config change.
- **Encrypted storage** — SQLCipher encryption at rest is always enabled. TLS required for all outbound connections.
- **Deny-by-default plugins** — Plugins receive no capabilities unless explicitly granted in their manifest and approved at install time.
- **Offline-capable** — Everything works without internet access except syncing from external services.

## Deployment Modes

- **Bundled with App** — Core runs as a sidecar subprocess of the Tauri desktop app. One install, zero server setup. The App manages Core's lifecycle.
- **Standalone binary** — Run on any machine directly. No runtime dependencies beyond the binary itself.
- **Docker container** — Single `docker run` command. Config mounted as a volume.
- **Home server** — Raspberry Pi, old laptop, NAS. Runs on 128 MB RAM minimum.

## System Requirements

- 64-bit processor (ARM or x86)
- 128 MB RAM (Core alone)
- 100 MB disk (application binary and initial data)

## Graceful Shutdown

When Core receives a `SIGTERM` signal:

1. Stop accepting new HTTP connections
2. Wait for in-flight requests to complete (up to 5 seconds)
3. Stop the background scheduler and wait for running tasks to finish
4. Call `on_unload` on every loaded plugin
5. Close the SQLite database connection
6. Terminate the Pocket ID sidecar process
7. If any step exceeds the 5-second timeout, force shutdown

## Structured Logging

Core uses the `tracing` crate for structured logging with JSON output. Every log entry includes a timestamp, level, module path, and structured fields. Request logs include method, path, status code, and duration.

Log levels are configurable via the `core.log_level` config key. Default is `info`.

## Acceptance Criteria

- `cargo run` starts Core, loads zero plugins, and binds to `127.0.0.1:3750`
- `GET /api/system/health` returns a valid health response
- Core shuts down cleanly on `SIGTERM` with no error output
- Config loads correctly from YAML, with env var and CLI overrides applied in the correct priority order
- Invalid or insecure config values produce clear error messages and prevent startup
- Database encryption key is derived correctly and the database opens on subsequent launches with the same passphrase
