---
title: "Phase 0 — Foundation"
tags:
  - life-engine
  - planning
  - phase-0
  - foundation
created: 2026-03-21
---

# Phase 0 — Foundation

## Goal

Everything needed before writing production code. Repository, tooling, CI/CD, architectural decisions finalised and documented, community presence established.

## Entry Criteria

- Project direction decided
- Tech stack chosen (Rust + Tauri v2)
- Design documents complete

## Table of Contents

- [[#0.1 — Repository and Monorepo Setup]]
- [[#0.2 — CI/CD Pipeline]]
- [[#0.3 — Architecture Decision Records]]
- [[#0.4 — Schema and Data Model Specification]]
- [[#0.5 — Plugin Manifest and SDK Design]]
- [[#0.6 — Development Environment]]
- [[#0.7 — Community and Public Presence]]
- [[#0.8 — Project Website]]
- [[#Exit Criteria]]
- [[#Phase-Specific Risks]]

---

## Work Packages

### 0.1 — Repository and Monorepo Setup

**Scope**

Create the GitHub organisation and scaffold the monorepo directory structure. The top-level layout includes:

- `apps/core` — Rust core binary
- `apps/app` — Tauri v2 desktop application
- `packages/types` — shared type definitions
- `packages/plugin-sdk-rs` — Rust plugin SDK
- `packages/plugin-sdk-js` — JavaScript/TypeScript plugin SDK
- `plugins/engine` — first-party engine plugins
- `plugins/life` — first-party life plugins
- `docs` — documentation and ADRs
- `tools` — development tooling
- `.github` — GitHub Actions workflows and templates

Initialise a Cargo workspace, initialise the Tauri v2 project, add root documentation files (README, LICENSE under Apache 2.0, CODE_OF_CONDUCT, SECURITY, CONTRIBUTING), configure `.gitignore`, and set up Nx for polyglot orchestration across Rust and JavaScript packages.

**Deliverables**

- Monorepo is cloneable and builds an empty Rust binary and an empty Tauri app
- Contribution process documented in CONTRIBUTING

**Dependencies**

- None (first work package)

---

### 0.2 — CI/CD Pipeline

**Scope**

Set up GitHub Actions with two primary workflows:

- **ci.yml** — runs on every PR: `cargo check`, `cargo clippy`, `cargo test`, `npm ci`, `npm run lint`, `npm run type-check`, `npm run test`, Tauri build check, and DCO (Developer Certificate of Origin) check
- **release.yml** — triggered on version tags: builds platform binaries (Linux, macOS, Windows), produces Tauri bundles, creates GitHub Release with checksums

Configure branch protection on `main` requiring CI pass and review approval. Set up Dependabot or Renovate for dependency updates. Add `cargo-deny` for licence compliance and vulnerability scanning.

**Deliverables**

- Every PR is validated automatically across all platforms
- Release pipeline produces platform binaries with checksums

**Dependencies**

- 0.1

---

### 0.3 — Architecture Decision Records

**Scope**

Write 13 ADRs in `/.odm/docs/adrs/` following a consistent Context/Decision/Consequences/Alternatives format:

- **ADR-001** — Rust for Core
- **ADR-002** — Tauri v2 for App
- **ADR-003** — Web Components as plugin boundary
- **ADR-004** — SQLite + SQLCipher as default storage
- **ADR-005** — axum as HTTP framework
- **ADR-006** — Pocket ID for OIDC auth
- **ADR-007** — Extism for WASM plugin isolation
- **ADR-008** — PowerSync for client sync
- **ADR-009** — DCO over CLA
- **ADR-010** — Apache 2.0 licence
- **ADR-011** — Design for WASM now, implement later
- **ADR-012** — Lit as recommended plugin framework
- **ADR-013** — Adoption of 11 governing [[03 - Projects/Life Engine/Design/Principles|Design Principles]] (Separation of Concerns, ADRs, Fail-Fast, Defence in Depth, Finish Before Widening, Least Privilege, Parse Don't Validate, Open/Closed, Single Source of Truth, Explicit Over Implicit, Pit of Success)

**Deliverables**

- All 13 ADRs published and linked from the repository README

**Dependencies**

- 0.1

---

### 0.4 — Schema and Data Model Specification

**Scope**

Define the CDM (Canonical Data Model) specification document covering 7 canonical collections:

- `events` — calendar events
- `tasks` — to-dos and projects
- `contacts` — people and organisations
- `notes` — freeform text and rich content
- `emails` — email messages and threads
- `files` — file metadata and references
- `credentials` — identity documents and secrets

Define the extensions namespacing convention for plugin-specific fields. Create JSON Schema files for each collection. Define the private collection convention for plugin-internal data. Define the `plugin_data` universal table schema. Define schema versioning rules and migration format.

**Deliverables**

- `packages/types/` contains Rust structs and TypeScript interfaces for all canonical collections
- JSON Schema files published in `.odm/docs/schemas/`

**Dependencies**

- 0.1

---

### 0.5 — Plugin Manifest and SDK Design

**Scope**

Define the two plugin manifest formats:

- **Core plugin manifest** — `CorePlugin` trait, capability declarations, route registration
- **App plugin manifest** — `plugin.json` schema with metadata, capabilities, and slot declarations

Define capability namespaces:

- `data` — read/write/subscribe to collections
- `network` — outbound HTTP, WebSocket
- `ipc` — inter-plugin communication
- `notify` — system notifications
- `storage` — local storage access

Define slot types for App plugins:

- `sidebar.item` — navigation entry
- `main.page` — full-page content area
- `command.palette` — command palette actions
- `dashboard.widget` — dashboard card
- `settings.page` — settings panel
- `context.menu` — right-click menu items

Define the shared module list for App plugins. Scaffold `plugin-sdk-rs` with trait definitions and `plugin-sdk-js` with TypeScript types.

**Deliverables**

- Both SDKs have published types and documentation

**Dependencies**

- 0.4

---

### 0.6 — Development Environment

**Scope**

Create a devcontainer configuration (`.devcontainer/`) for VS Code and GitHub Codespaces with all required toolchains pre-installed. Create a `justfile` with common commands:

- `dev-core` — run Core in development mode
- `dev-app` — run App in development mode
- `dev-all` — run both concurrently
- `test` — run all tests
- `lint` — run all linters
- `new-plugin` — scaffold a new plugin from template

Create example plugin templates:

- Engine plugin (Rust)
- Life plugin vanilla (plain JavaScript)
- Life plugin Lit (Lit framework)

**Deliverables**

- A new contributor can clone the repo, run one command, and have a working development environment in under 5 minutes

**Dependencies**

- 0.1, 0.2

---

### 0.7 — Community and Public Presence

**Scope**

Set up the community infrastructure:

- **Discord server** with channels: `general`, `development`, `plugins`, `help`, `showcase`
- **X/Twitter account** — for build-in-public updates
- **First build-in-public post** — announcing the project and vision
- **Open Collective page** — for accepting donations and sponsorships
- **Governance model document** — decision-making process, maintainer roles, contribution tiers

**Deliverables**

- Community can discover, follow, and begin contributing to the project
- Donation pipeline is open via Open Collective

**Dependencies**

- 0.1

---

### 0.8 — Project Website

**Scope**

Scaffold the project website at `apps/web/` using Astro + Starlight. This is the single site that will grow across all phases to serve as the marketing presence, documentation hub, SDK reference, blog, and download portal. See [[03 - Projects/Life Engine/Planning/specs/infrastructure/Website]] for the full specification and [[03 - Projects/Life Engine/Design/Website/Architecture]] for the technical design.

Phase 0 delivers:

- **Astro + Starlight scaffold** — `apps/web/` project with Astro, Starlight docs integration, and static build targeting GitHub Pages
- **Domain registration** — `lifeengine.dev` or chosen domain, configured with GitHub Pages custom domain and HTTPS
- **Homepage** — Hero section with value proposition, feature highlights, architecture diagram placeholder, download placeholder, GitHub and Open Collective links
- **Marketing pages** — About (philosophy, licence, governance), Community (channels, how to contribute), Pricing (free and open source explanation, Open Collective link)
- **Docs skeleton** — Starlight sidebar with placeholder sections: Getting Started, Architecture, Contributing. Initial Contributing page with dev setup, PR process, and DCO instructions
- **Blog** — Content collection with first post ("Introducing Life Engine"), RSS feed at `/blog/rss.xml`
- **CI/CD** — `deploy-web.yml` GitHub Actions workflow: builds on push to `main`, deploys `dist/` to GitHub Pages
- **SEO foundations** — `sitemap.xml`, `robots.txt`, Open Graph and Twitter Card meta tags on all pages
- **Search** — Pagefind integration (Starlight default) indexing all site content

**Deliverables**

- Project website is live at the custom domain with homepage, marketing pages, docs skeleton, and blog
- CI automatically deploys on merge to `main`

**Dependencies**

- 0.1, 0.7

---

## Exit Criteria

- Monorepo builds on CI for all platforms (Linux, macOS, Windows)
- All 13 ADRs published and linked from README
- CDM schema specification complete with JSON Schema validation files
- Both plugin SDKs (`plugin-sdk-rs` and `plugin-sdk-js`) have type definitions
- Development environment works in under 5 minutes from fresh clone
- Community server exists with governance document published
- Open Collective page is live and accepting donations
- Project website is live with homepage, marketing pages, docs skeleton, blog, and search

## Phase-Specific Risks

- **ADR analysis paralysis** — Risk of over-debating technical decisions before any code exists. Mitigation: timebound each ADR to one day of research, make decisions reversible where possible, and note that ADRs can be superseded.
- **Scope creep in SDK design** — Temptation to over-engineer the plugin SDKs before building any real plugins. Mitigation: define a minimal viable trait/interface now, expand in Phase 1 as real plugin needs emerge from implementation.
