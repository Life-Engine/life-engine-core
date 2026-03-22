<!--
domain: core
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Core Plugin System — Requirements

## 1. Plugin Discovery

- **1.1** — WHEN Core starts, THEN it SHALL scan the configured plugins directory for subdirectories containing `plugin.wasm` and `manifest.toml`.
- **1.2** — WHEN a subdirectory contains both `plugin.wasm` and `manifest.toml`, THEN Core SHALL treat it as a discovered plugin.
- **1.3** — WHEN a subdirectory is missing either `plugin.wasm` or `manifest.toml`, THEN Core SHALL log a warning and skip it without blocking other plugins.
- **1.4** — WHEN a `manifest.toml` fails to parse, THEN Core SHALL log an error with the plugin directory name and skip it.

## 2. Manifest Parsing

- **2.1** — WHEN a `manifest.toml` is read, THEN Core SHALL extract the plugin id, name, version, and description from the `[plugin]` section.
- **2.2** — WHEN a `manifest.toml` is read, THEN Core SHALL extract all declared actions from the `[actions.*]` sections, including each action's description, input schema, and output schema.
- **2.3** — WHEN a `manifest.toml` is read, THEN Core SHALL extract the required capabilities from the `[capabilities]` section.
- **2.4** — WHEN a `manifest.toml` is read, THEN Core SHALL extract the config schema from the `[config]` section if present.
- **2.5** — WHEN a `manifest.toml` is missing the `[plugin]` section or required fields (id, name, version), THEN Core SHALL reject the plugin with a descriptive error.

## 3. WASM Loading and Isolation

- **3.1** — WHEN a plugin passes manifest validation and capability approval, THEN Core SHALL load its `plugin.wasm` binary into a dedicated Extism instance with its own memory sandbox.
- **3.2** — WHEN a plugin executes, THEN it SHALL have no shared memory with the host process or other plugins.
- **3.3** — WHEN a plugin attempts direct filesystem, network, or OS access, THEN the WASM runtime SHALL deny the operation.
- **3.4** — WHEN a plugin panics or traps, THEN the Extism runtime SHALL contain the failure and Core SHALL remain operational.
- **3.5** — WHEN a plugin exceeds its memory or execution-time budget, THEN Core SHALL terminate the plugin instance and log the event.

## 4. Plugin Lifecycle

- **4.1** — WHEN Core scans the plugins directory, THEN it SHALL transition each valid plugin to the Discover state.
- **4.2** — WHEN a discovered plugin passes capability approval, THEN Core SHALL load its WASM binary into the Extism runtime (Load state).
- **4.3** — WHEN a plugin is loaded, THEN Core SHALL call its init function with scoped config and capabilities (Init state).
- **4.4** — WHEN init completes successfully, THEN the plugin's actions SHALL become available to the workflow engine (Running state).
- **4.5** — WHEN Core initiates shutdown or a plugin is disabled, THEN Core SHALL call the plugin's stop function (Stop state).
- **4.6** — WHEN stop completes, THEN Core SHALL release the WASM instance and remove the plugin's actions from the workflow engine (Unload state).

## 5. Capability Enforcement

- **5.1** — WHEN a first-party plugin (shipped in the monorepo) is discovered, THEN its declared capabilities SHALL be auto-granted.
- **5.2** — WHEN a third-party plugin is discovered, THEN its declared capabilities SHALL be checked against the explicit approval list in Core config.
- **5.3** — WHEN a third-party plugin's manifest declares a capability not in the approved list, THEN Core SHALL refuse to load the plugin and log a warning.
- **5.4** — WHEN a plugin calls a storage host function, THEN Core SHALL verify the plugin holds `storage:read` or `storage:write` before executing.
- **5.5** — WHEN a plugin calls the HTTP host function, THEN Core SHALL verify the plugin holds `http:outbound` before executing.
- **5.6** — WHEN a plugin attempts to emit an event, THEN Core SHALL verify the plugin holds `events:emit`.
- **5.7** — WHEN a plugin attempts to subscribe to events, THEN Core SHALL verify the plugin holds `events:subscribe`.
- **5.8** — WHEN a plugin calls the config host function, THEN Core SHALL verify the plugin holds `config:read`.
- **5.9** — WHEN any capability check fails, THEN the host function SHALL return an error to the plugin without executing the operation.

## 6. Host Functions

- **6.1** — WHEN Core initialises the Extism runtime for a plugin, THEN it SHALL register only the host functions matching the plugin's approved capabilities.
- **6.2** — WHEN a storage host function is called, THEN it SHALL route through the `StorageBackend` trait.
- **6.3** — WHEN the config host function is called, THEN it SHALL return only the calling plugin's configuration section.
- **6.4** — WHEN the logging host function is called, THEN it SHALL tag log entries with the calling plugin's ID and forward to the structured logger.
- **6.5** — WHEN the HTTP host function is called with an approved capability, THEN it SHALL execute the request and return the response to the plugin.
- **6.6** — WHEN the events host function is called, THEN it SHALL route through the workflow engine's event bus.

## 7. Plugin Actions and Execution

- **7.1** — WHEN a workflow step references a plugin action, THEN the workflow engine SHALL call the plugin's `execute` function with the action name and a `PipelineMessage`.
- **7.2** — WHEN a plugin action completes, THEN it SHALL return a `PipelineMessage` as output.
- **7.3** — WHEN a plugin action returns an error, THEN the workflow engine SHALL handle it according to the workflow's error handling configuration (retry, fallback, or abort).

## 8. Plugin-to-Plugin Communication

- **8.1** — WHEN two plugins are chained in a workflow, THEN the output `PipelineMessage` of step N SHALL be passed as the input `PipelineMessage` to step N+1.
- **8.2** — WHEN two plugins declare access to the same canonical collection, THEN both SHALL be able to read and write records in that collection independently.
- **8.3** — WHEN a plugin attempts a direct function call to another plugin, THEN the system SHALL deny it; all communication goes through Core.

## 9. Community Plugin Support

- **9.1** — WHEN a third-party plugin directory is placed in the configured plugins path, THEN Core SHALL discover it using the same mechanism as first-party plugins.
- **9.2** — WHEN a third-party plugin is discovered, THEN Core SHALL require explicit capability approval in config before loading.
- **9.3** — WHEN a third-party plugin is approved, THEN it SHALL have access to the same host functions as first-party plugins, scoped to its approved capabilities.
