<!--
domain: infrastructure
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Requirements Document — Monorepo and Tooling

## Introduction

Life Engine uses a monorepo structure to co-locate all first-party code (Core binary, shared crates, transports, plugins) in a single repository. The tooling layer provides Cargo workspaces for Rust, Nx for polyglot orchestration, pnpm for JavaScript package management, and a justfile for developer-facing commands. The repository contains many more crates than before, reflecting the new architecture where each module is an independent crate. Community plugins build independently against the published `plugin-sdk`.

## Alignment with Product Vision

- **Atomic Changes** — A single commit can update types, traits, storage, auth, transports, and tests together, preventing version drift across components.
- **Single Source of Truth** — One repository and one lockfile per language ensure consistent dependency versions.
- **Separation of Concerns** — Each module is its own crate with a standard internal layout.
- **The Pit of Success** — `just dev-all` starts the full stack in one command; new contributors are productive immediately.

## Requirements

### Requirement 1 — Cargo Workspace

**User Story:** As a developer, I want all Rust crates in a single Cargo workspace, so that they share a lockfile and build together without version coordination.

#### Acceptance Criteria

- 1.1. WHEN a developer opens the repository root THEN the system SHALL have a `Cargo.toml` workspace definition listing all Rust crates as members: `apps/core`, all `packages/*` crates, and all `plugins/*` crates.
- 1.2. WHEN any Rust crate is built THEN the system SHALL use the single shared `Cargo.lock` at the repository root for consistent dependency versions.
- 1.3. WHEN a new Rust crate is added to the repository THEN the system SHALL require it to be registered in the workspace `members` array.
- 1.4. WHEN the workspace is configured THEN the system SHALL list all crates in the dependency graph: `types`, `traits`, `crypto`, `plugin-sdk`, `storage-sqlite`, `auth`, `workflow-engine`, `transport-rest`, `transport-graphql`, `transport-caldav`, `transport-carddav`, `transport-webhook`, `test-utils`, and all plugin crates.

---

### Requirement 2 — Nx Orchestration

**User Story:** As a developer, I want Nx to orchestrate builds and tests across Rust and JS/TS, so that only affected packages are rebuilt and tested.

#### Acceptance Criteria

- 2.1. WHEN `nx affected:test` is run THEN the system SHALL execute tests only for packages impacted by the current changeset.
- 2.2. WHEN a build or test task completes successfully THEN the system SHALL cache the output locally so that subsequent identical runs skip rebuilding.
- 2.3. WHEN the dependency graph changes THEN the system SHALL automatically derive the correct build order without manual configuration.
- 2.4. WHEN Nx wraps Cargo commands THEN the system SHALL present them through the same unified task interface as JS/TS commands.

---

### Requirement 3 — pnpm Workspace

**User Story:** As a developer, I want pnpm workspaces to link internal JS packages, so that I can develop across packages without publishing intermediate versions.

#### Acceptance Criteria

- 3.1. WHEN `pnpm install` is run at the repository root THEN the system SHALL install dependencies for all workspace packages defined in `pnpm-workspace.yaml`.
- 3.2. WHEN an internal JS package is referenced by another workspace package THEN the system SHALL resolve it via the `workspace:*` protocol without publishing.
- 3.3. WHEN a developer adds a new JS/TS package THEN the system SHALL require it to be listed in `pnpm-workspace.yaml`.

---

### Requirement 4 — Justfile Commands

**User Story:** As a developer, I want short justfile commands for common workflows, so that I can start development, run tests, and scaffold plugins without remembering tool-specific flags.

#### Acceptance Criteria

