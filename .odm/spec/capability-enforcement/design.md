<!--
domain: capability-enforcement
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Spec — Capability Enforcement

Reference: [Plugins Design Document](../../doc/Design/Core/Plugins.md)

## Purpose

This spec defines the capability system that governs plugin access to Core host functions. Capabilities are declared in the plugin's `manifest.toml`, approved via config policy, and enforced at the WASM boundary. It is the implementor contract for all capability checking, host function injection, and error handling logic.

## Principle

Deny by default. A plugin receives no access to any host function unless it explicitly declares the capability in its manifest and the capability is approved. First-party plugins are auto-granted. Third-party plugins require explicit approval in `config.toml`. There are no install dialogs, no interactive prompts, and no implicit grants.

## Available Capabilities

The following capability strings are recognized by Core:

- `storage:read` — Read from collections via StorageContext host functions. The plugin can execute read queries through the storage query builder.
- `storage:write` — Write to collections via StorageContext host functions. The plugin can execute mutations (create, update, delete) through the storage query builder.
- `http:outbound` — Make outbound HTTP requests via HTTP host functions. The plugin can call external APIs and services.
- `events:emit` — Emit events into the workflow engine. The plugin can produce events that trigger other workflows.
- `events:subscribe` — Subscribe to events from the workflow engine. The plugin can register interest in specific event types.
- `config:read` — Read the plugin's own config section from `config.toml`. The plugin can access its configuration values at runtime.

Each capability maps directly to a set of host functions. A plugin only receives the host functions corresponding to its approved capabilities.

## Approval Policy

### First-Party Plugins

Plugins located in the monorepo `plugins/` directory are first-party. All declared capabilities are auto-granted at load time. No config entry is required.

First-party plugins are trusted because they are developed, reviewed, and shipped as part of the same codebase. The monorepo path acts as the trust boundary.

### Third-Party Plugins

Plugins not in the monorepo require explicit approval in `config.toml`:

```toml
[plugins.some-third-party]
approved_capabilities = ["storage:read", "http:outbound"]
```

Core compares the manifest's declared capabilities against the `approved_capabilities` array. If the manifest declares any capability not in the approved list, Core refuses to load the plugin and logs a warning identifying each unapproved capability.

If a third-party plugin has no `[plugins.<plugin-id>]` section, Core treats it as having an empty approved set and refuses to load it if it declares any capabilities.

## Host Function Injection

When Core loads a plugin into the Extism WASM runtime, it constructs the set of host functions to inject based on the plugin's approved capabilities:

- `storage:read` approved — inject storage read host functions (`storage_query`)
- `storage:write` approved — inject storage write host functions (`storage_mutate`)
- `http:outbound` approved — inject HTTP host functions (`http_request`)
- `events:emit` approved — inject event emit host function (`event_emit`)
- `events:subscribe` approved — inject event subscribe host function (`event_subscribe`)
- `config:read` approved — inject config read host function (`config_get`), scoped to the plugin's own section

Host functions not corresponding to an approved capability are not registered in the plugin's WASM instance. This provides a structural guarantee: the plugin literally cannot call a function that does not exist in its runtime.

## Runtime Enforcement

Even though unapproved host functions are not injected, a second enforcement layer exists at the host function level. Every host function checks the calling plugin's capability set synchronously before executing:

- If the plugin has the required capability, the function executes normally.
- If the plugin does not have the required capability, the function returns a fatal `EngineError` immediately without performing any work.

This two-layer approach (injection gating + runtime checking) follows the defence-in-depth principle. The injection layer prevents calls structurally; the runtime layer catches any edge case where a host function is shared across capabilities or invoked through an unexpected path.

Enforcement is synchronous. There is no async overhead, no network call, and no external lookup. The approved capability set is held in memory for each loaded plugin.

## EngineError for Capability Violations

Capability violations produce an error implementing the `EngineError` trait:

```rust
// Example error
CapabilityViolation {
    plugin_id: "com.example.weather",
    attempted_capability: "storage:write",
    source_module: "capability-enforcement",
}
```

The error fields:

- `code()` — Returns `CAP_001` for load-time violations (unapproved capability preventing load) or `CAP_002` for runtime violations (denied host function call).
- `severity()` — Always `Fatal`. Capability violations are not retryable or ignorable.
- `source_module()` — Always `capability-enforcement`.

The error message follows the format:

```text
CapabilityViolation: Plugin "com.example.weather" attempted "storage:write"
  but does not have that capability [CAP_002]
```

Core never returns a default result or silently succeeds on a capability violation. It is always an explicit error.

## Load-Time Flow

When Core discovers a plugin during startup:

1. Read `manifest.toml` and extract the `[capabilities].required` array.
2. Determine if the plugin is first-party (in monorepo `plugins/` directory) or third-party.
3. If first-party, auto-grant all declared capabilities.
4. If third-party, look up `[plugins.<plugin-id>].approved_capabilities` in `config.toml`.
5. Compare declared capabilities against the approved set.
6. If any declared capability is not approved, refuse to load the plugin. Log a `CAP_001` error identifying each unapproved capability.
7. If all capabilities are approved, load the WASM module and inject only the approved host functions.

## Config Format

Third-party approval is declared in `config.toml` under the plugin's section:

```toml
[plugins.some-third-party]
approved_capabilities = ["storage:read", "http:outbound"]
```

Rules:

- The key under `[plugins.*]` matches the plugin's `id` from its `manifest.toml`.
- `approved_capabilities` is an array of capability strings.
- An empty array means the plugin loads with no host functions (it can still receive and return `PipelineMessage` values).
- Invalid capability strings in the array are logged as warnings and ignored.
- A missing `approved_capabilities` key is treated as an empty set.

## Scope

This spec covers Core-only enforcement. There is no App-side capability checking. The WASM boundary is the single enforcement point. All capability logic lives in the plugin loading and host function infrastructure within Core.

## Acceptance Criteria

- A plugin cannot access any host function (storage, HTTP, events, config) without the corresponding capability declared in its manifest and approved by the policy.
- First-party plugins have all declared capabilities auto-granted.
- Third-party plugins require explicit `approved_capabilities` in `config.toml`.
- If a manifest declares an unapproved capability, Core refuses to load the plugin.
- Host functions are injected per-plugin based on the approved capability set.
- Attempting an unapproved host function call returns a fatal `EngineError` with the plugin ID, attempted capability, and `CAP_xxx` error code.
- Runtime capability checks are synchronous with no async overhead.
- Capability violations never result in silent success or default return values.
