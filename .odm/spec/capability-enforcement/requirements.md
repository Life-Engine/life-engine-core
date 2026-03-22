<!--
domain: capability-enforcement
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Requirements Document — Capability Enforcement

## Introduction

The capability enforcement system is the security boundary between plugins and Core host functions. It implements a deny-by-default model where plugins must declare capabilities in their `manifest.toml` and receive approval before loading. At the WASM boundary, only approved host functions are injected, and any invocation of an unapproved host function returns a fatal `EngineError`. This document specifies the load-time approval logic, host function injection rules, runtime enforcement, error handling, and config format for the capability system.

## Alignment with Product Vision

- **Defence in depth** — Capabilities are enforced at two layers: load-time (refuse to load unapproved plugins) and runtime (host function invocation returns error if not granted).
- **Principle of least privilege** — Plugins receive only the host functions they declared and were approved for. No implicit grants.
- **Explicit over implicit** — Approval is config-based (`config.toml`), not interactive. No install dialogs or user prompts.
- **Developer clarity** — Fatal `EngineError` with plugin ID, attempted capability, and source module enables immediate diagnosis.

## Requirements

### Requirement 1 — Load-Time Capability Approval

**User Story:** As a platform operator, I want Core to validate plugin capabilities before loading so that no unapproved plugin can enter the runtime.

#### Acceptance Criteria

- 1.1. WHEN Core discovers a plugin with a `manifest.toml` THEN it SHALL read the `[capabilities].required` array and compare it against the plugin's approved set.
- 1.2. WHEN the plugin is first-party (located in the monorepo `plugins/` directory) THEN Core SHALL auto-grant all declared capabilities without requiring config entries.
- 1.3. WHEN the plugin is third-party THEN Core SHALL look up `[plugins.<plugin-id>].approved_capabilities` in `config.toml` and grant only those capabilities.
- 1.4. WHEN the manifest declares a capability not present in the approved set THEN Core SHALL refuse to load the plugin, log a warning identifying the unapproved capability, and skip that plugin.
- 1.5. WHEN a third-party plugin has no `[plugins.<plugin-id>]` section in `config.toml` THEN Core SHALL treat it as having an empty approved set and refuse to load it.

---

### Requirement 2 — Host Function Injection

**User Story:** As a plugin author, I want Core to inject only the host functions my plugin was granted so that I know exactly what is available at runtime.

#### Acceptance Criteria

- 2.1. WHEN Core loads a plugin into the Extism WASM runtime THEN it SHALL inject host functions corresponding only to the plugin's approved capabilities.
- 2.2. WHEN a plugin is granted `storage:read` THEN Core SHALL inject the storage read host functions but SHALL NOT inject storage write host functions unless `storage:write` is also granted.
- 2.3. WHEN a plugin is granted `http:outbound` THEN Core SHALL inject the HTTP host functions.
- 2.4. WHEN a plugin is granted `events:emit` THEN Core SHALL inject the event emit host function but SHALL NOT inject the event subscribe host function unless `events:subscribe` is also granted.
- 2.5. WHEN a plugin is granted `config:read` THEN Core SHALL inject the config read host function scoped to the plugin's own config section.
- 2.6. WHEN a capability is not granted THEN the corresponding host functions SHALL NOT be injected into the WASM runtime for that plugin.

---

### Requirement 3 — Runtime Capability Enforcement

**User Story:** As a security-conscious operator, I want every host function call checked at invocation so that a plugin cannot bypass its capability set.

#### Acceptance Criteria

- 3.1. WHEN a plugin calls a host function at runtime THEN Core SHALL check the plugin's approved capability set synchronously before executing any logic.
- 3.2. WHEN the plugin has the required capability THEN the host function SHALL execute normally and return the expected result.
- 3.3. WHEN the plugin does not have the required capability THEN the host function SHALL return a fatal `EngineError` immediately without performing any work.
- 3.4. WHEN a capability check executes THEN it SHALL be synchronous with no async overhead or network call.
- 3.5. WHEN a host function is not injected for a plugin (per Requirement 2) THEN calling it from WASM SHALL result in a WASM-level trap or error before reaching Core's enforcement layer.

---

### Requirement 4 — EngineError for Capability Violations

**User Story:** As a developer, I want capability errors to include enough context for immediate diagnosis so that I do not need to guess which capability is missing.

#### Acceptance Criteria

- 4.1. WHEN a capability violation occurs at runtime THEN Core SHALL return an `EngineError` implementing the `EngineError` trait.
- 4.2. WHEN the `EngineError` is constructed THEN it SHALL include: the plugin ID, the attempted capability (e.g., `storage:write`), and the source module (`capability-enforcement`).
- 4.3. WHEN the `EngineError` severity is queried THEN it SHALL return `Fatal`.
- 4.4. WHEN the `EngineError` code is queried THEN it SHALL return a code in the format `CAP_xxx` (e.g., `CAP_001` for unapproved capability at load time, `CAP_002` for denied host function at runtime).
- 4.5. WHEN a capability violation occurs THEN Core SHALL NOT return a default/empty result or silently succeed — it SHALL always be an explicit error.

---

### Requirement 5 — Config Format for Third-Party Approval

**User Story:** As a system administrator, I want a simple TOML config format for approving third-party plugin capabilities so that approval is explicit and version-controlled.

#### Acceptance Criteria

- 5.1. WHEN a third-party plugin requires approval THEN the administrator SHALL add a section to `config.toml` in the format:
  ```toml
  [plugins.some-third-party]
  approved_capabilities = ["storage:read", "http:outbound"]
  ```
- 5.2. WHEN `approved_capabilities` is an empty array THEN Core SHALL load the plugin with no capabilities (the plugin can only receive `PipelineMessage` input and produce output, with no host function access).
- 5.3. WHEN `approved_capabilities` contains an invalid capability string THEN Core SHALL log a warning and ignore the invalid entry.
- 5.4. WHEN `approved_capabilities` is not present in the plugin's config section THEN Core SHALL treat it as an empty set and refuse to load the plugin if it declares any capabilities.

---

### Requirement 6 — Available Capabilities

**User Story:** As a plugin author, I want a clear set of capability strings so that I know exactly what to declare in my manifest.

#### Acceptance Criteria

- 6.1. The system SHALL recognize the following capability strings:
  - `storage:read` — Read from collections via StorageContext host functions
  - `storage:write` — Write to collections via StorageContext host functions
  - `http:outbound` — Make outbound HTTP requests via HTTP host functions
  - `events:emit` — Emit events into the workflow engine via event host functions
  - `events:subscribe` — Subscribe to events via event host functions
  - `config:read` — Read own config section via config host functions
- 6.2. WHEN a manifest declares a capability string not in the recognized set THEN Core SHALL treat it as an error and refuse to load the plugin.
- 6.3. WHEN a new capability is added to Core THEN it SHALL be added to this recognized set and documented before any plugin can declare it.
