<!--
domain: infrastructure
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Implementation Plan — Monorepo and Tooling

## Task Overview

This plan sets up the monorepo structure, build tooling, and developer workflow. Work starts with the foundational configuration files (Cargo workspace, Nx, pnpm), then adds the developer-facing justfile commands, plugin scaffolding templates, directory structure verification, and community plugin build verification.

**Progress:** 0 / 9 tasks complete

## Steering Document Compliance

- Single Cargo workspace with shared lockfile follows Single Source of Truth
- Nx affected detection keeps CI focused on changed code (Finish Before Widening)
- `just dev-all` provides one-command onboarding (The Pit of Success)
- Community plugins build independently against published SDKs

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
  <!-- purpose: Create [workspace] section listing apps/core, packages/plugin-sdk-rs, and all plugins/engine/* as members with shared dependency declarations -->
  <!-- requirements: 1.1, 1.2, 1.3 -->
  <!-- leverage: none -->

---

## 1.2 — Nx Configuration
> spec: ./brief.md

- [ ] Set up Nx for polyglot task orchestration with caching
  <!-- file: nx.json -->
  <!-- purpose: Create task pipeline definitions for build, test, and lint with task caching and Cargo plugin configuration -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.4 -->
  <!-- leverage: none -->

---

## 1.3 — pnpm Workspace Configuration
> spec: ./brief.md

- [ ] Set up pnpm workspace for JavaScript packages
  <!-- file: pnpm-workspace.yaml -->
  <!-- purpose: List apps/app, packages/types, packages/plugin-sdk-js, and plugins/life/* as workspace members with workspace:* protocol -->
  <!-- requirements: 3.1, 3.2, 3.3 -->
  <!-- leverage: none -->

---

## 2.1 — Justfile Development Commands
> spec: ./brief.md
> depends: 1.1, 1.2, 1.3

- [ ] Create justfile with dev-core, dev-app, and dev-all recipes
  <!-- file: justfile -->
  <!-- purpose: Add dev-core (cargo-watch), dev-app (Tauri dev server), and dev-all (concurrent) recipes -->
  <!-- requirements: 4.1, 4.2, 4.3 -->
  <!-- leverage: none -->

---

## 2.2 — Justfile Quality Commands
> spec: ./brief.md
> depends: 2.1

- [ ] Add test, lint, and new-plugin recipes to the justfile
  <!-- file: justfile -->
  <!-- purpose: Add test (cargo test + pnpm test via Nx), lint (clippy + eslint + tsc), and new-plugin (scaffold from template) recipes -->
  <!-- requirements: 4.4, 4.5, 4.6 -->
  <!-- leverage: justfile from WP 2.1 -->

---

## 3.1 — Plugin Scaffold Template
> spec: ./brief.md

- [ ] Create plugin scaffold templates for Core and App plugins
  <!-- file: tools/templates/core-plugin/Cargo.toml -->
  <!-- file: tools/templates/core-plugin/src/lib.rs -->
  <!-- file: tools/templates/app-plugin/package.json -->
  <!-- purpose: Create templates with {{name}} and {{id}} placeholders for just new-plugin scaffolding -->
  <!-- requirements: 4.6 -->
  <!-- leverage: none -->

---

## 3.2 — Directory Layout Verification
> spec: ./brief.md

- [ ] Verify and create the documented directory structure
  <!-- file: apps/, packages/, plugins/, .odm/docs/, tools/, .github/ -->
  <!-- purpose: Confirm all required directories exist; create missing directories with .gitkeep files -->
  <!-- requirements: 5.1, 5.2, 5.3, 5.4 -->
  <!-- leverage: none -->

---

## 4.1 — Affected Detection Verification
> spec: ./brief.md
> depends: 1.2

- [ ] Verify Nx affected detection and task caching work correctly
  <!-- file: nx.json -->
  <!-- purpose: Make a change to a single package, run nx affected:test, confirm only changed packages are tested and caching prevents re-runs -->
  <!-- requirements: 2.1, 2.2 -->
  <!-- leverage: Nx config from WP 1.2 -->

---

## 4.2 — Community Plugin Build Verification
> spec: ./brief.md
> depends: 1.1, 1.3

- [ ] Verify community plugins build independently from the monorepo
  <!-- file: (external test projects) -->
  <!-- purpose: Create test Core plugin with only life-engine-plugin-sdk and test App plugin with only @life-engine/plugin-sdk; confirm both compile -->
  <!-- requirements: 6.1, 6.2, 6.3 -->
  <!-- leverage: published SDK packages -->
