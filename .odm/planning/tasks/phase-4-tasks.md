---
title: "Phase 4 Tasks"
tags:
  - life-engine
  - tasks
  - phase-4
  - tdd
created: 2026-03-21
category: tasks
type: long-term
---

# Phase 4 Tasks

Phase document: [[03 - Projects/Life Engine/Planning/phases/Phase 4 — WASM and Advanced]]

This file breaks every Phase 4 work package into individual checkbox tasks. Each task is a 2-4 hour unit with a verifiable outcome. Tasks marked `[BLOCKER]` must complete before dependent work can begin.

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

## 4.1 — WASM Plugin Runtime

- [x] Write tests: WASM plugin can read declared collections `[TDD:RED]`
- [x] Write tests: WASM plugin cannot access undeclared collections `[TDD:RED]`
- [x] Write tests: WASM plugin cannot exceed memory limit `[TDD:RED]`
- [x] Write tests: WASM plugin times out on long-running requests `[TDD:RED]`
- [x] Write tests: host function rate limiting enforced `[TDD:RED]`
- [x] Write tests: migrated email connector produces identical output `[TDD:RED]`
- [x] Integrate Extism as WASM runtime in Core `[TDD:GREEN]`
- [x] Implement host function: `store_read` (scoped to declared collections) `[TDD:GREEN]`
- [x] Implement host function: `store_write` (scoped) `[TDD:GREEN]`
- [x] Implement host function: `store_query` (scoped) `[TDD:GREEN]`
- [x] Implement host function: `store_delete` (scoped) `[TDD:GREEN]`
- [x] Implement host function: `config_get` `[TDD:GREEN]`
- [x] Implement host function: `event_subscribe` `[TDD:GREEN]`
- [x] Implement host function: `event_emit` `[TDD:GREEN]`
- [x] Implement host functions: `log_info`, `log_warn`, `log_error` `[TDD:GREEN]`
- [x] Implement host function: `http_request` (scoped to declared domains) `[TDD:GREEN]`
- [x] Implement capability enforcement (undeclared = function not available) `[TDD:GREEN]`
- [x] Implement memory limit per plugin (default 64 MB) `[TDD:GREEN]`
- [x] Implement execution timeout per request (default 30 seconds) `[TDD:GREEN]`
- [x] Implement rate limit on host function calls `[TDD:GREEN]`
- [x] Migrate email connector to WASM and verify identical behaviour `[TDD:GREEN]`
- [x] Migrate calendar connector to WASM
- [x] Benchmark WASM vs native plugin performance
- [x] Update `plugin-sdk-rs` with WASM compilation targets and docs
- [x] Refactor: extract shared host function scaffolding (DRY across `store_*` functions) `[TDD:REFACTOR]`
- [x] Review gate: all isolation tests pass, capability enforcement verified, benchmark documented

## 4.2 — Plugin Signing and Verification

- [x] Write tests: valid signature passes verification `[TDD:RED]`
- [x] Write tests: tampered `.wasm` file rejected on load `[TDD:RED]`
- [x] Write tests: unsigned plugin triggers warning and requires opt-in `[TDD:RED]`
- [x] Write tests: revoked key rejects previously signed plugin `[TDD:RED]`
- [x] Write tests: manifest hash included in signature `[TDD:RED]`
- [x] Implement Ed25519 signing for `.wasm` bundles `[TDD:GREEN]`
- [x] Implement signature verification before plugin loading `[TDD:GREEN]`
- [x] Implement unsigned plugin warning and explicit opt-in `[TDD:GREEN]`
- [x] Implement key revocation list `[TDD:GREEN]`
- [x] Implement manifest hash inclusion in signature `[TDD:GREEN]`
- [x] Define verification tiers: unverified, reviewed, official
- [x] Prototype verification badge UI in Google Stitch `[STITCH]`
- [x] Adapt Stitch output to shell design system `[STITCH:ADAPT]`
- [x] Update plugin store UI with verification badges `[TDD:GREEN]`
- [x] Refactor: DRY audit on signature verification code `[TDD:REFACTOR]`
- [x] Review gate: all signing tests pass, tampered code rejected, badge UI in plugin store

## 4.3 — Multi-User / Household Support

