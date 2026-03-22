---
title: "Phase 1 Tasks"
tags:
  - life-engine
  - tasks
  - phase-1
  - tdd
created: 2026-03-21
category: tasks
type: short-term
---

# Phase 1 Tasks

Phase document: [[03 - Projects/Life Engine/Planning/phases/Phase 1 — Core and Shell]]

This file breaks every Phase 1 work package into individual checkbox tasks. Each task is a 2-4 hour unit with a verifiable outcome. Tasks marked `[BLOCKER]` must complete before dependent work can begin.

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

## 1.1 — Core Binary

- [x] Write unit tests for config loading and validation `[TDD:RED]`
- [x] Write unit tests for plugin loader lifecycle `[TDD:RED]`
- [x] Create `apps/core/` Rust binary crate `[BLOCKER]`
- [x] Implement YAML config file loading (`~/.life-engine/config.yaml`) `[TDD:GREEN]`
- [x] Implement environment variable overrides (`LIFE_ENGINE_*`) `[TDD:GREEN]`
- [x] Implement CLI argument overrides `[TDD:GREEN]`
- [x] Implement config validation with clear error messages `[TDD:GREEN]`
- [x] Implement plugin loader: discover plugins from configured paths `[TDD:GREEN]`
- [x] Implement plugin loader: read and validate plugin manifests `[TDD:GREEN]`
- [x] Implement plugin loader: validate capability declarations `[TDD:GREEN]`
- [x] Implement plugin loader: call `on_load` lifecycle hook `[TDD:GREEN]`
- [x] Implement plugin loader: graceful error handling (one failing plugin cannot crash Core) `[TDD:GREEN]`
- [x] Implement message bus using tokio broadcast channels
- [x] Define typed events: `NewRecords`, `SyncComplete`, `PluginLoaded`, `PluginError`
- [x] Implement `StorageAdapter` trait definition
- [x] Implement health check endpoint (`GET /api/system/health`)
- [x] Implement structured logging (tracing crate with JSON output)
- [x] Implement graceful shutdown (SIGTERM handler, `on_unload`, 5s timeout)
- [x] Refactor: extract shared config/test utilities to `packages/test-utils/` `[TDD:REFACTOR]`
- [x] Review gate: all tests pass, 80% coverage, DRY audit, spec compliance — 334/334 tests pass, 0 clippy warnings. DRY audit: `GoodPlugin`/`BadPlugin` test helpers are local to `plugin_loader.rs` (acceptable). `PluginInfo` and `PluginManifest` share `id`/`display_name`/`version` fields but serve different purposes (runtime vs. file manifest) and are intentionally kept separate.

## 1.2 — SQLite Storage Plugin

- [x] Write integration tests: CRUD operations against fresh SQLite `[TDD:RED]`
- [x] Write integration tests: query filters and sorting `[TDD:RED]`
- [x] Write integration tests: concurrent access `[TDD:RED]`
- [x] Write integration tests: encryption round-trip `[TDD:RED]`
- [x] Write integration tests: audit log entries `[TDD:RED]`
- [x] Implement `StorageAdapter` for SQLite using rusqlite `[BLOCKER]` `[TDD:GREEN]`
- [x] Create `plugin_data` table on first run `[TDD:GREEN]`
- [x] Implement SQLCipher encryption with Argon2id key derivation `[TDD:GREEN]`
- [x] Implement master passphrase prompt on first run `[TDD:GREEN]`
- [x] Implement passphrase change command (`life-engine rekey`) `[TDD:GREEN]`
- [x] Implement query filter parsing: equality filters `[TDD:GREEN]`
- [x] Implement query filter parsing: comparison operators (`$gte`, `$lte`) `[TDD:GREEN]`
- [x] Implement query filter parsing: text search (`$contains`) `[TDD:GREEN]`
- [x] Implement query filter parsing: logical operators (`$and`, `$or`) `[TDD:GREEN]`
- [x] Implement sort and pagination (`limit`, `offset`, `sort_by`, `sort_dir`) `[TDD:GREEN]`
- [x] Implement subscribe method using SQLite triggers + tokio channels `[TDD:GREEN]`
- [x] Implement automatic version increment on update `[TDD:GREEN]`
- [x] Implement audit log table and security event logging `[TDD:GREEN]`
- [x] Implement audit log retention (90 days default) `[TDD:GREEN]`
- [x] Refactor: extract shared test database factory to `packages/test-utils/` `[TDD:REFACTOR]`
- [x] Review gate: all integration tests pass, encryption verified, DRY audit

