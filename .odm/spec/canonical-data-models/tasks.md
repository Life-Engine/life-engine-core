<!--
domain: canonical-data-models
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Implementation Plan — Canonical Data Models

## Task Overview

This plan implements the 7 canonical collection schemas and the `PipelineMessage` envelope. Work begins with the JSON Schema definitions (the source of truth), then implements Rust structs in `packages/types`, defines the `PipelineMessage` envelope, and ensures the plugin SDK re-exports everything. Validation tests verify consistency. Extension handling and private collection support are built on top of the schemas.

**Progress:** 0 / 16 tasks complete

## Steering Document Compliance

- All 7 canonical schemas defined as JSON Schema and Rust structs
- `PipelineMessage`, `MessageMetadata`, and `TypedPayload` defined in `packages/types`
- Plugin SDK re-exports all CDM types and the `PipelineMessage` envelope
- Extensions use reverse-domain namespace convention
- Credentials collection has no extensions field (claims serves that purpose)
- Additive-only versioning coupled to SDK semver
- Private collections declared via JSON Schema in `manifest.toml`
- Rust/WASM only — no TypeScript interfaces in scope

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
  <!-- file: .odm/doc/schemas/events.schema.json -->
  <!-- purpose: Define JSON Schema Draft 2020-12 for the Events collection with all required/optional fields and extensions -->
  <!-- requirements: 3.1, 3.2 -->
  <!-- leverage: existing .odm/doc/schemas/events.schema.json -->

- [ ] Create Tasks JSON Schema
  <!-- file: .odm/doc/schemas/tasks.schema.json -->
  <!-- purpose: Define JSON Schema for Tasks with status and priority enums -->
  <!-- requirements: 3.1, 3.2, 3.4 -->
  <!-- leverage: existing .odm/doc/schemas/tasks.schema.json -->

- [ ] Create Contacts JSON Schema
  <!-- file: .odm/doc/schemas/contacts.schema.json -->
  <!-- purpose: Define JSON Schema for Contacts with nested name object, emails, phones, and addresses arrays -->
  <!-- requirements: 3.1, 3.2 -->
  <!-- leverage: existing .odm/doc/schemas/contacts.schema.json -->

- [ ] Create Notes, Emails, Files, and Credentials JSON Schemas
  <!-- file: .odm/doc/schemas/notes.schema.json -->
  <!-- file: .odm/doc/schemas/emails.schema.json -->
  <!-- file: .odm/doc/schemas/files.schema.json -->
  <!-- file: .odm/doc/schemas/credentials.schema.json -->
  <!-- purpose: Define JSON Schemas for remaining 4 collections; verify no extensions field on Credentials -->
  <!-- requirements: 3.1, 3.2, 4.5 -->
  <!-- leverage: existing .odm/doc/schemas/ files -->

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
  <!-- file: packages/types/src/credentials.rs -->
  <!-- purpose: Rust structs for remaining collections; Credentials uses claims object instead of extensions -->
  <!-- requirements: 1.1, 1.2, 1.3 -->
  <!-- leverage: existing packages/types/src/ files -->

---

## 1.3 — PipelineMessage Envelope
> spec: ./brief.md

- [ ] Define PipelineMessage, MessageMetadata, and TypedPayload
  <!-- file: packages/types/src/pipeline.rs -->
  <!-- file: packages/types/src/lib.rs -->
  <!-- purpose: Define PipelineMessage struct with MessageMetadata and TypedPayload enum (Cdm/Custom variants); add CdmType enum with one variant per canonical collection -->
  <!-- requirements: 2.1, 2.2, 2.3 -->
  <!-- leverage: existing packages/types/src/lib.rs -->

- [ ] Define SchemaValidated wrapper type
  <!-- file: packages/types/src/pipeline.rs -->
  <!-- purpose: Define SchemaValidated<T> newtype wrapper that guarantees the inner value has been validated against a JSON Schema -->
  <!-- requirements: 2.3 -->
  <!-- leverage: none -->

---

## 1.4 — Plugin SDK Re-exports
> spec: ./brief.md

- [ ] Re-export all canonical types and PipelineMessage from plugin-sdk
  <!-- file: packages/plugin-sdk/src/types.rs -->
  <!-- file: packages/plugin-sdk/src/lib.rs -->
  <!-- purpose: Re-export all 7 canonical types, PipelineMessage, MessageMetadata, TypedPayload, and CdmType from the types crate so plugin authors import from plugin-sdk -->
  <!-- requirements: 7.1, 7.2, 7.3 -->
  <!-- leverage: existing packages/plugin-sdk/src/lib.rs -->

---

## 1.5 — Schema Validation Tests
> spec: ./brief.md

- [ ] Create test fixtures for all 7 collections
  <!-- file: packages/test-utils/fixtures/schemas/valid/ -->
  <!-- file: packages/test-utils/fixtures/schemas/invalid/ -->
  <!-- purpose: Valid and invalid JSON fixtures for each collection to test schema validation -->
  <!-- requirements: 3.3 -->
  <!-- leverage: existing packages/test-utils/ -->

- [ ] Add JSON Schema validation tests
  <!-- file: packages/types/tests/schema_validation.rs -->
  <!-- purpose: Load each JSON Schema, validate against test fixtures, assert valid passes and invalid fails with descriptive errors -->
  <!-- requirements: 3.3, 3.4 -->
  <!-- leverage: packages/test-utils fixtures -->

---

## 1.6 — Extension Namespace Enforcement
> spec: ./brief.md

- [ ] Implement extension namespace validation
  <!-- file: packages/types/src/extensions.rs -->
  <!-- purpose: On write, verify extension keys match the writing plugin's ID; reject cross-namespace writes; preserve all extension data during sync/merge -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4 -->
  <!-- leverage: none -->

---

## 1.7 — Private Collection Support
> spec: ./brief.md

- [ ] Implement private collection registration and validation
  <!-- file: packages/workflow-engine/src/schema_registry.rs -->
  <!-- purpose: Read private collection schemas from plugin manifest.toml, register in schema registry, validate on write, deny cross-plugin access -->
  <!-- requirements: 6.1, 6.2, 6.3 -->
  <!-- leverage: none -->

---

## 1.8 — Schema Versioning Verification
> spec: ./brief.md

- [ ] Add script to verify additive-only schema changes
  <!-- file: scripts/check-schema-compat.sh -->
  <!-- purpose: Compare JSON Schemas between current and previous minor release; fail if required fields were removed or types changed -->
  <!-- requirements: 5.1, 5.2 -->
  <!-- leverage: none -->