- [x] Write tests: user A cannot read user B's private data `[TDD:RED]`
- [x] Write tests: shared collections accessible by all members `[TDD:RED]`
- [x] Write tests: role-based access enforced (admin, member, guest) `[TDD:RED]`
- [x] Write tests: invite flow creates new member with correct role `[TDD:RED]`
- [x] Write Playwright E2E tests: household management UI `[TDD:RED]`
- [x] Write Playwright E2E tests: invite member flow `[TDD:RED]`
- [x] Write Playwright E2E tests: shared collection creation and access `[TDD:RED]`
- [x] Write Playwright accessibility audit for household UI `[TDD:RED]`
- [x] Create `tests/e2e/pages/household.page.ts` page object `[PLAYWRIGHT:POM]`
- [x] Implement multiple user accounts via Pocket ID `[TDD:GREEN]`
- [x] Implement per-user data namespace isolation `[TDD:GREEN]`
- [x] Implement shared collection support (family calendar, shopping list) `[TDD:GREEN]`
- [x] Implement role-based access: admin, member, guest `[TDD:GREEN]`
- [x] Prototype household management UI in Google Stitch `[STITCH]`
- [x] Adapt Stitch output to shell design system `[STITCH:ADAPT]`
- [x] Implement household management UI: invite members `[TDD:GREEN]`
- [x] Implement household management UI: manage roles `[TDD:GREEN]`
- [x] Implement household management UI: create/manage shared collections `[TDD:GREEN]`
- [x] Implement household management UI: activity feed for shared collections `[TDD:GREEN]`
- [x] Refactor: extract shared permission checking logic (DRY across isolation and roles) `[TDD:REFACTOR]`
- [x] Review gate: all isolation tests pass, Playwright E2E pass, RBAC verified

## 4.4 — GraphQL API Plugin

- [x] Write tests: auto-generated types match CDM schemas `[TDD:RED]`
- [x] Write tests: queries return correct data for all collections `[TDD:RED]`
- [x] Write tests: mutations create/update/delete correctly `[TDD:RED]`
- [x] Write tests: subscriptions deliver real-time updates `[TDD:RED]`
- [x] Write tests: nested queries resolve relationships `[TDD:RED]`
- [x] Write tests: filtering, sorting, pagination work correctly `[TDD:RED]`
- [x] Auto-generate GraphQL types from CDM schemas `[TDD:GREEN]`
- [x] Implement queries for all canonical collections `[TDD:GREEN]`
- [x] Implement mutations for all canonical collections `[TDD:GREEN]`
- [x] Implement subscriptions via WebSocket `[TDD:GREEN]`
- [x] Implement nested queries (event -> attendees -> contacts) `[TDD:GREEN]`
- [x] Implement filtering, sorting, pagination `[TDD:GREEN]`
- [x] Serve GraphQL Playground `[TDD:GREEN]`
- [x] Refactor: ensure GraphQL resolvers reuse `StorageAdapter` (DRY with REST API) `[TDD:REFACTOR]`
- [x] Review gate: all GraphQL tests pass, types match CDM, resolvers share storage layer

## 4.5 — Federated Sync (Hub-to-Hub)

- [x] Write tests: record created on instance A appears on instance B `[TDD:RED]`
- [x] Write tests: selective sync only transfers declared collections `[TDD:RED]`
- [x] Write tests: mTLS rejects unauthenticated peers `[TDD:RED]`
- [x] Write tests: federation status API reports correctly `[TDD:RED]`
- [x] Design federation protocol specification
- [x] Implement mTLS encrypted transport between instances `[TDD:GREEN]`
- [x] Implement selective sync (choose collections and records) `[TDD:GREEN]`
- [x] Implement pull-based sync model `[TDD:GREEN]`
- [x] Implement federation API: `POST /api/federation/peers` `[TDD:GREEN]`
- [x] Implement federation API: `POST /api/federation/sync` `[TDD:GREEN]`
- [x] Implement federation API: `GET /api/federation/status` `[TDD:GREEN]`
- [x] Refactor: extract shared sync primitives (DRY with Core-to-App sync) `[TDD:REFACTOR]`
- [x] Review gate: all federation tests pass, protocol spec documented, sync logic DRY

## 4.6 — Encrypted Remote Backup

