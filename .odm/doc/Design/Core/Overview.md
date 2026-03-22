---
title: "Core — Overview"
tags: [life-engine, core, architecture, rust]
created: 2026-03-14
updated: 2026-03-23
---

# Core Overview

Core is a self-hosted Rust backend that aggregates personal data from external services, stores it locally with encryption, and exposes it through configurable transports. Core contains no business logic — it is a thin orchestrator that wires together independent modules.

All features are provided by plugins (WASM modules loaded at runtime). Data flows through declarative workflows that chain plugin steps into pipelines.

## Design Principles

Core's architecture is governed by the project-wide Design Principles. The most prominent in Core:

- **Separation of Concerns** — Core owns orchestration only. Transports handle protocols, the workflow engine chains plugins, plugins provide logic, the data layer persists.
- **Open/Closed Principle** — Adding a feature means adding a plugin or a workflow. Core's binary does not change.
- **Defence in Depth** — Every layer is independently secured: TLS on transport, auth middleware on every request, SQLCipher encryption at rest, WASM isolation for plugins.
- **Principle of Least Privilege** — Plugins get no capabilities unless explicitly granted. All scoping is enforced at runtime by the WASM host.
- **Fail-Fast with Defined States** — Config is validated on startup, schemas are validated at pipeline boundaries, and workflow compatibility is checked at creation time.

## Architecture

Four layers, each an independent crate:

```
  Client Request (App, web, mobile, CLI)
         |
         v
  +---------------------+
  |    Transport Layer   |  <- REST, GraphQL, CalDAV, CardDAV, Webhook
  |    Auth + Validate   |     (configurable — activate via config)
  +---------+-----------+
            |  PipelineMessage
            v
  +---------------------+
  |  Workflow Engine     |  <- Declarative YAML pipelines
  |                      |     Event bus + cron scheduler
  |  Plugin A -> B -> C  |  <- WASM-isolated (Extism)
  +---------+-----------+
            |  StorageContext
            v
  +---------------------+
  |    Data Layer        |  <- StorageBackend trait
  |                      |     SQLite/SQLCipher (current impl)
  +---------------------+
```

- **Transport Layer** — Protocol-specific entry points. Each transport is its own crate. The admin enables transports via config. See Transports.md.
- **Workflow Engine** — Chains plugin steps into pipelines. Owns the event bus and cron scheduler. Triggered by endpoints, events, or schedules. See Workflow.md.
- **Plugins** — WASM modules loaded at runtime. Accept and return a standard `PipelineMessage`. See Plugins.md.
- **Data Layer** — Abstract storage behind a `StorageBackend` trait. Plugins interact via `StorageContext` query builder. See Data.md.

## Tech Stack

- **Language** — Rust (edition 2024)
- **Async runtime** — tokio
- **WASM runtime** — Extism
- **Storage** — SQLite/SQLCipher (via StorageBackend trait)
- **Auth** — Pocket ID (OIDC)
- **TLS** — rustls
- **Config** — TOML (app settings) + YAML (workflow definitions)

## Core Binary

Core is a thin orchestrator. After all modules are extracted, `apps/core/src/` contains three files:

- `main.rs` — Startup wiring
- `config.rs` — Config loading and validation
- `shutdown.rs` — Graceful shutdown coordination

```rust
fn main() {
    let config = Config::load()?;
    let storage = storage::init(&config.storage)?;
    let auth = auth::init(&config.auth)?;
    let engine = WorkflowEngine::new(storage, auth);
    engine.load_workflows(&config.workflows)?;
    engine.load_plugins(&config.plugins)?;
    let transports = transports::start(&config.transports, &engine)?;
    shutdown::wait(transports)?;
}
```

## Startup Flow

1. Load config (TOML + env vars)
2. Validate config — reject invalid settings with clear errors
3. Derive database key from master passphrase (Argon2id)
4. Initialize storage backend (SQLite/SQLCipher)
5. Initialize auth module
6. Create workflow engine (with storage and auth)
7. Load workflow definitions from `workflows/` directory (YAML)
8. Discover and load plugins from configured plugins directory (WASM)
9. Start active transports (as configured)
10. Wait for shutdown signal

## Defaults

- **Localhost-only** — Binds to `127.0.0.1` by default. No network exposure without explicit config change.
- **Encrypted** — SQLCipher encryption at rest, TLS for all outbound connections.
- **Deny-by-default** — Plugins get no capabilities unless explicitly granted.
- **Offline-capable** — Everything works without internet except syncing from external services.

## Deployment Modes

- **Bundled with App** — Core runs as a subprocess of the Tauri app. One install, zero server setup.
- **Standalone binary** — Run on any machine. No runtime dependencies.
- **Docker container** — Single `docker run` command.
- **Home server** — Raspberry Pi, old laptop, NAS. Runs on 128 MB RAM.

## System Requirements

- 64-bit processor (ARM or x86)
- 128 MB RAM (Core alone)
- 100 MB disk (application)