## 1.3 — Auth Layer

- [x] Write integration tests for full auth flow (generate, validate, revoke, expire, rate-limit) `[TDD:RED]`
- [x] Implement `POST /api/auth/token` (generate token from master passphrase) `[TDD:GREEN]`
- [x] Implement token storage as salted hashes in database `[TDD:GREEN]`
- [x] Implement configurable token expiry (default 30 days) `[TDD:GREEN]`
- [x] Implement token revocation (`DELETE /api/auth/token/{id}`) `[TDD:GREEN]`
- [x] Implement auth middleware: extract bearer token from Authorization header `[TDD:GREEN]`
- [x] Implement auth middleware: validate against stored hashes `[TDD:GREEN]`
- [x] Implement auth middleware: reject expired tokens `[TDD:GREEN]`
- [x] Implement auth middleware: rate-limit failed attempts (5/min/IP) `[TDD:GREEN]`
- [x] Define `AuthProvider` trait (local-token and pocket-id swappable)
- [x] Refactor: extract shared auth test helpers `[TDD:REFACTOR]`
- [x] Review gate: all auth tests pass, rate limiting verified, trait is DRY

## 1.4 — REST API Layer

- [x] Write API integration tests: full request/response cycle for CRUD routes `[TDD:RED]`
- [x] Write API integration tests: SSE event stream `[TDD:RED]`
- [x] Write API integration tests: error responses `[TDD:RED]`
- [x] Set up axum router with TLS termination (rustls) `[TDD:GREEN]`
- [x] Add auth middleware to router `[TDD:GREEN]`
- [x] Add rate limiting middleware (governor crate, 60 req/min default) `[TDD:GREEN]`
- [x] Add CORS middleware (configurable origins) `[TDD:GREEN]`
- [x] Add request logging middleware (method, path, status, duration) `[TDD:GREEN]`
- [x] Add error handling middleware (consistent error shape) `[TDD:GREEN]`
- [x] Implement `GET /api/data/{collection}` with filters, sort, pagination `[TDD:GREEN]`
- [x] Implement `GET /api/data/{collection}/{id}` `[TDD:GREEN]`
- [x] Implement `POST /api/data/{collection}` `[TDD:GREEN]`
- [x] Implement `PUT /api/data/{collection}/{id}` `[TDD:GREEN]`
- [x] Implement `DELETE /api/data/{collection}/{id}` `[TDD:GREEN]`
- [x] Implement `GET /api/system/info` (version, plugins, storage stats) `[TDD:GREEN]`
- [x] Implement `GET /api/system/plugins` (list with metadata) `[TDD:GREEN]`
- [x] Implement SSE endpoint: `GET /api/events/stream` `[TDD:GREEN]`
- [x] Implement SSE event filtering via query params `[TDD:GREEN]`
- [x] Implement plugin route registration under `/api/plugins/{plugin-id}/` `[TDD:GREEN]`
- [x] Refactor: extract shared API test helpers (request builder, assertion macros) `[TDD:REFACTOR]`
- [x] Review gate: all API tests pass, error shapes consistent, middleware stack DRY

## 1.5 — Email Connector (IMAP/SMTP)

