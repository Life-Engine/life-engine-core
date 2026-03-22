<!--
domain: core
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Core Plugin System — Tasks

> spec: ./brief.md

## Task Overview

This plan implements the Core plugin system: directory-based discovery, manifest parsing, WASM loading via Extism, capability enforcement, host function injection, lifecycle management, and plugin execution. This spec absorbs the previously separate plugin-loader spec — discovery and loading are included here.

**Progress:** 0 / 18 tasks complete

## Steering Document Compliance

- Plugins are WASM modules from day one (via Extism) — no compiled-in Rust traits
- Plugins are loaded at runtime — Core does not compile against any plugin
- Discovery by scanning a configured directory for `plugin.wasm` + `manifest.toml` folders
- Manifest declares plugin identity, actions, capabilities, and config schema
- First-party capabilities auto-granted; third-party requires explicit approval in config
- Host functions gated by approved capabilities
- Plugin-to-plugin communication via workflow chaining and shared collections only

## Atomic Task Requirements

- **File Scope:** 1-3 related files maximum
- **Time Boxing:** 15-30 minutes per task
- **Single Purpose:** one testable outcome per task
- **Specific Files:** exact file paths specified
- **Agent-Friendly:** clear input/output, minimal context switching

---

## 1.1 — Plugin Directory Scanner

> spec: ./brief.md
> depends: none

- [ ] Implement directory scanner that finds plugin subdirectories
  <!-- file: packages/plugin-system/src/discovery.rs -->
  <!-- purpose: Scan configured plugins directory, identify subdirectories containing both plugin.wasm and manifest.toml, return list of discovered plugin paths -->
  <!-- requirements: 1.1, 1.2, 1.3 -->

- [ ] Add directory scanner tests
  <!-- file: packages/plugin-system/src/discovery.rs -->
  <!-- purpose: Test valid plugin directories are discovered, directories missing plugin.wasm or manifest.toml are skipped with warning, empty directory returns empty list -->
  <!-- requirements: 1.1, 1.2, 1.3 -->

---

## 1.2 — Manifest Parser

> spec: ./brief.md
> depends: 1.1

- [ ] Implement manifest.toml parser with struct definitions
  <!-- file: packages/plugin-system/src/manifest.rs -->
  <!-- purpose: Define PluginManifest, ActionDef, CapabilitySet, ConfigSchema structs; parse manifest.toml into PluginManifest; validate required fields (id, name, version) -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.4, 2.5 -->

- [ ] Add manifest parser tests
  <!-- file: packages/plugin-system/src/manifest.rs -->
  <!-- purpose: Test valid manifest parses correctly, missing [plugin] section rejected, missing required fields rejected, actions and capabilities extracted, config schema optional -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.4, 2.5 -->

---

## 1.3 — Capability Types and Approval Policy

> spec: ./brief.md
> depends: 1.2

- [ ] Define capability enum and approval checker
  <!-- file: packages/plugin-system/src/capability.rs -->
  <!-- purpose: Define Capability enum (StorageRead, StorageWrite, HttpOutbound, EventsEmit, EventsSubscribe, ConfigRead) with Display/FromStr; implement approval checker that auto-grants first-party and checks config for third-party -->
  <!-- requirements: 5.1, 5.2, 5.3 -->

- [ ] Add capability approval tests
  <!-- file: packages/plugin-system/src/capability.rs -->
  <!-- purpose: Test first-party auto-grant, third-party approved capabilities pass, third-party unapproved capability triggers rejection, Display/FromStr round-trip -->
  <!-- requirements: 5.1, 5.2, 5.3 -->

---

## 1.4 — Extism Runtime Setup

> spec: ./brief.md
> depends: none

- [ ] Add extism crate dependency and create runtime wrapper
  <!-- file: packages/plugin-system/Cargo.toml -->
  <!-- file: packages/plugin-system/src/runtime.rs -->
  <!-- purpose: Add extism dependency; create ExtismRuntime struct with load_plugin(wasm_path) -> Result<PluginInstance> that creates an isolated WASM instance with memory limits and execution timeouts -->
  <!-- requirements: 3.1, 3.2, 3.5 -->

