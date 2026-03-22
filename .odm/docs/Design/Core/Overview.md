---
title: "Engine — Overview"
tags: [life-engine, engine, architecture, rust]
created: 2026-03-14
---

# Core Overview

Core is a self-hosted Rust backend that aggregates personal data from external services, stores it locally, and exposes it through a REST API. It contains no business logic — all features are provided by plugins.

## Design Principles

Core's architecture is governed by the project-wide [[03 - Projects/Life Engine/Design/Principles|Design Principles]]. The most prominent in Core:

- **Separation of Concerns** — Core contains no business logic. It is an empty orchestrator: plugins provide all features, the workflow engine chains them, and the data layer stores their output.
- **Open/Closed Principle** — Adding a feature means adding a plugin. Core's binary does not change when a connector, processor, or data model is added.
- **Defence in Depth** — Every layer is independently secured: TLS on transport, auth middleware on every request, SQLCipher encryption at rest, individual credential encryption, and WASM isolation for plugins.
- **Principle of Least Privilege** — Plugins get no capabilities unless explicitly granted. All scoping is enforced at runtime by the host, not by convention.
- **Fail-Fast with Defined States** — Config is validated on startup, schema is validated before storage, and workflow compatibility is validated at creation time. The system never starts or persists data in an ambiguous state.

## Tech Stack

- **Language** — Rust
- **HTTP** — `axum`
- **Async runtime** — `tokio`
- **Storage** — `rusqlite` (SQLite)
- **Plugin isolation** — Extism (WASM)
- **Auth** — Pocket ID sidecar (OIDC)
- **TLS** — `rustls`
- **Config** — YAML + environment variables

## Architecture

```
  Client Request (App, web, mobile, CLI)
         |
         v
  +---------------------+
  |    API Layer         |  <- REST/JSON (axum), auth via Pocket ID / API key
  |    Auth + Validate   |
  +---------+-----------+
            |  validated input
            v
  +---------------------+
  |  Workflow Engine     |  <- Chains plugins in sequence
  |                     |
  |  Plugin A -> B -> C |  <- WASM-isolated (Extism)
  +---------+-----------+
            |  read/write
            v
  +---------------------+
  |    Data Layer        |  <- SQLite (rusqlite), encrypted (SQLCipher)
  +---------------------+

  +---------------------+
  |  Background         |  <- Cron-like scheduler (separate from workflows)
  |  Scheduler          |  <- Connector syncs, token rotation, cleanup
  +---------------------+
```

- **[[03 - Projects/Life Engine/Design/Core/API|API]]** — Receives requests, authenticates via Pocket ID or API key, validates input
- **[[03 - Projects/Life Engine/Design/Core/Workflow|Workflow Engine]]** — Chains plugins in sequence to process requests. Workflow definitions are API-managed.
- **[[03 - Projects/Life Engine/Design/Core/Plugins|Plugins]]** — All features are plugins running in WASM sandboxes. This includes connector plugins that sync data from external services. See [[03 - Projects/Life Engine/Design/Core/Connectors]].
- **[[03 - Projects/Life Engine/Design/Core/Data|Data]]** — SQLite storage, encryption, schema validation, and isolation per plugin

## Startup Flow

1. Load config (YAML + env vars)
2. Validate config — reject insecure settings with clear errors
3. Derive database key from master passphrase (Argon2id)
4. Open encrypted SQLite database (SQLCipher)
5. Spawn Pocket ID sidecar process
6. Discover and load plugins from configured paths
7. Each plugin registers its routes and runs `on_load`
8. Load workflow definitions from database
9. Start background scheduler (connector syncs, token rotation, cleanup)
10. Bind `axum` server to `127.0.0.1:3750`

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
