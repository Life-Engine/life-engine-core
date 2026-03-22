<!--
domain: core
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Core Plugin System — Tasks

> spec: ./brief.md

## 1.1 — Extism Runtime Setup

> spec: ./brief.md
> depends: none

- Add `extism` crate dependency to `crates/core/Cargo.toml`
- Create `crates/core/src/plugins/runtime.rs` with an `ExtismRuntime` struct
- Implement `load_plugin(path: &Path) -> Result<PluginInstance>` that creates an isolated WASM instance
- Configure memory limits and execution timeouts per plugin instance

**Files:** `crates/core/Cargo.toml`, `crates/core/src/plugins/runtime.rs`
**Est:** 25 min

## 1.2 — Plugin Loader

> spec: ./brief.md
> depends: 1.1

- Create `crates/core/src/plugins/loader.rs` with a `PluginLoader` struct
- Implement `discover(config: &PluginConfig) -> Vec<PluginEntry>` that reads YAML config and resolves paths
- Implement `load(entry: &PluginEntry) -> Result<PluginHandle>` that validates the manifest, checks capabilities against approved list, and loads into Extism
- Handle missing/corrupt binaries gracefully with error logging

**Files:** `crates/core/src/plugins/loader.rs`, `crates/core/src/plugins/mod.rs`
**Est:** 30 min

## 1.3 — Host Function Exports

> spec: ./brief.md
> depends: 1.1

- Create `crates/core/src/plugins/host_functions.rs`
- Implement and register host functions: `host_storage_read`, `host_storage_write`, `host_credentials_read`, `host_credentials_write`, `host_config_read`, `host_events_subscribe`, `host_events_emit`, `host_log`, `host_http_request`
- Each function deserialises the plugin's request, delegates to the appropriate Core service, and serialises the response

**Files:** `crates/core/src/plugins/host_functions.rs`
**Est:** 30 min

## 1.4 — Capability Checker

> spec: ./brief.md
> depends: 1.2

- Create `crates/core/src/plugins/capability.rs`
- Implement `CapabilityChecker` that holds the approved capability set for each plugin
- Implement `check(plugin_id: &str, capability: &Capability, scope: &str) -> Result<()>`
- Integrate with host functions so every host call passes through the checker before execution
- Return structured `CapabilityDenied` errors with plugin ID and requested capability

**Files:** `crates/core/src/plugins/capability.rs`, `crates/core/src/plugins/host_functions.rs`
**Est:** 25 min

## 1.5 — Lifecycle Manager

> spec: ./brief.md
> depends: 1.2, 1.3, 1.4

- Create `crates/core/src/plugins/lifecycle.rs`
- Implement `PluginManager` with methods for `start_all`, `stop_all`, `enable`, `disable`
- Implement the six-phase lifecycle: Discover, Load, Init (call `on_load`), Running (mount routes), Stop (call `on_unload`), Unload (release WASM)
- Integrate with the HTTP router to mount/unmount plugin routes dynamically
- Handle errors at each phase and log plugin state transitions

**Files:** `crates/core/src/plugins/lifecycle.rs`, `crates/core/src/plugins/mod.rs`
**Est:** 30 min

## 1.6 — Config Parser

> spec: ./brief.md
> depends: 1.2

- Extend `crates/core/src/config.rs` with the `plugins` YAML section schema
- Parse `plugins.paths`, `plugins.enabled` entries, and `auto_enable` flag
- Validate that each entry has a valid `id` and `path`
- Expose `PluginConfig` struct for use by the loader

**Files:** `crates/core/src/config.rs`, `crates/core/src/plugins/loader.rs`
**Est:** 20 min
