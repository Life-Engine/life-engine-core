<!--
qa-name: full-project
updated: 2026-03-22
qa-report: ./report.md
-->

# QA Fix Plan — Full Project

## Task Overview

The full-project QA inspection of 262 source files produced 91 findings: 2 critical, 19 high, 44 medium, 20 low, and 6 info. The most impactful issues cluster around three systemic patterns:

1. **Security: innerHTML XSS** — Nearly all web components interpolate attributes into innerHTML without escaping. A shared `escapeHtml` utility exists but is inconsistently used.
2. **Security: credential storage** — Passphrase hashing uses a trivially reversible algorithm. Auth tokens, database credentials, and connector passwords are stored in plaintext localStorage.
3. **Performance: fetch-all-then-filter** — Multiple services (budget, task, currency, search-filter) fetch the entire collection from the API and filter client-side, creating O(N) or worse per operation.

Recommended execution order: critical fixes first (security vulnerabilities), then high-priority bugs and security issues, then medium improvements.

**Progress:** 0 / 40 tasks complete

## Severity Guide

- **Critical** — Fix immediately. Security vulnerabilities, data loss, production crashes.
- **High** — Fix before next release. Bugs in normal usage paths, missing auth checks.
- **Medium** — Fix within current sprint. Maintainability issues, test gaps.
- **Low** — Fix when touching the file. Style, naming, minor docs.

---

## Tasks

## 1.1 — Critical Fixes
> qa-report: ./report.md

- [ ] Replace simpleHash with PBKDF2 for passphrase storage [BLOCKER]
  <!-- file: apps/app/src/lib/crypto.js -->
  <!-- purpose: Replace DJB2-variant hash with crypto.subtle PBKDF2 + random salt. Update login.js and onboarding.js to use the new async hash function. Store salt alongside hash in localStorage. -->
  <!-- findings: F-001 -->
  <!-- severity: critical -->
  <!-- impact: Current 32-bit hash is trivially brute-forceable; any localStorage access exposes passphrase -->

- [ ] Remove allow-same-origin from email iframe sandbox [BLOCKER]
  <!-- file: plugins/life/email-viewer/index.js -->
  <!-- purpose: Change sandbox="allow-same-origin" to sandbox="" on the email body iframe. If styling breaks, sanitize HTML with DOMPurify before rendering instead. -->
  <!-- findings: F-002 -->
  <!-- severity: critical -->
  <!-- impact: Malicious emails can currently access parent DOM, localStorage, and auth tokens -->

## 1.2 — High Priority Fixes
> depends: 1.1
> qa-report: ./report.md

- [ ] Add shared escapeHtml utility and fix high-risk XSS in shell components
  <!-- file: apps/app/src/components/shell-input.js, shell-textarea.js, shell-select.js, shell-avatar.js -->
  <!-- purpose: Create a shared escapeHtml function (string-replace based). Apply it to error, value, placeholder, src, name, label, and option interpolations in these 4 high-risk components. -->
  <!-- findings: F-003 -->
  <!-- severity: high -->
  <!-- impact: User-influenced attribute values can execute arbitrary JavaScript via innerHTML -->

- [ ] Secure token storage: warn on localStorage fallback
  <!-- file: apps/app/src/lib/secure-token.js -->
  <!-- purpose: Log a visible warning when falling back to localStorage. Consider sessionStorage for browser builds. Ensure production builds always use Tauri keychain. -->
  <!-- findings: F-004 -->
  <!-- severity: high -->
  <!-- impact: XSS vulnerability would expose all auth tokens in browser fallback mode -->

- [ ] Fix URL path injection in conflict-resolver and sync-adapter
  <!-- file: apps/app/src/lib/conflict-resolver.js, apps/app/src/lib/sync-adapter.js -->
  <!-- purpose: Apply encodeURIComponent(id) to all URL-interpolated IDs in both files -->
  <!-- findings: F-005, F-006 -->
  <!-- severity: high -->
  <!-- impact: Malformed record IDs could manipulate API request URLs -->

- [ ] Fix sync-manager flush to dequeue by mutation identity
  <!-- file: apps/app/src/lib/sync-manager.js -->
  <!-- purpose: Track which specific mutations succeeded by ID and remove only those from the queue, instead of dequeuing the first N by count -->
  <!-- findings: F-007 -->
  <!-- severity: high -->
  <!-- impact: Partial sync failures can cause the wrong mutations to be dequeued, leading to data loss -->