- [ ] Add runtime loading tests
  <!-- file: packages/plugin-system/src/runtime.rs -->
  <!-- purpose: Test WASM binary loads into isolated instance, corrupt binary returns error, memory isolation is enforced -->
  <!-- requirements: 3.1, 3.2, 3.4 -->

---

## 1.5 — Host Function Registration — Storage

> spec: ./brief.md
> depends: 1.4, 1.3

- [ ] Implement storage host functions
  <!-- file: packages/plugin-system/src/host_functions/storage.rs -->
  <!-- purpose: Implement host_storage_read and host_storage_write functions; deserialise plugin request, check storage capability, delegate to StorageBackend, serialise response -->
  <!-- requirements: 5.4, 6.2 -->

- [ ] Add storage host function tests
  <!-- file: packages/plugin-system/src/host_functions/storage.rs -->
  <!-- purpose: Test read succeeds with storage:read capability, write succeeds with storage:write, missing capability returns error, StorageBackend delegation works -->
  <!-- requirements: 5.4, 5.9, 6.2 -->

---

## 1.6 — Host Function Registration — Config

> spec: ./brief.md
> depends: 1.4, 1.3

- [ ] Implement config host function
  <!-- file: packages/plugin-system/src/host_functions/config.rs -->
  <!-- purpose: Implement host_config_read that returns only the calling plugin's config section; check config:read capability before executing -->
  <!-- requirements: 5.8, 6.3 -->

- [ ] Add config host function tests
  <!-- file: packages/plugin-system/src/host_functions/config.rs -->
  <!-- purpose: Test config read returns plugin-specific section only, missing capability returns error, nonexistent config section returns empty -->
  <!-- requirements: 5.8, 5.9, 6.3 -->

---

## 1.7 — Host Function Registration — Events

> spec: ./brief.md
> depends: 1.4, 1.3

- [ ] Implement events host functions
  <!-- file: packages/plugin-system/src/host_functions/events.rs -->
  <!-- purpose: Implement host_events_emit and host_events_subscribe; check events:emit and events:subscribe capabilities; route through workflow engine event bus -->
  <!-- requirements: 5.6, 5.7, 6.6 -->

- [ ] Add events host function tests
  <!-- file: packages/plugin-system/src/host_functions/events.rs -->
  <!-- purpose: Test emit succeeds with events:emit capability, subscribe succeeds with events:subscribe, missing capability returns error -->
  <!-- requirements: 5.6, 5.7, 5.9, 6.6 -->

---

## 1.8 — Host Function Registration — HTTP

> spec: ./brief.md
> depends: 1.4, 1.3

- [ ] Implement HTTP host function
  <!-- file: packages/plugin-system/src/host_functions/http.rs -->
  <!-- purpose: Implement host_http_request; check http:outbound capability; execute outbound HTTP request and return response to plugin -->
  <!-- requirements: 5.5, 6.5 -->

- [ ] Add HTTP host function tests
  <!-- file: packages/plugin-system/src/host_functions/http.rs -->
  <!-- purpose: Test request succeeds with http:outbound capability, missing capability returns error, response is serialised back to plugin -->
  <!-- requirements: 5.5, 5.9, 6.5 -->

---

## 1.9 — Host Function Registration — Logging

> spec: ./brief.md
> depends: 1.4

- [ ] Implement logging host function
  <!-- file: packages/plugin-system/src/host_functions/logging.rs -->
  <!-- purpose: Implement host_log that tags log entries with calling plugin's ID and forwards to structured logger; no capability required -->
  <!-- requirements: 6.4 -->

---

## 2.1 — Plugin Loader

> spec: ./brief.md
> depends: 1.1, 1.2, 1.3, 1.4

- [ ] Implement plugin loader that orchestrates discovery through loading
  <!-- file: packages/plugin-system/src/loader.rs -->
  <!-- purpose: Orchestrate the full loading flow: scan directory, parse manifest, check capability approval, load WASM into Extism, register host functions matching approved capabilities; return PluginHandle or skip with error log -->
  <!-- requirements: 1.1, 1.2, 1.3, 1.4, 2.1, 2.5, 3.1, 5.1, 5.2, 5.3 -->

- [ ] Add plugin loader integration tests
  <!-- file: packages/plugin-system/src/loader.rs -->
  <!-- purpose: Test full loading flow with valid plugin directory, test skip on missing manifest, test rejection on unapproved capability, test skip on corrupt WASM -->
  <!-- requirements: 1.3, 1.4, 3.1, 5.3 -->

