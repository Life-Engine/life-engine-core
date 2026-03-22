---
title: "Phase 0 Tasks"
tags:
  - life-engine
  - tasks
  - phase-0
  - tdd
created: 2026-03-21
category: tasks
type: short-term
---

# Phase 0 Tasks

Phase document: [[03 - Projects/Life Engine/Planning/phases/Phase 0 — Foundation]]

This file breaks every Phase 0 work package into individual checkbox tasks. Each task is a 2-4 hour unit with a verifiable outcome. Tasks marked `[BLOCKER]` must complete before dependent work can begin.

## Methodology

Every task in this file follows the TDD workflow defined in [[03 - Projects/Life Engine/Planning/Test Plan]]:

1. **Red** — Write a failing test that defines the expected behaviour
2. **Green** — Write the minimum code to make the test pass
3. **Refactor** — Clean up, apply DRY, extract shared utilities
4. **Review** — Code review, coverage check, DRY audit

For UI tasks, add before step 1:

- **Stitch** — Prototype the component in Google Stitch, then adapt to the design system
- **E2E** — Write Playwright tests for user-facing interactions

Tasks are ordered test-first within each work package. Each work package ends with a review gate.

---

## 0.1 — Repository and Monorepo Setup

- [x] Create GitHub organisation and monorepo repository `[BLOCKER]`
- [x] Scaffold monorepo directory structure (`apps/`, `packages/`, `plugins/`, `.odm/docs/`, `tools/`, `.github/`)
- [x] Initialise Rust workspace (root `Cargo.toml` with workspace members)
- [x] Initialise Tauri v2 project inside `apps/app/`
- [x] Add `README.md` with project overview and quick start
- [x] Add `LICENSE` (Apache 2.0), `CODE_OF_CONDUCT.md`, `SECURITY.md`
- [x] Write `CONTRIBUTING.md` with DCO sign-off instructions and PR process
- [x] Configure `.gitignore` for Rust, Node, Tauri, macOS, IDE files
- [x] Set up Nx for polyglot task orchestration
- [x] Configure pnpm workspaces for JS packages
- [x] Write test: verify monorepo builds empty Rust binary and empty Tauri app `[TDD:RED]`
- [x] Verify monorepo builds correctly `[TDD:GREEN]`
- [x] Review gate: repo structure matches spec, all config files present, test passes

## 0.2 — CI/CD Pipeline

- [x] Write test: CI check script validates a known-good and known-bad commit `[TDD:RED]`
- [x] Create `ci.yml` workflow (`cargo check`, `clippy`, `test`) `[BLOCKER]`
- [x] Add JS/TS checks to `ci.yml` (`npm ci`, `eslint`, `tsc --noEmit`, `vitest`)
- [x] Add Tauri build check to `ci.yml`
- [x] Add DCO commit check to `ci.yml`
- [x] Set up `cargo-deny` for licence compliance and vulnerability scanning
- [x] Create `release.yml` workflow (platform binaries, Tauri bundles)
- [x] Configure `release.yml` to create GitHub Release with checksums
- [-] Add branch protection rules on main (require CI pass)
- [x] Configure Dependabot or Renovate for dependency updates
- [x] Verify CI catches intentional test failure `[TDD:GREEN]`
- [x] Review gate: CI pipeline runs end-to-end, branch protection active

## 0.3 — Architecture Decision Records

