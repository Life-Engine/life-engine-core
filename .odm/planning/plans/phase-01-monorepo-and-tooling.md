<!--
project: life-engine-core
phase: 1
specs: monorepo-and-tooling
updated: 2026-03-23
-->

# Phase 1 — Monorepo Structure and Tooling

## Plan Overview

This phase establishes the foundational monorepo structure, build tooling, and developer workflow that all subsequent phases depend on. It restructures the Cargo workspace to include the new modular crate layout defined in ARCHITECTURE.md, configures Nx for polyglot task orchestration with caching, sets up pnpm for JavaScript packages, scaffolds all package and plugin crates with the standard internal layout, and creates developer-facing justfile commands. No application logic is written in this phase — it is purely structural.

This phase must complete before any crate-level implementation work begins. The output is a compilable workspace with empty crate shells following the standard convention: `lib.rs`, `config.rs`, `error.rs`, `handlers/` (or `steps/` for plugins), `types.rs`, `tests/`.

> spec: .odm/spec/monorepo-and-tooling/brief.md

Progress: 0 / 11 work packages complete

---

## 1.1 — Cargo Workspace Configuration
> spec: .odm/spec/monorepo-and-tooling/brief.md

- [x] Restructure root Cargo.toml workspace members to match new architecture
  <!-- file: Cargo.toml -->
  <!-- purpose: Update [workspace] members list to include apps/core, packages/types, packages/traits, packages/crypto, packages/plugin-sdk, packages/storage-sqlite, packages/auth, packages/workflow-engine, packages/transport-rest, packages/transport-graphql, packages/transport-caldav, packages/transport-carddav, packages/transport-webhook, packages/test-utils, packages/test-fixtures, and all plugins/* crates. Add [workspace.dependencies] section with shared dependency versions (serde = "1", serde_json = "1", tokio = { version = "1", features = ["full"] }, uuid = { version = "1", features = ["v4", "serde"] }, chrono = { version = "0.4", features = ["serde"] }, thiserror = "2", anyhow = "1", tracing = "0.1", async-trait = "0.1"). Remove any members that no longer exist in the new structure. Preserve existing workspace settings like resolver = "2" and edition = "2024". -->
  <!-- requirements: 1.1, 1.2, 1.3, 1.4 -->
  <!-- leverage: existing Cargo.toml -->

---

## 1.2 — Nx Configuration
> spec: .odm/spec/monorepo-and-tooling/brief.md

- [x] Configure Nx task pipelines for Rust build, test, and lint
  <!-- file: nx.json -->
  <!-- purpose: Define task pipeline with build depending on ^build (upstream first), test depending on build, lint independent. Configure task caching for build and test outputs. Add @monodon/rust plugin configuration for Cargo integration. Set defaultBase to main branch. Configure namedInputs for Rust source files (src/**/*.rs, Cargo.toml, Cargo.lock). Ensure affected detection works by tracking Cargo.toml dependency changes. -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.4 -->
  <!-- leverage: existing nx.json -->

---

## 1.3 — pnpm Workspace Configuration
> spec: .odm/spec/monorepo-and-tooling/brief.md

- [x] Configure pnpm workspace for JavaScript/TypeScript packages
  <!-- file: pnpm-workspace.yaml -->
  <!-- purpose: List apps/app (Tauri frontend) and any JS/TS packages as workspace members. Use workspace:* protocol for internal dependencies. Ensure pnpm-lock.yaml is committed. Verify pnpm install succeeds with the updated workspace configuration. -->
  <!-- requirements: 3.1, 3.2, 3.3 -->
  <!-- leverage: existing pnpm-workspace.yaml -->

---

## 1.4 — Package Crate Scaffolding
> depends: 1.1
> spec: .odm/spec/monorepo-and-tooling/brief.md

- [x] Scaffold packages/traits crate with standard layout
  <!-- file: packages/traits/Cargo.toml -->
  <!-- file: packages/traits/src/lib.rs -->
  <!-- purpose: Create Cargo.toml with name = "life-engine-traits", dependencies on life-engine-types. Create src/lib.rs with module declarations for storage, transport, plugin, error. Create src/storage.rs (empty StorageBackend trait placeholder), src/transport.rs (empty Transport trait placeholder), src/plugin.rs (empty Plugin trait placeholder), src/error.rs (EngineError trait placeholder), src/types.rs (module-internal types), src/tests/mod.rs (empty test module). All files compile with no errors. -->
  <!-- requirements: 5.1, 6.1, 6.2, 7.1 -->
  <!-- leverage: none -->

- [x] Scaffold packages/crypto crate with standard layout
  <!-- file: packages/crypto/Cargo.toml -->
  <!-- file: packages/crypto/src/lib.rs -->
  <!-- purpose: Create Cargo.toml with name = "life-engine-crypto", dependencies on life-engine-types, aes-gcm, argon2, hmac, sha2. Create src/lib.rs with module declarations. Create src/encryption.rs (AES-256-GCM placeholder), src/kdf.rs (Argon2id placeholder), src/hmac.rs (HMAC placeholder), src/error.rs (CryptoError types), src/tests/mod.rs. All files compile with no errors. -->
  <!-- requirements: 5.1, 6.1, 7.1 -->
  <!-- leverage: none -->

