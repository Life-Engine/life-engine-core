<!--
project: life-engine-core
phase: 2
specs: canonical-data-models
updated: 2026-03-23
-->

# Phase 2 — Canonical Data Models and Types

## Plan Overview

This phase implements the 7 canonical collection schemas and the `PipelineMessage` envelope that all data flows through in the new architecture. Work begins with JSON Schema definitions (the source of truth for validation), then implements the corresponding Rust structs in `packages/types`, defines the `PipelineMessage` envelope with `MessageMetadata` and `TypedPayload`, and ensures the plugin SDK re-exports everything. Validation tests verify JSON Schema / Rust struct consistency. Extension namespace enforcement and schema versioning verification complete the phase.

This phase depends on Phase 1 (crate scaffolding must exist). The output is a fully-defined type system that all other crates depend on.

> spec: .odm/spec/canonical-data-models/brief.md

Progress: 0 / 11 work packages complete

---

## 2.1 — Events and Tasks JSON Schemas
> spec: .odm/spec/canonical-data-models/brief.md

- [x] Create Events JSON Schema with all calendar event fields
  <!-- file: .odm/doc/schemas/events.schema.json -->
  <!-- purpose: Define JSON Schema Draft 2020-12 for the Events collection. Required fields: id (uuid), title (string), start (datetime), source (string), source_id (string), created_at (datetime), updated_at (datetime). Optional fields: end (datetime), description (string), location (string), all_day (boolean), recurrence (object with frequency, interval, until, count, by_day), attendees (array of objects with name, email, status enum: accepted/declined/tentative/needs-action), reminders (array of objects with minutes_before, method enum: notification/email), timezone (string), status (enum: confirmed/tentative/cancelled), extensions (object with additionalProperties keyed by reverse-domain namespace). Include $id, $schema, title, description metadata. -->
  <!-- requirements: 3.1, 3.2 -->
  <!-- leverage: existing .odm/doc/schemas/events.schema.json -->

- [x] Create Tasks JSON Schema with status and priority enums
  <!-- file: .odm/doc/schemas/tasks.schema.json -->
  <!-- purpose: Define JSON Schema for the Tasks collection. Required fields: id (uuid), title (string), source (string), source_id (string), created_at (datetime), updated_at (datetime). Optional fields: description (string), status (enum: pending/in_progress/completed/cancelled), priority (enum: low/medium/high/urgent), due_date (datetime), completed_at (datetime), tags (array of strings), assignee (string), parent_id (uuid for subtasks), extensions (object). Include TaskStatus and TaskPriority as enum definitions referenced by $ref. -->
  <!-- requirements: 3.1, 3.2, 3.4 -->
  <!-- leverage: existing .odm/doc/schemas/tasks.schema.json -->

---

## 2.2 — Contacts, Notes, Emails, Files, and Credentials JSON Schemas
> spec: .odm/spec/canonical-data-models/brief.md

- [x] Create Contacts JSON Schema with nested structured types
  <!-- file: .odm/doc/schemas/contacts.schema.json -->
  <!-- purpose: Define JSON Schema for the Contacts collection. Required fields: id (uuid), name (object with given, family required; prefix, suffix, middle optional), source (string), source_id (string), created_at (datetime), updated_at (datetime). Optional fields: emails (array of objects with address required, type enum: home/work/other, primary boolean), phones (array of objects with number required, type enum: mobile/home/work/fax/other, primary boolean), addresses (array of objects with street, city, region, postal_code, country, type enum: home/work/other), organization (string), title (string), birthday (date), photo_url (string), notes (string), groups (array of strings), extensions (object). -->
  <!-- requirements: 3.1, 3.2 -->
  <!-- leverage: existing .odm/doc/schemas/contacts.schema.json -->

- [x] Create Notes, Emails, Files, and Credentials JSON Schemas
  <!-- file: .odm/doc/schemas/notes.schema.json -->
  <!-- file: .odm/doc/schemas/emails.schema.json -->
  <!-- file: .odm/doc/schemas/files.schema.json -->
  <!-- file: .odm/doc/schemas/credentials.schema.json -->
  <!-- purpose: Notes schema: id, title, body (string), tags (array), format (enum: plain/markdown/html), pinned (boolean), extensions. Emails schema: id, subject, from (object: name, address), to/cc/bcc (arrays of address objects), body_text, body_html, date (datetime), message_id (string), in_reply_to (string for threading), attachments (array of objects: filename, mime_type, size_bytes, content_id), read (boolean), starred (boolean), labels (array), extensions. Files schema: id, filename, path, mime_type, size_bytes, checksum (sha256 hex string), storage_backend (string), extensions. Credentials schema: id, name, credential_type (enum: oauth_token/api_key/identity_document/passkey), service (string), claims (object for type-specific data — NO extensions field on Credentials), encrypted (boolean), expires_at (datetime optional). All schemas include common fields: source, source_id, created_at, updated_at. -->
  <!-- requirements: 3.1, 3.2, 4.5 -->
  <!-- leverage: existing .odm/doc/schemas/ files -->

