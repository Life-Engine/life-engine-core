<!--
domain: capability-enforcement
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Implementation Plan — Capability Enforcement

## Task Overview

This plan implements the capability enforcement system for Life Engine Core. Work begins with the capability error types and the approval policy logic, then builds the host function injection layer, wires runtime enforcement into each host function, and finishes with integration tests. All enforcement code lives in Core since the WASM boundary is the single security enforcement point. There is no App-side capability checking.

**Progress:** 0 / 10 tasks complete

## Steering Document Compliance

- Deny-by-default: no access without declaration and approval
- Host functions injected per-plugin based on approved capability set
- EngineError (Fatal severity) returned on capability violations — never silent failure
- First-party plugins auto-granted; third-party requires config-based approval
- Capabilities checked synchronously at host function invocation
- No install dialogs or interactive prompts — config-based only

## Atomic Task Requirements

- **File Scope:** 1-3 related files maximum
- **Time Boxing:** 15-30 minutes per task
- **Single Purpose:** one testable outcome per task
- **Specific Files:** exact file paths specified
- **Agent-Friendly:** clear input/output, minimal context switching

---

## 1.1 — Capability Types and Error Definitions

> spec: ./brief.md

- [ ] Define capability enum and CapabilityViolation error type
  <!-- file: packages/traits/src/capability.rs -->
  <!-- purpose: Define Capability enum (StorageRead, StorageWrite, HttpOutbound, EventsEmit, EventsSubscribe, ConfigRead) with string conversion; define CapabilityViolation error implementing EngineError with Fatal severity and CAP_xxx codes -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4, 6.1 -->

- [ ] Add unit tests for capability types and error formatting
  <!-- file: packages/traits/src/capability.rs (inline tests) -->
  <!-- purpose: Test Capability enum string round-trip; test CapabilityViolation error fields (code, severity, source_module, message format) -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4, 4.5 -->

---

## 1.2 — Manifest Capability Parsing

> spec: ./brief.md

- [ ] Parse capabilities from manifest.toml
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Extend manifest parsing to extract [capabilities].required array; validate each string against recognized Capability enum; reject unknown capability strings -->
  <!-- requirements: 1.1, 6.1, 6.2 -->

- [ ] Add tests for manifest capability parsing
  <!-- file: apps/core/src/config.rs (inline tests) -->
  <!-- purpose: Test valid capability arrays parse correctly; test unknown capability strings cause load rejection; test missing capabilities section treated as empty -->
  <!-- requirements: 1.1, 6.1, 6.2 -->

---

## 1.3 — Approval Policy Logic

> spec: ./brief.md

- [ ] Implement first-party detection and third-party config lookup
  <!-- file: apps/core/src/config.rs -->
  <!-- purpose: Determine if plugin is first-party (in monorepo plugins/ directory) and auto-grant; for third-party, read [plugins.<id>].approved_capabilities from config.toml; compare declared vs approved; return CapabilityViolation (CAP_001) if unapproved -->
  <!-- requirements: 1.2, 1.3, 1.4, 1.5, 5.1, 5.2, 5.3, 5.4 -->

- [ ] Add tests for approval policy
  <!-- file: apps/core/src/config.rs (inline tests) -->
  <!-- purpose: Test first-party auto-grant; test third-party approval with matching capabilities; test third-party rejection when manifest declares unapproved capability; test missing config section refuses load; test empty approved_capabilities allows load with no host functions -->
  <!-- requirements: 1.2, 1.3, 1.4, 1.5, 5.2, 5.4 -->

---

## 1.4 — Host Function Injection Gating

> spec: ./brief.md

- [ ] Inject host functions per-plugin based on approved capabilities
  <!-- file: apps/core/src/main.rs -->
  <!-- file: packages/workflow-engine/src/lib.rs -->
  <!-- purpose: During WASM module loading, construct the host function set from the plugin's approved capabilities; register only approved host functions in the Extism plugin instance -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6 -->

- [ ] Add tests for host function injection
  <!-- file: apps/core/src/tests/mod.rs -->
  <!-- purpose: Test that a plugin with storage:read only gets storage read host functions; test that storage:write without storage:read does not inject read functions; test that no capabilities results in no host functions injected -->
  <!-- requirements: 2.1, 2.2, 2.6 -->

---

## 1.5 — Runtime Capability Checks in Host Functions

> spec: ./brief.md

- [ ] Add synchronous capability check to each host function
  <!-- file: packages/workflow-engine/src/handlers/mod.rs -->
  <!-- purpose: At the start of every host function (storage_query, storage_mutate, http_request, event_emit, event_subscribe, config_get), check the calling plugin's approved capability set; return CapabilityViolation (CAP_002, Fatal) if not approved -->
  <!-- requirements: 3.1, 3.2, 3.3, 3.4 -->

- [ ] Add tests for runtime capability enforcement
  <!-- file: packages/workflow-engine/src/tests/mod.rs -->
  <!-- purpose: Test that approved capability allows host function execution; test that unapproved capability returns Fatal EngineError with CAP_002 code; test that check is synchronous -->
  <!-- requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 4.5 -->

---

## 1.6 — Integration Tests

> spec: ./brief.md

- [ ] Add end-to-end capability enforcement integration tests
  <!-- file: apps/core/tests/capability_enforcement.rs -->
  <!-- purpose: Load a first-party test plugin and verify all capabilities auto-granted; load a third-party test plugin with partial approval and verify approved operations succeed and unapproved operations return Fatal EngineError; verify plugin with unapproved manifest capability refuses to load -->
  <!-- requirements: 1.2, 1.4, 2.1, 3.3, 4.1, 4.3 -->
