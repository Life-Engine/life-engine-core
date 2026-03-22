<!--
domain: core
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Core Plugin System — Requirements

## 1. WASM Loading and Isolation

- **1.1** — WHEN Core starts, THEN it SHALL load each enabled plugin's `.wasm` binary into a dedicated Extism instance with its own memory sandbox.
- **1.2** — WHEN a plugin executes, THEN it SHALL have no shared memory with the host process or other plugins.
- **1.3** — WHEN a plugin attempts direct filesystem, network, or OS access, THEN the WASM runtime SHALL deny the operation.
- **1.4** — WHEN a plugin panics or traps, THEN the Extism runtime SHALL contain the failure and Core SHALL remain operational.
- **1.5** — WHEN a plugin exceeds its memory or execution-time budget, THEN Core SHALL terminate the plugin instance and log the event.

## 2. Plugin Lifecycle

- **2.1** — WHEN Core reads the YAML config, THEN it SHALL discover all plugins listed under the `plugins.enabled` section.
- **2.2** — WHEN a plugin is discovered, THEN Core SHALL load its WASM binary and validate its manifest against approved capabilities before proceeding.
- **2.3** — WHEN a plugin's manifest requests unapproved capabilities, THEN Core SHALL refuse to load the plugin and log a warning.
- **2.4** — WHEN a plugin passes validation, THEN Core SHALL call `on_load` with a scoped `PluginContext` to begin initialisation.
- **2.5** — WHEN `on_load` returns `Ok(())`, THEN Core SHALL mount the plugin's routes on the HTTP server and activate event handlers.
- **2.6** — WHEN Core initiates shutdown or a plugin is disabled, THEN Core SHALL call `on_unload` before releasing the WASM instance.
- **2.7** — WHEN `on_unload` completes, THEN Core SHALL unmount the plugin's routes and remove its event subscriptions.

## 3. Capability Enforcement

- **3.1** — WHEN a plugin calls a storage host function, THEN Core SHALL verify the plugin holds `storage:read` or `storage:write` for the target collection before executing.
- **3.2** — WHEN a plugin calls the HTTP host function, THEN Core SHALL verify the target domain is in the plugin's declared `allowedDomains` list.
- **3.3** — WHEN a plugin calls the credentials host function, THEN Core SHALL verify the plugin holds `credentials:read` or `credentials:write` for the credential type.
- **3.4** — WHEN a plugin attempts to emit an event, THEN Core SHALL verify the plugin holds the `events:emit` capability.
- **3.5** — WHEN a plugin attempts to subscribe to events, THEN Core SHALL verify the plugin holds the `events:subscribe` capability.
- **3.6** — WHEN any capability check fails, THEN the host function SHALL return an error to the plugin without executing the operation.

## 4. Host Functions

- **4.1** — WHEN Core initialises the Extism runtime, THEN it SHALL register host functions for storage, credentials, config, events, logging, and HTTP.
- **4.2** — WHEN a storage host function is called, THEN it SHALL route through the same `StorageAdapter` trait used by the REST API.
- **4.3** — WHEN the logging host function is called, THEN it SHALL tag log entries with the calling plugin's ID and forward to the structured logger.
- **4.4** — WHEN the config host function is called, THEN it SHALL return only the calling plugin's configuration section.
- **4.5** — WHEN the HTTP host function is called with an approved domain, THEN it SHALL execute the request and return the response to the plugin.

## 5. Plugin Discovery and Configuration

- **5.1** — WHEN the YAML config lists a plugin under `plugins.enabled`, THEN Core SHALL attempt to load it at startup.
- **5.2** — WHEN a plugin is not listed in `plugins.enabled`, THEN Core SHALL not load it even if its binary exists in a scanned path.
- **5.3** — WHEN `auto_enable` is set to `false` (the default), THEN newly discovered plugins SHALL require explicit enablement before loading.
- **5.4** — WHEN a plugin's WASM binary is missing or corrupt at the configured path, THEN Core SHALL log an error and skip it without blocking other plugins.

## 6. Plugin-to-Plugin Communication

- **6.1** — WHEN two plugins declare access to the same canonical collection, THEN both SHALL be able to read and write records in that collection independently.
- **6.2** — WHEN a plugin emits a Core event, THEN all plugins subscribed to that event type SHALL receive it asynchronously.
- **6.3** — WHEN plugins are chained in a workflow, THEN the output of step N SHALL be passed as input to step N+1.
- **6.4** — WHEN a plugin attempts a direct function call to another plugin, THEN the system SHALL deny it; all communication goes through Core.