---

## 2.3 — Events and Tasks Rust Structs
> spec: .odm/spec/canonical-data-models/brief.md

- [x] Define Events Rust struct with recurrence and attendee support
  <!-- file: packages/types/src/events.rs -->
  <!-- purpose: Define CalendarEvent struct with serde Serialize/Deserialize derives. Required fields as non-optional: id (Uuid), title (String), start (DateTime<Utc>), source (String), source_id (String), created_at (DateTime<Utc>), updated_at (DateTime<Utc>). Optional fields as Option<T> with #[serde(skip_serializing_if = "Option::is_none")]: end, description, location, all_day, timezone, status (EventStatus enum). Define Recurrence struct (frequency: RecurrenceFrequency enum, interval: u32, until: Option<DateTime<Utc>>, count: Option<u32>, by_day: Option<Vec<String>>). Define Attendee struct (name: Option<String>, email: String, status: AttendeeStatus enum). Define Reminder struct (minutes_before: u32, method: ReminderMethod enum). Add extensions: Option<serde_json::Value> field. Ensure all existing tests still pass after modification. -->
  <!-- requirements: 1.1, 1.2, 1.3 -->
  <!-- leverage: existing packages/types/src/events.rs -->

- [x] Define Tasks Rust struct with status and priority enums
  <!-- file: packages/types/src/tasks.rs -->
  <!-- purpose: Define Task struct with serde derives. Required fields: id (Uuid), title (String), source (String), source_id (String), created_at (DateTime<Utc>), updated_at (DateTime<Utc>). Optional fields: description, status (TaskStatus enum: Pending/InProgress/Completed/Cancelled), priority (TaskPriority enum: Low/Medium/High/Urgent), due_date, completed_at, tags (Vec<String>), assignee, parent_id (Uuid), extensions. Implement Default for TaskStatus (Pending) and TaskPriority (Medium). Ensure serde rename_all = "snake_case" on enums for JSON compatibility. Ensure all existing tests pass. -->
  <!-- requirements: 1.1, 1.2, 1.3 -->
  <!-- leverage: existing packages/types/src/tasks.rs -->

---

## 2.4 — Contacts Rust Struct with Nested Types
> spec: .odm/spec/canonical-data-models/brief.md

- [x] Define Contacts Rust struct with ContactName, ContactEmail, ContactPhone, ContactAddress
  <!-- file: packages/types/src/contacts.rs -->
  <!-- purpose: Define Contact struct with required fields: id (Uuid), name (ContactName), source (String), source_id (String), created_at (DateTime<Utc>), updated_at (DateTime<Utc>). ContactName struct: given (String), family (String), prefix/suffix/middle as Option<String>. Optional fields on Contact: emails (Vec<ContactEmail>), phones (Vec<ContactPhone>), addresses (Vec<ContactAddress>), organization, title, birthday (NaiveDate), photo_url, notes, groups (Vec<String>), extensions. ContactEmail: address (String), email_type (ContactInfoType enum: Home/Work/Other), primary (bool). ContactPhone: number (String), phone_type (ContactInfoType), primary (bool). ContactAddress: street, city, region, postal_code, country (all Option<String>), address_type (ContactInfoType). Use serde rename for snake_case JSON serialization. Ensure existing tests pass. -->
  <!-- requirements: 1.1, 1.4 -->
  <!-- leverage: existing packages/types/src/contacts.rs -->

---

## 2.5 — Notes, Emails, Files, and Credentials Rust Structs
> spec: .odm/spec/canonical-data-models/brief.md

