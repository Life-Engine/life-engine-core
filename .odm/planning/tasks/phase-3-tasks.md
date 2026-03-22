---
title: "Phase 3 Tasks"
tags:
  - life-engine
  - tasks
  - phase-3
  - tdd
created: 2026-03-21
category: tasks
type: mid-term
---

# Phase 3 Tasks

Phase document: [[03 - Projects/Life Engine/Planning/phases/Phase 3 — Ecosystem and Polish]]

This file breaks every Phase 3 work package into individual checkbox tasks. Each task is a 2-4 hour unit with a verifiable outcome. Tasks marked `[BLOCKER]` must complete before dependent work can begin.

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

## 3.1 — Plugin Store Infrastructure

- [x] Write tests: plugin submission CI validates manifest, size, and capabilities `[TDD:RED]`
- [x] Write tests: plugin registry index contains valid entries `[TDD:RED]`
- [x] Write Playwright E2E tests: browse plugins by category `[TDD:RED]`
- [x] Write Playwright E2E tests: search plugins by name/description `[TDD:RED]`
- [x] Write Playwright E2E tests: install and uninstall a plugin `[TDD:RED]`
- [x] Write Playwright E2E tests: capabilities approval flow on install `[TDD:RED]`
- [x] Write Playwright accessibility audit for plugin store `[TDD:RED]`
- [x] Create `tests/e2e/pages/plugin-store.page.ts` page object `[PLAYWRIGHT:POM]`
- [x] Create plugin registry JSON index in monorepo `[TDD:GREEN]`
- [x] Define plugin submission PR template and validation rules `[TDD:GREEN]`
- [x] Implement CI validation for plugin submissions (manifest, size, capability audit) `[TDD:GREEN]`
- [x] Prototype plugin store UI in Google Stitch (browse, detail, install) `[STITCH]`
- [x] Adapt Stitch output to shell design system `[STITCH:ADAPT]`
- [x] Implement plugin store UI: browse by category `[TDD:GREEN]`
- [x] Implement plugin store UI: search by name/description `[TDD:GREEN]`
- [x] Implement plugin store UI: plugin detail page `[TDD:GREEN]`
- [x] Implement plugin store UI: install/uninstall buttons `[TDD:GREEN]`
- [x] Implement plugin store UI: installed tab with update check `[TDD:GREEN]`
- [x] Implement capabilities approval flow on install `[TDD:GREEN]`
- [x] Refactor: extract shared list/search/detail pattern (DRY with email viewer, calendar) `[TDD:REFACTOR]`
- [x] Review gate: all tests pass, Playwright E2E pass, accessibility clean, no Stitch artifacts

## 3.2 — Pipeline Canvas (Visual Builder)

- [x] Write Playwright E2E tests: add connector node via drag-and-drop `[TDD:RED]`
- [x] Write Playwright E2E tests: connect two nodes `[TDD:RED]`
- [x] Write Playwright E2E tests: node configuration panel opens and saves `[TDD:RED]`
- [x] Write Playwright E2E tests: sync status updates in real-time `[TDD:RED]`
- [x] Write Playwright accessibility audit for canvas `[TDD:RED]`
- [x] Create `tests/e2e/pages/pipeline-canvas.page.ts` page object `[PLAYWRIGHT:POM]`
- [x] Prototype pipeline canvas in Google Stitch (node editor, config panel, status dashboard) `[STITCH]`
- [x] Adapt Stitch output to shell design system `[STITCH:ADAPT]`
- [x] Implement node-based visual editor (Canvas or SVG) `[TDD:GREEN]`
- [x] Implement source node type (connectors, sync status, record count) `[TDD:GREEN]`
- [x] Implement processor node type (search indexer, deduplicator) `[TDD:GREEN]`
- [x] Implement output node type (REST API, CalDAV, CardDAV) `[TDD:GREEN]`
- [x] Implement drag-and-drop to add/remove/connect nodes `[TDD:GREEN]`
- [x] Implement node configuration panel (auto-generated from plugin config schema) `[TDD:GREEN]`
- [x] Implement OAuth flow launch from settings panel `[TDD:GREEN]`
- [x] Implement real-time sync status dashboard `[TDD:GREEN]`
- [x] Implement error indicators and quarantine count display `[TDD:GREEN]`
- [x] Implement total record counts per collection `[TDD:GREEN]`
- [x] Refactor: extract reusable canvas interaction primitives `[TDD:REFACTOR]`
- [x] Review gate: all Playwright E2E pass, accessibility clean, canvas interactions tested

