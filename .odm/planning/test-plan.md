---
title: "Life Engine — Test Plan"
tags:
  - life-engine
  - planning
  - testing
  - tdd
created: 2026-03-21
---

# Life Engine — Test Plan

Every feature in Life Engine is built test-first, prototyped through Google Stitch where applicable, validated with Playwright, and reviewed before merging. No code ships without passing tests and a completed review gate.

## TDD Methodology

Every feature follows the Red-Green-Refactor cycle:

1. **Red** — Write a failing test that defines the expected behaviour. The test must compile but fail. This forces you to think about the interface before the implementation
2. **Green** — Write the minimum code to make the test pass. No optimisation, no abstraction — just make it work
3. **Refactor** — Clean up the implementation. Apply DRY, extract shared utilities, improve naming. All tests must still pass after refactoring
4. **Review** — Code review against the review checklist. Check test coverage, DRY compliance, and adherence to the spec

This cycle applies to every task. Implementation tasks begin with a test. "Write the code" means "write the test, then write the code."

### TDD in Practice

- **Rust** — Write `#[test]` functions in the same file as the code under test. Run `cargo test` continuously with `cargo-watch`
- **TypeScript** — Write `.test.ts` files colocated with source. Run `npx vitest --watch` during development
- **E2E** — Write Playwright tests before implementing UI features. Use `npx playwright codegen` to scaffold initial selectors, then refine into Page Objects

## DRY Testing Principles

Duplication in tests is as costly as duplication in production code. These rules prevent test rot:

- **Factory functions over fixtures** — Use builder patterns (`create_test_email()`, `create_test_contact()`) instead of copying JSON between tests
- **Shared test utilities** — Common assertions, setup, and teardown live in `packages/test-utils/` (Rust) and `packages/test-utils-js/` (TS)
- **Page Object Model for Playwright** — Every page or component gets a page object class. Tests call `emailList.clickFirstUnread()`, not `page.locator('.email-item:first-child').click()`
- **Single source of truth for test data** — Canonical test fixtures live in `packages/test-fixtures/`. All tests reference these, never hardcode values
- **No test duplication across layers** — Unit tests cover logic, integration tests cover wiring, E2E tests cover user flows. If a unit test validates a transformation, the integration test does not re-test it

## Google Stitch UI Workflow

Every UI component follows this Stitch-to-production pipeline:

1. **Prompt** — Describe the component in Stitch with specific requirements: layout, interactions, accessibility, responsive behaviour, design tokens
2. **Generate** — Export the generated code (HTML/CSS/JS or Lit component)
3. **Adapt** — Integrate into the Life Engine design system. Replace inline styles with CSS custom properties, replace generic elements with `shell-*` components, apply theme tokens
4. **Test** — Write Playwright tests against the adapted component before finalising. Include accessibility audit via `@axe-core/playwright`
5. **Review** — Verify accessibility, responsiveness, and design system compliance against the review checklist

### Where Stitch Is Used

- Shell design system components (`shell-button`, `shell-card`, `shell-modal`, etc.)
- Plugin UI layouts (email list/detail, calendar views, task manager)
- Onboarding wizard screens
- Pipeline canvas visual builder
- Settings and configuration pages

### DRY with Stitch

- Never generate the same component twice — extract to the design system
- Generated code is a starting point, not a final product — always refactor to use shared tokens
- Document which components were Stitch-generated in the component's source file header

## Unit Testing

- **Rust** — Every crate contains `#[cfg(test)]` modules alongside the code they test. Tests are written before implementation per TDD. Run via `cargo test`
- **JS/TS** — Vitest for the plugin SDK and shell components. Tests cover component rendering, API surface validation, and utility functions
- **Coverage target** — 80% for Core crates and shell code. This is a guide, not a gate. Uncovered code must have a documented reason (e.g., platform-specific paths that cannot run in CI)

## Integration Testing

- **Core** — Test API endpoints with a real SQLite database, not mocked. Each test gets a fresh database to avoid state leakage. Tests cover full request/response cycles including auth, validation, and storage
- **App** — Test plugin loading with a mock shell that provides a scoped API. Verify that plugins receive the correct API surface and render correctly in the container
- **Cross-component** — Core-to-App sync tests with both components running locally. Verify that records created through the Core API appear in the App's local SQLite within the sync latency target

## E2E Testing with Playwright CLI

Playwright is the sole E2E testing framework. All user-facing interactions are tested through Playwright against the running Tauri app.

### Setup

