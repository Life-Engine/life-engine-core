<!--
domain: capability-enforcement
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Implementation Plan — Capability Enforcement

## Task Overview

This plan implements the capability enforcement system for the Life Engine shell (App). Work begins with the CapabilityError class and the core capability checker utility, then builds the install-time approval dialog, wires scoping enforcement for data, network, and IPC, and integrates the checker into every ShellAPI method. The final tasks add the high-trust warning UI and integration tests. All enforcement code lives in the App frontend since the shell is the security boundary.

**Progress:** 0 / 13 tasks complete

## Steering Document Compliance

- Deny-by-default: no access without declaration and approval
- Synchronous enforcement at the start of every ShellAPI method
- CapabilityError thrown (never silent failure) with plugin ID, operation, and missing capability
- High-trust capabilities (data:write, network:fetch) receive visible warnings in the install dialog
- Scoping enforced for data collections, network domains, and IPC targets

## Atomic Task Requirements

- **File Scope:** 1-3 related files maximum
- **Time Boxing:** 15-30 minutes per task
- **Single Purpose:** one testable outcome per task
- **Specific Files:** exact file paths specified
- **Agent-Friendly:** clear input/output, minimal context switching

---

## 1.1 — CapabilityError and Types
> spec: ./brief.md

- [ ] Define CapabilityError class and capability types
  <!-- file: apps/app/src/lib/capabilities.js -->
  <!-- purpose: Define CapabilityError class with pluginId, operation, and missingCapability fields; define capability string constants -->
  <!-- requirements: 7.1, 7.2, 7.3 -->
  <!-- leverage: existing apps/app/src/lib/capabilities.js -->

- [ ] Add CapabilityError unit tests
  <!-- file: apps/app/src/lib/__tests__/capabilities.test.js -->
  <!-- purpose: Test CapabilityError construction, message format, and that it is a proper Error subclass -->
  <!-- requirements: 7.1, 7.2, 7.3 -->
  <!-- leverage: existing apps/app/src/lib/__tests__/ -->

---

## 1.2 — Capability Checker
> spec: ./brief.md

- [ ] Implement capability checker utility
  <!-- file: apps/app/src/lib/capabilities.js -->
  <!-- purpose: checkCapability(pluginId, requiredCapability) function that looks up approved set in memory and throws CapabilityError if missing -->
  <!-- requirements: 1.1, 1.2, 1.3, 1.5 -->
  <!-- leverage: existing apps/app/src/lib/capabilities.js -->

- [ ] Add capability checker unit tests
  <!-- file: apps/app/src/lib/__tests__/capabilities.test.js -->
  <!-- purpose: Test that valid capability passes, missing capability throws, and check is synchronous -->
  <!-- requirements: 1.1, 1.2, 1.3, 1.4, 1.5 -->
  <!-- leverage: existing apps/app/src/lib/__tests__/ -->

---

## 1.3 — Install-Time Approval Dialog
> spec: ./brief.md

- [ ] Implement capability approval dialog component
  <!-- file: apps/app/src/components/plugin-store.js -->
  <!-- purpose: Show approval dialog listing all requested capabilities with human-readable descriptions; handle approve/reject -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.4 -->
  <!-- leverage: existing apps/app/src/components/plugin-store.js -->

- [ ] Implement capability persistence and update detection
  <!-- file: apps/app/src/lib/plugin-manifest.js -->
  <!-- file: apps/app/src/lib/plugin-storage.js -->
  <!-- purpose: Store approved capabilities in settings.json; detect new capabilities on plugin update; skip dialog on subsequent loads -->
  <!-- requirements: 2.3, 2.5, 2.6 -->
  <!-- leverage: existing apps/app/src/lib/plugin-manifest.js, apps/app/src/lib/plugin-storage.js -->

---

## 1.4 — Data Capability Scoping
> spec: ./brief.md

- [ ] Implement data capability scoping in ShellAPI
  <!-- file: apps/app/src/lib/scoped-api.js -->
  <!-- purpose: Wrap data.query, data.subscribe, data.create, data.update, data.delete with collection-level capability checks; data:write implies read -->
  <!-- requirements: 3.1, 3.2, 3.3, 3.4 -->
  <!-- leverage: existing apps/app/src/lib/scoped-api.js -->

- [ ] Add data scoping tests
  <!-- file: apps/app/src/lib/__tests__/scoped-api.test.js -->
  <!-- purpose: Test that data:read:todos allows query on todos but throws on contacts; test write-implies-read -->
  <!-- requirements: 3.1, 3.2, 3.3 -->
  <!-- leverage: existing apps/app/src/lib/__tests__/ -->

---

## 1.5 — Network Capability Scoping
> spec: ./brief.md

- [ ] Implement network domain scoping
  <!-- file: apps/app/src/lib/scoped-api.js -->
  <!-- purpose: Wrap http.fetch with network:fetch capability check and domain validation against allowedDomains -->
  <!-- requirements: 4.1, 4.2, 4.3 -->
  <!-- leverage: existing apps/app/src/lib/scoped-api.js -->

---

## 1.6 — IPC Capability Scoping
> spec: ./brief.md

- [ ] Implement IPC target scoping
  <!-- file: apps/app/src/lib/scoped-api.js -->
  <!-- purpose: Wrap ipc.send with ipc:send:{targetId} capability check; throw CapabilityError for undeclared targets -->
  <!-- requirements: 5.1, 5.2 -->
  <!-- leverage: existing apps/app/src/lib/scoped-api.js -->

---

## 1.7 — High-Trust Warnings
> spec: ./brief.md

- [ ] Add high-trust warning indicators to install dialog
  <!-- file: apps/app/src/components/plugin-store.js -->
  <!-- purpose: Show yellow warning icon and descriptive text for data:write and network:fetch capabilities listing affected collections/domains -->
  <!-- requirements: 6.1, 6.2, 6.3 -->
  <!-- leverage: existing apps/app/src/components/plugin-store.js -->

---

## 1.8 — Integration Tests
> spec: ./brief.md

- [ ] Add end-to-end capability enforcement tests
  <!-- file: tests/capabilities/enforcement_test.js -->
  <!-- purpose: Load a test plugin with limited capabilities, verify allowed operations succeed, verify unauthorized operations throw CapabilityError with correct fields -->
  <!-- requirements: 1.1, 1.3, 1.4, 3.3, 4.2, 5.2 -->
  <!-- leverage: packages/test-utils-js -->