## 3.3 — First-Run Onboarding Flow

- [x] Write Playwright E2E tests: complete onboarding in under 5 minutes `[TDD:RED]`
- [x] Write Playwright E2E tests: skip option works at each step `[TDD:RED]`
- [x] Write Playwright E2E tests: passphrase strength indicator updates `[TDD:RED]`
- [x] Write Playwright E2E tests: OAuth flow completes for chosen connector `[TDD:RED]`
- [x] Write Playwright accessibility audit for onboarding flow `[TDD:RED]`
- [x] Update `tests/e2e/pages/onboarding.page.ts` page object with full flow `[PLAYWRIGHT:POM]`
- [x] Prototype onboarding screens in Google Stitch (welcome, passphrase, connect, progress, done) `[STITCH]`
- [x] Adapt Stitch output to shell design system `[STITCH:ADAPT]`
- [x] Implement welcome screen with brief explanation `[TDD:GREEN]`
- [x] Implement passphrase creation with strength indicator `[TDD:GREEN]`
- [x] Implement "What do you want to connect first?" screen `[TDD:GREEN]`
- [x] Implement OAuth/credential flow for chosen connectors `[TDD:GREEN]`
- [x] Implement real-time sync progress with record count `[TDD:GREEN]`
- [x] Implement "Your data is synced" suggestions screen `[TDD:GREEN]`
- [x] Implement pipeline canvas auto-population `[TDD:GREEN]`
- [x] Implement skip option at each step `[TDD:GREEN]`
- [x] Test: complete onboarding in under 5 minutes `[TDD:GREEN]`
- [x] Refactor: DRY audit on wizard step components (extract shared wizard framework) `[TDD:REFACTOR]`
- [x] Review gate: Playwright E2E pass, onboarding under 5 minutes, accessibility clean

## 3.4 — PostgreSQL Storage Plugin

- [x] Write tests: same test suite as SQLite passes with PostgreSQL `[TDD:RED]`
- [x] Write tests: migration from SQLite to PostgreSQL (record count match) `[TDD:RED]`
- [x] Write tests: atomic migration with rollback on failure `[TDD:RED]`
- [x] Write tests: full-text search via tsvector `[TDD:RED]`
- [x] Implement `StorageAdapter` for PostgreSQL with connection pooling `[TDD:GREEN]`
- [x] Implement JSONB column for document storage `[TDD:GREEN]`
- [x] Implement full-text search via tsvector `[TDD:GREEN]`
- [x] Implement storage migration command (sqlite -> postgres) `[TDD:GREEN]`
- [x] Implement migration verification (record count match) `[TDD:GREEN]`
- [x] Implement atomic migration with rollback on failure `[TDD:GREEN]`
- [x] Implement progress indicator for large datasets `[TDD:GREEN]`
- [x] Implement storage picker in onboarding wizard `[TDD:GREEN]`
- [x] Refactor: ensure `StorageAdapter` trait is fully DRY (same interface, no PG-specific leaks) `[TDD:REFACTOR]`
- [x] Review gate: PG test suite identical to SQLite suite, migration verified, trait interface DRY

## 3.5 — CalDAV/CardDAV API Plugins

