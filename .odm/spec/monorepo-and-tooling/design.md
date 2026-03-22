<!--
domain: infrastructure
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Spec — Monorepo and Tooling

## Purpose

This spec defines the monorepo structure, build tooling, and developer workflow for the Life Engine project. All first-party code lives in a single repository with unified builds, shared types, and a consistent developer experience.

## Why Monorepo

Life Engine uses a monorepo for the following reasons:

- **Atomic cross-component changes** — A change to a canonical data type updates the type definition, trait implementations, storage queries, transport handlers, and tests in a single commit. No version coordination across repos.
- **Shared types without version overhead** — The `packages/types` crate is consumed directly by all other crates without publishing intermediate versions. Changes are immediately visible to all consumers.
- **Solo founder efficiency** — Context switching between repos costs time. A monorepo keeps everything reachable from one editor window and one terminal session.
- **Contributor onboarding** — One clone, one command. New contributors run `just dev-all` and have Core + App running locally.

## Tooling

The monorepo uses four complementary tools, each handling a specific concern.

### Cargo Workspaces

All Rust crates are members of a single Cargo workspace. They share one `Cargo.lock` at the repository root, ensuring consistent dependency versions across all Rust code. The workspace includes the core binary, all shared packages, all transports, and all first-party plugins.

### Nx

Nx provides polyglot task orchestration across the monorepo. It understands the dependency graph between Cargo crates and JS/TS packages, enabling affected-only builds and test runs.

Key Nx capabilities used:

- **Task caching** — Build and test outputs are cached locally. Unchanged packages skip rebuilding.
- **Affected detection** — `nx affected:test` runs tests only for packages impacted by the current changeset.
- **Task pipeline** — Build order is derived from the dependency graph. No manual ordering required.
- **Polyglot support** — Nx wraps both Cargo commands and JS/TS commands behind a unified interface.

### pnpm

pnpm is the JavaScript package manager. It handles node_modules for the App and any JS-based tooling. Workspace protocol (`workspace:*`) links internal JS packages without publishing.

### justfile

The `justfile` provides developer-facing commands — short, memorable aliases for common workflows. These are the primary interface for day-to-day development.

Commands:

- `just dev-core` — Start Core in development mode with hot-reload
- `just dev-app` — Start the App in development mode with Tauri dev server
- `just dev-all` — Start both Core and App concurrently
- `just test` — Run all Rust tests across the workspace
- `just lint` — Run clippy across all Rust crates
- `just new-plugin` — Scaffold a new first-party plugin (prompts for name, scaffolds standard layout with manifest.toml)

## Directory Layout

```text
apps/
  core/                           → Thin binary (config, startup, shutdown)
packages/
  types/                          → CDM types, PipelineMessage, envelopes, shared enums
  traits/                         → Infrastructure contracts (StorageBackend, Transport, Plugin, EngineError)
  crypto/                         → Shared encryption primitives (AES-256-GCM, key derivation, HMAC)
  plugin-sdk/                     → Plugin author DX (re-exports types + traits, StorageContext, test helpers)
  storage-sqlite/                 → StorageBackend impl for SQLite/SQLCipher
  auth/                           → Auth module (Pocket ID/OIDC, API keys)
  workflow-engine/                → Pipeline executor, event bus, cron scheduler, YAML config parsing
  transport-rest/                 → REST transport
  transport-graphql/              → GraphQL transport
  transport-caldav/               → CalDAV transport
  transport-carddav/              → CardDAV transport
  transport-webhook/              → Inbound webhook transport
  test-utils/                     → Shared test utilities
plugins/
  connector-email/                → Email fetch/send (WASM)
  connector-calendar/             → Calendar sync (WASM)
  connector-contacts/             → Contact sync (WASM)
  connector-filesystem/           → File operations (WASM)
  webhook-sender/                 → Outbound webhook step (WASM)
  search-indexer/                 → Full-text search indexing (WASM)
  backup/                         → Backup pipeline steps (WASM)
.odm/docs/
  adrs/                           → Architecture Decision Records
tools/
  templates/                      → Plugin scaffolding templates
  scripts/                        → Build and utility scripts
Cargo.toml                        → Workspace-level Cargo config
Cargo.lock                        → Shared Rust dependency lock
nx.json                           → Nx configuration
pnpm-workspace.yaml               → pnpm workspace definition
justfile                          → Developer command aliases
config.toml                       → Application configuration (not checked in — example provided)
```

Directory responsibilities:

- **apps/** — Deployable application. Core is the thin orchestrator binary with three files: `main.rs`, `config.rs`, `shutdown.rs`.
- **packages/** — Shared libraries consumed by the core binary and each other. Each follows the standard crate layout. Not independently deployable.
- **plugins/** — First-party WASM plugins that ship with the platform. Each compiles to `wasm32-wasi` and contains `plugin.wasm` + `manifest.toml` after build.
- **.odm/docs/** — All project documentation. ADRs record architectural decisions.
- **tools/** — Build tooling, scaffolding templates, and utility scripts. Not deployed.

## Standard Crate Layout

Every crate follows the same internal convention:

```text
src/
  lib.rs          → Public API (init, Config re-export, trait impls)
  config.rs       → Config struct + TOML deserialization
  error.rs        → Module-specific error types implementing EngineError
  handlers/       → Request/response handling (transports) or steps/ (plugins)
    mod.rs
    ...
  types.rs        → Module-internal types (not shared)
  tests/
    mod.rs
    ...
```

For plugins, `handlers/` is replaced with `steps/` (one file per pipeline action) and `transform/` (input/output mapping to `PipelineMessage`).

## Dependency Graph

```text
types (no dependencies)
  ↑
traits (depends on types)
  ↑
crypto (depends on types)
  ↑
plugin-sdk (depends on types + traits, re-exports both)
  ↑
storage-sqlite (depends on types + traits + crypto)
auth (depends on types + traits + crypto)
workflow-engine (depends on types + traits)
transport-* (depends on types + traits)
  ↑
apps/core (wires everything together)

Plugins depend only on plugin-sdk (which re-exports types + traits)
```

Key rules:

- No circular dependencies — the Cargo workspace enforces this at build time
- Plugins never import storage, auth, transport, or core crates directly
- Transports never import each other
- `plugin-sdk` re-exports `types` and `traits` so plugin authors have a single dependency

## WASM Plugin Compilation

All first-party plugins compile to the `wasm32-wasi` target and are loaded at runtime via Extism. Core does not compile against any plugin.

Each plugin produces a directory:

```text
connector-email/
  plugin.wasm       → Compiled WASM module
  manifest.toml     → Plugin metadata, actions, config schema, capabilities
```

Build command: `cargo build --target wasm32-wasi -p connector-email`

## Community Plugins

Community plugins live outside the monorepo. They are independent repositories that depend on the published SDK.

For community plugins:

- Create a new Rust project
- Add `life-engine-plugin-sdk` as a Cargo dependency (from crates.io)
- Implement the plugin trait, compile to `wasm32-wasi`
- Distribute the `.wasm` file + `manifest.toml`

Community plugins have no build-time dependency on the monorepo. They depend only on the published `plugin-sdk` package.
