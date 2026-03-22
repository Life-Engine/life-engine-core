---
title: "Life Engine — Success Criteria"
tags:
  - life-engine
  - planning
  - success-criteria
created: 2026-03-21
---

# Life Engine — Success Criteria

Each phase has measurable criteria that must be met before moving to the next. Every criterion includes a concrete verification method so that pass/fail is objective, not subjective.

## Phase 0 — Foundations

- **Monorepo builds on CI for all platforms** — Verification: CI pipeline reports green on macOS, Linux, and Windows runners.
- **All 13 ADRs published in .odm/docs/adrs/** — Verification: file count matches 13, each file follows the Context/Decision/Consequences format. ADR-013 covers the adoption of the 11 governing Design Principles.
- **CDM JSON Schema files validate against test fixtures** — Verification: JSON Schema validation passes for all 7 canonical collections.
- **Both plugin SDKs have type definitions** — Verification: `plugin-sdk-rs` compiles without errors, `plugin-sdk-js` passes type-checking.
- **Dev environment works in under 5 minutes** — Verification: timed test from clean clone to running dev server completes within 5 minutes.
- **Community server exists with governance document** — Verification: Discord server is accessible and `governance.md` is published.
- **Project website is live** — Verification: homepage loads at the custom domain in under 2 seconds, marketing pages (About, Community, Pricing) render correctly, docs skeleton has sidebar navigation with placeholder sections, blog has at least one published post, Pagefind search returns results, Lighthouse performance score above 90 and accessibility score above 95.

## Phase 1 — Core MVP

- **Core binary starts and responds to /api/system/health** — Verification: `curl` returns HTTP 200 with version and plugin list in the response body.
- **SQLite storage passes CRUD integration tests** — Verification: `cargo test` for the storage module passes all assertions.
- **Auth middleware rejects unauthenticated requests** — Verification: requests without a token receive 401, requests with a valid token receive 200.
- **Email connector syncs from IMAP test server** — Verification: GreenMail Docker container provides test emails, sync produces canonical email records.
- **App shell launches and renders plugin container** — Verification: Tauri app opens and shows shell UI within 2 seconds.
- **Plugin loader completes 11-step lifecycle** — Verification: test plugin loads, receives scoped API, and renders in the plugin container.
- **Data syncs from Core to App local SQLite** — Verification: a record created via the Core API appears in the App within 5 seconds.
- **Sidecar mode works end-to-end** — Verification: single app install, Core starts as a subprocess, email viewer shows synced emails.
- **Documentation covers Getting Started and Architecture** — Verification: all Getting Started pages (install per platform, first-run, connect service, install plugin) are published. Architecture overview and Core/App/Data/Security pages are published. SDK reference pages auto-generate from source.
- **Downloads page serves platform artifacts** — Verification: downloads page detects visitor OS, lists correct artifacts from GitHub Releases, displays SHA-256 checksums.

## Phase 2 — Data Platform

- **Schema registry validates and quarantines invalid data** — Verification: a malformed record is rejected with a descriptive error and stored in `_quarantine`.
- **Calendar and contacts connectors sync bidirectionally** — Verification: Radicale Docker container provides CalDAV/CardDAV, sync round-trip succeeds.
- **Full-text search returns relevant results** — Verification: tantivy index query for known terms returns the correct records.
- **Conflict resolution handles concurrent edits** — Verification: simultaneous edits from two clients are resolved per the configured strategy.
- **Pocket ID OIDC flow completes** — Verification: login, token issuance, token refresh, and passkey auth all pass.
- **Docker deployment works** — Verification: `docker-compose up` serves Core and Pocket ID, and the App connects remotely.
- **API reference auto-generates from OpenAPI spec** — Verification: `apps/core/openapi.yaml` produces rendered API docs with all endpoints, schemas, and error codes. CI fails if generated docs are stale.
- **Connector and deployment documentation published** — Verification: connector authoring guide covers concepts, quick start, protocol examples, sync strategies, and testing. Deployment guides cover standalone binary, Docker Compose, and reverse proxy setup.

## Phase 3 — Ecosystem

- **Plugin store lists and installs plugins** — Verification: the browse, search, install, and uninstall cycle completes without errors.
- **Pipeline canvas shows connected sources** — Verification: add a connector node, configure it, and observe sync status.
- **First-run onboarding completes in under 5 minutes** — Verification: timed test from app launch to first synced data.
- **PostgreSQL storage passes same test suite as SQLite** — Verification: `cargo test` with the PG feature flag passes all assertions.
- **CalDAV/CardDAV API plugins serve data to native apps** — Verification: iOS Calendar and Contacts connect and display data.
- **At least 7 first-party App plugins available** — Verification: email, calendar, tasks, notes, contacts, files, and dashboard plugins are functional.
- **Plugin authoring guide complete end-to-end** — Verification: a new developer can follow the guide from scaffold to published plugin without external help. Tested by following the guide from scratch.
- **Full SDK reference auto-generates from source** — Verification: Rust SDK pages generate from rustdoc, JS SDK pages generate from TypeDoc, both include all public types with doc comments and examples. CI fails if stale.
- **All user guide pages published with screenshots** — Verification: every first-party plugin feature has a corresponding user guide page with annotated screenshots.

## Phase 4 — Advanced

- **WASM plugins run in isolated sandboxes** — Verification: a plugin cannot access undeclared collections or domains.
- **Plugin signing prevents tampered code** — Verification: a modified `.wasm` file is rejected on load.
- **Multi-user data isolation enforced** — Verification: user A cannot read user B's private collections.
- **GraphQL API serves all canonical collections** — Verification: GraphQL Playground queries return correct data for all collections.
- **Two Core instances federate and sync** — Verification: a record created on instance A appears on instance B.
- **Mobile apps launch on iOS and Android** — Verification: Tauri mobile builds install and display the shell UI.
- **Advanced documentation published** — Verification: guides for WASM plugins, plugin signing, multi-user, federation, encrypted backups, GraphQL, and mobile deployment are published and reviewed.
- **i18n framework in place** — Verification: Starlight i18n is configured, translation contribution guide published, at least one non-English locale is scaffolded.

## Methodology Gates

These gates apply to every phase and every work package. A work package cannot be considered complete until all applicable gates pass.

- **TDD compliance** — Every implementation task has a corresponding test written before the implementation code. Verification: git history shows test commits preceding or within the same PR as implementation commits
- **Test coverage** — 80% line coverage for all changed code. Verification: CI coverage report confirms threshold on every PR
- **DRY audit** — No duplicated logic across modules or test files. Shared utilities extracted to `packages/test-utils/` or `packages/test-utils-js/`. Verification: code review checklist confirmed by reviewer
- **Playwright E2E pass** — All Playwright tests pass headless in CI for every PR that touches UI code. Verification: `npx playwright test` exits 0
- **Stitch-to-code validation** — Every Stitch-generated UI component has been adapted to use shell design system tokens and passes accessibility audit. Verification: no inline styles or hard-coded colours in UI components
- **Review gate complete** — Every work package has a completed review checklist before the next work package begins. Verification: review checklist checked off in the PR description
- **Page Object coverage** — Every new UI surface has a corresponding Playwright page object in `tests/e2e/pages/`. Verification: page object file exists and is used by at least one test

## Design Principles Compliance Gates

These gates enforce the [[03 - Projects/Life Engine/Design/Principles|Design Principles]] on every work package. Each gate includes a verification method.

- **Separation of Concerns** — Each new module or component has a single, stated responsibility. No module mixes orchestration with business logic. Verification: code review confirms each module's responsibility can be stated in one sentence
- **ADR required for architectural changes** — Any change that introduces a new dependency, replaces a core component, or changes a plugin contract has a corresponding ADR in `.odm/docs/adrs/`. Verification: ADR file exists and follows Context/Decision/Consequences/Alternatives format
- **Fail-Fast validation** — Invalid data is rejected at the boundary before processing or storage. No silent data corruption paths exist. Verification: unit tests confirm that malformed input produces immediate, descriptive errors
- **Defence in Depth** — Security is not delegated to a single layer. Auth, transport, storage, and plugin isolation each enforce their own boundaries. Verification: security review confirms each layer is independently testable and enforceable
- **Finish Before Widening** — Phase exit criteria pass before work on the next phase begins. No partially-wired modules are left unused. Verification: exit criteria checklist in the PR description for the final work package of each phase
- **Least Privilege** — New plugins and components request only the capabilities they use. No blanket permissions. Verification: manifest review confirms each declared capability maps to a concrete code path
- **Parse, Don't Validate** — New data types entering the system are validated against a schema at the boundary and consumed as typed values downstream. No defensive re-validation in inner code. Verification: code review confirms downstream code trusts the types rather than re-checking
- **Open/Closed** — New features are added as plugins or trait implementations, not by modifying Core or Shell internals. Verification: diff shows no changes to Core binary or Shell framework code when adding a new plugin or connector
- **Single Source of Truth** — Types, schemas, and capability declarations exist in exactly one location. No duplicated definitions. Verification: grep confirms each type/schema has one definition
- **Explicit Over Implicit** — Plugin behaviour is declared in manifests, not discovered at runtime. Verification: code review confirms no hidden side effects or undeclared capabilities
- **Pit of Success** — The easiest path for a plugin author produces correct, safe, minimal-capability code. Verification: the scaffolding template and SDK documentation are tested by following them to build a plugin from scratch

## Performance Targets

These targets apply across all phases once the relevant component exists.

- **Core idle** — Less than 50 MB RAM, less than 1% CPU.
- **Core sync** — Process 1,000 emails per minute.
- **App startup** — Under 2 seconds to interactive.
- **Plugin load** — Under 500ms from import to `connectedCallback`.
- **Data query** — Under 50ms for typical queries (under 1,000 records).
- **Sync latency** — Under 5 seconds from Core write to App UI update.
- **Verification** — Benchmarks run in CI with results compared against these targets.

## Accessibility Gates

Every release must meet these accessibility standards.

- **WCAG 2.1 AA compliance** for all shell components.
- **Keyboard navigation** with visible focus indicators on every interactive element.
- **Screen reader compatibility** with ARIA labels and live regions.
- **Colour contrast minimum 4.5:1** for all text and interactive elements.
- **Responsive text sizing** using rem units throughout.
- **Verification** — axe-core automated audit runs in CI. Manual screen reader testing (VoiceOver, NVDA) performed per release.
