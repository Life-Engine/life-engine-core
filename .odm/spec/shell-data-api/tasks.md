<!--
domain: app
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Shell Data API — Tasks

> spec: ./brief.md

## 1.1 — ShellAPI Factory
> spec: ./brief.md
> depends: none

- Create `packages/shell/src/api/shell-api-factory.js`
- Implement `createScopedAPI(pluginManifest)` that reads the manifest's capabilities and builds a scoped `ShellAPI` object
- Exclude namespace methods for undeclared capabilities (throw `CapabilityError` on access)
- Export the factory for use by the plugin container

**Files:** `packages/shell/src/api/shell-api-factory.js`, `packages/shell/src/api/capability-error.js`
**Est:** 25 min

## 1.2 — Data Namespace Implementation
> spec: ./brief.md
> depends: 1.1

- Create `packages/shell/src/api/data-namespace.js`
- Implement `query(collection, filter)` — build SQL from filter, execute against local SQLite via Tauri IPC
- Implement `create(collection, data)` — insert record, generate `id`, `_version`, `_created`, `_updated`
- Implement `update(collection, id, data)` — merge fields, increment `_version`, update `_updated`
- Implement `delete(collection, id)` — remove record
- Implement `subscribe(collection, callback)` — register a listener, fire immediately with current data, fire on changes

**Files:** `packages/shell/src/api/data-namespace.js`
**Est:** 30 min

## 1.3 — HTTP Proxy
> spec: ./brief.md
> depends: 1.1

- Create `packages/shell/src/api/http-namespace.js`
- Implement `get`, `post`, `put`, `delete` methods that proxy through Tauri's HTTP plugin
- Check the URL's domain against the plugin's `allowedDomains` before each request
- Throw `CapabilityError` for undeclared domains or missing `network:fetch` capability
- Support `options.timeout` via `AbortController`

**Files:** `packages/shell/src/api/http-namespace.js`
**Est:** 20 min

## 1.4 — Storage Adapter
> spec: ./brief.md
> depends: 1.1

- Create `packages/shell/src/api/storage-namespace.js`
- Implement `get(key)`, `set(key, value)`, `delete(key)` using a plugin-scoped key-value store
- JSON-serialise values before persisting
- Scope all keys by plugin ID to prevent cross-plugin access
- Throw `CapabilityError` if the plugin lacks `storage:local`

**Files:** `packages/shell/src/api/storage-namespace.js`
**Est:** 15 min

## 1.5 — Settings Manager
> spec: ./brief.md
> depends: 1.1

- Create `packages/shell/src/api/settings-namespace.js`
- Implement `get(key)`, `set(key, value)`, `subscribe(key, callback)`
- Persist settings in `settings.json` scoped by plugin ID
- Fire subscribe callbacks immediately with current value, then on each subsequent change
- No capability required — always available

**Files:** `packages/shell/src/api/settings-namespace.js`
**Est:** 20 min

## 1.6 — UI Methods
> spec: ./brief.md
> depends: 1.1

- Create `packages/shell/src/api/ui-namespace.js`
- Implement `navigate(route)` and `back()` using the shell router
- Implement `toast(message, variant)` that dispatches to the shell's toast system
- Implement `openModal(element)` and `closeModal()` with `ui:overlay` capability check
- Implement `setTitle(title)` that updates the top bar title

**Files:** `packages/shell/src/api/ui-namespace.js`
**Est:** 20 min

## 1.7 — IPC Bridge
> spec: ./brief.md
> depends: 1.1

- Create `packages/shell/src/api/ipc-namespace.js`
- Implement `send(targetPluginId, event, payload)` with `ipc:send:<target>` capability check
- Implement `on(event, handler)` that registers a listener (no capability required)
- Use a central message bus in the shell to route messages between plugins
- Silently drop messages when the target plugin is not running

**Files:** `packages/shell/src/api/ipc-namespace.js`, `packages/shell/src/api/ipc-bus.js`
**Est:** 25 min

## 1.8 — Capability Wrapper Integration
> spec: ./brief.md
> depends: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7

- Update the plugin container in `packages/shell/src/plugin-container.js`
- Call `createScopedAPI(manifest)` when creating a plugin element
- Set `el.__shellAPI = scopedAPI` before adding the element to the DOM
- Verify that `this.__shellAPI` is available in `connectedCallback` during integration testing

**Files:** `packages/shell/src/plugin-container.js`
**Est:** 15 min