- [ ] Fix token-manager to distinguish network errors from auth failures
  <!-- file: apps/app/src/lib/token-manager.js -->
  <!-- purpose: Only emit auth-expired for HTTP 401/403 responses. Allow retry/backoff for network errors. -->
  <!-- findings: F-008 -->
  <!-- severity: high -->
  <!-- impact: Transient network issues cause premature logouts -->

- [ ] Add confirmation dialog before app reset
  <!-- file: apps/app/src/shell/login.js -->
  <!-- purpose: Add a modal requiring passphrase entry before clearing all localStorage data -->
  <!-- findings: F-009 -->
  <!-- severity: high -->
  <!-- impact: Accidental click permanently deletes all local user data -->

- [ ] Clear passphrase from memory after onboarding completes
  <!-- file: apps/app/src/shell/onboarding.js -->
  <!-- purpose: Set this.#passphrase = '' and this.#passphraseConfirm = '' immediately after step 3 completes -->
  <!-- findings: F-010 -->
  <!-- severity: high -->
  <!-- impact: Raw passphrase persists in JS memory for the entire session -->

- [ ] Add file validation and path sanitization to receipt service
  <!-- file: plugins/life/expenses/src/services/receipt-service.js -->
  <!-- purpose: Validate file size, sanitize filename (strip directory separators, use UUID), verify MIME against allowlist -->
  <!-- findings: F-011 -->
  <!-- severity: high -->
  <!-- impact: Unsanitized filenames enable path traversal; no size limit allows DoS -->

- [ ] Fix expenses search argument mismatch and race condition
  <!-- file: plugins/life/expenses/src/index.js -->
  <!-- purpose: Fix search call to pass correct criteria object. Await loadData on clear. Store filtered results separately from the full transaction list. -->
  <!-- findings: F-012, F-013 -->
  <!-- severity: high -->
  <!-- impact: Search is broken (wrong arguments) and clearing search has a race condition -->

- [ ] Optimize budget status computation to avoid repeated full-collection fetches
  <!-- file: plugins/life/expenses/src/services/budget-service.js, plugins/life/expenses/src/index.js -->
  <!-- purpose: Fetch transactions once, pass to pure computation functions. Use Promise.all for parallel budget processing. -->
  <!-- findings: F-014 -->
  <!-- severity: high -->
  <!-- impact: O(N*B*P) database queries on every data load; freezes UI with many budgets -->

- [ ] Add iteration cap to scheduling-engine getMissedDueDates
  <!-- file: plugins/life/expenses/src/services/scheduling-engine.js -->
  <!-- purpose: Add max 365 iteration limit to the while loop in getMissedDueDates. Log warning when cap is hit. -->
  <!-- findings: F-015 -->
  <!-- severity: high -->
  <!-- impact: Stale recurring rules with old next_due dates can freeze the UI in an infinite loop -->

- [ ] Optimize balance trend to single-pass computation
  <!-- file: plugins/life/expenses/src/components/account-overview.js -->
  <!-- purpose: Replace O(30*N) loop with single-pass date-ordered iteration that records daily snapshots -->
  <!-- findings: F-016 -->
  <!-- severity: high -->
  <!-- impact: Renders become very slow with large transaction histories -->

- [ ] Replace fragile markdown renderer with a library
  <!-- file: plugins/life/notes/index.js -->
  <!-- purpose: Replace the escape-then-regex-unescape pattern with a proper markdown library (marked/markdown-it) + DOMPurify sanitizer -->
  <!-- findings: F-017 -->
  <!-- severity: high -->
  <!-- impact: The fragile escape/unescape pattern is one regex change away from XSS -->

- [ ] Fix vanilla plugin template shadowRoot crash and XSS
  <!-- file: tools/templates/life-plugin-vanilla/src/index.js -->
  <!-- purpose: Change mode to 'open' or store attachShadow return. Apply escapeHtml to interpolated plugin ID. -->
  <!-- findings: F-018, F-019 -->
  <!-- severity: high -->
  <!-- impact: Template crashes on use (closed shadowRoot returns null) and has innerHTML XSS -->