---

## 2.2 — Lifecycle Manager

> spec: ./brief.md
> depends: 2.1

- [ ] Implement lifecycle manager with six-phase state machine
  <!-- file: packages/plugin-system/src/lifecycle.rs -->
  <!-- purpose: Implement PluginManager with start_all, stop_all, and per-plugin state tracking; enforce Discover -> Load -> Init -> Running -> Stop -> Unload transitions; log state changes -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6 -->

- [ ] Add lifecycle manager tests
  <!-- file: packages/plugin-system/src/lifecycle.rs -->
  <!-- purpose: Test six-phase lifecycle transitions, test start_all loads and inits all discovered plugins, test stop_all stops and unloads, test invalid state transitions are rejected -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6 -->

---

## 2.3 — Plugin Execution Bridge

> spec: ./brief.md
> depends: 2.2

- [ ] Implement execute bridge between workflow engine and plugin WASM
  <!-- file: packages/plugin-system/src/execute.rs -->
  <!-- purpose: Implement execute(plugin_id, action, PipelineMessage) -> Result<PipelineMessage> that serialises input, calls the plugin's WASM execute function, deserialises output; validate action exists in manifest -->
  <!-- requirements: 7.1, 7.2, 7.3 -->

- [ ] Add plugin execution tests
  <!-- file: packages/plugin-system/src/execute.rs -->
  <!-- purpose: Test execute with valid action returns PipelineMessage, test unknown action returns error, test plugin error propagates correctly -->
  <!-- requirements: 7.1, 7.2, 7.3 -->

---

## 2.4 — Plugin Config Section Parser

> spec: ./brief.md
> depends: 1.2

- [ ] Extend Core config to parse plugin-specific sections
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Parse [plugins] section with path and per-plugin config; parse [plugins.<id>] sections for approved_capabilities and plugin-specific settings; expose PluginConfig struct for use by the loader -->
  <!-- requirements: 5.2, 6.3 -->

- [ ] Add plugin config parsing tests
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Test plugins path is parsed, per-plugin approved_capabilities are extracted, plugin-specific config values are accessible, missing plugins section uses defaults -->
  <!-- requirements: 5.2, 6.3 -->

---

## 3.1 — Crash Isolation Verification

> spec: ./brief.md
> depends: 2.3

- [ ] Add crash isolation integration tests
  <!-- file: packages/plugin-system/tests/crash_isolation.rs -->
  <!-- purpose: Load a test WASM plugin that panics; verify Core remains operational; verify error is logged with plugin ID; verify other plugins continue running -->
  <!-- requirements: 3.3, 3.4, 3.5 -->

---

## 3.2 — Capability Enforcement Integration Tests

> spec: ./brief.md
> depends: 2.3

- [ ] Add end-to-end capability enforcement tests
  <!-- file: packages/plugin-system/tests/capability_enforcement.rs -->
  <!-- purpose: Load a test plugin with limited capabilities; verify approved host functions succeed; verify unapproved host function calls return CapabilityDenied error; verify third-party plugin with unapproved manifest capability is rejected at load time -->
  <!-- requirements: 5.3, 5.4, 5.5, 5.6, 5.7, 5.8, 5.9 -->

---

## 3.3 — Plugin-to-Plugin Communication Tests

> spec: ./brief.md
> depends: 2.3

- [ ] Add workflow chaining and shared collection tests
  <!-- file: packages/plugin-system/tests/communication.rs -->
  <!-- purpose: Test two plugins chained in a workflow where output of step 1 is input to step 2; test two plugins reading/writing the same canonical collection; verify direct plugin-to-plugin calls are not possible -->
  <!-- requirements: 8.1, 8.2, 8.3 -->

---

## 3.4 — Community Plugin Loading Test

> spec: ./brief.md
> depends: 2.1

- [ ] Add community plugin discovery and approval test
  <!-- file: packages/plugin-system/tests/community_plugin.rs -->
  <!-- purpose: Place a third-party plugin directory in the plugins path; verify it is discovered; verify it is rejected without config approval; add approval to config; verify it loads and runs -->
  <!-- requirements: 9.1, 9.2, 9.3 -->
