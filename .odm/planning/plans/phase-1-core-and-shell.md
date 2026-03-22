---
title: "Phase 1 — Core and Shell"
tags:
  - life-engine
  - planning
  - phase-1
  - core
  - shell
created: 2026-03-21
---

# Phase 1 — Core and Shell

## Goal

A working Core binary that loads plugins, stores data, and exposes a REST API. A working App shell that connects to Core, loads one plugin, and displays data. Runs as a single desktop app via Tauri sidecar.

## Entry Criteria

- Phase 0 complete
- Monorepo builds on CI
- All ADRs published
- Both plugin SDKs scaffolded with type definitions

## Table of Contents

- [[#1.1 — Core Binary]]
- [[#1.2 — SQLite Storage Plugin]]
- [[#1.3 — Auth Layer]]
- [[#1.4 — REST API Layer]]
- [[#1.5 — First Connector: Email (IMAP/SMTP)]]
- [[#1.6 — App Shell]]
- [[#1.7 — Plugin Loader (App)]]
- [[#1.8 — Data Layer (App)]]
- [[#1.9 — First App Plugin: Email Viewer]]
- [[#1.10 — Tauri Sidecar Integration]]
- [[#1.11 — Website: Documentation and Downloads]]
- [[#Exit Criteria]]
- [[#Phase-Specific Risks]]

---

## Work Packages

### 1.1 — Core Binary

> BLOCKER — all other Core work packages depend on this.

**Scope**

Create the `apps/core/` Rust binary with the following subsystems:

- **Config loading** — YAML configuration file with environment variable overrides and CLI argument overrides (clap)
- **Plugin loader** — discover plugins from configured directories, validate declared capabilities against known namespaces, call `on_load` lifecycle hook, handle errors gracefully without crashing Core
- **Message bus** — tokio broadcast channels with typed events: `NewRecords`, `SyncComplete`, `PluginLoaded`, `PluginError`
- **StorageAdapter trait** — abstract interface for storage backends (implemented by plugins)
- **Health check endpoint** — `GET /api/system/health` returning status, uptime, and loaded plugin count
- **Structured logging** — tracing crate with JSON output, configurable log levels
- **Graceful shutdown** — handle SIGTERM, call `on_unload` on all plugins, timeout after 5 seconds

**Deliverables**

- `cargo run` starts Core, loads zero plugins, responds to `/api/system/health`, and shuts down cleanly

**Dependencies**

- 0.1, 0.4, 0.5

**Spec:** [[03 - Projects/Life Engine/Planning/specs/core/Binary and Startup]]

---

### 1.2 — SQLite Storage Plugin

> BLOCKER — REST API and connectors depend on this.

**Scope**

Implement `StorageAdapter` for SQLite using rusqlite with the following capabilities:

- **plugin_data table** — universal JSON document store matching CDM specification
- **SQLCipher encryption** — Argon2id key derivation from master passphrase, rekey command for passphrase changes
- **Query filter parsing** — equality, comparison operators, text search (LIKE), logical operators (AND/OR/NOT)
- **Sort and pagination** — configurable sort fields, cursor-based and offset pagination
- **Subscribe** — SQLite triggers combined with tokio channels for real-time change notifications
- **Optimistic locking** — version increment on every write, reject stale updates
- **Audit logging** — separate `audit_log` table recording all mutations with timestamps, user, and operation type; 90-day retention with automatic cleanup

Write integration tests covering CRUD operations, encryption, concurrent access, and query edge cases.

**Deliverables**

- Core can persist and query JSON documents with encryption at rest

**Dependencies**

- 1.1

**Spec:** [[03 - Projects/Life Engine/Planning/specs/core/Data Layer]]

---

### 1.3 — Auth Layer

**Scope**

Implement authentication for the Core API:

- **Local token auth** — `POST /api/auth/token` endpoint, salted password hashes (Argon2id), configurable token expiry, token revocation
- **Auth middleware** — axum middleware layer for bearer token extraction, validation, and rate limiting (5 attempts per minute per IP)
- **AuthProvider trait** — abstract interface allowing `local-token` and `pocket-id` implementations to be swapped via configuration

**Deliverables**

- Core requires authentication for all `/api/` routes
- Local token auth works end-to-end (generate, use, revoke)

**Dependencies**

- 1.1

**Spec:** [[03 - Projects/Life Engine/Planning/specs/core/Auth and Pocket ID]]

---

### 1.4 — REST API Layer

**Scope**

Set up the axum router with a full middleware stack:

- **Middleware** — TLS termination, auth (from 1.3), rate limiting, CORS configuration, request/response logging, structured error handling
- **Data routes** — full CRUD for `/api/data/{collection}` (GET list, GET by ID, POST create, PUT update, DELETE)
- **System routes** — `/api/system/health`, `/api/system/info`, `/api/system/plugins`
- **SSE (Server-Sent Events)** — `GET /api/events/stream` with collection and event-type filtering via query parameters
- **Plugin route mounting** — plugins register routes under `/api/plugins/{plugin-id}/`, Core mounts them dynamically

Write API integration tests covering all routes, error cases, and auth enforcement.

**Deliverables**

- Full CRUD REST API operational
- SSE event stream works for real-time updates
- Plugin routes mount correctly

**Dependencies**

- 1.1, 1.2, 1.3

**Spec:** [[03 - Projects/Life Engine/Planning/specs/core/REST API]]

---

### 1.5 — First Connector: Email (IMAP/SMTP)

> Can be built in parallel with 1.6 (App Shell).

**Scope**

Implement the `Connector` trait and the first concrete connector:

- **Connector trait** — `id`, `display_name`, `supported_collections`, `authenticate`, `sync`, `on_event` methods
- **IMAP connector** — TLS connection, authentication (password and OAuth2), fetch headers and bodies, incremental sync via UIDVALIDITY + UIDs, map messages to `emails` canonical collection, handle attachments (store metadata, optional body download)
- **SMTP send** — send capability using the same credential set
- **Sync scheduling** — configurable interval (default 5 minutes), manual trigger via API, exponential backoff on failure
- **Credential storage** — encrypted storage for provider credentials, CRUD API for managing credentials, credential values never appear in logs

Test against a GreenMail Docker container in CI for reproducible IMAP/SMTP testing.

**Deliverables**

- Core connects to any IMAP provider, syncs emails incrementally, and sends via SMTP

**Dependencies**

- 1.2, 1.4

**Spec:** [[03 - Projects/Life Engine/Planning/specs/core/Connector Architecture]]

---

### 1.6 — App Shell

> Can be built in parallel with 1.5 (Email Connector).

**Scope**

Configure the Tauri v2 project and build the App shell:

- **Tauri configuration** — Rust backend, HTML/CSS/JS frontend, default window 1200x800
- **Shell UI layout** — sidebar (navigation), main content area, top bar (search, user), status bar (sync status, connection indicator)
- **Design system** — 17 Web Components with CSS custom properties for consistent styling across all plugins
- **Theming** — light and dark themes, respects `prefers-color-scheme` with manual toggle override
- **Settings page** — Core URL configuration, auth token entry, theme selection, plugin management (list, enable/disable, remove)
- **Plugin container** — loading state indicator, error boundary that isolates plugin crashes from the shell

**Deliverables**

- Tauri app launches and shows the shell UI with sidebar and content area
- App connects to a running Core instance

**Dependencies**

- 0.5, 0.6

**Specs:** [[03 - Projects/Life Engine/Planning/specs/app/Shell Framework]], [[03 - Projects/Life Engine/Planning/specs/app/Design System]]

---

### 1.7 — Plugin Loader (App)

**Scope**

Implement the App-side plugin loading system:

- **Manifest reader** — read and validate `plugin.json` files against the manifest schema
- **Shared module host** — import maps providing `lit`, `react`, and other shared dependencies to plugins
- **Scoped API creation** — each plugin receives a capability-locked `ShellAPI` instance (only methods matching declared capabilities are available)
- **11-step loading lifecycle** — discover, validate, resolve dependencies, create scope, register routes, mount slots, initialise, activate, ready, deactivate, unload
- **Plugin unloading** — DOM node removal, subscription cleanup, memory release
- **Sidebar navigation** — plugins register sidebar items during the mount phase

**Deliverables**

- Shell can install, load, and unload plugins at runtime with scoped API enforcement

**Dependencies**

- 1.6

**Specs:** [[03 - Projects/Life Engine/Planning/specs/app/Plugin Loader]], [[03 - Projects/Life Engine/Planning/specs/app/Capability Enforcement]]

---

### 1.8 — Data Layer (App)

**Scope**

Implement the App-side data layer for offline-first operation:

- **Local SQLite database** — same `plugin_data` schema as Core, stored in Tauri app data directory
- **Shell Data API** — `query`, `create`, `update`, `delete`, `subscribe` methods operating against local SQLite
- **SyncAdapter interface** — abstract interface for sync implementations
- **REST polling SyncAdapter** — poll Core every 30 seconds for changes, push local writes immediately, queue writes when offline, last-write-wins conflict resolution
- **Sync status indicator** — status bar shows current sync state (synced, syncing, offline, error)

**Deliverables**

- Plugins read and write data instantly against local SQLite
- Data syncs to Core in the background
- Offline writes queue and sync when connection restores

**Dependencies**

- 1.6, 1.7

**Specs:** [[03 - Projects/Life Engine/Planning/specs/app/Shell Data API]], [[03 - Projects/Life Engine/Planning/specs/app/Sync Layer]]

---

### 1.9 — First App Plugin: Email Viewer

**Scope**

Create `plugins/life/email-viewer/` as the first App plugin:

- **plugin.json** — capabilities: `data:read:emails`, `data:read:contacts`; slots: `sidebar.item`, `main.page`
- **Implementation** — built with Lit framework
- **Features** — email list view (sender, subject, date, preview), detail view (full body, headers, attachments), thread grouping by conversation, search and filter (sender, subject, date range, read/unread), unread indicator with count in sidebar
- **Reactive updates** — subscribe to `emails` collection for real-time UI updates

Test end-to-end: IMAP provider -> Core connector -> SQLite storage -> REST API -> App local SQLite -> plugin UI.

**Deliverables**

- Open the app, see synced emails displayed in a functional email viewer
- The entire data pipeline works from external provider to plugin UI

**Dependencies**

- 1.5, 1.7, 1.8

---

### 1.10 — Tauri Sidecar Integration

**Scope**

Configure Core as a Tauri sidecar binary for single-app deployment:

- **Sidecar configuration** — Core binary bundled with Tauri app, started automatically on app launch
- **First-run setup flow** — guided wizard: welcome screen, passphrase creation, Core starts in background, auto-generate auth token, connect App to sidecar Core, prompt for email provider, run first sync, display email viewer with synced data
- **Crash handling** — detect Core process exit, show error state in App, provide restart button
- **Graceful shutdown** — on app close: finish in-progress sync, close database connections, shut down Core with 5-second timeout

**Deliverables**

- Double-click the app, create a passphrase, connect an email account, see emails
- Zero server setup required for single-user desktop use

**Dependencies**

- 1.5, 1.6, 1.8, 1.9

**Spec:** [[03 - Projects/Life Engine/Planning/specs/app/Tauri Integration]]

---

### 1.11 — Website: Documentation and Downloads

**Scope**

Expand the project website (scaffolded in Phase 0) with core documentation and the downloads page:

- **Getting Started guides** — Platform-specific installation walkthroughs (macOS, Windows, Linux, Docker), first-run setup, connecting an email account, installing a plugin. Each guide includes annotated screenshots
- **Architecture documentation** — System overview, Core architecture, App architecture, data model, security model
- **SDK reference (initial)** — Set up auto-generation pipelines (rustdoc → MDX for Rust SDK, TypeDoc → MDX for JS SDK). Generate initial reference pages from current SDK source
- **ADR archive** — Browsable index of all Architecture Decision Records published in Phase 0
- **Downloads page** — Platform detection (auto-detect OS), artifact listing sourced from GitHub Releases API, version selector (stable, previous), SHA-256 checksum display and verification instructions
- **First release blog post** — Phase 1 release announcement with highlights
- **CI validation** — SDK doc generation runs in CI. PR that changes SDK types without regenerating docs fails the build

**Deliverables**

- New users can find, download, install, and get started using only the website
- Developers can browse SDK reference and architecture docs
- Downloads page serves the correct installer for the visitor's platform

**Dependencies**

- 0.8, 1.10

**Spec:** [[03 - Projects/Life Engine/Planning/specs/infrastructure/Website]]

---

## Exit Criteria

- Core starts, loads the SQLite storage plugin, and responds to the REST API
- Core authenticates requests with bearer tokens
- Email connector syncs from an IMAP provider and normalises messages to the CDM `emails` collection
- App shell launches via Tauri, loads the email viewer plugin, and displays synced emails
- Data flows end-to-end: IMAP -> Core -> SQLite -> REST API -> App local SQLite -> Plugin UI
- Sidecar mode bundles Core with App as a single desktop application
- First-run onboarding completes in under 5 minutes
- Website has complete Getting Started guides, architecture docs, SDK reference stubs, downloads page, and first release blog post

## Phase-Specific Risks

- **IMAP inconsistency across providers** — Different providers implement IMAP with varying quirks and extensions. Mitigation: start with Gmail and Fastmail as primary targets, use a battle-tested Rust IMAP library, and add provider-specific workarounds as issues surface.
- **Plugin loader edge cases** — Malformed manifests, missing dependencies, and plugin crashes could destabilise the shell. Mitigation: extensive error handling at every lifecycle step, test with intentionally malformed manifests, and ensure the error boundary isolates plugin failures.
- **Sync reliability** — Network interruptions, large mailboxes, and concurrent writes introduce sync complexity. Mitigation: start with simple REST polling (proven pattern), defer PowerSync integration to Phase 2, and implement an offline write queue from day one.
