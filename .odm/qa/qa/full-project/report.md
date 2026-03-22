<!--
qa-name: full-project
target: All source code (~270 JS/TS/CSS/HTML/Astro files)
status: complete
updated: 2026-03-22
files-inspected: 262
-->

# QA Report — Full Project

## Summary

- **Target:** All source code in life-engine repository
- **Scope:** 262 files inspected across 15 directories
- **Date:** 2026-03-22

### Findings by Severity

- **Critical:** 2
- **High:** 19
- **Medium:** 40
- **Low:** 20
- **Info:** 6

### Findings by Dimension

- **Bug Risk:** 31
- **Security:** 25
- **Performance:** 12
- **Code Quality:** 10
- **Testing:** 6
- **Consistency:** 2
- **Documentation:** 1

## Files Inspected

### apps/app/src/components/

- `pull-to-refresh.js` — 2 findings
- `shell-avatar.js` — 2 findings
- `shell-badge.js` — 0 findings (covered by systemic)
- `shell-button.js` — 1 finding
- `shell-card.js` — 0 findings
- `shell-checkbox.js` — 2 findings
- `shell-conflict-list.js` — 2 findings
- `shell-conflict-viewer.js` — 0 findings
- `shell-empty-state.js` — 0 findings (covered by systemic)
- `shell-error-state.js` — 0 findings (covered by systemic)
- `shell-input.js` — 2 findings
- `shell-list-item.js` — 0 findings (covered by systemic)
- `shell-list.js` — 0 findings
- `shell-modal.js` — 2 findings
- `shell-select.js` — 1 finding
- `shell-sheet.js` — 1 finding
- `shell-spinner.js` — 0 findings
- `shell-textarea.js` — 1 finding
- `shell-toast.js` — 0 findings
- `shell-toggle.js` — 1 finding

### apps/app/src/lib/

- `canvas-primitives.js` — 1 finding
- `capabilities.js` — 0 findings
- `conflict-resolver.js` — 2 findings
- `crypto.js` — 1 finding
- `data-store.js` — 0 findings
- `mobile-sync.js` — 0 findings
- `offline-queue.js` — 0 findings
- `plugin-loader.js` — 1 finding
- `plugin-manifest.js` — 0 findings
- `plugin-storage.js` — 1 finding
- `push-notifications.js` — 0 findings
- `router.js` — 0 findings
- `scoped-api.js` — 0 findings
- `search-client.js` — 0 findings
- `secure-token.js` — 1 finding
- `shared-modules.js` — 0 findings
- `sync-adapter.js` — 1 finding
- `sync-manager.js` — 1 finding
- `theme.js` — 0 findings
- `token-manager.js` — 2 findings
- `webauthn.js` — 1 finding
- `main.js` — 4 findings

### apps/app/src/shell/

- `bottom-nav.js` — 1 finding
- `conflict-resolution.js` — 0 findings
- `household.js` — 2 findings
- `login.js` — 2 findings
- `onboarding.js` — 5 findings
- `pipeline-canvas.js` — 1 finding
- `plugin-container.js` — 1 finding
- `plugin-store.js` — 1 finding
- `settings.js` — 0 findings
- `sidebar.js` — 1 finding
- `statusbar.js` — 0 findings
- `topbar.js` — 0 findings

### apps/app/src/styles/

- `shared-styles.js` — 1 finding
- `shell.css` — 0 findings
- `theme-dark.css` — 0 findings
- `theme-light.css` — 0 findings
- `tokens.css` — 0 findings

### apps/app/tests/

- 34 test files — 3 findings (systemic patterns)

### apps/web/

- 28 files — 3 findings

### packages/

- 16 files — 1 finding

### plugins/life/expenses/

- 32 files — 14 findings

### plugins/life/ (other plugins + todos + shared)

- 21 files — 10 findings

### tests/e2e/

- 65 files — 3 findings

### scripts/ + tools/ + config

- 5 files — 1 finding

## Findings

### Critical

#### F-001 — Weak passphrase hash allows trivial brute-force

- **File:** `./apps/app/src/lib/crypto.js`
- **Line(s):** 12-20
- **Dimension:** Security
- **Severity:** Critical
- **Description:** `simpleHash` uses a DJB2-variant non-cryptographic hash for passphrase verification. The hash output is only 32 bits (`hash |= 0`), making collisions and brute-force trivial. This hash is stored in localStorage and used by `login.js` (line 36) and `onboarding.js` (line 258) for passphrase verification. The e2e test at `tests/e2e/login.spec.ts:26` confirms the algorithm is `hash = ((hash << 5) - hash + ch) | 0`.
- **Recommendation:** Replace with `crypto.subtle.importKey` + PBKDF2 with a random salt stored alongside the hash. The SubtleCrypto API is available in all modern browsers.

---

#### F-002 — Email iframe sandbox allows same-origin access (XSS)

- **File:** `./plugins/life/email-viewer/index.js`
- **Line(s):** 693-698
- **Dimension:** Security
- **Severity:** Critical
- **Description:** HTML email bodies are rendered via `<iframe sandbox="allow-same-origin" srcdoc="...">`. The `allow-same-origin` flag allows content within the iframe to access the parent's DOM, localStorage, cookies, and tokens. Any malicious email with embedded HTML/CSS (form phishing, CSS data exfiltration) can interact with the host application context.
- **Recommendation:** Remove `allow-same-origin` — use `sandbox=""` (empty). If styling requires same-origin access, sanitize email HTML with a dedicated library (e.g., DOMPurify) before rendering.

---

### High

#### F-003 — Systemic innerHTML XSS in shell components (high-risk instances)