- [x] Set up GreenMail Docker container for testing `[BLOCKER]`
- [x] Write connector tests: auth flow against GreenMail `[TDD:RED]`
- [x] Write connector tests: full sync produces canonical email records `[TDD:RED]`
- [x] Write connector tests: incremental sync detects new messages `[TDD:RED]`
- [x] Write connector tests: attachment handling `[TDD:RED]`
- [x] Write connector tests: send via SMTP `[TDD:RED]`
- [x] Implement `Connector` trait (`id`, `display_name`, `supported_collections`, `authenticate`, `sync`, `on_event`) `[TDD:GREEN]`
- [x] Implement IMAP connection with TLS (async-imap or similar) `[TDD:GREEN]`
- [x] Implement IMAP authentication (username/password, OAuth2 App Passwords) `[TDD:GREEN]`
- [x] Implement email header and body fetching `[TDD:GREEN]`
- [x] Implement incremental sync using UIDVALIDITY + UIDs `[TDD:GREEN]`
- [x] Map IMAP messages to emails canonical collection `[TDD:GREEN]`
- [x] Handle attachments (metadata in emails, binary in files collection) `[TDD:GREEN]`
- [x] Implement SMTP send capability (`POST /api/plugins/connector-email/send`) `[TDD:GREEN]`
- [x] Implement sync scheduling (configurable interval, default 5 min)
- [x] Implement manual sync trigger (`POST /api/connectors/{id}/sync`)
- [x] Implement backoff on repeated sync failures
- [x] Implement credential storage (encrypted, CRUD API, never logged)
- [x] Refactor: extract shared connector test utilities (Docker helpers, assertion macros) `[TDD:REFACTOR]`
- [x] Review gate: all connector tests pass against GreenMail, credential storage verified, DRY audit

## 1.6 — App Shell

- [x] Prototype shell layout in Google Stitch (sidebar, main area, top bar, status bar) `[STITCH]`
- [x] Prototype design system components in Stitch (button, input, card, modal) `[STITCH]`
- [x] Adapt Stitch output to use CSS custom properties and design tokens `[STITCH:ADAPT]`
- [x] Write Playwright E2E tests: shell layout renders correctly `[TDD:RED]`
- [x] Write Playwright E2E tests: sidebar navigation switches views `[TDD:RED]`
- [x] Write Playwright E2E tests: theme toggle switches light/dark `[TDD:RED]`
- [x] Write Playwright E2E tests: settings page accessible and functional `[TDD:RED]`
- [x] Write Playwright accessibility audit for all shell components `[TDD:RED]`
- [x] Configure Tauri v2 project (Rust backend, HTML/CSS/JS frontend) `[TDD:GREEN]`
- [x] Set window configuration (1200x800, resizable, title "Life Engine") `[TDD:GREEN]`
- [x] Restrict WebView capabilities to minimum needed `[TDD:GREEN]`
- [x] Implement sidebar layout (collapsible, plugin navigation) `[TDD:GREEN]`
- [x] Implement main content area (plugin container) `[TDD:GREEN]`
- [x] Implement top bar (app title, settings gear) `[TDD:GREEN]`
- [x] Implement status bar (sync status, connection indicator) `[TDD:GREEN]`
- [x] Implement `shell-button` component (primary, secondary, danger, ghost variants) `[TDD:GREEN]`
- [x] Implement `shell-input` component (text, number, date, search types) `[TDD:GREEN]`
- [x] Implement `shell-textarea` component `[TDD:GREEN]`
- [x] Implement `shell-select` component `[TDD:GREEN]`
- [x] Implement `shell-checkbox` and `shell-toggle` components `[TDD:GREEN]`
- [x] Implement `shell-card` component (default, elevated, bordered variants) `[TDD:GREEN]`
- [x] Implement `shell-list` and `shell-list-item` components `[TDD:GREEN]`
- [x] Implement `shell-badge` and `shell-avatar` components `[TDD:GREEN]`
- [x] Implement `shell-modal` and `shell-sheet` components `[TDD:GREEN]`
- [x] Implement `shell-toast`, `shell-spinner`, `shell-empty-state`, `shell-error-state` components `[TDD:GREEN]`
- [x] Implement CSS custom properties theme system `[TDD:GREEN]`
- [x] Implement light/dark mode (`prefers-color-scheme` + manual toggle) `[TDD:GREEN]`
- [x] Implement settings page (Core URL, auth token, theme, plugin management) `[TDD:GREEN]`
- [x] Implement plugin container element with loading state and error boundary `[TDD:GREEN]`
- [x] Create `shell.page.ts` Playwright page object `[PLAYWRIGHT:POM]`
- [x] Refactor: extract shared component styles to design tokens, eliminate duplication `[TDD:REFACTOR]`
- [x] Review gate: all Playwright E2E pass, accessibility audit clean, no inline styles, DRY audit

