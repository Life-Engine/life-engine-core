<!--
domain: infrastructure
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Implementation Plan — Monorepo and Tooling

## Task Overview

This plan sets up the monorepo structure, build tooling, and developer workflow. Work starts with the foundational configuration files (Cargo workspace, Nx, pnpm), then adds the crate scaffolding, justfile commands, plugin scaffolding templates, and community plugin build verification.

**Progress:** 0 / 11 tasks complete

## Steering Document Compliance

- Single Cargo workspace with shared lockfile follows Single Source of Truth
- Nx affected detection keeps feedback loops fast
- `just dev-all` provides one-command onboarding (The Pit of Success)
- Every crate follows the standard internal layout (lib.rs, config.rs, error.rs, handlers/, types.rs, tests/)
- Dependency graph: types -> traits -> crypto -> plugin-sdk -> storage/auth/workflow/transports -> apps/core
- Plugins compile to WASM (wasm32-wasi) and depend only on plugin-sdk
- Community plugins build independently against published SDK

## Atomic Task Requirements

- **File Scope:** 1-3 related files maximum
- **Time Boxing:** 15-30 minutes per task
- **Single Purpose:** one testable outcome per task
- **Specific Files:** exact file paths specified
- **Agent-Friendly:** clear input/output, minimal context switching

---

## 1.1 — Cargo Workspace Configuration
> spec: ./brief.md

- [ ] Set up the root Cargo workspace with all Rust crate members
  <!-- file: Cargo.toml -->
  <!-- purpose: Create [workspace] section listing apps/core, all packages/* crates (types, traits, crypto, plugin-sdk, storage-sqlite, auth, workflow-engine, transport-rest, transport-graphql, transport-caldav, transport-carddav, transport-webhook, test-utils), and all plugins/* crates as members with shared dependency declarations -->
  <!-- requirements: 1.1, 1.2, 1.3, 1.4 -->
  <!-- leverage: existing Cargo.toml -->

---

## 1.2 — Nx Configuration
> spec: ./brief.md

- [ ] Set up Nx for polyglot task orchestration with caching
  <!-- file: nx.json -->
  <!-- purpose: Create task pipeline definitions for build, test, and lint with task caching and Cargo plugin configuration -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.4 -->
  <!-- leverage: existing nx.json -->

---

## 1.3 — pnpm Workspace Configuration
> spec: ./brief.md

- [ ] Set up pnpm workspace for JavaScript packages
  <!-- file: pnpm-workspace.yaml -->
  <!-- purpose: List apps/app and any JS/TS packages as workspace members with workspace:* protocol -->
  <!-- requirements: 3.1, 3.2, 3.3 -->
  <!-- leverage: existing pnpm-workspace.yaml -->

---

## 2.1 — Package Crate Scaffolding
> spec: ./brief.md
> depends: 1.1

- [ ] Scaffold all package crates with standard internal layout
  <!-- file: packages/types/src/lib.rs -->
  <!-- file: packages/traits/src/lib.rs -->
  <!-- file: packages/crypto/src/lib.rs -->
  <!-- purpose: Create crate directories and standard files (lib.rs, config.rs, error.rs, handlers/, types.rs, tests/) for types, traits, crypto, plugin-sdk, storage-sqlite, auth, workflow-engine, all transport-* crates, and test-utils -->
  <!-- requirements: 5.1, 6.1, 6.2, 7.1 -->
  <!-- leverage: none -->

---

## 2.2 — Plugin Crate Scaffolding
> spec: ./brief.md
> depends: 1.1

- [ ] Scaffold all first-party plugin crates with WASM target configuration
  <!-- file: plugins/connector-email/Cargo.toml -->
  <!-- file: plugins/connector-email/src/lib.rs -->
  <!-- purpose: Create plugin crate directories with standard layout (lib.rs, config.rs, error.rs, steps/, transform/, types.rs, tests/) and manifest.toml for connector-email, connector-calendar, connector-contacts, connector-filesystem, webhook-sender, search-indexer, and backup -->
  <!-- requirements: 5.2, 6.3, 7.2, 8.1, 8.2, 8.3 -->
  <!-- leverage: none -->

---

## 3.1 — Justfile Development Commands
> spec: ./brief.md
> depends: 1.1, 1.2, 1.3

- [ ] Create justfile with dev-core, dev-app, and dev-all recipes
  <!-- file: justfile -->
  <!-- purpose: Add dev-core (cargo-watch), dev-app (Tauri dev server), and dev-all (concurrent) recipes -->
  <!-- requirements: 4.1, 4.2, 4.3 -->
  <!-- leverage: existing justfile -->

---

## 3.2 — Justfile Quality Commands
> spec: ./brief.md
> depends: 3.1

- [ ] Add test, lint, and new-plugin recipes to the justfile
  <!-- file: justfile -->
  <!-- purpose: Add test (cargo test across workspace), lint (clippy across all crates), and new-plugin (scaffold from template with manifest.toml) recipes -->
  <!-- requirements: 4.4, 4.5, 4.6 -->
  <!-- leverage: justfile from WP 3.1 -->

---

## 4.1 — Plugin Scaffold Template
> spec: ./brief.md

- [ ] Create plugin scaffold template for WASM plugins
  <!-- file: tools/templates/plugin/Cargo.toml -->
  <!-- file: tools/templates/plugin/src/lib.rs -->
  <!-- file: tools/templates/plugin/manifest.toml -->
  <!-- purpose: Create template with {{name}} and {{id}} placeholders for just new-plugin scaffolding; include standard layout (lib.rs, config.rs, error.rs, steps/, transform/, types.rs, tests/) and manifest.toml -->
  <!-- requirements: 4.6 -->
  <!-- leverage: none -->

---

## 4.2 — Directory Layout Verification
> spec: ./brief.md

- [ ] Verify and create the documented directory structure
  <!-- file: apps/, packages/, plugins/, .odm/docs/, tools/ -->
  <!-- purpose: Confirm all required directories exist; create missing directories with .gitkeep files; verify standard crate layout in each existing crate -->
  <!-- requirements: 5.1, 5.2, 5.3 -->
  <!-- leverage: none -->

---

## 5.1 — Affected Detection Verification
> spec: ./brief.md
> depends: 1.2

- [ ] Verify Nx affected detection and task caching work correctly
  <!-- file: nx.json -->
  <!-- purpose: Make a change to a single package, run nx affected:test, confirm only changed packages are tested and caching prevents re-runs -->
  <!-- requirements: 2.1, 2.2 -->
  <!-- leverage: Nx config from WP 1.2 -->

---

## 5.2 — Community Plugin Build Verification
> spec: ./brief.md
> depends: 1.1

- [ ] Verify community plugins build independently from the monorepo
  <!-- file: (external test project) -->
  <!-- purpose: Create test plugin project with only life-engine-plugin-sdk as a Cargo dependency; confirm it compiles to wasm32-wasi without monorepo access -->
  <!-- requirements: 9.1, 9.2 -->
  <!-- leverage: published plugin-sdk package -->