- [x] Scaffold packages/storage-sqlite crate with standard layout
  <!-- file: packages/storage-sqlite/Cargo.toml -->
  <!-- file: packages/storage-sqlite/src/lib.rs -->
  <!-- purpose: Create Cargo.toml with name = "life-engine-storage-sqlite", dependencies on life-engine-types, life-engine-traits, life-engine-crypto, rusqlite with bundled-sqlcipher feature. Create src/lib.rs, src/config.rs, src/error.rs, src/schema.rs, src/backend.rs, src/validation.rs, src/credentials.rs, src/audit.rs, src/export.rs, src/types.rs, src/tests/mod.rs. All files compile. -->
  <!-- requirements: 5.1, 6.1, 7.1 -->
  <!-- leverage: none -->

- [x] Scaffold packages/auth crate with standard layout
  <!-- file: packages/auth/Cargo.toml -->
  <!-- file: packages/auth/src/lib.rs -->
  <!-- purpose: Create Cargo.toml with name = "life-engine-auth", dependencies on life-engine-types, life-engine-traits, life-engine-crypto, jsonwebtoken, reqwest. Create src/lib.rs, src/config.rs, src/error.rs, src/handlers/mod.rs, src/handlers/validate.rs, src/handlers/rate_limit.rs, src/handlers/keys.rs, src/types.rs, src/tests/mod.rs. All files compile. -->
  <!-- requirements: 5.1, 6.1, 7.1 -->
  <!-- leverage: none -->

- [x] Scaffold packages/workflow-engine crate with standard layout
  <!-- file: packages/workflow-engine/Cargo.toml -->
  <!-- file: packages/workflow-engine/src/lib.rs -->
  <!-- purpose: Create Cargo.toml with name = "life-engine-workflow-engine", dependencies on life-engine-types, life-engine-traits, serde_yaml, cron, tokio. Create src/lib.rs, src/config.rs, src/error.rs, src/types.rs, src/loader.rs, src/executor.rs, src/event_bus.rs, src/scheduler.rs, src/tests/mod.rs. All files compile. -->
  <!-- requirements: 5.1, 6.1, 7.1 -->
  <!-- leverage: none -->

- [x] Scaffold all transport-* crates with standard layout
  <!-- file: packages/transport-rest/Cargo.toml -->
  <!-- file: packages/transport-rest/src/lib.rs -->
  <!-- purpose: Create five transport crates (transport-rest, transport-graphql, transport-caldav, transport-carddav, transport-webhook) each with Cargo.toml depending on life-engine-types and life-engine-traits. Each crate gets src/lib.rs, src/config.rs, src/error.rs, src/handlers/mod.rs, src/types.rs, src/tests/mod.rs. transport-rest depends on axum; transport-graphql depends on async-graphql; transport-caldav and transport-carddav depend on life-engine-dav-utils; transport-webhook depends on axum. All files compile. -->
  <!-- requirements: 5.1, 6.1, 7.1 -->
  <!-- leverage: none -->

---

## 1.5 — Plugin Crate Scaffolding
> depends: 1.1
> spec: .odm/spec/monorepo-and-tooling/brief.md

- [x] Scaffold all first-party plugin crates with WASM-compatible layout
  <!-- file: plugins/connector-email/Cargo.toml -->
  <!-- file: plugins/connector-email/src/lib.rs -->
  <!-- file: plugins/connector-email/manifest.toml -->
  <!-- purpose: Create seven plugin crates (connector-email, connector-calendar, connector-contacts, connector-filesystem, webhook-sender, search-indexer, backup) each with Cargo.toml depending only on life-engine-plugin-sdk. Each crate gets src/lib.rs, src/config.rs, src/error.rs, src/steps/mod.rs (pipeline step handlers), src/transform/mod.rs (PipelineMessage input/output mapping), src/types.rs, src/tests/mod.rs. Each crate also gets a manifest.toml with [plugin] section (id, name, version, description), [actions.*] sections listing available actions, [capabilities] section declaring required capabilities, and optional [config] section with JSON Schema. Set crate-type = ["cdylib"] for WASM compilation. All files compile natively (WASM target tested in Phase 1.11). -->
  <!-- requirements: 5.2, 6.3, 7.2, 8.1, 8.2, 8.3 -->
  <!-- leverage: none -->

---

## 1.6 — Justfile Development Commands
> depends: 1.1, 1.2, 1.3
> spec: .odm/spec/monorepo-and-tooling/brief.md

- [x] Create justfile with dev-core, dev-app, and dev-all recipes
  <!-- file: justfile -->
  <!-- purpose: Add dev-core recipe that runs cargo-watch on apps/core with automatic restart on source changes. Add dev-app recipe that starts the Tauri development server for the frontend. Add dev-all recipe that runs both dev-core and dev-app concurrently using just's parallel execution. Each recipe should include clear console output indicating which service is starting. dev-core should watch packages/ and apps/core/ for changes. -->
  <!-- requirements: 4.1, 4.2, 4.3 -->
  <!-- leverage: existing justfile if present -->

