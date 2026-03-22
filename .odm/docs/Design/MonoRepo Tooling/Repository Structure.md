---
title: "Life Engine — Repository Structure"
tags: [life-engine, architecture, monorepo, git]
created: 2026-03-14
---

# Repository Structure

> Single monorepo for all core components. Community plugins live in their own repos.

---

## Why a Monorepo

- **Atomic cross-component changes** — A change to the `StorageAdapter` trait in Core and the Shell Data API in App is one PR, one CI run, one merge. No coordination across repos.
- **Shared types without version overhead** — `packages/types/` is a workspace member. During development, Core and App always use the same version. No publishing, no "which version of the types crate am I on?" confusion.
- **Single CI pipeline** — One `ci.yml` validates everything. Nx's affected detection means PRs only build what they touch.
- **Solo founder efficiency** — One clone, one branch, one mental model. No juggling multiple repos, no release coordination, no cross-repo dependency bumps.
- **Contributor onboarding** — `git clone` once, run one command, everything works.

## What Lives Outside the Monorepo

**Community/third-party plugins** are independent repositories. Plugin authors:

1. Create their own repo
2. Add `plugin-sdk-rs` (Core plugins) or `plugin-sdk-js` (App plugins) as a dependency
3. Implement the plugin contract
4. Compile and distribute independently

This keeps the monorepo focused on core components while the ecosystem scales without permission.

---

## Directory Layout

```
life-engine/
├── Cargo.toml              # Rust workspace root
├── nx.json                 # Nx config for polyglot task orchestration
├── justfile                # Common dev commands (just dev, just test, etc.)
├── README.md
├── LICENSE                 # Apache 2.0
├── CONTRIBUTING.md
├── CODE_OF_CONDUCT.md
├── SECURITY.md
│
├── apps/
│   ├── core/             # Rust Core binary
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs         # Entry point, config loading, startup
│   │       ├── config.rs       # YAML + env var config
│   │       ├── plugin_loader.rs # Discover, validate, load plugins
│   │       ├── message_bus.rs  # In-process async event emitter
│   │       ├── storage.rs      # StorageAdapter trait definition
│   │       ├── auth.rs         # AuthProvider trait, middleware
│   │       └── api/            # axum router, routes, middleware
│   │
│   └── app/               # Tauri v2 client
│       ├── src-tauri/      # Rust backend (Tauri commands, sidecar management)
│       │   ├── Cargo.toml
│       │   └── src/
│       └── src/            # Shell UI (HTML/CSS/JS)
│           ├── index.html
│           ├── shell/          # Shell framework (layout, navigation, theming)
│           ├── components/     # Shell design system (17 Web Components)
│           ├── plugin-loader/  # Plugin manifest reader, scoped API, lifecycle
│           ├── data/           # Local SQLite, SyncAdapter, Shell Data API
│           └── styles/         # CSS custom properties, theme tokens
│
├── packages/
│   ├── types/              # Shared types (Rust structs + TS interfaces)
│   │   ├── Cargo.toml      # Rust crate with serde derives
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── events.rs       # CalendarEvent, etc.
│   │       ├── tasks.rs        # Task CDM
│   │       ├── contacts.rs     # Contact CDM
│   │       ├── emails.rs       # Email CDM
│   │       ├── files.rs        # File metadata CDM
│   │       ├── notes.rs        # Note CDM
│   │       └── credentials.rs  # Credential CDM
│   │
│   ├── plugin-sdk-rs/      # Rust SDK for Core plugin authors
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── traits.rs       # HubPlugin, Connector traits
│   │       ├── store.rs        # Store trait (data access for plugins)
│   │       ├── route.rs        # Route registration types
│   │       └── events.rs       # Event types for message bus
│   │
│   └── plugin-sdk-js/      # JS/TS SDK for App plugin authors
│       ├── package.json
│       └── src/
│           ├── index.ts
│           ├── types.ts        # ShellAPI type definitions
│           ├── manifest.ts     # Plugin manifest schema + validation
│           └── helpers.ts      # Utilities for plugin development
│
├── plugins/
│   ├── engine/             # First-party Core plugins
│   │   ├── storage-sqlite/     # Default storage (SQLite + SQLCipher)
│   │   ├── connector-email/    # IMAP/SMTP connector
│   │   ├── connector-caldav/   # CalDAV connector
│   │   ├── connector-carddav/  # CardDAV connector
│   │   ├── connector-google-calendar/
│   │   ├── connector-google-contacts/
│   │   ├── connector-filesystem/
│   │   └── processor-search/   # Full-text search (tantivy)
│   │
│   └── life/               # First-party App plugins (Web Components)
│       ├── settings/            # Settings page plugin
│       ├── layout/              # Responsive sidebar/navigation plugin
│       ├── core-config/         # Core backend configuration plugin
│       ├── email-viewer/        # Email list + detail view
│       ├── calendar/            # Calendar views (month, week, day, agenda)
│       ├── tasks/               # Task manager
│       ├── notes/               # Notes editor
│       ├── contacts/            # Contact list + detail
│       ├── files/               # File browser
│       └── dashboard/           # Overview widgets
│
├── .odm/docs/
│   ├── site/               # Documentation site (Docusaurus or similar)
│   ├── adrs/               # Architecture Decision Records
│   └── schemas/            # JSON Schema files for canonical collections
│       ├── events.schema.json
│       ├── tasks.schema.json
│       └── ...
│
├── tools/
│   ├── templates/          # Plugin scaffolding templates
│   │   ├── engine-plugin/      # Minimal Core plugin (Rust)
│   │   ├── life-plugin-vanilla/ # Minimal App plugin (vanilla JS)
│   │   └── life-plugin-lit/    # Minimal App plugin (Lit)
│   └── scripts/            # Dev scripts, release helpers
│
└── .github/
    ├── workflows/
    │   ├── ci.yml              # PR validation (check, clippy, test, lint)
    │   └── release.yml         # Build + publish platform binaries
    ├── ISSUE_TEMPLATE/
    │   ├── bug_report.yml
    │   └── feature_request.yml
    └── PULL_REQUEST_TEMPLATE.md
```

