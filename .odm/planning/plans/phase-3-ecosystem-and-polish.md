---
title: "Phase 3 — Ecosystem and Polish"
tags:
  - life-engine
  - planning
  - phase-3
  - ecosystem
  - plugins
created: 2026-03-21
---

# Phase 3 — Ecosystem and Polish

## Goal

Non-technical users can set up Life Engine through a visual UI. Plugin store enables discovery. The ecosystem begins.

## Entry Criteria

- Phase 2 complete
- Multiple connectors working (email, calendar, contacts, files)
- Full-text search operational
- Pocket ID authentication deployed
- Standalone Core deployment documented

## Table of Contents

- [[#3.1 — Plugin Store Infrastructure]]
- [[#3.2 — Pipeline Canvas (Visual Builder)]]
- [[#3.3 — First-Run Onboarding Flow]]
- [[#3.4 — PostgreSQL Storage Plugin]]
- [[#3.5 — CalDAV/CardDAV API Plugins]]
- [[#3.6 — Webhook Support]]
- [[#3.7 — Additional App Plugins]]
- [[#3.8 — Website: Full Documentation and Plugin Authoring]]
- [[#Exit Criteria]]
- [[#Phase-Specific Risks]]

---

## Work Packages

### 3.1 — Plugin Store Infrastructure

**Scope**

Build the plugin discovery and installation system:

- **Plugin registry** — JSON index file maintained in the monorepo, listing all available plugins with metadata (name, description, version, author, capabilities, category)
- **Plugin submission process** — PR-based workflow where plugin authors submit a PR adding their plugin entry; CI validates the manifest, runs security checks, and verifies the plugin builds
- **Plugin store UI** — in-App page with browse by category, search, detail page (description, screenshots, capabilities, install count), install/uninstall buttons, capabilities approval flow (user must explicitly approve each capability before installation)

**Deliverables**

- Users discover and install plugins from within the app
- Plugin submission has a documented, CI-validated process

**Dependencies**

- Phase 2

---

### 3.2 — Pipeline Canvas (Visual Builder)

**Scope**

Build a visual interface for managing data sources and processing pipelines:

- **Node-based visual editor** — Canvas or SVG-based editor with drag-and-drop
- **Node types** — source nodes (connectors), processor nodes (transforms, filters), output nodes (storage, notifications)
- **Node configuration** — auto-generated forms from plugin configuration schemas, no code required
- **Pipeline status dashboard** — real-time sync status per source, error indicators with details, record counts per collection, last sync timestamp

**Deliverables**

- Non-technical users add connectors and monitor sync status through a visual interface

**Dependencies**

- Phase 2

---

### 3.3 — First-Run Onboarding Flow

**Scope**

Build a guided setup wizard for new users:

- **Wizard steps** — welcome screen, passphrase creation, service connection (list popular providers with OAuth buttons), sync progress (real-time progress bar), explore suggestions (recommend plugins based on connected services), pipeline canvas auto-populated with connected sources
- **Target** — complete setup in under 5 minutes for a user with one email and one calendar
- **Skip option** — technical users can skip the wizard and configure manually

**Deliverables**

- A complete beginner can set up Life Engine without reading documentation

**Dependencies**

- 3.2

---

### 3.4 — PostgreSQL Storage Plugin

**Scope**

Implement an alternative storage backend for power users:

- **StorageAdapter for PostgreSQL** — connection pooling (deadpool-postgres), JSONB storage for document data, full-text search via `tsvector` (integrated with the search system)
- **Migration command** — `life-engine migrate --from sqlite --to postgres` with atomic migration, progress indicator, data verification, and rollback on failure
- **Storage picker** — option in onboarding wizard and settings to choose storage backend

**Deliverables**

- Power users can run Core with PostgreSQL as the storage backend
- Migration between SQLite and PostgreSQL is seamless and safe

**Dependencies**

- Phase 2

---

### 3.5 — CalDAV/CardDAV API Plugins

**Scope**

Expose Core data as standard calendar and contacts servers:

- **CalDAV server plugin** — expose `events` collection as CalDAV calendars, implement PROPFIND, REPORT, GET, PUT, DELETE methods, iCalendar serialisation (VEVENT output)
- **CardDAV server plugin** — expose `contacts` collection as CardDAV address books, vCard serialisation
- **Service discovery** — `.well-known/caldav` and `.well-known/carddav` endpoints, DNS SRV record documentation for custom domains

**Deliverables**

- Native apps (iOS Calendar, iOS Contacts, Thunderbird, GNOME Calendar) connect to Core as a calendar and contacts server

**Dependencies**

- Phase 2

---

### 3.6 — Webhook Support

**Scope**

Enable integration with external services via webhooks:

- **Webhook receiver plugin** — configurable endpoint per webhook source, payload mapping to CDM collections, HMAC-SHA256 signature verification, replay protection
- **Webhook sender plugin** — subscribe to message bus events, send HTTP POST to configured URLs, retry with exponential backoff (max 5 retries), delivery log with status tracking

**Deliverables**

- Core integrates with external services via webhooks in both directions

**Dependencies**

- Phase 2

---

### 3.7 — Additional App Plugins

**Scope**

Build five first-party App plugins to form a complete personal productivity suite:

- **Task Manager** — CRUD for tasks with projects, labels, priorities, and due dates; operates on the `tasks` canonical collection; list and board views
- **Notes** — rich text editing with markdown support; operates on the `notes` canonical collection; folder organisation and tagging
- **Contacts** — contact list with search, group management, detail view with communication history; operates on the `contacts` canonical collection
- **Files** — file browser showing indexed files, preview for common formats, upload to connected storage; operates on the `files` canonical collection
- **Dashboard** — overview widgets: upcoming events, recent emails, pending tasks, sync status, storage usage

**Deliverables**

- Life Engine has 5+ plugins functioning as a full personal productivity suite

**Dependencies**

- Phase 2

---

### 3.8 — Website: Full Documentation and Plugin Authoring

**Scope**

Complete the documentation site with the plugin authoring guide, full user guide, SDK reference, and ecosystem features:

- **Plugin authoring guide** — End-to-end guide covering concepts, scaffold (`plugctl new`), development (`plugctl dev`), App plugin tutorial, Core plugin tutorial, data access, UI development with the design system, testing, capabilities reference, and publishing to the plugin store. Validated by following the guide to build a plugin from scratch.
- **Full SDK reference** — Update auto-generation pipelines to include complete doc comments and usage examples. Rust SDK pages from rustdoc, JS SDK pages from TypeDoc.
- **User guide completion** — Pages for tasks, notes, file browser, pipeline canvas, plugin store, settings, backup and restore. All pages include annotated screenshots.
- **Release note automation** — `scripts/generate-changelog.ts` converts git tags and GitHub Release bodies to blog posts. Automated release note generation for each tagged release.
- **PR preview deployments** — Configure Cloudflare Pages previews or CI artifact for PRs that change website content
- **Analytics** — Evaluate and configure privacy-respecting analytics (Plausible or Fathom). Page views and download clicks only, no personal data.
- **Community sections** — Contributor spotlight and plugin showcase sections on the Community page
- **Release blog post** — Phase 3 release announcement

**Deliverables**

- Plugin authors can follow the authoring guide from zero to published plugin
- All user-facing features are documented with screenshots
- SDK reference is comprehensive and auto-generated from source
- Release notes are automated

**Dependencies**

- 2.10, 3.1, 3.7

**Spec:** [[03 - Projects/Life Engine/Planning/specs/infrastructure/Website]]

---

## Exit Criteria

- Plugin store works end-to-end (browse, search, view details, install, manage, uninstall)
- Pipeline canvas shows connected sources with real-time sync status
- First-run onboarding completes in under 5 minutes for a non-technical user
- PostgreSQL storage backend supported with migration tooling
- CalDAV and CardDAV APIs expose data to native calendar and contacts apps
- Webhooks work in both directions (receive and send)
- At least 5 first-party App plugins available (email, calendar, tasks, notes, contacts, files, dashboard)
- Plugin authoring guide is complete and validated (tested by building a plugin from scratch)
- Full SDK reference auto-generates from source with CI validation
- All user guide pages published with screenshots
- Release note automation works end-to-end from git tag to blog post

## Phase-Specific Risks

- **Plugin store abuse** — Malicious or low-quality plugins could harm users or the ecosystem reputation. Mitigation: PR-based submission with CI validation catches obvious issues, introduce review tiers (unreviewed, community-reviewed, official) to set user expectations.
- **Visual builder complexity** — A full drag-and-drop pipeline editor is a significant UI undertaking. Mitigation: start with a read-only status view showing connected sources and their sync state, then add drag-and-drop configuration incrementally.
- **CalDAV/CardDAV specification complexity** — These protocols are large and clients implement them inconsistently. Mitigation: test against specific popular clients (iOS Calendar, Thunderbird) rather than aiming for full specification compliance initially.
