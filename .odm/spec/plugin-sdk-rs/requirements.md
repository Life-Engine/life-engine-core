<!--
domain: sdk
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Plugin SDK RS — Requirements

## 1 — CorePlugin Trait

- **1.1** — WHEN a plugin author implements `CorePlugin` on a struct, THEN the compiler SHALL enforce that all eight methods (`id`, `display_name`, `version`, `capabilities`, `on_load`, `on_unload`, `routes`, `handle_event`) are implemented.
- **1.2** — WHEN `id()` is called, THEN it SHALL return a non-empty string in reverse-domain format (e.g. `com.life-engine.todos`).
- **1.3** — WHEN `version()` is called, THEN it SHALL return a valid semver string.
- **1.4** — WHEN `capabilities()` is called, THEN it SHALL return the full list of capabilities the plugin requires; Core uses this list for capability enforcement.
- **1.5** — WHEN `on_load()` is called with a `PluginContext`, THEN the plugin SHALL perform initialisation and return `Ok(())` on success or an error describing the failure.
- **1.6** — WHEN `on_unload()` is called, THEN the plugin SHALL release resources and return `Ok(())` on success.
- **1.7** — WHEN `routes()` is called, THEN it SHALL return a `Vec<PluginRoute>` that Core mounts under `/api/plugins/{plugin-id}/`.
- **1.8** — WHEN `handle_event()` is called with a `CoreEvent`, THEN the plugin SHALL process the event and return `Ok(())` or an error.

## 2 — PluginContext

- **2.1** — WHEN `on_load` is called, THEN the `PluginContext` SHALL provide scoped storage access limited to the plugin's own namespace.
- **2.2** — WHEN a plugin reads config via `PluginContext`, THEN it SHALL receive only its own configuration values.
- **2.3** — WHEN a plugin subscribes to events via `PluginContext`, THEN only events matching declared capabilities SHALL be delivered.
- **2.4** — WHEN a plugin logs via `PluginContext`, THEN log entries SHALL be tagged with the plugin's ID automatically.
- **2.5** — WHEN a plugin attempts an operation outside its granted capabilities, THEN the `PluginContext` SHALL return an error.

## 3 — Capability Enum

- **3.1** — WHEN the `Capability` enum is defined, THEN it SHALL include at minimum: `StorageRead`, `StorageWrite`, `HttpOutbound`, `CredentialsRead`, `CredentialsWrite`, `EventsSubscribe`, `EventsEmit`, `ConfigRead`, `Logging`.
- **3.2** — WHEN a new capability is added in a minor release, THEN existing plugins that do not use it SHALL continue to compile without changes.
- **3.3** — WHEN `Capability` values are serialised (for manifests), THEN each variant SHALL map to a colon-delimited string (e.g. `StorageRead` to `storage:read`).

## 4 — Route Registration

- **4.1** — WHEN a `PluginRoute` is constructed, THEN it SHALL specify an HTTP method, a relative path, and a handler function.
- **4.2** — WHEN Core mounts plugin routes, THEN each route SHALL be prefixed with `/api/plugins/{plugin-id}/`.
- **4.3** — WHEN a plugin is unloaded, THEN its routes SHALL be unmounted and return 404.

## 5 — Canonical Collection Types

- **5.1** — WHEN the SDK is compiled, THEN it SHALL include Rust struct definitions for all 7 canonical types: Event, Task, Contact, Note, Email, File, Credential.
- **5.2** — WHEN a canonical struct is serialised with serde, THEN it SHALL produce valid JSON matching the platform schema.
- **5.3** — WHEN a canonical struct is deserialised from JSON, THEN unknown fields SHALL be preserved in an `extensions` map.
- **5.4** — WHEN a canonical struct includes an `id` field, THEN it SHALL be typed as `String` and required.
- **5.5** — WHEN a canonical struct includes timestamp fields (`_created`, `_updated`), THEN they SHALL be ISO 8601 / RFC 3339 strings.

## 6 — WASM Target

- **6.1** — WHEN the SDK crate is compiled with `--target wasm32-wasi`, THEN it SHALL produce a valid WASM module with no host-specific dependencies.
- **6.2** — WHEN the resulting `.wasm` binary is loaded by Extism, THEN it SHALL expose the expected entry points for lifecycle and route dispatch.
- **6.3** — WHEN the WASM module executes, THEN it SHALL have no direct filesystem, network, or OS access outside of host-provided functions.

## 7 — Builder Pattern

- **7.1** — WHEN `PluginBuilder::new(id)` is called, THEN it SHALL accept a plugin ID string and return a builder instance.
- **7.2** — WHEN the builder is used, THEN `display_name`, `version`, and at least one capability SHALL be required before `build()` succeeds.
- **7.3** — WHEN `build()` is called with missing required fields, THEN it SHALL return a descriptive error (not panic).
- **7.4** — WHEN the builder adds routes via `.route(method, path, handler)`, THEN each route SHALL be included in the built plugin's `routes()` output.
- **7.5** — WHEN the builder adds capabilities via `.capability(cap)`, THEN each capability SHALL be included in the built plugin's `capabilities()` output.

## 8 — Versioning

- **8.1** — WHEN the SDK ships a v1.x minor release, THEN it SHALL contain only additive changes; no removals or breaking signature changes.
- **8.2** — WHEN a v2.x release ships, THEN v1.x SHALL continue to receive security fixes for 12 months.
- **8.3** — WHEN Core loads a plugin, THEN it SHALL check the SDK version declared in the WASM manifest and reject plugins built against unsupported major versions.