- [ ] Optimize todo task-service to avoid fetch-all-then-filter
  <!-- file: plugins/life/todos/src/services/task-service.js -->
  <!-- purpose: Pass specific query filters to the API for getByProject, getBySection, getSubtasks. Batch reorder updates. -->
  <!-- findings: F-020 -->
  <!-- severity: high -->
  <!-- impact: Every task operation fetches the entire collection; reorder makes N sequential calls -->

- [ ] Replace DOM-dependent escapeHtml with pure string implementation
  <!-- file: plugins/life/_shared/escape.js -->
  <!-- purpose: Replace document.createElement-based escapeHtml with string .replace() chains matching escapeAttr pattern. Add null/undefined handling. -->
  <!-- findings: F-021 -->
  <!-- severity: high -->
  <!-- impact: Crashes in any non-browser context (SSR, workers, Node) -->

## 1.3 — Medium Priority Fixes
> depends: 1.1
> qa-report: ./report.md

- [ ] Apply escapeHtml to remaining shell components (medium-risk XSS)
  <!-- file: apps/app/src/components/shell-checkbox.js, shell-empty-state.js, shell-error-state.js, shell-modal.js, shell-sheet.js -->
  <!-- purpose: Apply the shared escapeHtml utility to all attribute interpolations in these 7 components -->
  <!-- findings: F-022 -->
  <!-- severity: medium -->
  <!-- impact: Reduces XSS surface area across all shell components -->

- [ ] Fix shared focusVisible/disabledState CSS selector composition
  <!-- file: apps/app/src/styles/shared-styles.js -->
  <!-- purpose: Change exports to declaration-only (without selectors) or provide a focusVisibleFor(selector) function. Update all consuming components. -->
  <!-- findings: F-023 -->
  <!-- severity: medium -->
  <!-- impact: Focus-visible and disabled styles silently broken in 8+ components -->

- [ ] Add focus trapping to shell-modal and shell-sheet
  <!-- file: apps/app/src/components/shell-modal.js, apps/app/src/components/shell-sheet.js -->
  <!-- purpose: Implement Tab/Shift+Tab focus trapping. Add focus management to shell-sheet (save/restore focus, auto-focus on open). -->
  <!-- findings: F-024 -->
  <!-- severity: medium -->
  <!-- impact: Accessibility violation — Tab escapes modal/sheet despite aria-modal="true" -->

- [ ] Fix shell-checkbox/toggle double-render and shell-input value setter
  <!-- file: apps/app/src/components/shell-checkbox.js, shell-toggle.js, shell-input.js -->
  <!-- purpose: Guard attribute setters against redundant updates. Update input value directly without full re-render. -->
  <!-- findings: F-025, F-026 -->
  <!-- severity: medium -->
  <!-- impact: Every toggle causes double DOM teardown; programmatic value set loses focus -->

- [ ] Fix main.js bugs (null crash, duplicate listener, missing await, dropped tokenManager)
  <!-- file: apps/app/src/main.js -->
  <!-- purpose: Add null guard in clearOverlayPages. Remove duplicate beforeunload. Await notificationManager.init(). Pass tokenManager to sync restart. -->
  <!-- findings: F-027, F-028, F-029 -->
  <!-- severity: medium -->
  <!-- impact: Route changes can crash; sync loses token refresh after pull-to-refresh -->

- [ ] Secure plugin-loader, webauthn, and plugin-storage
  <!-- file: apps/app/src/lib/plugin-loader.js, webauthn.js, plugin-storage.js -->
  <!-- purpose: Validate entry URL against allowedDomains. Validate coreBaseUrl origin. Wrap JSON.parse in try/catch. Validate expires_in in token-manager. -->
  <!-- findings: F-030, F-031, F-032, F-033, F-034 -->
  <!-- severity: medium -->
  <!-- impact: Dynamic imports from untrusted URLs; NaN prevents token refresh; JSON parse can crash plugins -->

- [ ] Fix listener leaks in bottom-nav, sidebar, and receipt-viewer
  <!-- file: apps/app/src/shell/bottom-nav.js, sidebar.js, plugins/life/expenses/src/components/receipt-viewer.js -->
  <!-- purpose: Store unsubscribe handles in connectedCallback. Add disconnectedCallback to clean up router and keydown listeners. -->
  <!-- findings: F-035, F-045 -->
  <!-- severity: medium -->
  <!-- impact: Listeners accumulate on connect/disconnect cycles; receipt-viewer leaks on every open -->

