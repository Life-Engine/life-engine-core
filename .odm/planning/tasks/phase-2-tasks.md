---
title: "Phase 2 Tasks"
tags:
  - life-engine
  - tasks
  - phase-2
  - tdd
created: 2026-03-21
category: tasks
type: mid-term
---

# Phase 2 Tasks

Phase document: [[03 - Projects/Life Engine/Planning/phases/Phase 2 — Connectors and Features]]

This file breaks every Phase 2 work package into individual checkbox tasks. Each task is a 2-4 hour unit with a verifiable outcome. Tasks marked `[BLOCKER]` must complete before dependent work can begin.

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

## 2.1 — Schema Registry and CDM Validation

- [x] Write tests: valid records pass validation `[TDD:RED]`
- [x] Write tests: invalid records rejected with descriptive errors `[TDD:RED]`
- [x] Write tests: quarantine stores rejected records `[TDD:RED]`
- [x] Write tests: re-process quarantined records `[TDD:RED]`
- [x] Implement JSON Schema loading on Core startup `[TDD:GREEN]`
- [x] Implement record validation before storage `[TDD:GREEN]`
- [x] Implement quarantine collection for invalid records `[TDD:GREEN]`
- [x] Implement admin endpoint: `GET /api/system/quarantine` `[TDD:GREEN]`
- [x] Implement re-process quarantined records endpoint `[TDD:GREEN]`
- [x] Implement private collection schema validation from manifests `[TDD:GREEN]`
- [x] Implement schema version tracking per record `[TDD:GREEN]`
- [x] Refactor: extract shared validation logic (reuse CDM schemas from Phase 0) `[TDD:REFACTOR]`
- [x] Review gate: all validation tests pass, quarantine verified, no duplicated schema logic

## 2.2 — Calendar Connector (CalDAV + Google Calendar)

- [x] Set up Radicale Docker container for CalDAV testing `[BLOCKER]`
- [x] Write connector tests: CalDAV auth and connection `[TDD:RED]`
- [x] Write connector tests: CalDAV sync round-trip (create, sync, verify) `[TDD:RED]`
- [x] Write connector tests: CalDAV incremental sync via sync-token `[TDD:RED]`
- [x] Write connector tests: recurrence rule handling `[TDD:RED]`
- [x] Write connector tests: Google Calendar sync `[TDD:RED]`
- [x] Implement CalDAV connector: connect to any CalDAV server `[TDD:GREEN]`
- [x] Implement CalDAV connector: Basic auth and OAuth2 support `[TDD:GREEN]`
- [x] Implement CalDAV connector: bidirectional event sync `[TDD:GREEN]`
- [x] Implement CalDAV connector: VEVENT to events collection mapping `[TDD:GREEN]`
- [x] Implement CalDAV connector: recurrence rule handling (RRULE) `[TDD:GREEN]`
- [x] Implement CalDAV connector: incremental sync via sync-token/ctag `[TDD:GREEN]`
- [x] Implement Google Calendar connector: OAuth2 PKCE flow `[TDD:GREEN]`
- [x] Implement Google Calendar connector: Calendar API v3 integration `[TDD:GREEN]`
- [x] Implement Google Calendar connector: incremental sync via syncToken `[TDD:GREEN]`
- [x] Implement Google Calendar connector: Google-specific extensions `[TDD:GREEN]`
- [x] Refactor: extract shared connector test helpers (reuse Docker utilities from Phase 1) `[TDD:REFACTOR]`
- [x] Review gate: all connector tests pass against Radicale, no duplicated Docker/auth helpers

## 2.3 — Contacts Connector (CardDAV + Google Contacts)

- [x] Write connector tests: CardDAV sync round-trip `[TDD:RED]`
- [x] Write connector tests: vCard to contacts mapping `[TDD:RED]`
- [x] Write connector tests: contact photo handling `[TDD:RED]`
- [x] Write connector tests: Google Contacts sync `[TDD:RED]`
- [x] Implement CardDAV connector: bidirectional contact sync `[TDD:GREEN]`
- [x] Implement CardDAV connector: vCard to contacts collection mapping `[TDD:GREEN]`
- [x] Implement CardDAV connector: contact photo handling `[TDD:GREEN]`
- [x] Implement Google Contacts connector: People API integration `[TDD:GREEN]`
- [x] Implement Google Contacts connector: reuse Google OAuth token `[TDD:GREEN]`
- [x] Refactor: extract shared vCard/iCal parsing utilities (DRY with CalDAV connector) `[TDD:REFACTOR]`
- [x] Review gate: all connector tests pass, shared parsing utilities extracted

## 2.4 — File System Connector