- Install: `npx playwright install`
- Config: `playwright.config.ts` at repo root, configured for the Tauri WebView
- Test directory: `tests/e2e/`
- Page objects: `tests/e2e/pages/`

### CLI Commands

- `npx playwright test` — Run all E2E tests headless
- `npx playwright test --ui` — Interactive UI mode for debugging
- `npx playwright codegen` — Record interactions to generate initial test code
- `npx playwright show-report` — View HTML test report after a run
- `npx playwright test --grep "email"` — Run tests matching a pattern
- `npx playwright test --project=desktop` — Run tests for a specific viewport
- `npx playwright test --trace on` — Capture trace for debugging failures

### Page Object Model

Every UI surface has a corresponding page object in `tests/e2e/pages/`:

- `shell.page.ts` — Sidebar navigation, top bar, settings
- `onboarding.page.ts` — First-run wizard flow
- `email-list.page.ts` — Email viewer plugin
- `calendar.page.ts` — Calendar plugin
- `plugin-store.page.ts` — Plugin store browse/install
- `pipeline-canvas.page.ts` — Pipeline visual builder
- `conflict-resolution.page.ts` — Conflict resolution UI
- `search.page.ts` — Search bar and results

Each page object encapsulates selectors and actions:

```typescript
export class EmailListPage {
  constructor(private page: Page) {}

  async goto() {
    await this.page.click('[data-nav="email"]');
  }

  get items() {
    return this.page.locator('[data-testid="email-item"]');
  }

  async clickFirstUnread() {
    await this.page.locator('[data-testid="email-item"][data-unread="true"]').first().click();
  }

  async auditAccessibility() {
    const results = await new AxeBuilder({ page: this.page }).analyze();
    expect(results.violations).toEqual([]);
  }
}
```

### CI Execution

- Runs headless on every PR via GitHub Actions
- Uses `npx playwright test --reporter=html,github` for CI reporting
- Screenshots and traces captured on failure, attached to the test report
- Tests must be deterministic — no dependency on external services
- Parallel test execution across shards for faster CI runs

## Connector Testing

- **Docker test servers** — GreenMail for IMAP/SMTP, Radicale for CalDAV/CardDAV, MinIO for S3. Run as Docker containers in CI
- **TDD approach** — Write connector tests against Docker servers before implementing the connector. Tests define expected sync behaviour, then implementation makes them pass
- **Test coverage** — Auth flow, initial full sync, incremental sync, error handling (server unavailable, auth expired), and rate limiting behaviour
- **Compatibility matrix** — Maintained per provider. Documents which providers have been tested, known quirks, and provider-specific code paths

## Plugin Compatibility Testing

- **Version matrix** — Test plugins built with each SDK version against each shell version within the same major version
- **Backward compatibility** — Verify that plugins built with SDK v1.0 still work on shell v1.x. Breaking changes require a major version bump
- **Shared module loading** — Test that shared modules (Lit, React) load correctly across plugin versions and that multiple plugins using the same shared module do not conflict

## Security Testing

- **Dependency auditing** — `cargo-deny` for Rust, `npm audit` for JS. Both run in CI on every PR. Known vulnerabilities block merging
- **OWASP top 10 checks** — Input validation on all API endpoints, XSS prevention in email rendering (sanitised HTML), SQL injection prevention through parameterised queries
- **Auth testing** — Token expiry enforcement, token revocation, rate limiting on failed authentication attempts, session fixation prevention
- **Encryption verification** — SQLCipher encryption round-trip (encrypt, close, reopen, decrypt), credential isolation between connectors, key derivation parameter validation

## Accessibility Testing

- **Automated** — axe-core audit runs against all shell components in CI. Violations at "critical" or "serious" level block merging
- **Playwright accessibility** — `@axe-core/playwright` runs accessibility audits within E2E tests. Every page object includes an `auditAccessibility()` method
- **Manual screen reader testing** — VoiceOver (macOS) and NVDA (Windows) tested per release. Test script covers navigation, form interaction, and dynamic content updates
- **Keyboard navigation** — All interactive elements audited for focus order, focus visibility, and keyboard operability via Playwright keyboard tests. Tab traps are blocking bugs
- **Colour contrast** — All text and interactive elements meet 4.5:1 minimum contrast ratio

## Performance Testing

- **Benchmarks** — Core startup time, query latency, sync throughput, and plugin load time. Measured using Criterion (Rust) and browser performance APIs (JS)
- **CI integration** — Benchmarks run in CI and results compared against targets in the [Success Criteria](success-criteria.md). Regressions beyond 10% trigger a warning
- **Load testing** — Concurrent API requests, large dataset queries (10,000+ records), and rapid sync cycles