- [ ] Fix shell bugs: household error state, session bypass, plugin-store install
  <!-- file: apps/app/src/shell/household.js, login.js, plugin-store.js -->
  <!-- purpose: Handle HTTP error in household. Use signed session instead of sessionStorage boolean. Persist installed plugin IDs and implement real installation. -->
  <!-- findings: F-036, F-037, F-042 -->
  <!-- severity: medium -->
  <!-- impact: Server errors show wrong UI; login screen trivially bypassed; plugins not actually installed -->

- [ ] Secure onboarding and pipeline-canvas
  <!-- file: apps/app/src/shell/onboarding.js, pipeline-canvas.js, plugin-container.js -->
  <!-- purpose: Escape error messages in onboarding innerHTML. Use storeToken for credentials. Add auth headers to pipeline API calls. Escape pluginId in plugin-container. -->
  <!-- findings: F-038, F-039, F-040, F-041 -->
  <!-- severity: medium -->
  <!-- impact: Credentials in plaintext localStorage; API calls unauthenticated; innerHTML XSS in error paths -->

- [ ] Fix expenses plugin bugs (keyboard scope, delete confirm, null safety)
  <!-- file: plugins/life/expenses/src/index.js, services/account-service.js, services/budget-engine.js, services/collection-helpers.js, services/recurring-service.js -->
  <!-- purpose: Attach keyboard shortcuts to component not document. Add delete confirmation. Fix initial_balance null. Add loop guards. Fix formatCurrency null. Fix Object.assign mutation. -->
  <!-- findings: F-043, F-044, F-046, F-047, F-048, F-055 -->
  <!-- severity: medium -->
  <!-- impact: Global keyboard shortcuts conflict with typing; single keypress deletes transactions; NaN balances -->

- [ ] Fix plugin CSS @import, weak IDs, and crud-scaffold error swallowing
  <!-- file: plugins/life/contacts/index.js, notes/index.js, files/index.js, task-manager/index.js, _shared/crud-scaffold.js -->
  <!-- purpose: Replace @import with <link> or inline styles. Use crypto.randomUUID() for IDs. Log errors in crud-scaffold. -->
  <!-- findings: F-049, F-050, F-051 -->
  <!-- severity: medium -->
  <!-- impact: Plugins may render unstyled; ID collisions after ~50K records; silent CRUD failures -->

- [ ] Optimize expenses fetch-all patterns and rendering
  <!-- file: plugins/life/expenses/src/services/search-filter-service.js, currency-service.js, components/expenses-sidebar.js, email-viewer/index.js -->
  <!-- purpose: Push filters to query layer in search-filter and currency services. Single-pass net worth. Use event delegation in email-viewer. -->
  <!-- findings: F-052, F-053, F-054 -->
  <!-- severity: medium -->
  <!-- impact: Unnecessary full-collection fetches on every search/render; O(M*N) sidebar computation -->

- [ ] Deduplicate spending-by-category and doughnut chart implementations
  <!-- file: plugins/life/expenses/src/components/expenses-dashboard.js, reports-view.js -->
  <!-- purpose: Remove duplicate aggregation and chart rendering. Use shared chart-helpers.js functions. -->
  <!-- findings: F-059 -->
  <!-- severity: medium -->
  <!-- impact: Three copies of the same logic create maintenance burden and divergence risk -->

- [ ] Fix test infrastructure: fetch mock leak, broken page objects, brittle waits
  <!-- file: apps/app/tests/lib/conflict-resolver.test.js, search-client.test.js, sync-adapter.test.js, tests/e2e/sidecar-e2e-flow.spec.ts, sidecar-integration.spec.ts, expenses-keyboard.spec.ts -->
  <!-- purpose: Use vi.spyOn(globalThis, 'fetch') instead of direct assignment. Fix sidecar tests to use existing OnboardingPage methods. Replace waitForTimeout with condition-based waits. -->
  <!-- findings: F-056, F-057, F-058 -->
  <!-- severity: medium -->
  <!-- impact: Fetch mock leaks between suites; sidecar tests fail at runtime; expenses tests are flaky -->