## 1.7 — Plugin Loader

- [x] Write tests: valid plugin loads correctly through 11-step lifecycle `[TDD:RED]`
- [x] Write tests: invalid manifest rejected with descriptive error `[TDD:RED]`
- [x] Write tests: oversized bundle rejected `[TDD:RED]`
- [x] Write tests: capability scoping enforced (undeclared access rejected) `[TDD:RED]`
- [x] Write Playwright E2E tests: plugin appears in sidebar after loading `[TDD:RED]`
- [x] Write Playwright E2E tests: plugin renders in container `[TDD:RED]`
- [x] Implement `plugin.json` manifest reader `[TDD:GREEN]`
- [x] Implement manifest validation (required fields, `minShellVersion`, element name) `[TDD:GREEN]`
- [x] Implement bundle size validation (warn >200KB, reject >2MB) `[TDD:GREEN]`
- [x] Implement shared module host (pre-load lit/react via import maps) `[TDD:GREEN]`
- [x] Implement scoped `ShellAPI` creation per plugin `[TDD:GREEN]`
- [x] Implement data API scoping (reject undeclared collection access) `[TDD:GREEN]`
- [x] Implement HTTP API scoping (reject undeclared domain access) `[TDD:GREEN]`
- [x] Implement IPC API scoping (reject undeclared target plugin) `[TDD:GREEN]`
- [x] Implement 11-step loading lifecycle `[TDD:GREEN]`
- [x] Implement plugin unloading (DOM removal, subscription cleanup) `[TDD:GREEN]`
- [x] Implement sidebar navigation registration from plugin slots `[TDD:GREEN]`
- [x] Refactor: DRY audit on plugin validation code `[TDD:REFACTOR]`
- [x] Review gate: all loader tests pass, capability enforcement verified, Playwright E2E pass — 204 plugin-related unit tests pass (47 loader, 104 manifest, 32 scoped-api, 21 capabilities). E2E specs exist in `tests/e2e/plugin-loader.spec.ts`. DRY audit: extracted `validateActionEntries()` helper, replaced hardcoded `VALID_DEFAULT_SIZES` duplicate.

## 1.8 — Data Layer (App)

- [x] Write tests: data CRUD against local SQLite `[TDD:RED]`
- [x] Write tests: offline queue stores and replays mutations `[TDD:RED]`
- [x] Write tests: sync status transitions (Synced/Syncing/Offline/Error) `[TDD:RED]`
- [x] Implement local SQLite database with `plugin_data` table schema `[TDD:GREEN]`
- [x] Implement Shell Data API: `query(collection, filter)` `[TDD:GREEN]`
- [x] Implement Shell Data API: `create(collection, data)` `[TDD:GREEN]`
- [x] Implement Shell Data API: `update(collection, id, data)` `[TDD:GREEN]`
- [x] Implement Shell Data API: `delete(collection, id)` `[TDD:GREEN]`
- [x] Implement Shell Data API: `subscribe(collection, callback)` `[TDD:GREEN]`
- [x] Define `SyncAdapter` TypeScript interface
- [x] Implement REST polling `SyncAdapter` (poll every 30s) `[TDD:GREEN]`
- [x] Implement push local mutations on write `[TDD:GREEN]`
- [x] Implement offline queue (store mutations, replay on reconnect) `[TDD:GREEN]`
- [x] Implement last-write-wins conflict resolution `[TDD:GREEN]`
- [x] Implement sync status indicator (Synced/Syncing/Offline/Error) `[TDD:GREEN]`
- [x] Refactor: extract shared data test factory to `packages/test-utils-js/` `[TDD:REFACTOR]`
- [x] Review gate: all data layer tests pass, offline queue verified, DRY audit

