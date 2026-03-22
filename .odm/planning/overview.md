---
title: "Life Engine — Planning Overview"
tags:
  - life-engine
  - planning
  - index
created: 2026-03-21
---

# Life Engine — Planning Overview

Life Engine is a personal data sovereignty platform with two components: **Core** (Rust backend) and **App** (Tauri v2 client). This directory contains all planning documents that guide the project from architecture decisions through to delivery.

## Design Principles

All architecture and implementation decisions are governed by the [[03 - Projects/Life Engine/Design/Principles|Design Principles]]. These are not aspirational — every design document demonstrates how it applies the relevant principles, and every review gate checks for compliance.

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

All implementation follows these core practices. See [[03 - Projects/Life Engine/Planning/Test Plan]] for the full testing methodology.

- **TDD (Test-Driven Development)** — Every feature begins with a failing test. Red-Green-Refactor cycle: write the test, make it pass, then refactor. No implementation without a test first
- **DRY (Don't Repeat Yourself)** — Shared test fixtures, factory functions, page objects, and utility packages prevent duplication across tests and production code. Every work package includes a DRY audit during review
- **Google Stitch** — UI components are prototyped in Stitch, adapted to the shell design system, then validated with automated tests. Stitch output is a starting point — always refactor to use shared tokens and components
- **Playwright CLI** — All user-facing interactions are E2E tested with Playwright. Page Object Model pattern keeps tests maintainable. `npx playwright test` runs on every PR in CI
- **Design Principles Compliance** — Every work package is reviewed against the [[03 - Projects/Life Engine/Design/Principles|Design Principles]]. New modules must demonstrate Separation of Concerns. New data types must follow Parse, Don't Validate. New plugin interfaces must follow The Pit of Success. Architectural changes require an ADR.
- **Review Gates** — Every work package ends with a review: test coverage (80% target), DRY audit, spec compliance, design principles compliance, accessibility check, and performance verification. No work package is complete until the review passes

## Specs

Detailed specifications for each major subsystem.

### Core

- [[03 - Projects/Life Engine/Planning/specs/core/Binary and Startup]]
- [[03 - Projects/Life Engine/Planning/specs/core/REST API]]
- [[03 - Projects/Life Engine/Planning/specs/core/Data Layer]]
- [[03 - Projects/Life Engine/Planning/specs/core/Plugin System]]
- [[03 - Projects/Life Engine/Planning/specs/core/Workflow Engine]]
- [[03 - Projects/Life Engine/Planning/specs/core/Connector Architecture]]
- [[03 - Projects/Life Engine/Planning/specs/core/Auth and Pocket ID]]
- [[03 - Projects/Life Engine/Planning/specs/core/Background Scheduler]]

### App

- [[03 - Projects/Life Engine/Planning/specs/app/Shell Framework]]
- [[03 - Projects/Life Engine/Planning/specs/app/Plugin Loader]]
- [[03 - Projects/Life Engine/Planning/specs/app/Shell Data API]]
- [[03 - Projects/Life Engine/Planning/specs/app/Design System]]
- [[03 - Projects/Life Engine/Planning/specs/app/Capability Enforcement]]
- [[03 - Projects/Life Engine/Planning/specs/app/Sync Layer]]
- [[03 - Projects/Life Engine/Planning/specs/app/Tauri Integration]]

### SDK

- [[03 - Projects/Life Engine/Planning/specs/sdk/Plugin SDK RS]]
- [[03 - Projects/Life Engine/Planning/specs/sdk/Plugin SDK JS]]
- [[03 - Projects/Life Engine/Planning/specs/sdk/Canonical Data Models]]

### Infrastructure

- [[03 - Projects/Life Engine/Planning/specs/infrastructure/Monorepo and Tooling]]
- [[03 - Projects/Life Engine/Planning/specs/infrastructure/CI and CD]]
- [[03 - Projects/Life Engine/Planning/specs/infrastructure/Deployment Modes]]
- [[03 - Projects/Life Engine/Planning/specs/infrastructure/Website]] — Marketing site, documentation, SDK reference, blog, and downloads

### Plugins (Development Targets)

- [[03 - Projects/Life Engine/Planning/specs/plugins/Todo List]] — Full-featured task management (Todoist-class), exercises canonical collections and most plugin capabilities
- [[03 - Projects/Life Engine/Planning/specs/plugins/Expense Tracker]] — Personal finance tracker, exercises private-only collections, charting, and file attachments

## Phases

Phased delivery plan from foundations through to federation and mobile.

- [[03 - Projects/Life Engine/Planning/phases/Phase 0 — Foundation]]
- [[03 - Projects/Life Engine/Planning/phases/Phase 1 — Core and Shell]]
- [[03 - Projects/Life Engine/Planning/phases/Phase 2 — Connectors and Features]]
- [[03 - Projects/Life Engine/Planning/phases/Phase 3 — Ecosystem and Polish]]
- [[03 - Projects/Life Engine/Planning/phases/Phase 4 — WASM and Advanced]]

## Tasks

Granular task breakdowns organised by phase and plugin.

- [[03 - Projects/Life Engine/Planning/tasks/Phase 0 Tasks]]
- [[03 - Projects/Life Engine/Planning/tasks/Phase 1 Tasks]]
- [[03 - Projects/Life Engine/Planning/tasks/Phase 2 Tasks]]
- [[03 - Projects/Life Engine/Planning/tasks/Phase 3 Tasks]]
- [[03 - Projects/Life Engine/Planning/tasks/Phase 4 Tasks]]
- [[03 - Projects/Life Engine/Planning/tasks/Plugin — Todo List Tasks]] — 17 work packages, buildable after Phase 1 shell completes
- [[03 - Projects/Life Engine/Planning/tasks/Plugin — Expense Tracker Tasks]] — 19 work packages, buildable after Phase 1 shell completes
- [[03 - Projects/Life Engine/Planning/tasks/Website Tasks]] — Marketing site, documentation, SDK reference, and release pages across all phases

## Supporting Documents

- [[03 - Projects/Life Engine/Planning/Success Criteria]] — Measurable pass/fail criteria for each phase
- [[03 - Projects/Life Engine/Planning/Test Plan]] — Testing strategy across all layers
- [[03 - Projects/Life Engine/Planning/Risk Register]] — Categorised risks with mitigations

## Related

Design documentation that underpins these plans:

- [[03 - Projects/Life Engine/Design/Principles]] — The 11 governing design principles
- [[03 - Projects/Life Engine/Design/Core/Overview]]
- [[03 - Projects/Life Engine/Design/App/Architecture]]
- [[03 - Projects/Life Engine/Design/Website/Architecture]]
- [[03 - Projects/Life Engine/Design/MonoRepo Tooling/Technical Overview]]