- [x] Write tests: filesystem change detection and indexing `[TDD:RED]`
- [x] Write tests: S3 operations against MinIO Docker container `[TDD:RED]`
- [x] Set up MinIO Docker container for S3 testing
- [x] Implement local filesystem connector: directory watching (notify crate) `[TDD:GREEN]`
- [x] Implement local filesystem connector: index to files collection (metadata only) `[TDD:GREEN]`
- [x] Implement local filesystem connector: track moves, renames, deletions `[TDD:GREEN]`
- [x] Implement local filesystem connector: configurable include/exclude patterns `[TDD:GREEN]`
- [x] Define `CloudStorageConnector` trait
- [x] Implement S3-compatible connector (list, download, upload, delete) `[TDD:GREEN]`
- [x] Refactor: extract shared file metadata helpers `[TDD:REFACTOR]`
- [x] Review gate: all filesystem/S3 tests pass, connector trait is DRY with other connectors

## 2.5 — Full-Text Search

- [x] Write tests: index creation from canonical records `[TDD:RED]`
- [x] Write tests: query accuracy and relevance ranking `[TDD:RED]`
- [x] Write tests: search API pagination and filtering `[TDD:RED]`
- [x] Write Playwright E2E tests: search bar returns results `[TDD:RED]`
- [x] Write Playwright E2E tests: click result navigates to plugin `[TDD:RED]`
- [x] Create `tests/e2e/pages/search.page.ts` page object `[PLAYWRIGHT:POM]`
- [x] Implement search processor plugin: subscribe to `NewRecords` events `[TDD:GREEN]`
- [x] Implement tantivy index for text content `[TDD:GREEN]`
- [x] Implement query syntax: simple text, phrases, field-specific, boolean `[TDD:GREEN]`
- [x] Implement search API: `GET /api/search` with collection filtering `[TDD:GREEN]`
- [x] Implement paginated results with relevance scoring `[TDD:GREEN]`
- [x] Prototype search UI in Google Stitch (search bar, grouped results) `[STITCH]`
- [x] Adapt Stitch output to shell design system `[STITCH:ADAPT]`
- [x] Implement search bar in App top bar `[TDD:GREEN]`
- [x] Implement search results grouped by collection `[TDD:GREEN]`
- [x] Implement click-to-navigate from search result to plugin `[TDD:GREEN]`
- [x] Refactor: DRY audit on search query parsing `[TDD:REFACTOR]`
- [x] Review gate: all search tests pass, Playwright E2E pass, accessibility audit clean

## 2.6 — Conflict Resolution Engine

- [x] Write tests: concurrent edits from two clients resolved correctly `[TDD:RED]`
- [x] Write tests: offline/online transition replays without data loss `[TDD:RED]`
- [x] Write tests: field-level merge produces correct output `[TDD:RED]`
- [x] Write tests: manual resolution flags conflicts for user `[TDD:RED]`
- [x] Write Playwright E2E tests: conflict notification appears in UI `[TDD:RED]`
- [x] Write Playwright E2E tests: side-by-side diff renders correctly `[TDD:RED]`
- [x] Write Playwright E2E tests: choosing resolution clears conflict `[TDD:RED]`
- [x] Create `tests/e2e/pages/conflict-resolution.page.ts` page object `[PLAYWRIGHT:POM]`
- [x] Implement last-write-wins resolution strategy `[TDD:GREEN]`
- [x] Implement field-level merge resolution strategy `[TDD:GREEN]`
- [x] Implement manual resolution strategy (flag for user) `[TDD:GREEN]`
- [x] Implement conflict detection via version comparison on sync `[TDD:GREEN]`
- [-] Prototype conflict resolution UI in Google Stitch (notification, side-by-side diff) `[STITCH]` — Stitch MCP unavailable, skipped
- [-] Adapt Stitch output to shell design system `[STITCH:ADAPT]` — skipped (depends on Stitch)
- [x] Implement conflict notification in App ("N conflicts need attention") `[TDD:GREEN]`
- [x] Implement conflict resolution UI (side-by-side diff, choose action) `[TDD:GREEN]`
- [x] Refactor: extract shared conflict strategy interface (DRY across strategies) `[TDD:REFACTOR]`
- [x] Review gate: all conflict tests pass, Playwright E2E pass, strategies share common interface

## 2.7 — Pocket ID Integration