- [x] Write tests: full backup encrypts and restores correctly `[TDD:RED]`
- [x] Write tests: incremental backup only includes changed records `[TDD:RED]`
- [x] Write tests: backup to S3 round-trip `[TDD:RED]`
- [x] Write tests: backup to WebDAV round-trip `[TDD:RED]`
- [x] Write tests: retention policy deletes old backups `[TDD:RED]`
- [x] Write tests: partial restore recovers specific collections `[TDD:RED]`
- [x] Implement full database backup encrypted with master passphrase `[TDD:GREEN]`
- [x] Implement incremental backup (changed records since last backup) `[TDD:GREEN]`
- [x] Implement backup to local directory `[TDD:GREEN]`
- [x] Implement backup to S3-compatible storage `[TDD:GREEN]`
- [x] Implement backup to WebDAV `[TDD:GREEN]`
- [x] Implement configurable backup schedule (daily, weekly) `[TDD:GREEN]`
- [x] Implement retention policy (keep last N backups) `[TDD:GREEN]`
- [x] Implement restore command with integrity verification `[TDD:GREEN]`
- [x] Implement partial restore (specific collections) `[TDD:GREEN]`
- [x] Refactor: extract shared storage backend interface (DRY across local/S3/WebDAV) `[TDD:REFACTOR]`
- [x] Review gate: all backup tests pass, encryption verified, storage backends share interface

## 4.7 — Identity and Credential System

- [x] Write tests: credential CRUD operations (create, read, update, delete) `[TDD:RED]`
- [x] Write tests: credentials never appear in logs `[TDD:RED]`
- [x] Write tests: selective disclosure produces valid time-limited tokens `[TDD:RED]`
- [x] Write tests: audit log records all disclosures `[TDD:RED]`
- [x] Write tests: W3C VC format validates against spec `[TDD:RED]`
- [x] Implement encrypted credential store with separate encryption key `[TDD:GREEN]`
- [x] Implement credential CRUD API (never log contents) `[TDD:GREEN]`
- [x] Implement selective disclosure: signed time-limited tokens `[TDD:GREEN]`
- [x] Implement audit log for disclosures (what, to whom, when) `[TDD:GREEN]`
- [x] Implement W3C Verifiable Credentials 2.0 format support `[TDD:GREEN]`
- [x] Implement DID alignment `[TDD:GREEN]`
- [x] Refactor: extract shared encryption utilities (DRY with SQLCipher and backup encryption) `[TDD:REFACTOR]`
- [x] Review gate: all credential tests pass, no credential leakage in logs, encryption DRY

## 4.8 — Mobile Releases

- [x] Write Playwright E2E tests: mobile viewport renders bottom navigation `[TDD:RED]`
- [x] Write Playwright E2E tests: bottom sheets replace modals `[TDD:RED]`
- [x] Write Playwright E2E tests: touch-optimised input sizes `[TDD:RED]`
- [x] Write Playwright E2E tests: pull-to-refresh triggers sync `[TDD:RED]`
- [x] Write Playwright accessibility audit for mobile UI `[TDD:RED]`
- [x] Create `tests/e2e/pages/mobile-shell.page.ts` page object `[PLAYWRIGHT:POM]`
- [x] Prototype mobile layout in Google Stitch (bottom nav, bottom sheets, touch targets) `[STITCH]`
- [x] Adapt Stitch output to shell design system with mobile breakpoints `[STITCH:ADAPT]`
- [x] Configure Tauri v2 iOS build (WKWebView, Xcode project) `[TDD:GREEN]`
- [x] Configure Tauri v2 Android build (WebView, Gradle project) `[TDD:GREEN]`
- [x] Implement bottom navigation bar (replace sidebar) `[TDD:GREEN]`
- [x] Implement bottom sheets (replace modals where appropriate) `[TDD:GREEN]`
- [x] Implement touch-optimised input sizes `[TDD:GREEN]`
- [x] Implement pull-to-refresh for sync `[TDD:GREEN]`
- [x] Implement iOS background fetch for sync `[TDD:GREEN]`
- [x] Implement Android WorkManager for background sync `[TDD:GREEN]`
- [x] Implement push notifications for important events `[TDD:GREEN]`
- [x] Handle battery optimisation (reduce sync frequency) `[TDD:GREEN]`
- [x] Submit to Apple App Store
- [x] Submit to Google Play Store
- [x] Refactor: ensure mobile components share design tokens with desktop (DRY across viewports) `[TDD:REFACTOR]`
- [x] Review gate: Playwright E2E pass on mobile viewport, accessibility clean, app store submissions accepted