## 1.9 — Email Viewer Plugin

- [x] Prototype email list and detail views in Google Stitch `[STITCH]`
- [x] Adapt Stitch output to use shell design system components `[STITCH:ADAPT]`
- [x] Write Playwright E2E tests: email list displays synced emails `[TDD:RED]`
- [x] Write Playwright E2E tests: clicking email shows detail view `[TDD:RED]`
- [x] Write Playwright E2E tests: thread grouping displays correctly `[TDD:RED]`
- [x] Write Playwright E2E tests: search filters email list `[TDD:RED]`
- [x] Write Playwright accessibility audit for email views `[TDD:RED]`
- [x] Create `tests/e2e/pages/email-list.page.ts` page object `[PLAYWRIGHT:POM]`
- [x] Create `plugins/life/email-viewer/` directory `[TDD:GREEN]`
- [x] Write `plugin.json` manifest (id, capabilities, slots) `[TDD:GREEN]`
- [x] Implement email list view (subject, from, date, preview) `[TDD:GREEN]`
- [x] Implement email detail view (full body in sandboxed container) `[TDD:GREEN]`
- [x] Implement thread grouping by `thread_id` `[TDD:GREEN]`
- [x] Implement search/filter (by sender, subject, date range) `[TDD:GREEN]`
- [x] Implement unread indicator `[TDD:GREEN]`
- [x] Implement reactive updates via `subscribe('emails', ...)` `[TDD:GREEN]`
- [x] Use shell design system components throughout `[TDD:GREEN]`
- [x] Test end-to-end: IMAP -> Core -> App -> plugin displays emails `[TDD:GREEN]`
- [x] Refactor: DRY audit on list/detail component patterns `[TDD:REFACTOR]`
- [x] Review gate: all Playwright E2E pass, accessibility clean, no Stitch artifacts remaining

## 1.10 — Tauri Sidecar Integration

- [x] Prototype first-run wizard screens in Google Stitch `[STITCH]`
- [x] Adapt Stitch output to use shell design system `[STITCH:ADAPT]`
- [x] Write Playwright E2E tests: first-run wizard completes successfully `[TDD:RED]`
- [x] Write Playwright E2E tests: app connects to Core and shows emails `[TDD:RED]`
- [x] Write Playwright E2E tests: Core crash shows error state with restart button `[TDD:RED]`
- [x] Create `tests/e2e/pages/onboarding.page.ts` page object `[PLAYWRIGHT:POM]`
- [x] Configure Core binary as Tauri sidecar `[TDD:GREEN]`
- [x] Implement sidecar subprocess lifecycle (start on launch, stop on close) `[TDD:GREEN]`
- [x] Implement first-run welcome screen `[TDD:GREEN]`
- [x] Implement master passphrase creation screen `[TDD:GREEN]`
- [x] Implement auto-generate local auth token on first run `[TDD:GREEN]`
- [x] Implement auto-connect App to local Core `[TDD:GREEN]`
- [x] Implement email connection prompt and IMAP credential flow `[TDD:GREEN]`
- [x] Implement first sync progress display `[TDD:GREEN]`
- [x] Implement Core crash detection and error state display `[TDD:GREEN]`
- [x] Implement Core restart button `[TDD:GREEN]`
- [x] Implement graceful shutdown (finish sync, close DB, 5s timeout) `[TDD:GREEN]`
- [x] Test end-to-end: install app, create passphrase, connect email, see emails `[TDD:GREEN]`
- [x] Refactor: DRY audit on wizard screen components `[TDD:REFACTOR]`
- [x] Review gate: all Playwright E2E pass, onboarding under 5 minutes, accessibility clean
