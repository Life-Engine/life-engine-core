<!--
domain: canonical-data-models
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Implementation Plan — Canonical Data Models

## Task Overview

This plan implements the 7 canonical collection schemas across three output formats: JSON Schema files, Rust structs, and TypeScript interfaces. Work begins with the JSON Schema definitions (the source of truth), then generates Rust structs in `plugin-sdk-rs` and TypeScript interfaces in `plugin-sdk-js`. Validation tests verify all three representations stay consistent. Extension handling and private collection support are built on top of the schemas.

**Progress:** 0 / 15 tasks complete

## Steering Document Compliance

- All 7 canonical schemas defined as JSON Schema, Rust structs, and TypeScript interfaces
- Extensions use reverse-domain namespace convention
- Credentials collection has no extensions field (claims serves that purpose)
- Additive-only versioning within a major SDK release
- Private collections namespaced by plugin ID

## Atomic Task Requirements

- **File Scope:** 1-3 related files maximum
- **Time Boxing:** 15-30 minutes per task
- **Single Purpose:** one testable outcome per task
- **Specific Files:** exact file paths specified
- **Agent-Friendly:** clear input/output, minimal context switching

---

## 1.1 — JSON Schema Files
> spec: ./brief.md

- [ ] Create Events JSON Schema
  <!-- file: .odm/docs/schemas/events.schema.json -->
  <!-- purpose: Define JSON Schema Draft 2020-12 for the Events collection with all required/optional fields and extensions -->
  <!-- requirements: 3.1, 3.2 -->
  <!-- leverage: existing .odm/docs/schemas/events.schema.json -->

- [ ] Create Tasks JSON Schema
  <!-- file: .odm/docs/schemas/tasks.schema.json -->
  <!-- purpose: Define JSON Schema for Tasks with status and priority enums -->
  <!-- requirements: 3.1, 3.2, 3.4 -->
  <!-- leverage: existing .odm/docs/schemas/tasks.schema.json -->

- [ ] Create Contacts JSON Schema
  <!-- file: .odm/docs/schemas/contacts.schema.json -->
  <!-- purpose: Define JSON Schema for Contacts with nested name object, emails, phones, and addresses arrays -->
  <!-- requirements: 3.1, 3.2 -->
  <!-- leverage: existing .odm/docs/schemas/contacts.schema.json -->

- [ ] Create Notes, Emails, Files, and Credentials JSON Schemas
  <!-- file: .odm/docs/schemas/notes.schema.json -->
  <!-- file: .odm/docs/schemas/emails.schema.json -->
  <!-- file: .odm/docs/schemas/files.schema.json -->
  <!-- purpose: Define JSON Schemas for remaining 3 collections (Credentials already exists); verify no extensions field on Credentials -->
  <!-- requirements: 3.1, 3.2, 4.5 -->
  <!-- leverage: existing .odm/docs/schemas/ files -->

---

## 1.2 — Rust Struct Definitions
> spec: ./brief.md

- [ ] Define Events and Tasks Rust structs
  <!-- file: packages/types/src/events.rs -->
  <!-- file: packages/types/src/tasks.rs -->
  <!-- purpose: Rust structs with serde derives, required fields as non-optional, optional fields as Option<T> with skip_serializing_if -->
  <!-- requirements: 1.1, 1.2, 1.3 -->
  <!-- leverage: existing packages/types/src/events.rs, packages/types/src/tasks.rs -->

- [ ] Define Contacts Rust struct with nested ContactName
  <!-- file: packages/types/src/contacts.rs -->
  <!-- purpose: Contacts struct with nested ContactName, ContactEmail, ContactPhone, and ContactAddress structs -->
  <!-- requirements: 1.1, 1.4 -->
  <!-- leverage: existing packages/types/src/contacts.rs -->

- [ ] Define Notes, Emails, Files, and Credentials Rust structs
  <!-- file: packages/types/src/notes.rs -->
  <!-- file: packages/types/src/emails.rs -->
  <!-- file: packages/types/src/files.rs -->
  <!-- purpose: Rust structs for remaining collections; Credentials uses claims object instead of extensions -->
  <!-- requirements: 1.1, 1.2, 1.3 -->
  <!-- leverage: existing packages/types/src/ files -->

- [ ] Re-export all canonical types from plugin-sdk-rs
  <!-- file: packages/plugin-sdk-rs/src/types.rs -->
  <!-- file: packages/plugin-sdk-rs/src/lib.rs -->
  <!-- purpose: Re-export all 7 canonical types from the types crate so plugin authors import from plugin-sdk-rs -->
  <!-- requirements: 1.1 -->
  <!-- leverage: existing packages/plugin-sdk-rs/src/lib.rs -->

---

## 1.3 — TypeScript Interface Definitions
> spec: ./brief.md

- [ ] Define all 7 canonical TypeScript interfaces
  <!-- file: packages/plugin-sdk-js/src/index.ts -->
  <!-- purpose: Export interfaces for Events, Tasks, Contacts, Notes, Emails, Files, Credentials with string literal unions for enums -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.4 -->
  <!-- leverage: existing packages/plugin-sdk-js/src/index.ts -->

---

## 1.4 — Schema Validation Tests
> spec: ./brief.md

- [ ] Create test fixtures for all 7 collections
  <!-- file: packages/test-fixtures/schemas/valid/ -->
  <!-- file: packages/test-fixtures/schemas/invalid/ -->
  <!-- purpose: Valid and invalid JSON fixtures for each collection to test schema validation -->
  <!-- requirements: 3.3 -->
  <!-- leverage: existing packages/test-fixtures/ -->

- [ ] Add JSON Schema validation tests
  <!-- file: tests/schemas/validation_test.rs -->
  <!-- purpose: Load each JSON Schema, validate against test fixtures, assert valid passes and invalid fails with descriptive errors -->
  <!-- requirements: 3.3, 3.4 -->
  <!-- leverage: packages/test-fixtures -->

---

## 1.5 — Extension Namespace Enforcement
> spec: ./brief.md

- [ ] Implement extension namespace validation in Core
  <!-- file: apps/core/src/schema_registry.rs -->
  <!-- file: apps/core/src/routes/data.rs -->
  <!-- purpose: On write, verify extension keys match the writing plugin's ID; reject cross-namespace writes -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4 -->
  <!-- leverage: existing apps/core/src/schema_registry.rs -->

---

## 1.6 — Private Collection Support
> spec: ./brief.md

- [ ] Implement private collection registration and validation
  <!-- file: apps/core/src/schema_registry.rs -->
  <!-- file: apps/core/src/plugin_loader.rs -->
  <!-- purpose: Read private collection schemas from plugin manifest, register in schema registry, validate on write, deny cross-plugin access -->
  <!-- requirements: 6.1, 6.2, 6.3 -->
  <!-- leverage: existing apps/core/src/schema_registry.rs, apps/core/src/plugin_loader.rs -->

---

## 1.7 — Schema Versioning CI Check
> spec: ./brief.md

- [ ] Add CI script to verify additive-only schema changes
  <!-- file: scripts/check-schema-compat.sh -->
  <!-- purpose: Compare JSON Schemas between current and previous minor release; fail CI if required fields were removed or types changed -->
  <!-- requirements: 5.1, 5.2 -->
  <!-- leverage: none -->
