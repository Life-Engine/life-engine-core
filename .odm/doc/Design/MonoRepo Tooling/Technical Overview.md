---
title: "Technical Overview"
tags:
  - life-engine
  - design
  - monorepo
  - tooling
created: 2026-03-21
---

# Life Engine — Technical Overview
*Architecture, plugin system design, and monorepo strategy*

---

## Project Summary

Life Engine is a self-hosted personal data sovereignty platform with two primary components: Core, a Rust backend that aggregates and manages personal data, and App, a Tauri v2 cross-platform client that acts as a plugin-driven shell. Both components are designed around extensibility — every meaningful feature is a plugin.

The architecture is governed by a set of [[03 - Projects/Life Engine/Design/Principles|Design Principles]] that apply across all components. The monorepo structure directly supports *Single Source of Truth* (shared types in one package, consumed everywhere) and *Finish Before Widening* (atomic cross-component changes in one PR, vertical integration before horizontal expansion).

---

## System Architecture

### Core (Backend)

Core is a self-deployed Rust service responsible for data storage, plugin orchestration, and exposing a local API. It does not contain business logic directly — all features are provided by plugins loaded at runtime.

- **Transport** — REST/JSON via `axum`
- **Storage** — SQLite via `rusqlite` (default, encrypted with SQLCipher), pluggable via `StorageAdapter` trait
- **Plugin model** — Rust trait-based plugins (Phase 1), WASM sandboxed plugins via Extism (Phase 4)
- **Auth** — Local bearer token (Phase 1), Pocket ID OIDC sidecar (Phase 2)
- **Config** — YAML + environment variables

### App (UI Client)

App is a Tauri v2 application that provides window management, navigation, and a plugin container system. The frontend is built with Web Components rendered in a system webview. The Rust backend handles native operations, IPC, sidecar management, and plugin lifecycle.

- **Framework** — Tauri v2 (Rust backend + webview frontend)
- **Frontend** — Web Components (framework-agnostic). Lit recommended for plugin authors, vanilla JS also recommended
- **Plugin model** — Plugins are Web Components loaded dynamically. Each runs in a closed Shadow DOM for isolation
- **Sync** — PowerSync (default, abstracted behind `SyncAdapter` interface)

### Communication

App and Core communicate over a well-defined REST API. Shared types live in `packages/types/`. This separation means Core can be accessed by other clients in the future (web UI, mobile, CLI) without changes to Core itself.

```
Client (Webview)  ── Tauri IPC ──►  Client (Rust)  ── REST/JSON ──►  Core  ── Plugin API ──►  Plugins
```

---

## Plugin System Design

### Core Plugins (Rust Traits / WASM)

Core plugins implement a Rust trait interface. In Phase 1, plugins are compiled Rust crates loaded at startup. In Phase 4, community plugins run as WASM modules in Extism sandboxes for crash and security isolation.

- Each plugin implements the `CorePlugin` trait from `packages/plugin-sdk-rs`
- Plugins communicate through shared data collections, Core events, and workflows — no direct plugin-to-plugin calls
- Capability declarations in plugin metadata (enforced by WASM in Phase 4)
- Core discovers plugins via config paths
- Community plugins are distributed as WASM modules — no repo forking required

### App Plugins (Web Components)

App plugins are pure JavaScript. Each plugin is a Web Component with a closed Shadow DOM. The shell injects a scoped API (`__shellAPI`) before the plugin's `connectedCallback` fires.

- Plugins declare capabilities, collections, and allowed domains in `plugin.json`
- Shell enforces permissions at the API level — a plugin cannot access undeclared collections or domains
- Shell provides a design system of 17 pre-built Web Components (buttons, cards, modals, etc.) at zero bundle cost
- Plugins can use Lit (recommended), vanilla JS, React (host-provided shared module), or any framework

### Plugin SDKs

Two SDKs serve as the contracts for plugin authors:

**`plugin-sdk-rs`** (Rust, for Core plugins):
```rust
use life_engine_plugin_sdk::{CorePlugin, Store, Route};
```
Defines `CorePlugin` trait, `StorageAdapter` trait, `Route` type, and canonical collection types.

**`plugin-sdk-js`** (TypeScript, for App plugins):
Defines `ShellAPI` types, manifest schema validation, and helper utilities for plugin development.

Both SDKs are versioned independently from the core apps. v1.x is additive only (no removals). v2.x breaking changes require 12-month v1 overlap.

---

## Monorepo Structure

### Tooling

- **Module linking** — Cargo workspaces (all Rust crates share one `Cargo.lock`)
- **Task orchestration** — Nx (polyglot, handles Cargo and JS/TS tasks, affected-only builds)
- **Core plugin system** — Rust traits (Phase 1), Extism WASM (Phase 4)
- **App framework** — Tauri v2
- **App frontend** — Web Components (Lit recommended)
- **Storage** — SQLite + SQLCipher (default)
- **JS Package Manager** — pnpm
- **Shared types** — `packages/types/` (Rust + TypeScript)

### Directory Layout

See [[03 - Projects/Life Engine/Design/MonoRepo Tooling/Repository Structure]] for the full annotated directory tree.

```
life-engine/
├── Cargo.toml          # Workspace root
├── nx.json
├── apps/
│   ├── core/         # Rust Core binary (axum, tokio, rusqlite)
│   └── app/           # Tauri v2 client (Rust + Web Components)
│       ├── src-tauri/  # Rust backend (Tauri)
│       └── src/        # Shell UI (HTML/CSS/JS, Web Components)
├── packages/
│   ├── types/          # Shared types (Rust structs + TS interfaces)
│   ├── plugin-sdk-rs/  # Rust plugin SDK (Core plugins)
│   └── plugin-sdk-js/  # JS/TS plugin SDK (App plugins)
├── plugins/
│   ├── engine/         # First-party Core plugins (connectors, storage, processors)
│   └── life/           # First-party App plugins (UI, Web Components)
├── .odm/docs/
│   ├── site/           # Documentation site source
│   ├── adrs/           # Architecture Decision Records
│   └── schemas/        # JSON Schema files for canonical collections
├── tools/
│   ├── templates/      # Plugin scaffolding templates
│   └── scripts/        # Dev scripts, release helpers
└── .github/            # CI/CD workflows, issue/PR templates
```

---

## Community Plugin Story

### Core Repo Contributors

Contributors clone the monorepo, require Node.js (for Nx and the Life client frontend) and the Rust toolchain. Nx's affected detection means contributors only build what their change touches. The `nx graph` command gives new contributors a visual map of the dependency tree.

### External Plugin Authors

Third-party authors do not interact with the monorepo at all.

**Core plugin authors** create their own Rust repo, add `life-engine-plugin-sdk` as a Cargo dependency, implement the `CorePlugin` trait, compile to a WASM module, and distribute it. Users install it by placing the `.wasm` file in their plugins directory.

**App plugin authors** create their own JS/TS repo, add `@life-engine/plugin-sdk` as an npm dependency, build a Web Component, write a `plugin.json` manifest, and bundle it. Users install it through the plugin store or by placing it in the plugins directory.

- No forking of the main repo required
- No knowledge of Nx or internal tooling required
- Only dependency is the relevant plugin SDK
- Core WASM plugins can be written in any language that compiles to WASM (Rust, Go, C, AssemblyScript)
- App plugins can use any JS framework (or none)

The quality of the community plugin experience is determined entirely by plugin SDK design and versioning — not by the monorepo build tooling.