- 4.1. WHEN a developer runs `just dev-core` THEN the system SHALL start the Core Rust API server in development mode with hot-reload.
- 4.2. WHEN a developer runs `just dev-app` THEN the system SHALL start the App with the Tauri dev server.
- 4.3. WHEN a developer runs `just dev-all` THEN the system SHALL start both Core and App concurrently.
- 4.4. WHEN a developer runs `just test` THEN the system SHALL run all Rust tests across the workspace.
- 4.5. WHEN a developer runs `just lint` THEN the system SHALL run clippy across all Rust crates.
- 4.6. WHEN a developer runs `just new-plugin` THEN the system SHALL prompt for name and scaffold a new plugin directory with the standard crate layout and `manifest.toml`.

---

### Requirement 5 — Directory Layout

**User Story:** As a developer, I want a documented directory structure, so that I know where to place new code and can find existing components predictably.

#### Acceptance Criteria

- 5.1. WHEN the repository is cloned THEN the system SHALL have the documented directory structure with `apps/core/`, `packages/` (types, traits, crypto, plugin-sdk, storage-sqlite, auth, workflow-engine, transport-rest, transport-graphql, transport-caldav, transport-carddav, transport-webhook, test-utils), `plugins/` (connector-email, connector-calendar, connector-contacts, connector-filesystem, webhook-sender, search-indexer, backup), `.odm/docs/`, and `tools/` at the root.
- 5.2. WHEN a first-party plugin is created THEN the system SHALL place it under `plugins/` with a WASM compilation target.
- 5.3. WHEN documentation is added THEN the system SHALL place it under `.odm/docs/` with ADRs in `adrs/`.

---

### Requirement 6 — Standard Crate Layout

**User Story:** As a developer, I want every crate to follow the same internal convention, so that navigating any crate is predictable.

#### Acceptance Criteria

- 6.1. WHEN a new crate is created THEN it SHALL follow the standard layout: `lib.rs`, `config.rs`, `error.rs`, `handlers/` (or `steps/` for plugins), `types.rs`, `tests/`.
- 6.2. WHEN a crate defines error types THEN they SHALL implement the `EngineError` trait with `code()`, `severity()`, and `source_module()` methods.
- 6.3. WHEN a plugin crate is structured THEN it SHALL use `steps/` instead of `handlers/` and include `transform/` for input/output mapping to `PipelineMessage`.

---

### Requirement 7 — Dependency Graph

**User Story:** As a developer, I want a clear dependency hierarchy, so that circular dependencies are impossible and build order is deterministic.

#### Acceptance Criteria

- 7.1. WHEN crate dependencies are declared THEN the system SHALL follow the dependency graph: `types` (no deps) -> `traits` (types) -> `crypto` (types) -> `plugin-sdk` (types + traits) -> `storage-sqlite`, `auth`, `workflow-engine`, `transport-*` (types + traits + crypto) -> `apps/core` (everything).
- 7.2. WHEN a plugin crate declares dependencies THEN it SHALL depend ONLY on `plugin-sdk`.
- 7.3. WHEN a circular dependency is introduced THEN the Cargo workspace SHALL refuse to build.

---

### Requirement 8 — WASM Plugin Compilation

**User Story:** As a plugin developer, I want plugins to compile to WASM, so that they run in a sandboxed environment with declared capabilities.

#### Acceptance Criteria

- 8.1. WHEN a first-party plugin is built THEN it SHALL compile to the `wasm32-wasi` target.
- 8.2. WHEN a compiled plugin is placed in the `plugins/` directory THEN it SHALL be a directory containing `plugin.wasm` and `manifest.toml`.
- 8.3. WHEN a plugin is compiled THEN its only Rust dependency SHALL be `plugin-sdk`.

---

### Requirement 9 — Community Plugin Independence

**User Story:** As a community plugin author, I want to build my plugin using only the published SDK, so that I do not need to clone the monorepo.

#### Acceptance Criteria

- 9.1. WHEN a community plugin author creates a project THEN the system SHALL allow them to build with only `life-engine-plugin-sdk` as a Cargo dependency from crates.io.
- 9.2. WHEN community plugin documentation references setup THEN the system SHALL confirm that no monorepo clone is required.