- [x] Create `/.odm/docs/adrs/` directory with ADR template
- [x] Write test: ADR format validation script checks Context/Decision/Consequences structure `[TDD:RED]`
- [x] Write ADR-001: Rust for Core (vs Go, TypeScript)
- [x] Write ADR-002: Tauri v2 for App (vs Electron, Flutter)
- [x] Write ADR-003: Web Components as plugin boundary (vs iframes, React)
- [x] Write ADR-004: SQLite + SQLCipher as default storage (vs Postgres-first)
- [x] Write ADR-005: axum as HTTP framework (vs actix-web, warp)
- [x] Write ADR-006: Pocket ID for OIDC auth (vs custom auth, Keycloak)
- [x] Write ADR-007: Extism for WASM plugin isolation (vs Wasmtime, wasmer)
- [x] Write ADR-008: PowerSync for client sync (vs libSQL sync, custom CRDT)
- [x] Write ADR-009: DCO over CLA for contributions
- [x] Write ADR-010: Apache 2.0 licence
- [x] Write ADR-011: Design for WASM now, implement later
- [x] Write ADR-012: Lit as recommended plugin framework
- [x] Write ADR-013: Adoption of 11 governing Design Principles
- [x] Implement ADR format validation script `[TDD:GREEN]`
- [x] Link all ADRs from README
- [x] Review gate: all 13 ADRs pass validation script, linked from README

## 0.4 — Schema and Data Model Specification

- [x] Define CDM specification document for 7 canonical collections `[BLOCKER]`
- [x] Define extensions namespacing convention
- [x] Write test fixtures for all 7 collections in `packages/test-fixtures/` `[TDD:RED]`
- [x] Write JSON Schema validation tests for each fixture `[TDD:RED]`
- [x] Create JSON Schema file for events collection `[TDD:GREEN]`
- [x] Create JSON Schema file for tasks collection `[TDD:GREEN]`
- [x] Create JSON Schema file for contacts collection `[TDD:GREEN]`
- [x] Create JSON Schema file for notes collection `[TDD:GREEN]`
- [x] Create JSON Schema file for emails collection `[TDD:GREEN]`
- [x] Create JSON Schema file for files collection `[TDD:GREEN]`
- [x] Create JSON Schema file for credentials collection `[TDD:GREEN]`
- [x] Define `plugin_data` universal table schema
- [x] Define schema versioning rules
- [x] Define migration format for plugin-owned collections
- [x] Write type validation tests for Rust structs `[TDD:RED]`
- [x] Create Rust structs in `packages/types/` for all CDMs `[TDD:GREEN]`
- [x] Write type validation tests for TypeScript interfaces `[TDD:RED]`
- [x] Create TypeScript interfaces in `packages/types/` for all CDMs `[TDD:GREEN]`
- [x] Refactor: extract shared validation helpers to `packages/test-utils/` `[TDD:REFACTOR]`
- [x] Review gate: all schemas validate against fixtures, types compile, no duplication in test data

## 0.5 — Plugin Manifest and SDK Design

- [x] Write tests for `CorePlugin` trait interface contract `[TDD:RED]`
- [x] Define `CorePlugin` trait in `plugin-sdk-rs` `[BLOCKER]` `[TDD:GREEN]`
- [x] Define capability enum with all namespaces
- [x] Define route registration types
- [x] Define `PluginContext` struct (scoped storage, config, events)
- [x] Write tests for `plugin.json` schema validation `[TDD:RED]`
- [x] Define `plugin.json` schema for App plugins `[TDD:GREEN]`
- [x] Write tests for `ShellAPI` TypeScript interface `[TDD:RED]`
- [x] Define `ShellAPI` TypeScript interface in `plugin-sdk-js` `[TDD:GREEN]`
- [x] Define manifest validation functions
- [x] Define slot types (`sidebar.item`, `main.page`, `command.palette`, `dashboard.widget`, `settings.page`, `context.menu`)
- [x] Define shared module list (lit, react)
- [x] Write SDK documentation for Core plugin authors
- [x] Write SDK documentation for App plugin authors
- [x] Refactor: DRY audit across SDK test utilities `[TDD:REFACTOR]`
- [x] Review gate: both SDKs compile, all tests pass, docs match implementation

## 0.6 — Development Environment

