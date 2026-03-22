<!--
domain: app
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Spec — Plugin Loader

Reference: [[03 - Projects/Life Engine/Design/App/Architecture/Plugin Loading Lifecycle]]

## Purpose

This spec defines the 11-step plugin loading lifecycle, manifest validation rules, shared module hosting, and plugin unloading behaviour. It is the implementor contract for all code that loads, validates, mounts, and removes plugins.

## Manifest Validation

Before a plugin enters the loading lifecycle, the shell validates its `plugin.json` manifest. Validation failures prevent loading entirely.

- **minShellVersion** — Must be compatible with the current shell version. Uses semver comparison. If the plugin requires a newer shell than installed, loading is rejected with a clear error message.
- **Bundle size** — The gzipped size of the plugin's entry bundle (excluding shared modules) is checked. Warn if over 200KB. Reject if over 2MB.
- **Element name** — Must contain a hyphen, per the Web Components specification (e.g. `todo-plugin`, not `todoplugin`). Names without a hyphen are rejected.
- **Required fields** — The following fields must be present and non-empty: `id`, `name`, `version`, `entry`, `element`, `minShellVersion`, `sidebar`, `capabilities`. Missing fields cause rejection.

## 11-Step Loading Lifecycle

1. **Install** — The user installs a plugin by placing its files in the `plugins/` directory or via the plugin store (future). The plugin directory must contain a `plugin.json` manifest and a `dist/` folder with the entry bundle.

2. **Read manifest** — The shell reads `plugin.json` from the plugin's directory. If the file is missing or malformed JSON, loading stops with an error.

3. **Validate manifest** — The shell runs all validation checks (size, version compatibility, element name, required fields). Failures are logged and the plugin is marked as invalid in the plugin management UI.

4. **Capability approval** — The shell presents the plugin's requested capabilities to the user in a confirmation dialog. High-trust capabilities (`data:write`, `network:fetch`) are shown with explicit warnings. The user must approve to continue. Rejection prevents installation.

5. **Register import maps** — For each entry in the manifest's `sharedModules` array, the shell registers an import map entry pointing to the shell's pre-loaded copy of that module. This ensures the plugin uses the shared instance rather than bundling its own.

6. **Dynamic import** — The shell dynamically imports the plugin's entry file: `await import(manifest.entry)`. This executes the plugin's module code, which must call `customElements.define()` to register its Web Component.

7. **Await element definition** — The shell waits for the custom element to be defined: `await customElements.whenDefined(manifest.element)`. A timeout of 10 seconds applies. If the element is not defined in time, loading fails.

8. **Create scoped API** — The shell creates a `ShellAPI` instance scoped to this plugin. The API only permits operations allowed by the plugin's approved capabilities. Data access is locked to declared collections, HTTP access is locked to declared domains, IPC is locked to declared targets.

9. **Create and inject** — The shell creates an instance of the custom element and injects the scoped API: `el.__shellAPI = scopedAPI`. This happens before the element is added to the DOM, so the API is available when `connectedCallback` fires.

10. **Mount** — The shell appends the element to the plugin container: `pluginContainer.appendChild(el)`. This triggers the browser's `connectedCallback` lifecycle.

11. **Running** — `connectedCallback` fires. The plugin is now running and can use `this.__shellAPI` to access data, navigate, show toasts, and interact with other plugins.

## Shared Module Hosting

The shell pre-loads shared modules at startup to avoid duplication across plugins.

- Shared modules (e.g. `lit`, `react`, `react-dom`) are loaded once at shell startup and registered in the import map.
- Only modules that at least one installed plugin declares as a dependency are loaded. Unused shared modules are not loaded.
- Plugins that declare a shared module in their `sharedModules` array will resolve imports to the shell's pre-loaded copy. Plugins must not bundle these modules themselves.
- The shell ships with hosting support for `lit` and `react`/`react-dom`. Additional shared modules can be added in future versions.

## Plugin Unloading

When a plugin is deactivated (navigated away from, disabled, or uninstalled):

- The plugin element is removed from the DOM. This triggers `disconnectedCallback`, which the plugin should use to clean up timers, event listeners, and subscriptions.
- The shell cleans up any remaining subscriptions the plugin registered via the ShellAPI (data subscriptions, settings subscriptions, IPC handlers).
- The plugin's sidebar navigation item is removed (if disabled or uninstalled).
- For uninstallation, the plugin's files are deleted from the `plugins/` directory. The user is prompted whether to also delete the plugin's private data.

## Sidebar Registration

Plugins declare sidebar presence in their manifest:

- The `sidebar` object in `plugin.json` contains `icon` (SVG path or built-in icon name), `label` (display text), and `order` (numeric sort position).
- Plugins without a `sidebar` declaration do not appear in the sidebar and can only be activated programmatically via IPC from another plugin.
- The shell sorts sidebar items by `order` value (ascending). Items with the same order are sorted alphabetically by label.

## Acceptance Criteria

- A valid plugin loads successfully through all 11 steps and its `connectedCallback` fires with `__shellAPI` available.
- A plugin with an invalid manifest (missing fields, incompatible version, no hyphen in element name) is rejected with a clear error.
- A plugin over 2MB (gzipped) is rejected. A plugin over 200KB shows a warning but loads.
- Shared modules are loaded once and shared across all plugins that declare them.
- Unloading removes the element from DOM, cleans up subscriptions, and removes the sidebar item.
- Capability approval dialog appears before first load, with warnings for high-trust capabilities.
