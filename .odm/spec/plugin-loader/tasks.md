<!--
domain: app
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Plugin Loader Tasks

## 1.1 — Manifest Validator
> spec: ./brief.md

Implement plugin manifest validation logic.

- Create `src/plugin-loader/manifest-validator.ts` with validation for required fields (`id`, `name`, `version`, `entry`, `element`, `minShellVersion`, `sidebar`, `capabilities`)
- Add element name hyphen check per Web Components specification
- Add `minShellVersion` semver comparison against the current shell version

> estimate: 25 min

## 1.2 — Bundle Size Check
> spec: ./brief.md

Implement gzipped bundle size validation.

- Add function to compute gzipped size of the plugin entry bundle
- Reject plugins over 2 MB with a clear error
- Log a warning for plugins over 200 KB but allow loading

> estimate: 15 min
> depends: 1.1

## 2.1 — Dynamic Import Loader
> spec: ./brief.md

Implement the dynamic import and element definition waiting.

- Create `src/plugin-loader/loader.ts` that calls `await import(manifest.entry)` after import maps are registered
- Add `customElements.whenDefined(manifest.element)` with a 10-second timeout
- Handle timeout by aborting load and logging an error

> estimate: 20 min
> depends: 1.1

## 2.2 — Capability Approval Dialog
> spec: ./brief.md

Implement the capability approval UI.

- Create `src/plugin-loader/capability-dialog.ts` Lit component that displays requested capabilities
- Add explicit warning badges for high-trust capabilities (`data:write`, `network:fetch`)
- Persist user approval to `storage:local` so the dialog does not reappear on subsequent loads
- Block loading if the user rejects

> estimate: 25 min
> depends: 1.1

## 3.1 — Shared Module Registry
> spec: ./brief.md

Implement shared module hosting via import maps.

- Create `src/plugin-loader/shared-modules.ts` that scans all installed plugin manifests for `sharedModules` declarations
- Pre-load only modules that at least one plugin declares
- Register import map entries pointing each shared module to the shell's pre-loaded copy

> estimate: 25 min

## 3.2 — Import Map Registration
> spec: ./brief.md

Wire import map entries into the browser before plugin entry import.

- Add import map injection logic that runs before any dynamic plugin import
- Support `lit` and `react`/`react-dom` as built-in shared modules
- Ensure plugins that declare a shared module resolve their imports to the shell's copy

> estimate: 20 min
> depends: 3.1

## 4.1 — Scoped ShellAPI Factory
> spec: ./brief.md

Implement per-plugin scoped API creation.

- Create `src/plugin-loader/scoped-api.ts` that wraps the global ShellAPI with capability-based restrictions
- Lock data access to the plugin's declared collections
- Lock HTTP requests to the plugin's `allowedDomains`
- Lock IPC `send` to declared target plugin IDs

> estimate: 30 min

## 4.2 — Plugin Container and Mount
> spec: ./brief.md

Implement element creation, API injection, and DOM mounting.

- Create the custom element instance and set `el.__shellAPI = scopedAPI` before DOM insertion
- Append the element to the plugin container div, triggering `connectedCallback`
- Verify `__shellAPI` is accessible within `connectedCallback`

> estimate: 20 min
> depends: 2.1, 4.1

## 5.1 — Unloading and Cleanup
> spec: ./brief.md

Implement plugin teardown on deactivation, disable, and uninstall.

- Remove the plugin element from DOM, triggering `disconnectedCallback`
- Clean up any remaining ShellAPI subscriptions (data, settings, IPC handlers) that the plugin registered
- Remove the sidebar navigation item for disabled or uninstalled plugins

> estimate: 25 min
> depends: 4.2

## 5.2 — Uninstall File Cleanup
> spec: ./brief.md

Implement plugin file deletion and data prompt on uninstall.

- Delete the plugin's directory from `plugins/` on uninstall
- Prompt the user whether to also delete the plugin's private collection data
- If confirmed, delete all records in the plugin's private collections

> estimate: 20 min
> depends: 5.1

## 6.1 — Sidebar Registration
> spec: ./brief.md

Implement sidebar item management.

- Create `src/plugin-loader/sidebar-manager.ts` that reads `sidebar` declarations from all active plugin manifests
- Sort items by `order` ascending, then alphabetically by `label` for ties
- Add navigation items with configured `icon` and `label`
- Exclude plugins without a `sidebar` declaration

> estimate: 20 min
> depends: 1.1

## 7.1 — Lifecycle Orchestrator
> spec: ./brief.md

Wire all 11 steps into a single sequential loader.

- Create `src/plugin-loader/lifecycle.ts` that executes: install detection, read manifest, validate, capability approval, register import maps, dynamic import, await element, create scoped API, create element, inject API, mount
- Abort and log at any step failure with a clear error message
- Mark the plugin as "running" on successful completion

> estimate: 25 min
> depends: 1.1, 1.2, 2.1, 2.2, 3.2, 4.1, 4.2, 6.1

## 7.2 — Integration Testing
> spec: ./brief.md

Verify end-to-end plugin loading and unloading.

- Test that a valid plugin loads through all 11 steps and `connectedCallback` fires with `__shellAPI` available
- Test that an invalid manifest (missing field, no hyphen, incompatible version) is rejected with a clear error
- Test that unloading removes DOM elements, subscriptions, and sidebar items cleanly

> estimate: 25 min
> depends: 7.1, 5.1