- [x] Create `.devcontainer/` configuration for VS Code / Codespaces
- [x] Write failing test: dev environment setup script completes in under 5 minutes `[TDD:RED]`
- [x] Write justfile with `dev-core` command (`cargo-watch`)
- [x] Write justfile with `dev-app` command
- [x] Write justfile with `dev-all` command (both together)
- [x] Write justfile with `test`, `lint`, `new-plugin` commands
- [x] Create `engine-plugin` template (minimal Rust Core plugin)
- [x] Create `life-plugin-vanilla` template (minimal vanilla JS App plugin)
- [x] Create `life-plugin-lit` template (minimal Lit App plugin)
- [x] Test: clone from scratch, run one command, verify dev environment works `[TDD:GREEN]`
- [x] Review gate: timed test passes, all templates scaffold correctly

## 0.7 — Community and Public Presence

- [-] Create Discord server (N/A — no Discord initially per Community strategy; GitHub-only channels)
- [-] Create X/Twitter account for build-in-public updates (deferred — external service, not a codebase deliverable)
- [-] Write first "Week 1" build-in-public post (deferred — external, not a codebase deliverable)
- [-] Set up Open Collective project page (deferred — external service, not a codebase deliverable)
- [x] Draft governance model document (published as `GOVERNANCE.md` at repo root)
- [-] Review gate: governance published; external channel tasks deferred or N/A per design

> Note: Discord was explicitly excluded by the Community strategy document. External service tasks (Twitter, Open Collective) are deferred as they are not codebase deliverables.

## 0.8 — Project Website

- [-] Register project domain (`lifeengine.dev` or chosen domain) `[BLOCKER]`
- [x] Scaffold `apps/web/` with Astro + Starlight (`pnpm create astro` with Starlight integration)
- [x] Configure Astro for static output and GitHub Pages deployment
- [x] Set up project-level design tokens (colours, typography, spacing) matching the App design system
- [x] Build homepage: hero section, value proposition, feature highlights, architecture diagram placeholder
- [x] Build "About" page with project philosophy, licence, and governance summary
- [x] Build "Community" page with GitHub, Discussions, and Open Collective links
- [x] Build "Pricing" page explaining the free and open-source model
- [x] Create docs skeleton with Starlight sidebar: Getting Started (placeholder), Architecture (placeholder), Contributing (placeholder)
- [x] Write initial Getting Started page: system requirements and installation placeholders for each deployment mode
- [x] Write initial Contributing page: development setup, PR process, DCO instructions
- [x] Set up Pagefind search integration (Starlight default)
- [x] Configure `sitemap.xml` and `robots.txt` generation
- [x] Add Open Graph and Twitter Card meta tags to all pages
- [x] Create `deploy-web.yml` GitHub Actions workflow for automatic deployment to GitHub Pages
- [-] Configure custom domain with CNAME record and HTTPS
- [x] Set up blog content collection with first post: "Introducing Life Engine"
- [x] Create RSS feed at `/blog/rss.xml`
- [x] Review gate: site builds, deploys to GitHub Pages, homepage loads under 2s, Lighthouse scores above 90 (performance) and 95 (accessibility)

## 0.9 — Test Infrastructure and Stitch Setup

- [x] Install and configure Playwright (`npx playwright install`) `[BLOCKER]`
- [x] Create `playwright.config.ts` at repo root with Tauri WebView configuration
- [x] Create `tests/e2e/` directory structure
- [x] Create `tests/e2e/pages/` directory with base page object class
- [x] Scaffold initial page objects: `shell.page.ts`, `onboarding.page.ts`
- [x] Write a smoke test: app launches and renders shell `[TDD:RED]`
- [x] Create `packages/test-utils/` Rust crate with factory function stubs
- [x] Create `packages/test-utils-js/` package with shared test helpers
- [x] Create `packages/test-fixtures/` package with canonical test data
- [x] Configure Google Stitch workspace for design system component prototyping
- [-] Prototype initial shell layout in Stitch (sidebar, main area, top bar) `[STITCH]`
- [x] Export Stitch output and document adaptation guidelines
- [x] Add `npx playwright test` to CI pipeline
- [x] Add test coverage reporting to CI (`cargo-llvm-cov`, `vitest --coverage`)
- [x] Review gate: Playwright config works, test-utils packages importable, Stitch workspace ready
