---
title: "Life Engine — Planning Overview"
tags:
  - life-engine
  - planning
  - index
created: 2026-03-21
updated: 2026-03-23
---

# Life Engine — Planning Overview

Life Engine is a personal data sovereignty platform. Core is the self-hosted Rust backend — a thin orchestrator that wires together independent modules. All features are provided by WASM plugins, data flows through declarative workflows, and clients connect via configurable transports. A Tauri v2 App client is planned as the user-facing interface.

This directory contains all planning documents that guide the project from architecture decisions through to delivery.

## Design Principles

All architecture and implementation decisions are governed by the Design Principles. These are not aspirational — every design document demonstrates how it applies the relevant principles, and every review gate checks for compliance.

The 11 principles:

- **Separation of Concerns** — One responsibility per module, layer, and component
- **Architecture Decision Records** — Document the *why* behind key decisions
- **Fail-Fast with Defined States** — Make invalid states unrepresentable, surface errors immediately
- **Defence in Depth** — Every layer independently secure
- **Finish Before Widening** — Fully integrated system before expanding scope
- **Principle of Least Privilege** — Access only what is explicitly declared, enforced by the runtime
- **Parse, Don't Validate** — Type system prevents invalid data at boundaries
- **Open/Closed Principle** — Open for extension via plugins, closed to modification
- **Single Source of Truth** — One canonical definition, consumed everywhere
- **Explicit Over Implicit** — Declare behaviour in manifests, not in runtime logic
- **The Pit of Success** — The easiest path for plugin authors is the correct path

## Methodology

All implementation follows these core practices. See the [Test Plan](test-plan.md) for the full testing methodology.

- **TDD (Test-Driven Development)** — Every feature begins with a failing test. Red-Green-Refactor cycle: write the test, make it pass, then refactor. No implementation without a test first
- **DRY (Don't Repeat Yourself)** — Shared test fixtures, factory functions, and utility packages prevent duplication across tests and production code. Every work package includes a DRY audit during review
- **Google Stitch** — UI components are prototyped in Stitch, adapted to the design system, then validated with automated tests. Stitch output is a starting point — always refactor to use shared tokens and components
- **Playwright** — All user-facing interactions are E2E tested with Playwright. Page Object Model pattern keeps tests maintainable
- **Design Principles Compliance** — Every work package is reviewed against the Design Principles. New modules must demonstrate Separation of Concerns. New data types must follow Parse, Don't Validate. New plugin interfaces must follow The Pit of Success. Architectural changes require an ADR
- **Review Gates** — Every work package ends with a review: test coverage (80% target), DRY audit, spec compliance, design principles compliance, accessibility check, and performance verification. No work package is complete until the review passes

## Design Documents

Architecture documentation for each Core subsystem in `.odm/doc/Design/Core/`:

- [Core Overview](../doc/Design/Core/Overview.md) — Four-layer architecture, tech stack, startup flow, deployment modes
- [Transport Layer](../doc/Design/Core/Transports.md) — Protocol-specific entry points (REST, GraphQL, CalDAV, CardDAV, Webhook)
- [Data Layer](../doc/Design/Core/Data.md) — StorageBackend trait, document model, canonical/private collections, encryption
- [Plugin System](../doc/Design/Core/Plugins.md) — WASM isolation via Extism, plugin lifecycle, capabilities, SDK contract
- [Workflow Engine](../doc/Design/Core/Workflow.md) — Declarative YAML pipelines, triggers, event bus, cron scheduler
- [Schema Versioning Rules](../doc/Design/Core/Schema%20Versioning%20Rules.md) — Versioning policy for canonical schemas

## Specs (Planned)

Detailed specifications to be written for each major subsystem.

### Core

- Binary and Startup
- Transport Layer
- Data Layer
- Plugin System
- Workflow Engine
- Auth and Pocket ID

### App

- Shell Framework
- Plugin Loader
- Shell Data API
- Design System
- Capability Enforcement
- Sync Layer
- Tauri Integration

### SDK

- Plugin SDK (Rust)
- Plugin SDK (JS)
- Canonical Data Models

### Infrastructure

- Monorepo and Tooling
- CI and CD
- Deployment Modes
- Website — Marketing site, documentation, SDK reference, blog, and downloads

### Plugins (Development Targets)

- **Todo List** — Full-featured task management (Todoist-class), exercises canonical collections and most plugin capabilities
- **Expense Tracker** — Personal finance tracker, exercises private-only collections, charting, and file attachments

## Phases

Phased delivery plan from foundations through to federation and mobile.

- Phase 0 — Foundation
- Phase 1 — Core and Shell
- Phase 2 — Data Platform
- Phase 3 — Ecosystem and Polish
- Phase 4 — WASM and Advanced

## Tasks

Granular task breakdowns organised by phase and plugin.

- Phase 0 Tasks
- Phase 1 Tasks
- Phase 2 Tasks
- Phase 3 Tasks
- Phase 4 Tasks
- Plugin — Todo List Tasks — 17 work packages, buildable after Phase 1 shell completes
- Plugin — Expense Tracker Tasks — 19 work packages, buildable after Phase 1 shell completes
- Website Tasks — Marketing site, documentation, SDK reference, and release pages across all phases

## Supporting Documents

- [Success Criteria](success-criteria.md) — Measurable pass/fail criteria for each phase
- [Test Plan](test-plan.md) — Testing strategy across all layers
- [Risk Register](risk-register.md) — Categorised risks with mitigations

## Related

- ADRs — Architecture Decision Records in `.odm/doc/adrs/`
- [Core Overview](../doc/Design/Core/Overview.md) — Primary architecture reference