- [x] Write tests: full OIDC auth flow end-to-end `[TDD:RED]`
- [x] Write tests: token refresh on expiry `[TDD:RED]`
- [x] Write tests: passkey/WebAuthn registration and login `[TDD:RED]`
- [x] Write Playwright E2E tests: login screen renders and accepts credentials `[TDD:RED]`
- [x] Write Playwright E2E tests: auto-refresh on token expiry `[TDD:RED]`
- [x] Create `tests/e2e/pages/login.page.ts` page object `[PLAYWRIGHT:POM]`
- [x] Bundle Pocket ID Go binary as Core sidecar `[TDD:GREEN]`
- [x] Implement Pocket ID subprocess management (spawn, health check, shutdown) `[TDD:GREEN]`
- [x] Implement OIDC token validation in Core auth middleware `[TDD:GREEN]`
- [x] Implement user registration endpoint (`POST /api/auth/register`) `[TDD:GREEN]`
- [x] Implement login endpoint (`POST /api/auth/login`) `[TDD:GREEN]`
- [x] Implement token refresh flow `[TDD:GREEN]`
- [x] Implement passkey/WebAuthn support via Pocket ID `[TDD:GREEN]`
- [-] Prototype login screen in Google Stitch `[STITCH]` — Stitch MCP unavailable, skipped
- [-] Adapt Stitch login screen to shell design system `[STITCH:ADAPT]` — skipped (depends on Stitch)
- [x] Update App: implement login screen `[TDD:GREEN]`
- [x] Update App: store tokens in OS keychain (Tauri secure storage) `[TDD:GREEN]`
- [x] Update App: implement auto-refresh on token expiry `[TDD:GREEN]`
- [x] Refactor: DRY audit on auth middleware (reuse `AuthProvider` trait from Phase 1) `[TDD:REFACTOR]`
- [x] Review gate: all auth tests pass, Playwright E2E pass, OIDC flow verified end-to-end

## 2.8 — Standalone Core Deployment

- [x] Write tests: Docker image builds and starts successfully `[TDD:RED]`
- [x] Write tests: `docker-compose up` serves Core and Pocket ID `[TDD:RED]`
- [x] Write tests: health check responds from Docker container `[TDD:RED]`
- [x] Create Docker multi-stage build (compile -> Alpine runtime) `[TDD:GREEN]`
- [x] Create `docker-compose.yml` (Core + Pocket ID, volume mounts) `[TDD:GREEN]`
- [x] Verify Docker image under 50 MB `[TDD:GREEN]`
- [x] Create Linux installation script (systemd service)
- [x] Create macOS installation script (launchd plist)
- [x] Build ARM64 binary for Raspberry Pi
- [x] Write deployment docs: Docker quick start
- [x] Write deployment docs: bare metal installation
- [x] Write deployment docs: reverse proxy configuration (nginx, Caddy)
- [x] Write deployment docs: TLS with Let's Encrypt
- [x] Review gate: Docker tests pass, image size verified, docs reviewed

## 2.9 — Calendar Plugin (App)

- [-] Prototype calendar views in Google Stitch (month, week, day, agenda) `[STITCH]` — Stitch MCP unavailable, skipped
- [-] Adapt Stitch output to use shell design system components `[STITCH:ADAPT]` — skipped (depends on Stitch)
- [x] Write Playwright E2E tests: month view renders with events `[TDD:RED]`
- [x] Write Playwright E2E tests: week view shows time-slot layout `[TDD:RED]`
- [x] Write Playwright E2E tests: day view shows hourly timeline `[TDD:RED]`
- [x] Write Playwright E2E tests: quick-add event from time slot `[TDD:RED]`
- [x] Write Playwright E2E tests: full event form submits and syncs `[TDD:RED]`
- [x] Write Playwright accessibility audit for calendar views `[TDD:RED]`
- [x] Create `tests/e2e/pages/calendar.page.ts` page object `[PLAYWRIGHT:POM]`
- [x] Create `plugins/life/calendar/` directory and `plugin.json` `[TDD:GREEN]`
- [x] Implement month view (grid with event dots/titles) `[TDD:GREEN]`
- [x] Implement week view (time-slot layout with event blocks) `[TDD:GREEN]`
- [x] Implement day view (detailed hourly timeline) `[TDD:GREEN]`
- [x] Implement agenda view (chronological upcoming list) `[TDD:GREEN]`
- [x] Implement quick-add event (click on time slot) `[TDD:GREEN]`
- [x] Implement full event form (title, time, location, description, recurrence) `[TDD:GREEN]`
- [x] Implement bidirectional sync (changes push to connected providers) `[TDD:GREEN]`
- [x] Implement multi-calendar colour coding and visibility toggle `[TDD:GREEN]`
- [x] Use shell design system components throughout `[TDD:GREEN]`
- [x] Refactor: extract shared list/detail patterns (DRY with email viewer) `[TDD:REFACTOR]`
- [x] Review gate: all Playwright E2E pass, accessibility clean, no Stitch artifacts, DRY with email plugin