---

## 1.7 — Justfile Quality Commands
> depends: 1.6
> spec: .odm/spec/monorepo-and-tooling/brief.md

- [x] Add test, lint, and new-plugin recipes to the justfile
  <!-- file: justfile -->
  <!-- purpose: Add test recipe that runs cargo test across the entire workspace with --workspace flag. Add lint recipe that runs cargo clippy across all crates with -D warnings to treat warnings as errors. Add new-plugin recipe that takes a plugin name argument, copies the plugin scaffold template from tools/templates/plugin/, replaces {{name}} and {{id}} placeholders with the provided name, adds the new crate to the Cargo.toml workspace members list, and prints success message with next steps. -->
  <!-- requirements: 4.4, 4.5, 4.6 -->
  <!-- leverage: justfile from WP 1.6 -->

---

## 1.8 — Plugin Scaffold Template
> spec: .odm/spec/monorepo-and-tooling/brief.md

- [ ] Create plugin scaffold template for WASM plugins
  <!-- file: tools/templates/plugin/Cargo.toml -->
  <!-- file: tools/templates/plugin/src/lib.rs -->
  <!-- file: tools/templates/plugin/manifest.toml -->
  <!-- purpose: Create a template directory at tools/templates/plugin/ containing: Cargo.toml with {{name}} as package name and life-engine-plugin-sdk as the only dependency with crate-type = ["cdylib"]; src/lib.rs with a skeleton Plugin trait implementation using register_plugin! macro and a single placeholder action; src/config.rs with empty config struct; src/error.rs with plugin-specific error enum; src/steps/mod.rs with placeholder step handler; src/transform/mod.rs with placeholder input/output mapping; src/types.rs empty; src/tests/mod.rs with a placeholder test; manifest.toml with [plugin] section using {{id}} and {{name}} placeholders, one placeholder action, empty capabilities section, and optional config schema. All template files use {{name}} for the Rust crate name and {{id}} for the plugin identifier. -->
  <!-- requirements: 4.6 -->
  <!-- leverage: none -->

---

## 1.9 — Directory Layout Verification
> spec: .odm/spec/monorepo-and-tooling/brief.md

- [ ] Verify and create the complete documented directory structure
  <!-- file: apps/, packages/, plugins/, .odm/doc/, tools/ -->
  <!-- purpose: Walk the directory tree and confirm all required directories from ARCHITECTURE.md exist: apps/core/, packages/types/, packages/traits/, packages/crypto/, packages/plugin-sdk/, packages/storage-sqlite/, packages/auth/, packages/workflow-engine/, packages/transport-rest/, packages/transport-graphql/, packages/transport-caldav/, packages/transport-carddav/, packages/transport-webhook/, packages/test-utils/, packages/test-fixtures/, plugins/connector-email/, plugins/connector-calendar/, plugins/connector-contacts/, plugins/connector-filesystem/, plugins/webhook-sender/, plugins/search-indexer/, plugins/backup/, tools/templates/plugin/. Create missing directories with .gitkeep files. Verify each crate has the standard internal layout (lib.rs, config.rs, error.rs, handlers/ or steps/, types.rs, tests/). Report any deviations. -->
  <!-- requirements: 5.1, 5.2, 5.3 -->
  <!-- leverage: none -->

---

## 1.10 — Affected Detection Verification
> depends: 1.2
> spec: .odm/spec/monorepo-and-tooling/brief.md

- [ ] Verify Nx affected detection and task caching work correctly
  <!-- file: nx.json -->
  <!-- purpose: Make a trivial change to packages/types/src/lib.rs (e.g., add a doc comment), run nx affected:test, confirm that only packages/types and its downstream dependents (packages/traits, packages/crypto, packages/plugin-sdk, packages/storage-sqlite, packages/auth, packages/workflow-engine, transport crates, apps/core) are tested — not unrelated crates. Run the same command again and confirm caching prevents re-execution. Revert the trivial change after verification. -->
  <!-- requirements: 2.1, 2.2 -->
  <!-- leverage: Nx config from WP 1.2 -->

---

## 1.11 — Community Plugin Build Verification
> depends: 1.1
> spec: .odm/spec/monorepo-and-tooling/brief.md

- [ ] Verify community plugins build independently from the monorepo
  <!-- file: (external test project) -->
  <!-- purpose: Create a temporary test plugin project outside the monorepo with only life-engine-plugin-sdk as a Cargo dependency (using a path dependency for now, simulating a future published crate). Implement a minimal Plugin trait with one action. Run cargo build --target wasm32-wasi and confirm it compiles to a valid WASM module without needing any other monorepo crates as direct dependencies. Verify the produced .wasm file is loadable by Extism. Clean up the temporary project after verification. -->
  <!-- requirements: 9.1, 9.2 -->
  <!-- leverage: published plugin-sdk package -->