- **File:** `./apps/app/src/components/shell-input.js`
- **Line(s):** 62-68
- **Dimension:** Security
- **Severity:** High
- **Description:** The `error` attribute is rendered as raw innerHTML: `<div class="error-message">${error}</div>`. A value like `<img src=x onerror=alert(1)>` executes arbitrary JS. The same pattern exists in `shell-textarea.js` (line 58-68), `shell-select.js` (line 57 — option values/labels), and `shell-avatar.js` (line 85 — `src` and `name` attributes). These four components have the highest XSS risk because they render user-influenced content.
- **Recommendation:** Add `#escapeHtml(str)` using the string-replace approach from `shell-conflict-viewer.js` (line 53). Apply it to all interpolated attribute values and content. Consider creating a shared `escape.js` utility for all shell components.

---

#### F-004 — Auth tokens stored in localStorage plaintext (browser fallback)

- **File:** `./apps/app/src/lib/secure-token.js`
- **Line(s):** 81-83
- **Dimension:** Security
- **Severity:** High
- **Description:** When Tauri keychain is unavailable (browser dev mode), auth tokens including refresh tokens are stored in `localStorage` in plaintext. Any XSS vulnerability would expose all auth tokens. There is no warning mechanism.
- **Recommendation:** Log a prominent warning when falling back. Consider `sessionStorage` for browser builds (tokens don't persist across tabs). For production browser builds, use in-memory-only storage.

---

#### F-005 — URL path injection in conflict-resolver.js

- **File:** `./apps/app/src/lib/conflict-resolver.js`
- **Line(s):** 154, 175, 205
- **Dimension:** Security
- **Severity:** High
- **Description:** The `id` parameter is interpolated directly into fetch URLs: `` `${this.#coreUrl}/api/conflicts/${id}/resolve` ``. Path traversal characters (e.g., `../../`) in the ID could manipulate the URL. Affects `resolveConflict`, `dismissConflict`, and `fetchConflict`.
- **Recommendation:** Use `encodeURIComponent(id)` or validate the ID matches an expected pattern (UUID).

---

#### F-006 — URL path injection in sync-adapter.js

- **File:** `./apps/app/src/lib/sync-adapter.js`
- **Line(s):** 273
- **Dimension:** Security
- **Severity:** High
- **Description:** `#buildUrl` interpolates `mutation.data?.id` directly into the URL path. If a mutation's data ID is malformed, this enables URL path manipulation.
- **Recommendation:** Use `encodeURIComponent(id)` in `#buildUrl`.

---

#### F-007 — sync-manager flush dequeues by count, not identity

- **File:** `./apps/app/src/lib/sync-manager.js`
- **Line(s):** 314-329
- **Dimension:** Bug Risk
- **Severity:** High
- **Description:** The `#flush` method dequeues mutations by index count (`result.pushed`) rather than by identity. If a middle mutation fails, `pushed` still counts the later successes, causing the dequeue to remove the failed mutation from the front of the queue — data loss.
- **Recommendation:** Track which specific mutations succeeded (by ID) and remove only those from the queue.

---

#### F-008 — token-manager conflates network errors with auth failures

- **File:** `./apps/app/src/lib/token-manager.js`
- **Line(s):** 149-153
- **Dimension:** Bug Risk
- **Severity:** High
- **Description:** `#doRefresh` checks `err.message.includes('Token refresh failed')` to decide whether to emit `auth-expired`. Network errors (e.g., "Failed to fetch") don't match, so `auth-expired` fires on transient network issues, causing premature logouts.
- **Recommendation:** Only emit `auth-expired` for HTTP 401/403 responses, not network errors.

---

#### F-009 — App reset deletes all data without confirmation

- **File:** `./apps/app/src/shell/login.js`
- **Line(s):** 82-93
- **Dimension:** Security
- **Severity:** High
- **Description:** `#resetApp()` clears all `life-engine:` keys from localStorage and reloads the page. Triggered by a single button click with no confirmation dialog.
- **Recommendation:** Add a confirmation modal requiring passphrase entry before executing the reset.

---

#### F-010 — Passphrase held in memory, never cleared

- **File:** `./apps/app/src/shell/onboarding.js`
- **Line(s):** 289
- **Dimension:** Security
- **Severity:** High
- **Description:** The raw passphrase is held in `#passphrase` (a JS string) throughout the wizard's lifetime and is never cleared after use. The field persists until GC collects the component.
- **Recommendation:** Set `this.#passphrase = ''` and `this.#passphraseConfirm = ''` immediately after step 3 completes.

---

#### F-011 — Receipt service: no file validation, path traversal

- **File:** `./plugins/life/expenses/src/services/receipt-service.js`
- **Line(s):** 37-47
- **Dimension:** Security
- **Severity:** High
- **Description:** Receipt files are stored with no validation of content size, MIME type verification, or filename sanitization. The path `receipts/${transactionId}/${file.name}` uses raw `file.name` which could contain `../` for path traversal.
- **Recommendation:** Validate file size against a maximum. Sanitize filename (strip separators, generate UUID). Verify MIME against an allowlist.

---

#### F-012 — Expenses search called with wrong arguments

- **File:** `./plugins/life/expenses/src/index.js`
- **Line(s):** 458-467
- **Dimension:** Bug Risk
- **Severity:** High
- **Description:** `#handleSearchInput` calls `this.#searchFilterService.search(this._transactions, { query })` but `SearchFilterService.search` takes only `criteria`. `this._transactions` is passed as `criteria` and `{ query }` is ignored, causing search to malfunction.
- **Recommendation:** Fix to `this.#searchFilterService.search({ query })`. Store filtered results in a separate `_filteredTransactions` property to preserve the full list.

---

#### F-013 — Expenses search clear race condition

- **File:** `./plugins/life/expenses/src/index.js`
- **Line(s):** 461-466
- **Dimension:** Bug Risk
- **Severity:** High
- **Description:** When the search query is cleared, `this.#loadData()` is called without `await`, creating a race condition. The filtered `_transactions` may display before the reload completes.
- **Recommendation:** `await` the `#loadData()` call. Use a separate `_filteredTransactions` property.

---

#### F-014 — Budget status computation fires O(N*B*P) queries

- **File:** `./plugins/life/expenses/src/services/budget-service.js`
- **Line(s):** 85-92
- **Dimension:** Performance
- **Severity:** High
- **Description:** `#fetchExpenses` fetches ALL transactions from the database each time. In `getStatus` with rollover, it's called once per past period plus once for the current period. For N budgets with P past periods each, this creates O(N*P) full-collection fetches — each returning every transaction.
- **Recommendation:** Fetch transactions once and pass the array to pure computation functions. Use `Promise.all` for parallel budget status computation.

---

#### F-015 — Scheduling engine unbounded loop risk

- **File:** `./plugins/life/expenses/src/services/scheduling-engine.js`
- **Line(s):** 64-77
- **Dimension:** Bug Risk
- **Severity:** High
- **Description:** `getMissedDueDates` has no upper bound on iterations. If `rule.next_due` is far in the past (years), this loop generates thousands of dates and could freeze the UI.
- **Recommendation:** Add a max iteration cap (e.g., 365) and log a warning when hit.

---

#### F-016 — Balance trend O(30*N) computation per render

- **File:** `./plugins/life/expenses/src/components/account-overview.js`
- **Line(s):** 394-418
- **Dimension:** Performance
- **Severity:** High
- **Description:** `#balanceTrend` iterates all transactions for each of 30 days. With large transaction histories, this becomes very expensive and runs on every render.
- **Recommendation:** Pre-compute balance by iterating transactions once in date order.

---

#### F-017 — Notes markdown renderer fragile escape-then-unescape pattern

- **File:** `./plugins/life/notes/index.js`
- **Line(s):** 196-217
- **Dimension:** Security
- **Severity:** High
- **Description:** `#renderMarkdown` first escapes HTML, then applies regex replacements that re-introduce HTML tags. This "escape then unescape" pattern is inherently fragile — if regex patterns evolve, XSS could slip through.
- **Recommendation:** Use a proper markdown library (marked, markdown-it) with a sanitizer.

---

#### F-018 — Vanilla plugin template crashes with closed shadowRoot

- **File:** `./tools/templates/life-plugin-vanilla/src/index.js`
- **Line(s):** 4, 9
- **Dimension:** Bug Risk
- **Severity:** High
- **Description:** Shadow root is created with `mode: 'closed'`, but `this.shadowRoot` is accessed in `connectedCallback`. With closed mode, `this.shadowRoot` returns `null`, causing a TypeError.
- **Recommendation:** Use `mode: 'open'` or store the return value: `this.#shadow = this.attachShadow({ mode: 'closed' })`.

---

#### F-019 — Vanilla plugin template innerHTML XSS

- **File:** `./tools/templates/life-plugin-vanilla/src/index.js`
- **Line(s):** 14
- **Dimension:** Security
- **Severity:** High
- **Description:** `shell?.plugin?.id` is interpolated directly into innerHTML without escaping. A malicious plugin ID would execute JS.
- **Recommendation:** Use `escapeHtml` from the shared module.

---

#### F-020 — Task service fetches entire collection for every operation

- **File:** `./plugins/life/todos/src/services/task-service.js`
- **Line(s):** 89-103, 106-121, 238-267
- **Dimension:** Performance
- **Severity:** High
- **Description:** `getByProject`, `getBySection`, `getSubtasks`, `getSubtaskProgress`, `getIncompleteSubtasks` all fetch the entire task collection and filter client-side. `reorder` calls `getById` N times sequentially.
- **Recommendation:** Pass specific query filters to the API. Batch reorder updates.

---

#### F-021 — escapeHtml relies on DOM, crashes in non-browser contexts

- **File:** `./plugins/life/_shared/escape.js`
- **Line(s):** 1-5
- **Dimension:** Bug Risk
- **Severity:** High
- **Description:** `escapeHtml` uses `document.createElement('div')` which requires a DOM. Any non-browser usage (SSR, workers, Node scripts) throws `ReferenceError`.
- **Recommendation:** Replace with pure string-based implementation using `.replace()` chains, matching the pattern in `escapeAttr`.

---

### Medium

#### F-022 — Systemic innerHTML XSS in shell components (medium-risk instances)

- **File:** `./apps/app/src/components/` (multiple)
- **Line(s):** shell-checkbox.js:117, shell-empty-state.js:61, shell-error-state.js:79, shell-modal.js:163, shell-sheet.js:124, shell-badge.js:66, shell-button.js:119
- **Dimension:** Security
- **Severity:** Medium
- **Description:** Seven additional shell components interpolate attribute values into innerHTML without escaping. These are lower risk than F-003 because the attributes (`title`, `label`, `icon`, `variant`) are less likely to be user-controlled, but the pattern is still unsafe.
- **Recommendation:** Apply `#escapeHtml` to all interpolated values across all shell components.

---

#### F-023 — Shared focusVisible/disabledState CSS produces wrong selectors

- **File:** `./apps/app/src/styles/shared-styles.js`
- **Line(s):** 16-21
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** `focusVisible` is exported as a full CSS rule (`:focus-visible { ... }`). When components concatenate it as `.btn${focusVisible}`, it produces `.btn :focus-visible { ... }` (descendant selector) instead of `.btn:focus-visible`. This silently breaks focus-visible and disabled styles in 8+ components: shell-button, shell-checkbox, shell-toggle, shell-conflict-list, shell-list-item, sidebar, topbar, conflict-resolution.
- **Recommendation:** Export only the declarations (without the selector). Provide a function `focusVisibleFor(selector)` or let each component wrap declarations in their own selector.

---

#### F-024 — Shell-modal and shell-sheet missing focus trapping

- **File:** `./apps/app/src/components/shell-modal.js`
- **Line(s):** 186-192
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** Both `shell-modal` (line 186) and `shell-sheet` (line 137) have `aria-modal="true"` but no focus trapping. Tab can move focus to elements outside the modal/sheet, breaking accessibility. Shell-sheet also lacks focus management on open/close.
- **Recommendation:** Implement focus trapping by intercepting Tab/Shift+Tab keydown events. For shell-sheet, add focus management matching shell-modal.

---

#### F-025 — Shell-checkbox and shell-toggle double-render on toggle

- **File:** `./apps/app/src/components/shell-checkbox.js`
- **Line(s):** 126-141
- **Dimension:** Performance
- **Severity:** Medium
- **Description:** `toggle()` calls `this.checked = !this.checked` which triggers `attributeChangedCallback → #render()`. Every toggle causes a full DOM teardown and rebuild. Same issue in `shell-toggle.js` (line 109-123).
- **Recommendation:** Guard `set checked()` to only call setAttribute if the value changed, or update DOM incrementally.

---

#### F-026 — Shell-input value setter destroys DOM and loses focus

- **File:** `./apps/app/src/components/shell-input.js`
- **Line(s):** 28-31
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** `set value(val)` calls `setAttribute('value', val)` which triggers `attributeChangedCallback → #render()`, rebuilding all DOM elements. This loses focus and cursor position.
- **Recommendation:** Update the input element's value directly without triggering a full re-render.

---

#### F-027 — main.js clearOverlayPages null crash

- **File:** `./apps/app/src/main.js`
- **Line(s):** 331
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** `settingsContainer` is from `document.querySelector('#content')` which can be `null`. Every route change would throw TypeError.
- **Recommendation:** Add null guard: `if (!settingsContainer) return;`

---

#### F-028 — main.js duplicate beforeunload listener

- **File:** `./apps/app/src/main.js`
- **Line(s):** 192-204
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** `showLogin` adds a `beforeunload` listener for `stopAllServices`, but the same listener is already registered in `init()`. After login, `stopAllServices()` will be called twice on window close.
- **Recommendation:** Register `beforeunload` listener only once in `init()`.

---

#### F-029 — main.js sync-requested drops tokenManager on restart

- **File:** `./apps/app/src/main.js`
- **Line(s):** 414-425
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** The `sync-requested` handler calls `syncManager.stop()` then `syncManager.start()` without passing `tokenManager`. After pull-to-refresh, subsequent 401s won't trigger automatic token refresh.
- **Recommendation:** Pass `tokenManager` to the restart call, or use a dedicated `resync()` method.

---

#### F-030 — plugin-loader dynamic import URL not validated

- **File:** `./apps/app/src/lib/plugin-loader.js`
- **Line(s):** 312
- **Dimension:** Security
- **Severity:** Medium
- **Description:** `import(entryUrl)` dynamically imports an arbitrary URL. While the manifest is validated, the entry URL is not checked against `allowedDomains`. A manifest with `entry: "http://evil.com/malicious.js"` would load.
- **Recommendation:** Validate the entry URL's origin against the manifest's `allowedDomains` or a trusted set.

---

#### F-031 — token-manager expires_in NaN prevents future refreshes

- **File:** `./apps/app/src/lib/token-manager.js`
- **Line(s):** 133
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** `this.#expiresAt = Date.now() + data.expires_in * 1000` — if `expires_in` is missing or non-numeric, this computes `NaN`. Since `Date.now() >= NaN` is always false, the token never appears expired, preventing all future refreshes.
- **Recommendation:** Validate `expires_in` and provide a default fallback (e.g., 3600).

---

#### F-032 — webauthn coreBaseUrl not validated

- **File:** `./apps/app/src/lib/webauthn.js`
- **Line(s):** 82-163
- **Dimension:** Security
- **Severity:** Medium
- **Description:** `startRegistration` and `startAuthentication` do not validate `coreBaseUrl`. A malicious input could redirect WebAuthn requests to an arbitrary URL, exfiltrating the auth token.
- **Recommendation:** Validate that `coreBaseUrl` matches the expected Core API origin.

---

#### F-033 — plugin-storage JSON.parse without try/catch

- **File:** `./apps/app/src/lib/plugin-storage.js`
- **Line(s):** 21
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** `JSON.parse(val)` in `get` throws if the stored value is invalid JSON.
- **Recommendation:** Wrap in try/catch, return null on parse failure.

---

#### F-034 — conflict-resolver timestamp NaN defaults to wrong winner

- **File:** `./apps/app/src/lib/conflict-resolver.js`
- **Line(s):** 43-48
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** If `_updated` is missing/malformed, `getTime()` returns `NaN`. The comparison `serverTime >= localTime` evaluates to `false` when either is NaN, causing local to always win — contradicting "server wins on tie".
- **Recommendation:** Validate `_updated` fields. Default to server-wins if either is NaN.

---

#### F-035 — Router/sidebar/bottom-nav listener never cleaned up

- **File:** `./apps/app/src/shell/bottom-nav.js`
- **Line(s):** 23
- **Dimension:** Performance
- **Severity:** Medium
- **Description:** `router.onChange(() => this.#render())` is registered in `connectedCallback` but never unregistered. No `disconnectedCallback` exists. If elements are removed and re-added, listeners accumulate. Same pattern in `sidebar.js` (line 20).
- **Recommendation:** Store the unsubscribe handle, call it in `disconnectedCallback()`.

---

#### F-036 — Household HTTP error falls through to idle state

- **File:** `./apps/app/src/shell/household.js`
- **Line(s):** 54-60
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** When `resp.ok` is false, the code does not set `this.#state = 'error'`, falling through to `idle` with `#household = null`. Server errors silently show the "create household" view.
- **Recommendation:** Add `else` branch for `!resp.ok` that sets state to `error`.

---

#### F-037 — Login session bypass via sessionStorage

- **File:** `./apps/app/src/shell/login.js`
- **Line(s):** 47
- **Dimension:** Security
- **Severity:** Medium
- **Description:** Session state is a simple boolean in `sessionStorage`. Any JS on the same origin can set `life-engine:session-active` to `'true'` to bypass the login screen.
- **Recommendation:** Use an in-memory token or signed session mechanism instead.

---

#### F-038 — Onboarding connector/PG credentials stored in localStorage plaintext

- **File:** `./apps/app/src/shell/onboarding.js`
- **Line(s):** 266-268, 451-452
- **Dimension:** Security
- **Severity:** Medium
- **Description:** PostgreSQL credentials (including password) and email connector credentials are stored in plain-text localStorage. Any XSS or malicious plugin can read them.
- **Recommendation:** Use the secure token storage mechanism (`storeToken`) for all credentials.

---

#### F-039 — Onboarding innerHTML with unescaped error messages

- **File:** `./apps/app/src/shell/onboarding.js`
- **Line(s):** 1027, 1032-1040
- **Dimension:** Security
- **Severity:** Medium
- **Description:** `#passphraseError`, `#statusMessage`, and `#errorMessage` are interpolated into innerHTML without escaping. Error messages from `catch(err)` (line 295, 317) could contain attacker-influenced content.
- **Recommendation:** Escape all error/status messages before interpolation.

---

#### F-040 — Pipeline-canvas API calls missing auth headers

- **File:** `./apps/app/src/shell/pipeline-canvas.js`
- **Line(s):** 167-169, 792, 816
- **Dimension:** Security
- **Severity:** Medium
- **Description:** Three API endpoints (`/api/pipeline`, `/api/pipeline/palette`, `/api/pipeline/nodes/.../config`) are called without authentication headers, unlike all other components. Errors are silently swallowed.
- **Recommendation:** Add authentication headers and user notification for failures.

---

#### F-041 — Plugin-container innerHTML with unescaped pluginId

- **File:** `./apps/app/src/shell/plugin-container.js`
- **Line(s):** 101-103, 130-139
- **Dimension:** Security
- **Severity:** Medium
- **Description:** `pluginId` and error `message` are interpolated into innerHTML without escaping.
- **Recommendation:** Escape before interpolation.

---

#### F-042 — Plugin-store install only tracks in-memory Set

- **File:** `./apps/app/src/shell/plugin-store.js`
- **Line(s):** 675-679
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** The "Allow" button adds the plugin ID to a local in-memory `Set`. There is no actual download, validation, or persistent installation. The Set is lost on page reload.
- **Recommendation:** Persist installed plugin IDs and implement actual installation logic.

---

#### F-043 — Keyboard shortcuts attached to document, not component

- **File:** `./plugins/life/expenses/src/index.js`
- **Line(s):** 324
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** `kb.attach(document)` attaches shortcuts to the global document. Single-letter shortcuts (`n`, `e`, `d`, `b`, `r`) fire when typing elsewhere on the page. Multiple instances would duplicate handlers.
- **Recommendation:** Attach to the plugin's host element or shadow root with `tabindex`.

---

#### F-044 — Expenses delete without confirmation

- **File:** `./plugins/life/expenses/src/index.js`
- **Line(s):** 355-369
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** `#handleDeleteSelected` permanently deletes the selected transaction on a single `Delete`/`Backspace` keypress with no confirmation.
- **Recommendation:** Add a confirmation dialog or undo toast.

---

#### F-045 — Receipt-viewer keydown listener memory leak

- **File:** `./plugins/life/expenses/src/components/receipt-viewer.js`
- **Line(s):** 129-131
- **Dimension:** Performance
- **Severity:** Medium
- **Description:** `#handleKeyDown.bind(this)` creates a new function each `connectedCallback`. No `disconnectedCallback` removes it. Repeated connect/disconnect leaks listeners.
- **Recommendation:** Store the bound reference, remove in `disconnectedCallback`.

---

#### F-046 — Recurring-service Object.assign mutates shared data

- **File:** `./plugins/life/expenses/src/services/recurring-service.js`
- **Line(s):** 200
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** `Object.assign(rule, updated)` mutates the original array element from `getAll()`. Other code referencing `rules` may see unexpected mutations.
- **Recommendation:** Use `const updatedRule = { ...updated }` instead.

---

#### F-047 — Budget-engine getPastPeriods unbounded loop

- **File:** `./plugins/life/expenses/src/services/budget-engine.js`
- **Line(s):** 130-143
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** `while (cursor < currentPeriod.start)` has no safety break. Misconfigured start_date could cause an infinite loop.
- **Recommendation:** Add max iteration guard (e.g., 1000).

---

#### F-048 — Account-service initial_balance null produces NaN

- **File:** `./plugins/life/expenses/src/services/account-service.js`
- **Line(s):** 75
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** `computeBalance` uses `account.initial_balance` without nullish coalescing. If null/undefined, arithmetic produces NaN.
- **Recommendation:** Use `account.initial_balance ?? 0`.

---

#### F-049 — CSS @import in shadow DOM unreliable

- **File:** `./plugins/life/contacts/index.js`
- **Line(s):** 76-77
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** `@import '../_shared/styles.css'` inside shadow DOM `<style>` is unreliable — relative URLs resolve differently. Same pattern in files/index.js, notes/index.js, task-manager/index.js.
- **Recommendation:** Use `import.meta.url` with a `<link>` element (like the calendar plugin) or inline the shared styles.

---

#### F-050 — Contacts/notes/task-manager weak ID generation

- **File:** `./plugins/life/contacts/index.js`
- **Line(s):** 310
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** IDs are generated as `prefix-${Math.random().toString(36).slice(2, 8)}` (~31 bits of entropy). Collisions become likely after ~50K records. Same in notes/index.js (line 291), task-manager/index.js (line 374).
- **Recommendation:** Use `crypto.randomUUID()`.

---

#### F-051 — Crud-scaffold silently swallows all errors

- **File:** `./plugins/life/_shared/crud-scaffold.js`
- **Line(s):** 140-152
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** `crudOperation` catches all errors and returns `false`. No logging, no user feedback for auth failures, network issues, or validation errors.
- **Recommendation:** At minimum log the error. Let auth failures propagate.

---

#### F-052 — Email-viewer listener duplication per render

- **File:** `./plugins/life/email-viewer/index.js`
- **Line(s):** 780-808
- **Dimension:** Performance
- **Severity:** Medium
- **Description:** `#attachListListeners()` adds new event listeners on every `#renderContent()` call. Since renders happen on each search keystroke, listeners multiply.
- **Recommendation:** Use event delegation on the root element.

---

#### F-053 — Search-filter and currency services fetch all transactions

- **File:** `./plugins/life/expenses/src/services/search-filter-service.js`
- **Line(s):** 40-43
- **Dimension:** Performance
- **Severity:** Medium
- **Description:** `search` and `aggregateInPrimaryCurrency` always fetch ALL transactions before filtering. Same issue in `currency-service.js` (lines 79-103).
- **Recommendation:** Push date range/account filters to the query layer.

---

#### F-054 — Sidebar net worth O(M*N) computation

- **File:** `./plugins/life/expenses/src/components/expenses-sidebar.js`
- **Line(s):** 162-184
- **Dimension:** Performance
- **Severity:** Medium
- **Description:** `#computeNetWorth` calls `#computeAccountBalance` per account, each iterating ALL transactions. O(M*N) on every render.
- **Recommendation:** Compute all account balances in a single pass.

---

#### F-055 — Expenses connectedCallback runs shortcuts even if API is null

- **File:** `./plugins/life/expenses/src/index.js`
- **Line(s):** 238-275
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** If `this.api` is null, services are never initialized, but `#setupKeyboardShortcuts` still runs. Shortcuts like `n` and `e` will call methods on null services, causing uncaught errors.
- **Recommendation:** Only set up shortcuts after confirming API availability.

---

#### F-056 — globalThis.fetch mock leaks across test suites

- **File:** `./apps/app/tests/lib/conflict-resolver.test.js`
- **Line(s):** 133
- **Dimension:** Testing
- **Severity:** Medium
- **Description:** `globalThis.fetch = vi.fn()` directly assigns without spying. `vi.restoreAllMocks()` won't restore it. Same issue in `search-client.test.js` (line 13) and `sync-adapter.test.js` (line 17).
- **Recommendation:** Use `vi.spyOn(globalThis, 'fetch')` instead.

---

#### F-057 — Sidecar e2e tests reference non-existent page object methods

- **File:** `./tests/e2e/sidecar-e2e-flow.spec.ts`
- **Line(s):** 178
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** References `onboarding.stepEmailSetup`, `onboarding.skipEmail()`, `onboarding.fillEmailAndAdvance()` — none of which exist on `OnboardingPage`. Same in `sidecar-integration.spec.ts` (line 47, 100-101). These tests fail at runtime.
- **Recommendation:** Update tests to use existing page object methods or add the missing methods.

---

#### F-058 — Expenses e2e tests use brittle waitForTimeout

- **File:** `./tests/e2e/expenses-keyboard.spec.ts`
- **Line(s):** 93, 126-127, 134-136
- **Dimension:** Testing
- **Severity:** Medium
- **Description:** Multiple `waitForTimeout(200)` and `waitForTimeout(300)` calls instead of condition-based waits. Same in `expenses-dashboard.spec.ts` (line 217).
- **Recommendation:** Replace with `waitForFunction` or Playwright auto-retry assertions.

---

#### F-059 — Spending-by-category and doughnut chart duplicated 3x each

- **File:** `./plugins/life/expenses/src/components/expenses-dashboard.js`
- **Line(s):** 388-433, 499-577
- **Dimension:** Code Quality
- **Severity:** Medium
- **Description:** The `#spendingByCategory` getter reimplements logic from `chart-helpers.js:aggregateSpendingByCategory`. The doughnut chart SVG is implemented in `expenses-dashboard.js`, `reports-view.js`, and `chart-helpers.js` — three copies of the same arc calculation.
- **Recommendation:** Use the shared `chart-helpers.js` functions consistently.

---

#### F-060 — fetch-releases.mjs and generate-api-docs.mjs error handling gaps

- **File:** `./apps/web/scripts/fetch-releases.mjs`
- **Line(s):** 36-43, 69
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** JSON parse doesn't verify the result is an array. The top-level Promise rejection is uncaught. `generate-api-docs.mjs` uses a fragile indentation-based YAML parser. `generate-sdk-docs.mjs` lacks file existence checks.
- **Recommendation:** Add array validation, `.catch()` handlers, and existence checks.

---

#### F-061 — rss.xml.js missing pubDate

- **File:** `./apps/web/src/pages/rss.xml.js`
- **Line(s):** 4-18
- **Dimension:** Bug Risk
- **Severity:** Medium
- **Description:** RSS feed items lack a `pubDate` field. RSS readers rely on this for sorting and display.
- **Recommendation:** Add `pubDate` to each RSS item.

---

### Low

#### F-062 — Onboarding file length (1321 lines)

- **File:** `./apps/app/src/shell/onboarding.js`
- **Line(s):** 1-1321
- **Dimension:** Code Quality
- **Severity:** Low
- **Description:** Multiple files significantly exceed the 300-line guideline. The worst offenders: `onboarding.js` (1321), `transaction-form.js` (1295+), `reports-view.js` (1287), `pipeline-canvas.js` (1218), `index.js` (987), `plugin-loader.test.js` (944), `calendar/index.js` (891), `email-viewer/index.js` (827), `conflict-resolution.js` (757), `plugin-manifest.js` (563).
- **Recommendation:** Extract CSS into separate files. Split large components into sub-components. Split large test files by concern area.

---

#### F-063 — Multiple escapeHtml implementations across codebase

- **File:** `./apps/app/src/components/shell-conflict-viewer.js`
- **Line(s):** 53-58
- **Dimension:** Code Quality
- **Severity:** Low
- **Description:** `#escapeHtml` is implemented independently in: `shell-conflict-viewer.js`, `shell-conflict-list.js`, `topbar.js` (DOM-based), `household.js` (DOM-based), `plugins/life/_shared/escape.js` (DOM-based), `escapeAttr` in escape.js (string-based). Six different implementations of the same function.
- **Recommendation:** Consolidate into a single shared utility used by all components.

---

#### F-064 — Transaction-list and transaction-form hardcode AUD currency

- **File:** `./plugins/life/expenses/src/components/transaction-list.js`
- **Line(s):** 234
- **Dimension:** Bug Risk
- **Severity:** Low
- **Description:** `#formatDailyTotal` hardcodes `'AUD'`. `transaction-form.js` (line 639-642) `#primaryCurrency` always returns `'AUD'` instead of reading from settings.
- **Recommendation:** Accept primary currency as a property or read from plugin settings.

---

#### F-065 — Shell-conflict-list silently swallows resolve/dismiss errors

- **File:** `./apps/app/src/components/shell-conflict-list.js`
- **Line(s):** 146-174
- **Dimension:** Bug Risk
- **Severity:** Low
- **Description:** `#handleResolve` and `#handleDismiss` catch blocks silently swallow exceptions with no user feedback.
- **Recommendation:** Show an error toast or log the error.

---

#### F-066 — Pull-to-refresh 5s auto-timeout may be too short

- **File:** `./apps/app/src/components/pull-to-refresh.js`
- **Line(s):** 247
- **Dimension:** Bug Risk
- **Severity:** Low
- **Description:** Auto-end timeout of 5 seconds dismisses the spinner regardless of whether sync completed. On slow networks, the spinner disappears prematurely.
- **Recommendation:** Make timeout configurable or require explicit `endRefresh()` call.

---

#### F-067 — Notes/contacts null safety in filter methods

- **File:** `./plugins/life/notes/index.js`
- **Line(s):** 56-58
- **Dimension:** Bug Risk
- **Severity:** Low
- **Description:** `#filtered()` accesses `n.title.toLowerCase()` and `n.body.toLowerCase()` without null checks. Same in contacts/index.js (line 59) for email `address`. Same in files/index.js (line 23) for `mime` parameter.
- **Recommendation:** Use `(n.title || '').toLowerCase()` pattern.

---

#### F-068 — Calendar events not re-fetched on navigation

- **File:** `./plugins/life/calendar/index.js`
- **Line(s):** 83-89
- **Dimension:** Bug Risk
- **Severity:** Low
- **Description:** `#loadEvents()` fetches events filtered by the current view's date range. Navigation methods only call `#renderContent()`, not `#loadEvents()`. Events outside the initial range never appear.
- **Recommendation:** Re-fetch events after navigation, or load a wider date window.

---

#### F-069 — collection-helpers formatCurrency null amount crash

- **File:** `./plugins/life/expenses/src/services/collection-helpers.js`
- **Line(s):** 53-62
- **Dimension:** Bug Risk
- **Severity:** Low
- **Description:** The fallback `${currency} ${amount.toFixed(2)}` is outside the try-catch. If `amount` is null/NaN, `.toFixed(2)` throws.
- **Recommendation:** Add `if (amount == null || isNaN(amount)) return '${currency} 0.00'`.

---

#### F-070 — Duplicate vitest config and test files in test-utils-js

- **File:** `./packages/test-utils-js/vitest.config.js`
- **Line(s):** 1-7
- **Dimension:** Code Quality
- **Severity:** Low
- **Description:** Two vitest configs exist (`.js` and `.ts`). Duplicate test file at `src/factories.test.ts` and `tests/factories.test.ts`.
- **Recommendation:** Remove `vitest.config.js` and `src/factories.test.ts`.

---

#### F-071 — Plugin-store always-passing assertion in a11y test

- **File:** `./tests/e2e/plugin-store-a11y.spec.ts`
- **Line(s):** 81
- **Dimension:** Testing
- **Severity:** Low
- **Description:** `expect(hasFocusedCard || true).toBe(true)` always passes. Provides false confidence.
- **Recommendation:** Implement the check properly or mark as `.fixme()`.

---

#### F-072 — Todos services duplicate BaseCrudService without extending

- **File:** `./plugins/life/todos/src/services/label-service.js`
- **Line(s):** 1-65
- **Dimension:** Code Quality
- **Severity:** Low
- **Description:** `LabelService`, `ProjectService`, and `SectionService` reimplement `getAll`, `getById`, `update`, `delete`, `reorder` from `BaseCrudService` without extending it.
- **Recommendation:** Extend `BaseCrudService`.

---

#### F-073 — Todos collection-helpers query result shape inconsistency

- **File:** `./plugins/life/todos/src/services/collection-helpers.js`
- **Line(s):** 46-49
- **Dimension:** Bug Risk
- **Severity:** Low
- **Description:** `BaseCrudService.getById` accesses `records[0]` assuming the query returns an array. Other plugins access `result.items`. This suggests either the API contract differs or there is a bug.
- **Recommendation:** Verify the API contract and make it consistent.

---

#### F-074 — Shell-avatar initials edge case with consecutive spaces

- **File:** `./apps/app/src/components/shell-avatar.js`
- **Line(s):** 27
- **Dimension:** Bug Risk
- **Severity:** Low
- **Description:** `#getInitials()` splits by space. Consecutive spaces produce empty segments that generate `undefined` initials.
- **Recommendation:** Add `.filter(Boolean)` after `.split(' ')`.

---

#### F-075 — Six plugins have no test files

- **File:** `./plugins/life/dashboard/index.js`
- **Line(s):** N/A
- **Dimension:** Testing
- **Severity:** Low
- **Description:** The following plugins have zero test coverage: dashboard, contacts, email-viewer, files, notes, task-manager. These contain data loading, formatting, CRUD, and rendering logic.
- **Recommendation:** Add test files for at least the utility functions and critical data paths.

---

#### F-076 — Expenses test coverage gaps

- **File:** `./plugins/life/expenses/tests.js`
- **Line(s):** N/A
- **Dimension:** Testing
- **Severity:** Low
- **Description:** The expenses test file covers service CRUD well but has no tests for: RecurringService, ReceiptService, KeyboardShortcutService, CsvExportService, scheduling-engine, or any UI components.
- **Recommendation:** Add tests for RecurringService (especially `processAllDue`) and scheduling-engine edge cases.

---

#### F-077 — Playwright config has no CI retries

- **File:** `./playwright.config.ts`
- **Line(s):** 18
- **Dimension:** Testing
- **Severity:** Low
- **Description:** `retries: 0` means any flaky test immediately fails the suite. WebKit rendering can be flaky in CI.
- **Recommendation:** Use `retries: process.env.CI ? 2 : 0`.

---

#### F-078 — Onboarding fixed setTimeout for core start

- **File:** `./apps/app/src/shell/onboarding.js`
- **Line(s):** 283
- **Dimension:** Bug Risk
- **Severity:** Low
- **Description:** `setTimeout(r, 1000)` as a fixed delay after `start_core`. If core takes longer, subsequent operations fail. If faster, user waits unnecessarily.
- **Recommendation:** Replace with health endpoint polling.

---

#### F-079 — validate-plugin-submission URL validation

- **File:** `./scripts/validate-plugin-submission.js`
- **Line(s):** 107-109
- **Dimension:** Bug Risk
- **Severity:** Low
- **Description:** The `repository` field only checks for non-empty string. No URL format validation.
- **Recommendation:** Add basic URL validation (must start with `https://`).

---

#### F-080 — Tag-chip and category-icon CSS injection via colour property

- **File:** `./plugins/life/expenses/src/components/tag-chip.js`
- **Line(s):** 70-72
- **Dimension:** Security
- **Severity:** Low
- **Description:** The `colour` property is injected directly into inline CSS. A crafted value could inject CSS properties. Same in `category-icon.js` (line 41).
- **Recommendation:** Validate `colour` matches a hex pattern before use.

---

#### F-081 — Budget-form unused import

- **File:** `./plugins/life/expenses/src/components/budget-form.js`
- **Line(s):** 9
- **Dimension:** Code Quality
- **Severity:** Low
- **Description:** `formatCurrency` is imported but never used.
- **Recommendation:** Remove the unused import.

---

### Info / Observations

#### F-082 — Shell-conflict-viewer duplicates escapeHtml and formatTimestamp

- **File:** `./apps/app/src/components/shell-conflict-viewer.js`
- **Line(s):** 53-58, 110-117
- **Dimension:** Code Quality
- **Observation:** `#escapeHtml` and `#formatTimestamp` are duplicated identically in both `ShellConflictList` and `ShellConflictViewer`. Extract to shared utility.

---

#### F-083 — CSS token duplication across theme files

- **File:** `./apps/app/src/styles/theme-dark.css`
- **Line(s):** 1-78
- **Dimension:** Code Quality
- **Observation:** Dark theme values are duplicated: once under `:root[data-theme="dark"]` and again under `@media (prefers-color-scheme: dark)`. Light theme values are in both `tokens.css` and `theme-light.css`.

---

#### F-084 — ShellAPI uses broad `object` type

- **File:** `./packages/plugin-sdk-js/src/index.ts`
- **Line(s):** 52-98
- **Dimension:** Code Quality
- **Observation:** `data: object` for `create` and `update` accepts any non-primitive. `Record<string, unknown>` would be more explicit.

---

#### F-085 — Standalone-binary doc section ordering inconsistency

- **File:** `./apps/web/src/content/docs/deployment/standalone-binary.mdx`
- **Line(s):** 1-322
- **Dimension:** Documentation
- **Observation:** Troubleshooting is placed after "Next steps". Other deployment docs put troubleshooting before "Next steps".

---

#### F-086 — Expenses index.js exceeds 980 lines

- **File:** `./plugins/life/expenses/src/index.js`
- **Line(s):** 1-987
- **Dimension:** Code Quality
- **Observation:** Contains view rendering, event handling, service orchestration, and keyboard shortcuts in one class. Could benefit from splitting into controller + view components.

---

#### F-087 — OnboardingPage test page object exceeds 486 lines

- **File:** `./tests/e2e/pages/onboarding.page.ts`
- **Line(s):** 1-486
- **Dimension:** Code Quality
- **Observation:** Largest page object. Could split into base locators class and actions/workflow class.

---

## Technical Debt Markers

- `./apps/app/src/shell/household.js:561` — `// TODO: implement rename API call`
- `./apps/web/scripts/generate-api-docs.mjs:178` — `<!-- TODO: Add description, parameters, request/response examples -->`
- `./tests/e2e/webauthn.spec.ts:67` — `// TODO: Set up Chrome DevTools Protocol virtual authenticator`
- `./tests/e2e/webauthn.spec.ts:77` — `// TODO: Set up virtual authenticator and register a passkey first`
- `./tests/e2e/webauthn.spec.ts:86` — `// TODO: Set up virtual authenticator and register a passkey first`
- `./tests/e2e/webauthn.spec.ts:156` — `// TODO: Set up virtual authenticator with a pre-registered passkey`
- `./tests/e2e/webauthn.spec.ts:166` — `// TODO: Set up scenario where passkey auth fails`
- `./apps/app/tests/schemas/plugin-manifest-schema.test.js:127` — `// TODO: calendar/plugin.json "collections" uses string array`
- `./apps/app/tests/schemas/plugin-manifest-schema.test.js:130` — `// TODO: calendar/plugin.json has "routes" field not defined`

## Suppressed Lint Rules

- `./packages/plugin-sdk-js/tests/types.test.ts` — 27 instances of `@ts-expect-error` (legitimate type error testing)