- [ ] Fix web build script error handling and RSS feed
  <!-- file: apps/web/scripts/fetch-releases.mjs, generate-api-docs.mjs, generate-sdk-docs.mjs, src/pages/rss.xml.js -->
  <!-- purpose: Add array validation and .catch() to fetch-releases. Add file existence checks to SDK docs. Add pubDate to RSS items. -->
  <!-- findings: F-060, F-061 -->
  <!-- severity: medium -->
  <!-- impact: Build scripts can crash silently; RSS items appear undated -->

## 1.4 — Low Priority Improvements
> depends: 1.1
> qa-report: ./report.md

- [ ] Reduce large file sizes across the codebase
  <!-- file: apps/app/src/shell/onboarding.js, plugins/life/expenses/src/components/transaction-form.js, reports-view.js, and 7 more -->
  <!-- purpose: Extract CSS into separate files. Split large components into sub-components. Split large test files by concern area. -->
  <!-- findings: F-062 -->
  <!-- severity: low -->
  <!-- impact: Files over 300 lines increase maintenance cost and cognitive load -->

- [ ] Consolidate escapeHtml implementations into shared utility
  <!-- file: apps/app/src/components/shell-conflict-viewer.js, shell-conflict-list.js, shell/topbar.js, shell/household.js, plugins/life/_shared/escape.js -->
  <!-- purpose: Create one authoritative escapeHtml in a shared location. Remove all duplicate implementations. -->
  <!-- findings: F-063 -->
  <!-- severity: low -->
  <!-- impact: Six different implementations create divergence risk -->

- [ ] Fix hardcoded AUD currency in expenses components
  <!-- file: plugins/life/expenses/src/components/transaction-list.js, transaction-form.js -->
  <!-- purpose: Accept primary currency as a property or read from plugin settings instead of hardcoding 'AUD' -->
  <!-- findings: F-064 -->
  <!-- severity: low -->
  <!-- impact: Daily totals and exchange rates always display in AUD regardless of user settings -->

- [ ] Fix minor bug risk items across codebase
  <!-- file: apps/app/src/components/shell-conflict-list.js, pull-to-refresh.js, shell-avatar.js, plugins/life/notes/index.js, contacts/index.js, calendar/index.js, and others -->
  <!-- purpose: Add error feedback in conflict-list. Make pull-to-refresh timeout configurable. Fix avatar initials with consecutive spaces. Add null guards in filter methods. Re-fetch calendar events on navigation. -->
  <!-- findings: F-065, F-066, F-067, F-068, F-069, F-073, F-074, F-078, F-079 -->
  <!-- severity: low -->
  <!-- impact: Minor edge cases and quality-of-life improvements -->

- [ ] Clean up duplicate configs, unused imports, and test quality
  <!-- file: packages/test-utils-js/vitest.config.js, plugins/life/expenses/src/components/budget-form.js, tests/e2e/plugin-store-a11y.spec.ts, plugins/life/todos/src/services/label-service.js -->
  <!-- purpose: Remove duplicate vitest config and test file. Remove unused formatCurrency import. Fix always-passing assertion. Extend BaseCrudService in todo services. -->
  <!-- findings: F-070, F-071, F-072, F-080, F-081 -->
  <!-- severity: low -->
  <!-- impact: Reduces confusion, removes dead code, fixes false-confidence test -->

- [ ] Add test coverage for untested plugins and services
  <!-- file: plugins/life/dashboard/index.js, contacts/index.js, email-viewer/index.js, files/index.js, notes/index.js, task-manager/index.js, expenses services -->
  <!-- purpose: Create test files for 6 untested plugins. Add tests for RecurringService, ReceiptService, KeyboardShortcutService, and scheduling-engine in expenses. -->
  <!-- findings: F-075, F-076 -->
  <!-- severity: low -->
  <!-- impact: Critical plugin logic has zero test coverage -->

- [ ] Add CI retries and minor CSS/security hardening
  <!-- file: playwright.config.ts, plugins/life/expenses/src/components/tag-chip.js, category-icon.js -->
  <!-- purpose: Add retries: process.env.CI ? 2 : 0. Validate colour property matches hex pattern before injecting into CSS. -->
  <!-- findings: F-077, F-080 -->
  <!-- severity: low -->
  <!-- impact: Prevents flaky CI failures; hardens CSS injection surface -->

<!-- Omit info-level tasks per QA process rules. -->