- [x] Write tests: CalDAV PROPFIND, REPORT, GET, PUT, DELETE operations `[TDD:RED]`
- [x] Write tests: iCalendar VEVENT serialisation round-trip `[TDD:RED]`
- [x] Write tests: CardDAV vCard serialisation round-trip `[TDD:RED]`
- [x] Write tests: service discovery (`.well-known`) endpoints `[TDD:RED]`
- [x] Implement CalDAV server plugin: PROPFIND, REPORT, GET, PUT, DELETE `[TDD:GREEN]`
- [x] Implement CalDAV server plugin: iCalendar VEVENT serialisation `[TDD:GREEN]`
- [x] Implement CardDAV server plugin: vCard serialisation `[TDD:GREEN]`
- [x] Implement service discovery (`.well-known/caldav`, `.well-known/carddav`) `[TDD:GREEN]`
- [x] Write DNS SRV records documentation
- [x] Test: iOS Calendar connects and displays events `[TDD:GREEN]`
- [x] Test: Thunderbird connects and displays contacts `[TDD:GREEN]`
- [x] Refactor: extract shared RFC serialisation helpers (DRY with CalDAV/CardDAV connectors) `[TDD:REFACTOR]`
- [x] Review gate: all protocol tests pass, native client connectivity verified, serialisation DRY

## 3.6 — Webhook Support

- [x] Write tests: webhook receiver accepts and maps payload `[TDD:RED]`
- [x] Write tests: HMAC-SHA256 signature verification `[TDD:RED]`
- [x] Write tests: webhook sender delivers on event with retry `[TDD:RED]`
- [x] Write tests: delivery log records status codes `[TDD:RED]`
- [x] Implement webhook receiver plugin (`POST /api/plugins/webhooks/receive/{id}`) `[TDD:GREEN]`
- [x] Implement configurable payload mapping (JSON path -> CDM fields) `[TDD:GREEN]`
- [x] Implement HMAC-SHA256 signature verification `[TDD:GREEN]`
- [x] Implement webhook sender plugin (subscribe to message bus events) `[TDD:GREEN]`
- [x] Implement retry with exponential backoff `[TDD:GREEN]`
- [x] Implement delivery log with status codes `[TDD:GREEN]`
- [x] Refactor: extract shared retry/backoff logic (DRY with connector sync) `[TDD:REFACTOR]`
- [x] Review gate: all webhook tests pass, retry logic shared with connectors

## 3.7 — Additional App Plugins

- [x] Prototype all plugin UIs in Google Stitch (tasks, notes, contacts, files, dashboard) `[STITCH]`
- [x] Adapt Stitch output to shell design system `[STITCH:ADAPT]`
- [x] Write Playwright E2E tests: task manager CRUD flow `[TDD:RED]`
- [x] Write Playwright E2E tests: notes create and edit with markdown `[TDD:RED]`
- [x] Write Playwright E2E tests: contacts list, search, detail view `[TDD:RED]`
- [x] Write Playwright E2E tests: files browser navigation `[TDD:RED]`
- [x] Write Playwright E2E tests: dashboard widgets render with data `[TDD:RED]`
- [x] Write Playwright accessibility audit for all plugins `[TDD:RED]`
- [x] Create page objects for each plugin in `tests/e2e/pages/` `[PLAYWRIGHT:POM]`
- [x] Implement Task Manager plugin (CRUD, projects, labels, priorities, due dates) `[TDD:GREEN]`
- [x] Implement Notes plugin (rich text with markdown support) `[TDD:GREEN]`
- [x] Implement Contacts plugin (list, search, groups, detail view) `[TDD:GREEN]`
- [x] Implement Files plugin (file browser for indexed files) `[TDD:GREEN]`
- [x] Implement Dashboard plugin (upcoming events, recent emails, pending tasks, sync status widgets) `[TDD:GREEN]`
- [x] Refactor: extract shared CRUD plugin scaffold (DRY across all data-driven plugins) `[TDD:REFACTOR]`
- [x] Review gate: all Playwright E2E pass, accessibility clean, shared CRUD scaffold extracted