---

## Tooling

- **Cargo workspaces** — Links all Rust crates. `cargo build` from root compiles everything. Each crate has its own `Cargo.toml` but shares a single `Cargo.lock`.
- **Nx** — Polyglot task orchestration. Runs Cargo commands for Rust crates and npm scripts for JS packages. `nx affected` ensures PRs only build/test what changed.
- **justfile** — Developer-facing commands:
  - `just dev-core` — Run Core in dev mode (cargo-watch)
  - `just dev-app` — Run App in dev mode
  - `just dev-all` — Run both (Core as App sidecar)
  - `just test` — Run all tests
  - `just lint` — Run all linters
  - `just new-plugin <name>` — Scaffold a new plugin from template

---

## Community Plugin Repos

Third-party plugin authors do not interact with the monorepo. Their repo structure is simple:

### Core Plugin (Rust)

```
my-connector/
├── Cargo.toml          # depends on life-engine-plugin-sdk
├── src/
│   └── lib.rs          # implements HubPlugin trait
└── README.md
```

### App Plugin (JS/TS)

```
my-widget/
├── package.json        # depends on @life-engine/plugin-sdk
├── plugin.json         # manifest (id, capabilities, collections, etc.)
├── src/
│   └── index.js        # Web Component definition
└── README.md
```

Community plugins are distributed as compiled artifacts (WASM modules for Core, JS bundles for App). Users install them by placing files in the plugins directory or through the plugin store (Phase 3).

---

## CI/CD

A single CI pipeline validates everything:

### `ci.yml` (on every PR)

- Rust: `cargo check`, `cargo clippy --deny warnings`, `cargo test`
- JS/TS: `npm ci`, `eslint`, `tsc --noEmit`, `vitest`
- Tauri: build check (compile, don't package)
- DCO: verify `Signed-off-by` on all commits
- `cargo-deny`: licence compliance + vulnerability scan

### `release.yml` (on version tag)

- Build platform binaries (macOS arm64/x86_64, Linux x86_64/aarch64, Windows x86_64)
- Build Tauri bundles (.dmg, .AppImage, .msi)
- Create GitHub Release with checksums
- Publish `plugin-sdk-rs` to crates.io (Core plugins)
- Publish `plugin-sdk-js` to npm (App plugins)

### Branch Strategy

- `main` — always releasable
- `feat/*`, `fix/*`, `.odm/docs/*` — short-lived branches merged via squash
- No long-lived branches other than `main`

---

## Related Documents

- [[03 - Projects/Life Engine/Planning/phases/Phase 0 — Foundation]] — Phase 0.1 covers initial repo setup tasks
- [[03 - Projects/Life Engine/Design/MonoRepo Tooling/Technical Overview]] — Tooling details and community plugin story