- [x] Define Notes, Emails, Files, and Credentials Rust structs
  <!-- file: packages/types/src/notes.rs -->
  <!-- file: packages/types/src/emails.rs -->
  <!-- file: packages/types/src/files.rs -->
  <!-- file: packages/types/src/credentials.rs -->
  <!-- purpose: Note struct: id, title, body (String), tags (Vec<String>), format (NoteFormat enum: Plain/Markdown/Html), pinned (bool), source, source_id, created_at, updated_at, extensions. Email struct: id, subject, from (EmailAddress: name Option<String>, address String), to/cc/bcc (Vec<EmailAddress>), body_text/body_html (Option<String>), date (DateTime<Utc>), message_id, in_reply_to (Option<String> for threading), attachments (Vec<EmailAttachment>: filename, mime_type, size_bytes u64, content_id Option<String>), read (bool), starred (bool), labels (Vec<String>), source, source_id, created_at, updated_at, extensions. FileMetadata struct: id, filename, path, mime_type, size_bytes (u64), checksum (String — sha256 hex), storage_backend, source, source_id, created_at, updated_at, extensions. Credential struct: id, name, credential_type (CredentialType enum: OauthToken/ApiKey/IdentityDocument/Passkey), service, claims (serde_json::Value — type-specific data), encrypted (bool), expires_at (Option<DateTime<Utc>>), source, source_id, created_at, updated_at. Credentials intentionally has NO extensions field — claims serves that purpose. Ensure existing tests pass. -->
  <!-- requirements: 1.1, 1.2, 1.3 -->
  <!-- leverage: existing packages/types/src/ files -->

---

## 2.6 — PipelineMessage Envelope
> spec: .odm/spec/canonical-data-models/brief.md

- [x] Define PipelineMessage, MessageMetadata, TypedPayload, and CdmType
  <!-- file: packages/types/src/pipeline.rs -->
  <!-- purpose: Define PipelineMessage struct with two fields: metadata (MessageMetadata) and payload (TypedPayload). MessageMetadata struct: correlation_id (Uuid — unique per request, propagated through all steps), source (String — trigger type and value, e.g., "endpoint:POST /email/sync"), timestamp (DateTime<Utc>), auth_context (Option<serde_json::Value> — authenticated identity from auth module). TypedPayload enum: Cdm(CdmType) for canonical data, Custom(SchemaValidated<serde_json::Value>) for plugin-defined types. CdmType enum with one variant per canonical collection: Event(CalendarEvent), Task(Task), Contact(Contact), Note(Note), Email(Email), File(FileMetadata), Credential(Credential), plus EventBatch(Vec<CalendarEvent>), TaskBatch(Vec<Task>), ContactBatch(Vec<Contact>), NoteBatch(Vec<Note>), EmailBatch(Vec<Email>), FileBatch(Vec<FileMetadata>), CredentialBatch(Vec<Credential>) for batch operations. All types derive Serialize, Deserialize, Debug, Clone. -->
  <!-- requirements: 2.1, 2.2, 2.3 -->
  <!-- leverage: existing packages/types/src/lib.rs -->

- [x] Define SchemaValidated wrapper type for custom payloads
  <!-- file: packages/types/src/pipeline.rs -->
  <!-- purpose: Define SchemaValidated<T> as a newtype wrapper (pub struct SchemaValidated<T>(T)) that guarantees the inner value has been validated against a JSON Schema. Implement Deref<Target = T> for transparent access. Implement SchemaValidated::new(value: T, schema: &serde_json::Value) -> Result<Self> that validates the value against the provided JSON Schema using the jsonschema crate before wrapping. Implement Serialize/Deserialize that delegate to the inner type. This ensures Custom payloads are always validated before entering the pipeline. -->
  <!-- requirements: 2.3 -->
  <!-- leverage: none -->

- [x] Re-export all types from packages/types lib.rs
  <!-- file: packages/types/src/lib.rs -->
  <!-- purpose: Add pub mod pipeline; and re-export PipelineMessage, MessageMetadata, TypedPayload, CdmType, SchemaValidated from the pipeline module. Re-export all 7 canonical types (CalendarEvent, Task, Contact, Note, Email, FileMetadata, Credential) and their supporting types (enums, nested structs) as a flat public API. Ensure the lib.rs public API is comprehensive — downstream crates should only need use life_engine_types::*. -->
  <!-- requirements: 2.1, 2.2 -->
  <!-- leverage: existing packages/types/src/lib.rs -->

---

## 2.7 — Plugin SDK Type Re-exports
> depends: 2.6
> spec: .odm/spec/canonical-data-models/brief.md

