<!--
domain: app
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Plugin Loader Requirements

> spec: ./brief.md

## 1 — Manifest Validation

- **1.1** — WHEN a plugin's `plugin.json` is read THEN the system SHALL verify that the required fields (`id`, `name`, `version`, `entry`, `element`, `minShellVersion`, `sidebar`, `capabilities`) are present and non-empty.
- **1.2** — WHEN the `element` field does not contain a hyphen THEN the system SHALL reject the plugin with a clear error message citing the Web Components specification.
- **1.3** — WHEN the `minShellVersion` requires a newer shell than currently installed THEN the system SHALL reject the plugin with an error indicating the version mismatch.
- **1.4** — WHEN the gzipped entry bundle exceeds 2 MB THEN the system SHALL reject the plugin.
- **1.5** — WHEN the gzipped entry bundle exceeds 200 KB but is under 2 MB THEN the system SHALL log a warning but allow loading to proceed.
- **1.6** — WHEN `plugin.json` is missing or contains malformed JSON THEN the system SHALL stop loading and log an error.

## 2 — Install Step

- **2.1** — WHEN a plugin is placed in the `plugins/` directory THEN the system SHALL detect it and begin the loading lifecycle.
- **2.2** — WHEN the plugin directory does not contain both `plugin.json` and a `dist/` folder THEN the system SHALL reject the plugin with a clear error.

## 3 — Capability Approval

- **3.1** — WHEN a plugin is loaded for the first time THEN the system SHALL present the plugin's requested capabilities in a confirmation dialog.
- **3.2** — WHEN the capabilities include high-trust items (`data:write`, `network:fetch`) THEN the system SHALL display explicit warnings alongside those items.
- **3.3** — WHEN the user rejects the capability approval THEN the system SHALL prevent the plugin from loading.
- **3.4** — WHEN the user approves the capabilities THEN the system SHALL persist the approval so the dialog does not appear on subsequent loads.

## 4 — Import Map Registration

- **4.1** — WHEN a plugin declares shared modules in its `sharedModules` array THEN the system SHALL register import map entries pointing to the shell's pre-loaded copies.
- **4.2** — WHEN a shared module is not used by any installed plugin THEN the system SHALL NOT load that module at startup.
- **4.3** — WHEN the shell starts THEN the system SHALL pre-load only shared modules declared by at least one installed plugin.

## 5 — Dynamic Import and Element Definition

- **5.1** — WHEN import maps are registered THEN the system SHALL dynamically import the plugin's entry file via `await import(manifest.entry)`.
- **5.2** — WHEN the entry module executes THEN the system SHALL expect it to call `customElements.define()` to register the plugin's Web Component.
- **5.3** — WHEN the custom element is not defined within 10 seconds THEN the system SHALL abort loading with a timeout error.

## 6 — Scoped API Creation

- **6.1** — WHEN a plugin's element is defined THEN the system SHALL create a `ShellAPI` instance scoped to the plugin's approved capabilities.
- **6.2** — WHEN the scoped API handles data access THEN the system SHALL restrict operations to the plugin's declared collections only.
- **6.3** — WHEN the scoped API handles HTTP requests THEN the system SHALL reject requests to domains not listed in the plugin's `allowedDomains`.
- **6.4** — WHEN the scoped API handles IPC THEN the system SHALL restrict `send` to target plugin IDs declared in the plugin's capabilities.

## 7 — Element Creation and Mount

- **7.1** — WHEN the scoped API is ready THEN the system SHALL create an instance of the custom element and set `el.__shellAPI = scopedAPI` before adding it to the DOM.
- **7.2** — WHEN the element is appended to the plugin container THEN the system SHALL trigger the browser's `connectedCallback` lifecycle.
- **7.3** — WHEN `connectedCallback` fires THEN the plugin SHALL have access to `this.__shellAPI` for data, navigation, toasts, and IPC.

## 8 — Plugin Unloading

- **8.1** — WHEN a plugin is deactivated (navigated away, disabled, or uninstalled) THEN the system SHALL remove the plugin element from the DOM, triggering `disconnectedCallback`.
- **8.2** — WHEN a plugin is unloaded THEN the system SHALL clean up any remaining subscriptions registered via ShellAPI (data, settings, IPC handlers).
- **8.3** — WHEN a plugin is disabled or uninstalled THEN the system SHALL remove its sidebar navigation item.
- **8.4** — WHEN a plugin is uninstalled THEN the system SHALL delete the plugin's files from the `plugins/` directory and prompt the user whether to also delete private data.

## 9 — Sidebar Registration

- **9.1** — WHEN a plugin declares a `sidebar` object in its manifest THEN the system SHALL add a navigation item with the specified `icon` and `label`.
- **9.2** — WHEN multiple plugins declare sidebar items THEN the system SHALL sort them by `order` value ascending.
- **9.3** — WHEN two plugins have the same `order` value THEN the system SHALL sort them alphabetically by `label`.
- **9.4** — WHEN a plugin has no `sidebar` declaration THEN the system SHALL NOT display it in the sidebar and require programmatic activation via IPC.
