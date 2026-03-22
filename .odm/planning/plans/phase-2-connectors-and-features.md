---
title: "Phase 2 — Connectors and Features"
tags:
  - life-engine
  - planning
  - phase-2
  - connectors
  - features
created: 2026-03-21
---

# Phase 2 — Connectors and Features

## Goal

Multiple data sources connected, full-text search, conflict resolution, Pocket ID auth, and standalone Core deployment.

## Entry Criteria

- Phase 1 complete
- Core and App working end-to-end with sidecar mode
- Email connector syncing and displaying in the email viewer plugin

## Table of Contents

- [[#2.1 — Schema Registry and CDM Validation]]
- [[#2.2 — Calendar Connector (CalDAV + Google Calendar)]]
- [[#2.3 — Contacts Connector (CardDAV + Google Contacts)]]
- [[#2.4 — File System Connector]]
- [[#2.5 — Full-Text Search]]
- [[#2.6 — Conflict Resolution Engine]]
- [[#2.7 — Pocket ID Integration]]
- [[#2.8 — Standalone Core Deployment]]
- [[#2.9 — Calendar Plugin (App)]]
- [[#2.10 — Website: API Reference and Guides]]
- [[#Exit Criteria]]
- [[#Phase-Specific Risks]]

---

## Work Packages

### 2.1 — Schema Registry and CDM Validation

**Scope**

Implement runtime schema validation for all data entering Core:

- **Schema loading** — load JSON Schema files from `.odm/docs/schemas/` on startup
- **Write-time validation** — validate every record against its collection schema before storage
- **Quarantine** — invalid records are stored in a `_quarantine` collection with error details (validation errors, original payload, source, timestamp)
- **Admin endpoint** — `GET /api/system/quarantine` for reviewing and reprocessing quarantined records
- **Private collection validation** — validate private collection schemas declared in plugin manifests
- **Schema version tracking** — store the schema version with each record to support future migrations

**Deliverables**

- Invalid data is caught at write time and quarantined rather than silently stored
- Schema evolution is tracked per record

**Dependencies**

- Phase 1

---

### 2.2 — Calendar Connector (CalDAV + Google Calendar)

**Scope**

Implement two calendar connectors:

- **CalDAV connector** — connects to any CalDAV server (Radicale, Nextcloud, iCloud), bidirectional sync, VEVENT parsing and mapping to `events` canonical collection, recurrence rule handling (RRULE expansion), incremental sync via `sync-token` and `ctag`
- **Google Calendar connector** — OAuth2 PKCE authentication flow, Google Calendar API v3 integration, `syncToken` for incremental sync, Google-specific extensions (colour, conferencing links, reminders)

Both connectors share the same `events` collection mapping.

**Deliverables**

- Core syncs calendar events from any CalDAV server and Google Calendar

**Dependencies**

- Phase 1

---

### 2.3 — Contacts Connector (CardDAV + Google Contacts)

**Scope**

Implement two contacts connectors:

- **CardDAV connector** — connects to any CardDAV server, bidirectional sync, vCard parsing and mapping to `contacts` canonical collection, contact photo handling (store as file reference)
- **Google Contacts connector** — Google People API integration, reuses Google OAuth token from the calendar connector, maps Google-specific fields to CDM extensions

**Deliverables**

- Core syncs contacts from any CardDAV server and Google Contacts

**Dependencies**

- Phase 1

---

### 2.4 — File System Connector

**Scope**

Implement file indexing connectors:

- **Local filesystem connector** — watch configured directories using the `notify` crate for filesystem events, index files to the `files` canonical collection (metadata only: name, path, size, type, modified date, hash), track moves/renames/deletions, configurable include/exclude glob patterns
- **Cloud storage connector interface** — S3-compatible API (list, download, upload, delete), works with AWS S3, MinIO, Backblaze B2, and other compatible providers

**Deliverables**

- Core indexes local files and connects to S3-compatible cloud storage

**Dependencies**

- Phase 1

---

### 2.5 — Full-Text Search

**Scope**

Implement cross-collection search:

- **Search processor plugin** — subscribes to `NewRecords` events on the message bus, indexes content using tantivy (Rust full-text search engine), supports query syntax with field-specific search (e.g., `from:alice subject:meeting`)
- **Search API** — `GET /api/search?q={query}` with results grouped by collection, relevance scoring, and pagination
- **Search bar in App** — top bar search input with live results, results grouped by collection with icons, click to navigate to item

**Deliverables**

- A single search bar searches across all synced data (emails, events, contacts, notes, files)

**Dependencies**

- 2.1

---

### 2.6 — Conflict Resolution Engine

**Scope**

Implement configurable conflict resolution:

- **Per-collection strategies** — configurable per collection: last-write-wins (default), field-level merge (for contacts and events), manual resolution (for notes)
- **Conflict detection** — version comparison on sync, detect when both local and remote have changed since last sync
- **Conflict UI** — notification badge in App status bar, conflict resolution page with side-by-side diff view, options to choose local, choose remote, or manually merge
- **Testing** — comprehensive tests for concurrent edits from multiple devices, offline/online transition scenarios, and rapid successive edits

**Deliverables**

- Multi-device editing works without data loss
- Users can review and resolve conflicts when automatic resolution is insufficient

**Dependencies**

- Phase 1

---

### 2.7 — Pocket ID Integration

**Scope**

Replace local token auth with full OIDC authentication:

- **Pocket ID sidecar** — bundle the Pocket ID Go binary as a second Tauri sidecar (alongside Core)
- **OIDC auth provider** — implement the `AuthProvider` trait for Pocket ID, validate OIDC tokens, replace `local-token` as the default authentication method
- **User flows** — registration, login, logout, session management
- **Passkey/WebAuthn support** — passwordless authentication via platform authenticators (Touch ID, Windows Hello, security keys)
- **App auth flow** — login screen in App, store tokens in OS keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service), auto-refresh before expiry

**Deliverables**

- Full OIDC authentication via Pocket ID with passkey support
- Multi-user foundation established (each user has an identity)

**Dependencies**

- Phase 1

---

### 2.8 — Standalone Core Deployment

**Scope**

Enable Core to run independently of the App:

- **Docker image** — multi-stage Alpine build, final image under 50MB, `docker-compose.yml` with Core + Pocket ID + volumes
- **Installation scripts** — Linux systemd service unit, macOS launchd plist, ARM64 builds for Raspberry Pi
- **Deployment documentation** — guides for Docker deployment, bare metal installation, reverse proxy configuration (nginx/Caddy), TLS setup (Let's Encrypt), firewall rules

**Deliverables**

- Core runs as a standalone server independent of the App
- Docker, bare metal, and Raspberry Pi deployments documented

**Dependencies**

- 2.7

---

### 2.9 — Calendar Plugin (App)

**Scope**

Create `plugins/life/calendar/` as a full calendar App plugin:

- **Views** — month view (grid), week view (time columns), day view (time slots), agenda view (chronological list)
- **Event management** — create, edit, and delete events with bidirectional sync to calendar providers
- **Multi-calendar support** — display events from multiple calendars with colour coding per calendar
- **Design system** — built using the shell design system Web Components for consistent look and feel

**Deliverables**

- Full calendar plugin synced with external providers (CalDAV, Google Calendar)

**Dependencies**

- 2.2, Phase 1 App

---

### 2.10 — Website: API Reference and Guides

**Scope**

Expand the website with auto-generated API documentation and hand-written guides for connectors and deployment:

- **API reference** — Set up OpenAPI → MDX generation pipeline. Install Starlight OpenAPI plugin for interactive endpoint documentation. Generate reference pages from `apps/core/openapi.yaml` covering all endpoints, schemas, and error codes. CI validates that generated docs match the current API surface.
- **Connector authoring guide** — End-to-end guide covering connector concepts, scaffolding, protocol implementation (IMAP, CalDAV, CardDAV walkthrough), canonical mapping, sync strategies, and Docker-based integration testing
- **Deployment guides** — Standalone binary on a home server, Docker Compose with Pocket ID, reverse proxy setup (Caddy, nginx)
- **User guide pages** — Email management, calendar and events, contacts walkthroughs with screenshots
- **Release blog post** — Phase 2 release announcement

**Deliverables**

- API reference auto-generates from the OpenAPI spec and stays in sync via CI
- Developers can follow the connector authoring guide to build a new connector
- Users can follow deployment guides to self-host Core

**Dependencies**

- 1.11, 2.8

**Spec:** [[03 - Projects/Life Engine/Planning/specs/infrastructure/Website]]

---

## Exit Criteria

- Core syncs email, calendar, contacts, and files from multiple providers
- Full-text search works across all data collections
- Conflicts are detected and resolvable through the UI
- OIDC authentication via Pocket ID with passkey support replaces local token auth
- Core deploys as a standalone server via Docker or bare metal installation
- Calendar plugin is functional with month/week/day/agenda views
- App connects to both sidecar Core (desktop) and remote Core (self-hosted)
- API reference auto-generates from OpenAPI spec with CI validation
- Connector authoring and deployment guides published on the website

## Phase-Specific Risks

- **CalDAV/CardDAV protocol complexity** — These protocols have many optional extensions and inconsistent server implementations. Mitigation: start with Radicale (well-documented, standards-compliant), expand provider support incrementally based on user demand.
- **Pocket ID bundling across platforms** — The Go binary must compile and run on Linux, macOS, and Windows (including ARM64). Mitigation: test Go cross-compilation for all target platforms early in development, before depending on it for auth.
- **PowerSync fit** — PowerSync may not integrate cleanly with the existing sync architecture. Mitigation: the `SyncAdapter` abstraction allows swapping sync implementations without changing plugin code; REST polling remains as a reliable fallback.