- [x] Re-export all canonical types and PipelineMessage from plugin-sdk
  <!-- file: packages/plugin-sdk/src/types.rs -->
  <!-- file: packages/plugin-sdk/src/lib.rs -->
  <!-- purpose: In packages/plugin-sdk/src/lib.rs, add pub use life_engine_types::{CalendarEvent, Task, Contact, Note, Email, FileMetadata, Credential, PipelineMessage, MessageMetadata, TypedPayload, CdmType, SchemaValidated} and all supporting types (enums, nested structs). Plugin authors must be able to import everything from life_engine_plugin_sdk without adding life-engine-types as a direct dependency. Verify this by writing a compile test that imports all types from the SDK only. -->
  <!-- requirements: 7.1, 7.2, 7.3 -->
  <!-- leverage: existing packages/plugin-sdk/src/lib.rs -->

---

## 2.8 — Schema Validation Test Fixtures
> spec: .odm/spec/canonical-data-models/brief.md

- [x] Create valid and invalid JSON test fixtures for all 7 collections
  <!-- file: packages/test-utils/fixtures/schemas/valid/ -->
  <!-- file: packages/test-utils/fixtures/schemas/invalid/ -->
  <!-- purpose: Create packages/test-utils/fixtures/schemas/valid/ directory with one JSON file per collection (events.json, tasks.json, contacts.json, notes.json, emails.json, files.json, credentials.json) containing a fully-populated valid record. Create packages/test-utils/fixtures/schemas/invalid/ directory with one JSON file per collection containing records that violate schema constraints: missing required fields, wrong types, invalid enum values, invalid UUID format, invalid datetime format. Each invalid file should contain an array of invalid records with a comment field explaining what is wrong. These fixtures are the canonical test data for schema validation. -->
  <!-- requirements: 3.3 -->
  <!-- leverage: existing packages/test-utils/ -->

---

## 2.9 — JSON Schema Validation Tests
> depends: 2.1, 2.2, 2.8
> spec: .odm/spec/canonical-data-models/brief.md

- [x] Add JSON Schema validation tests for all collections
  <!-- file: packages/types/tests/schema_validation.rs -->
  <!-- purpose: Load each of the 7 JSON Schema files from .odm/doc/schemas/. Load valid fixtures and assert they pass validation with no errors. Load invalid fixtures and assert they fail validation with descriptive error messages that identify the failing field and constraint. Test that extensions field accepts arbitrary nested JSON on all collections except Credentials. Test that Credentials has no extensions field. Test that required fields are enforced. Test that enum values are restricted to defined options. Use the jsonschema crate for validation. -->
  <!-- requirements: 3.3, 3.4 -->
  <!-- leverage: packages/test-utils fixtures -->

---

## 2.10 — Extension Namespace Enforcement
> spec: .odm/spec/canonical-data-models/brief.md

- [ ] Implement extension namespace validation logic
  <!-- file: packages/types/src/extensions.rs -->
  <!-- purpose: Define validate_extension_namespace(plugin_id: &str, extensions: &serde_json::Value) -> Result<()> function. On write, verify all top-level keys in the extensions object match the writing plugin's ID using reverse-domain convention (e.g., plugin "com.example.weather" can write to extensions["com.example.weather"] only). Reject writes where a plugin attempts to write to another plugin's namespace with a clear error message. Preserve all extension data during read operations — no filtering by plugin ID on reads. Define ExtensionError type with NamespaceMismatch variant. Add unit tests: valid namespace passes, cross-namespace write rejected, read returns all namespaces, empty extensions passes, nested extension data preserved. -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4 -->
  <!-- leverage: none -->

---

## 2.11 — Schema Versioning Verification Script
> spec: .odm/spec/canonical-data-models/brief.md

- [ ] Add script to verify additive-only schema changes between versions
  <!-- file: scripts/check-schema-compat.sh -->
  <!-- purpose: Write a shell script that compares JSON Schema files between the current version and the previous git tag. Check for breaking changes: required fields removed, field types changed, enum values removed, field renamed. Allow additive changes: new optional fields, new enum values, new collections. Exit with non-zero code and clear error message if breaking changes detected. Accept the previous tag as an argument (e.g., ./scripts/check-schema-compat.sh v0.1.0). Use jq for JSON diffing. Print a summary of changes found (added fields, added enums, new collections). -->
  <!-- requirements: 5.1, 5.2 -->
  <!-- leverage: none -->
