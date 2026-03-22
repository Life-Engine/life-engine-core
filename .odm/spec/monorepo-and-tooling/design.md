<!--
domain: infrastructure
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Spec — Monorepo and Tooling

## Contents

- [[#Purpose]]
- [[#Why Monorepo]]
- [[#Tooling]]
- [[#Directory Layout]]
- [[#Community Plugins]]
- [[#Acceptance Criteria]]

## Purpose

This spec defines the monorepo structure, build tooling, and developer workflow for the Life Engine project. All first-party code lives in a single repository with unified CI, shared types, and a consistent developer experience.

References:

- [[03 - Projects/Life Engine/Design/MonoRepo Tooling/Repository Structure]]
- [[03 - Projects/Life Engine/Design/MonoRepo Tooling/Technical Overview]]

## Why Monorepo

Life Engine uses a monorepo for the following reasons:

- **Atomic cross-component changes** — A change to a canonical data type updates the Rust struct, TypeScript interface, JSON Schema, and tests in a single commit. No version coordination across repos.
- **Shared types without version overhead** — The `types` package is consumed directly by Core and App without publishing intermediate versions. Changes are immediately visible to all consumers.
- **Single CI pipeline** — One workflow validates the entire system. Breaking changes are caught before merge, not after a downstream repo pulls a new dependency version.
- **Solo founder efficiency** — Context switching between repos costs time. A monorepo keeps everything reachable from one editor window and one terminal session.
- **Contributor onboarding** — One clone, one command. New contributors run `just dev-all` and have Core + App running locally.

## Tooling

The monorepo uses four complementary tools, each handling a specific concern.

### Cargo Workspaces

All Rust crates (Core, plugin-sdk-rs, first-party Rust plugins) are members of a single Cargo workspace. They share one `Cargo.lock` at the repository root, ensuring consistent dependency versions across all Rust code.

### Nx

Nx provides polyglot task orchestration across the monorepo. It understands the dependency graph between Cargo crates and JS/TS packages, enabling affected-only builds and test runs.

Key Nx capabilities used:

- **Task caching** — Build and test outputs are cached locally and in CI. Unchanged packages skip rebuilding.
- **Affected detection** — `nx affected:test` runs tests only for packages impacted by the current changeset.
- **Task pipeline** — Build order is derived from the dependency graph. No manual ordering required.
- **Polyglot support** — Nx wraps both Cargo commands and JS/TS commands behind a unified interface.

### pnpm

pnpm is the JavaScript package manager. It handles node_modules for the App, plugin-sdk-js, and any JS-based tooling. Workspace protocol (`workspace:*`) links internal JS packages without publishing.

### justfile

The `justfile` provides developer-facing commands — short, memorable aliases for common workflows. These are the primary interface for day-to-day development.

Commands:

- `just dev-core` — Start Core in development mode with hot-reload
- `just dev-app` — Start the App in development mode with Tauri dev server
- `just dev-all` — Start both Core and App concurrently
- `just test` — Run all tests (Rust + JS/TS)
- `just lint` — Run all linters (clippy + eslint + tsc)
- `just new-plugin` — Scaffold a new first-party plugin (prompts for name, type, language)

## Directory Layout

```text
life-engine/
  apps/
    core/                  # Rust — Core API server
    app/                   # TypeScript — Tauri + frontend shell
  packages/
    types/                 # Shared TypeScript types
    plugin-sdk-rs/         # Rust SDK crate for Core plugins
    plugin-sdk-js/         # TypeScript SDK for App plugins
  plugins/
    engine/                # First-party Core plugins (Rust)
    life/                  # First-party App plugins (JS/TS)
  .odm/docs/
    site/                  # Documentation site source
    adrs/                  # Architecture Decision Records
    schemas/               # Canonical JSON Schema files
  tools/
    templates/             # Plugin scaffolding templates
    scripts/               # Build and release scripts
  .github/
    workflows/             # CI/CD pipeline definitions
    ISSUE_TEMPLATE/        # Issue and PR templates
  Cargo.toml               # Workspace-level Cargo config
  Cargo.lock               # Shared Rust dependency lock
  nx.json                  # Nx configuration
  pnpm-workspace.yaml      # pnpm workspace definition
  justfile                 # Developer command aliases
```

Directory responsibilities:

- **apps/** — Deployable applications. Core is the Rust API server. App is the Tauri desktop application with the frontend shell.
- **packages/** — Shared libraries consumed by apps and plugins. Not independently deployable.
- **plugins/** — First-party plugins that ship with the platform. Organised by target: `engine/` for Core plugins (Rust), `life/` for App plugins (JS/TS).
- **.odm/docs/** — All project documentation. ADRs record architectural decisions. Schemas are the canonical JSON Schema files used for validation.
- **tools/** — Build tooling, scaffolding templates, and utility scripts. Not deployed.
- **.github/** — GitHub-specific configuration for CI/CD and issue management.

## Community Plugins

Community plugins live outside the monorepo. They are independent repositories that depend on the published SDKs.

For Core plugins:

- Create a new Rust project
- Add `life-engine-plugin-sdk` as a Cargo dependency (from crates.io)
- Implement `CorePlugin`, compile to `wasm32-wasi`
- Distribute the `.wasm` file

For App plugins:

- Create a new JS/TS project (or use `npx @life-engine/create-plugin`)
- Add `@life-engine/plugin-sdk` as a dev dependency (from npm)
- Write a Web Component, create `plugin.json`
- Distribute the built bundle

Community plugins have no build-time dependency on the monorepo. They depend only on the published SDK packages.

## Acceptance Criteria

1. The monorepo builds from a clean clone with a single command (`just dev-all` or equivalent)
2. Nx affected detection correctly identifies changed packages and runs only their tests
3. All `justfile` commands (`dev-core`, `dev-app`, `dev-all`, `test`, `lint`, `new-plugin`) execute successfully
4. A community Core plugin repo builds with only `life-engine-plugin-sdk` as a dependency
5. A community App plugin repo builds with only `@life-engine/plugin-sdk` as a dev dependency