## Review Gate Checklist

Every work package ends with this review. No work package is complete until the review passes.

- [ ] All tests pass (`cargo test`, `npx vitest`, `npx playwright test`)
- [ ] Test coverage meets 80% threshold for changed code
- [ ] No code duplication — DRY audit passed
- [ ] No hardcoded values that should be configurable
- [ ] Error handling covers realistic failure modes
- [ ] API contracts match the spec document
- [ ] UI components use the shell design system (no one-off styles)
- [ ] Stitch-generated code has been adapted to use design tokens
- [ ] Accessibility audit passes (axe-core + keyboard nav)
- [ ] Performance within targets defined in Success Criteria
- [ ] Documentation updated if public API changed
- [ ] **Design Principles compliance** — reviewed against the Design Principles:
  - [ ] Each new module has a single responsibility *(Separation of Concerns)*
  - [ ] Architectural changes have a corresponding ADR *(ADRs)*
  - [ ] Invalid data rejected at boundaries with descriptive errors *(Fail-Fast)*
  - [ ] Security not delegated to a single layer *(Defence in Depth)*
  - [ ] No partially-wired modules left unused *(Finish Before Widening)*
  - [ ] Plugins request only capabilities they use *(Least Privilege)*
  - [ ] Data types validated at boundaries, trusted downstream *(Parse, Don't Validate)*
  - [ ] New features added as plugins, not Core/Shell modifications *(Open/Closed)*
  - [ ] Types and schemas have one definition *(Single Source of Truth)*
  - [ ] Behaviour declared in manifests, not hidden in runtime *(Explicit Over Implicit)*
  - [ ] Easiest plugin path produces correct code *(Pit of Success)*

### Post-Review Improvement

After the review gate, improvements are tracked as follow-up tasks in the current phase:

- **Refactor findings** — DRY violations, overly complex code, missed abstractions
- **Test gaps** — Edge cases discovered during review that lack coverage
- **Performance issues** — Benchmarks that regress or approach limits
- **Accessibility issues** — axe-core warnings or manual testing findings

## Per-Phase Test Requirements

### Phase 0

- CI pipeline validates that all checks (build, lint, test, format) pass on every PR
- ADR format validation script confirms each ADR follows the Context/Decision/Consequences structure
- Playwright config and Page Object Model structure scaffolded
- Test utility packages (`test-utils`, `test-utils-js`, `test-fixtures`) initialised
- Google Stitch workspace configured for design system prototyping

### Phase 1

- TDD cycle followed for every Core module — tests written before implementation
- Core integration tests covering CRUD operations, auth middleware, and SSE event streams
- Connector tests against GreenMail for IMAP sync (test-first)
- App plugin loading tests verifying the 11-step lifecycle
- Sidecar lifecycle tests covering startup, health check, and graceful shutdown
- Playwright E2E: app launch, sidebar navigation, email list display, email detail view, first-run onboarding
- Stitch-generated shell components validated against design system spec
- Review gate passed for every work package (1.1 through 1.11)

### Phase 2

- Schema validation tests written before implementation (test-first)
- Conflict resolution tests simulating concurrent edits from multiple clients
- Pocket ID auth flow tests covering login, token refresh, and passkey authentication
- Docker deployment smoke tests verifying that `docker-compose up` produces a working system
- Playwright E2E: calendar views, conflict resolution UI, search interaction, login flow
- Stitch-generated calendar plugin UI validated with Playwright
- Review gate passed for every work package (2.1 through 2.10)

### Phase 3

- Plugin store lifecycle tests written before implementation (test-first)
- Onboarding flow E2E via Playwright verifying completion within the 5-minute target
- PostgreSQL test suite running the same assertions as the SQLite suite
- CalDAV/CardDAV API compliance tests against RFC 4791 and RFC 6352
- Playwright E2E: plugin store, onboarding wizard, pipeline canvas, all app plugins
- Stitch-generated plugin UIs validated with Playwright
- Review gate passed for every work package (3.1 through 3.8)

### Phase 4

- WASM isolation tests written before implementation (test-first)
- Plugin signing verification tests confirming tampered code is rejected
- Multi-user isolation tests ensuring data boundaries between users
- Mobile build smoke tests on iOS and Android
- Playwright E2E: mobile viewport interactions, bottom navigation, pull-to-refresh
- Review gate passed for every work package (4.1 through 4.8)
