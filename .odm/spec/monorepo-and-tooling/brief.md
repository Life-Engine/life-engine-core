<!--
domain: infrastructure
status: draft
tier: 1
updated: 2026-03-23
-->

# Monorepo and Tooling Spec

## Overview

Defines the monorepo structure, build tooling, and developer workflow for the Life Engine project. All first-party code lives in a single repository with unified builds, shared types, and a consistent developer experience powered by Cargo workspaces, Nx orchestration, pnpm, and a justfile. The repository contains a thin core binary, many independent Rust crates, WASM-compiled plugins, and supporting tooling.

## Goals

- **Atomic cross-component changes** — Allow a single commit to update types, traits, storage, auth, transports, and tests together.
- **Zero-coordination dependencies** — Share types and traits across all crates without publishing intermediate package versions.
- **Fast feedback loops** — Use task caching and affected-only builds to keep local iteration fast.
- **One-command onboarding** — Enable new contributors to clone the repo and run the full stack with a single command.
- **Community plugin independence** — Ensure community plugins depend only on published `plugin-sdk`, not the monorepo.
- **Standard crate layout** — Every crate follows the same internal convention: `lib.rs`, `config.rs`, `error.rs`, `handlers/` or `steps/`, `types.rs`, `tests/`.

## User Stories

- As a developer, I want to run `just dev-all` and have Core and App start concurrently so that I can begin working immediately.
- As a developer, I want `nx affected:test` to run only the tests impacted by my changes so that feedback is fast.
- As a developer, I want `just new-plugin` to scaffold a new plugin with all required files so that I can start coding without boilerplate.
- As a community plugin author, I want to depend only on the published SDK so that I do not need to clone the monorepo.

## Functional Requirements

- The system must organise all Rust crates under a single Cargo workspace with a shared `Cargo.lock`.
- The system must use Nx for polyglot task orchestration with caching and affected detection.
- The system must use pnpm with workspace protocol for all JavaScript package management.
- The system must provide a justfile with commands for `dev-core`, `dev-app`, `dev-all`, `test`, `lint`, and `new-plugin`.
- The system must follow the documented directory layout: `apps/core/`, `packages/*`, `plugins/*`.
- The system must ensure every crate follows the standard internal layout.
- The system must ensure plugins compile to WASM (`wasm32-wasi` target).
- The system must ensure community plugins build independently using only the published `plugin-sdk`.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
