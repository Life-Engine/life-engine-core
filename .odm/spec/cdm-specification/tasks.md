<!--
domain: cdm-specification
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Implementation Plan — CDM Specification

## Task Overview

This plan implements the 6 CDM recommended collection schemas, the extensions convention, plugin-scoped collection support, and JSON Schema files. Work begins with the Rust struct definitions (authoritative source), then generates JSON Schema files and TypeScript interfaces. Extension merge logic and plugin-scoped collection support are built on top of the structs. Implementor guidance is captured in documentation.

**Progress:** 0 / 18 tasks complete

## Steering Document Compliance

- All 6 CDM recommended schemas defined as Rust structs, JSON Schema files, and TypeScript interfaces
- Common fields (`id`, `source`, `source_id`, `created_at`, `updated_at`, `ext`) on all collections
- Credentials collection has no `ext` field (`claims` serves that purpose)
- Extensions use reverse-domain namespace convention with merge-not-replace semantics
- Plugin-scoped collections declared via `manifest.toml`
- Additive-only versioning within a major SDK release
- Implementor guidance documented for connector authors

## Atomic Task Requirements

- **File Scope:** 1-3 related files maximum
- **Time Boxing:** 15-30 minutes per task
- **Single Purpose:** one testable outcome per task
- **Specific Files:** exact file paths specified
- **Agent-Friendly:** clear input/output, minimal context switching

---

## 1.1 — Rust Struct Definitions
> spec: ./brief.md

- [ ] Define Events Rust struct
  <!-- file: packages/types/src/events.rs -->
  <!-- purpose: Rust struct with serde derives for the Events collection, all required/optional fields, skip_serializing_if annotations -->
  <!-- requirements: 1.1, 2.1, 2.2, 2.3, 11.1, 11.2, 11.3, 11.4 -->

- [ ] Define Tasks Rust struct with enums
  <!-- file: packages/types/src/tasks.rs -->
  <!-- purpose: Task struct with TaskStatus and TaskPriority enums serialised as lowercase strings -->
  <!-- requirements: 1.1, 3.1, 3.2, 3.3, 11.1, 11.2, 11.3, 11.4 -->

- [ ] Define Contacts Rust struct with nested types
  <!-- file: packages/types/src/contacts.rs -->
  <!-- purpose: Contact struct with nested ContactName, EmailAddress, PhoneNumber, and PostalAddress structs; serde rename for type fields -->
  <!-- requirements: 1.1, 4.1, 4.2, 4.3, 4.4, 4.5, 11.1, 11.2, 11.3 -->

- [ ] Define Notes Rust struct
  <!-- file: packages/types/src/notes.rs -->
  <!-- purpose: Note struct with title, body, tags with skip_serializing_if for empty Vec -->
  <!-- requirements: 1.1, 5.1, 5.2, 11.1, 11.2, 11.3, 11.4 -->

- [ ] Define Emails Rust struct with EmailAttachment
  <!-- file: packages/types/src/emails.rs -->
  <!-- purpose: Email struct with EmailAttachment nested type, all Vec fields with skip_serializing_if -->
  <!-- requirements: 1.1, 6.1, 6.2, 6.3, 11.1, 11.2, 11.3, 11.4 -->

- [ ] Define Credentials Rust struct with CredentialType enum
  <!-- file: packages/types/src/credentials.rs -->
  <!-- purpose: Credential struct with no ext field, CredentialType enum serialised as snake_case, claims as opaque JSON -->
  <!-- requirements: 1.5, 1.6, 7.1, 7.2, 7.3, 11.1, 11.2, 11.3 -->

- [ ] Create types module lib.rs with re-exports
  <!-- file: packages/types/src/lib.rs -->
  <!-- purpose: Module declarations and pub use re-exports for all 6 collection types and their nested types -->
  <!-- requirements: 11.1 -->

---

## 1.2 — JSON Schema Files
> spec: ./brief.md

- [ ] Create Events JSON Schema
  <!-- file: docs/schemas/events.schema.json -->
  <!-- purpose: JSON Schema Draft 2020-12 for Events with required/optional fields and ext -->
  <!-- requirements: 12.1, 12.2 -->

- [ ] Create Tasks JSON Schema
  <!-- file: docs/schemas/tasks.schema.json -->
  <!-- purpose: JSON Schema for Tasks with status and priority enum arrays -->
  <!-- requirements: 12.1, 12.2, 12.3 -->

- [ ] Create Contacts JSON Schema
  <!-- file: docs/schemas/contacts.schema.json -->
  <!-- purpose: JSON Schema for Contacts with nested name, emails, phones, and addresses definitions -->
  <!-- requirements: 12.1, 12.2 -->

- [ ] Create Notes JSON Schema
  <!-- file: docs/schemas/notes.schema.json -->
  <!-- purpose: JSON Schema for Notes with title, body, and tags -->
  <!-- requirements: 12.1, 12.2 -->

- [ ] Create Emails JSON Schema
  <!-- file: docs/schemas/emails.schema.json -->
  <!-- purpose: JSON Schema for Emails with attachment sub-schema -->
  <!-- requirements: 12.1, 12.2 -->

- [ ] Create Credentials JSON Schema
  <!-- file: docs/schemas/credentials.schema.json -->
  <!-- purpose: JSON Schema for Credentials with type enum, no ext field -->
  <!-- requirements: 12.1, 12.2, 12.3 -->

---

## 1.3 — TypeScript Interfaces
> spec: ./brief.md

- [ ] Define TypeScript interfaces for all 6 collections
  <!-- file: packages/plugin-sdk-js/src/index.ts -->
  <!-- purpose: TypeScript interfaces matching all Rust structs, including nested types and enums as union types -->
  <!-- requirements: 1.1 -->

---

## 1.4 — Extension Merge Logic
> spec: ./brief.md

- [ ] Implement ext namespace merge on write
  <!-- file: packages/core/src/ext_merge.rs -->
  <!-- purpose: Merge function that replaces only the writing plugin's namespace within ext, preserving all other namespaces -->
  <!-- requirements: 8.1, 8.2, 8.4, 8.5 -->

- [ ] Add ext namespace access control check
  <!-- file: packages/core/src/ext_access.rs -->
  <!-- purpose: Validation function that rejects writes to ext namespaces not owned by the calling plugin -->
  <!-- requirements: 8.3, 8.6, 8.7 -->

---

## 1.5 — Plugin-Scoped Collection Support
> spec: ./brief.md

- [ ] Validate plugin-scoped collection naming in manifest loading
  <!-- file: packages/core/src/manifest.rs -->
  <!-- purpose: Validate that plugin-declared collections use the plugin ID prefix; reject invalid names -->
  <!-- requirements: 9.1, 9.2 -->

- [ ] Enforce plugin-scoped collection access control
  <!-- file: packages/core/src/collection_access.rs -->
  <!-- purpose: Deny access to plugin-scoped collections unless the requesting plugin is the owner or has an explicit capability grant -->
  <!-- requirements: 9.3, 9.4 -->
