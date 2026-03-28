<!--
domain: cdm-specification
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Requirements Document — CDM Specification

## Introduction

The Canonical Data Model (CDM) defines 6 recommended collection schemas (Events, Tasks, Contacts, Notes, Emails, and Credentials) that form the shared data language of the Life Engine ecosystem. Every connector normalises external data into these types, and every plugin reads and writes through them.

Schemas are published as Rust structs in `packages/types/src/` (authoritative source), TypeScript interfaces in `packages/plugin-sdk-js/src/index.ts`, and JSON Schema files in `docs/schemas/`. The Rust structs are the single source of truth.

This document specifies requirements for schema definitions, common fields, the extensions convention, plugin-scoped collections, schema versioning, and implementor guidance.

## Alignment with Product Vision

- **Interoperability** — A single shared schema enables any plugin or connector to read/write data without per-integration mapping.
- **Developer experience** — Plugin and connector authors use well-documented, pre-defined types rather than inventing their own schemas.
- **Extensibility** — The namespaced `ext` convention lets plugins attach custom fields without conflicting with each other or with Core fields.
- **Stability** — Additive-only versioning within a major release guarantees backward compatibility for existing plugins.
- **Round-trip fidelity** — Implementor guidance ensures connectors preserve external data through sync cycles using `source`, `source_id`, and `ext` fields.

## Requirements

### Requirement 1 — Common Fields

**User Story:** As a plugin author, I want every CDM collection to share a consistent set of common fields so that I can write generic logic for identity, provenance, and timestamps.

#### Acceptance Criteria

- 1.1. WHEN a CDM collection struct is defined THEN it SHALL include the fields `id` (UUID v4), `source` (String), `source_id` (String), `created_at` (DateTime UTC), and `updated_at` (DateTime UTC).
- 1.2. WHEN a record is created THEN Core SHALL generate a new UUID v4 for the `id` field; source system IDs SHALL NOT be reused as CDM `id` values.
- 1.3. WHEN the `id` field is serialised THEN it SHALL be a lowercase hyphenated string (e.g. `"550e8400-e29b-41d4-a716-446655440000"`).
- 1.4. WHEN the `created_at` or `updated_at` fields are serialised THEN they SHALL use ISO 8601 UTC format.
- 1.5. WHEN a CDM collection is not Credentials THEN it SHALL include an `ext` field of type `Option<serde_json::Value>`.
- 1.6. WHEN the Credentials collection is defined THEN it SHALL NOT include an `ext` field; the `claims` field serves that purpose.

---

### Requirement 2 — Events Collection

**User Story:** As a connector author, I want a shared Events schema so that calendar data from different sources (CalDAV, Google Calendar, Outlook) can be stored and queried uniformly.

#### Acceptance Criteria

- 2.1. WHEN the Events collection is defined THEN it SHALL include the fields `title` (String, required), `start` (DateTime UTC, required), `end` (DateTime UTC, required), `recurrence` (Option String, iCal RRULE format), `attendees` (Vec String, defaults to empty), `location` (Option String), and `description` (Option String), in addition to common fields.
- 2.2. WHEN the `attendees` field is empty THEN it SHALL be omitted from serialisation.
- 2.3. WHEN the `recurrence` field is present THEN its value SHALL conform to iCal RRULE format (e.g. `"FREQ=WEEKLY;BYDAY=MO"`).

---

### Requirement 3 — Tasks Collection

**User Story:** As a connector author, I want a shared Tasks schema so that to-do items from different task managers can be stored and queried uniformly.

#### Acceptance Criteria

- 3.1. WHEN the Tasks collection is defined THEN it SHALL include the fields `title` (String, required), `description` (Option String), `status` (TaskStatus enum, required), `priority` (TaskPriority enum, required), `due_date` (Option DateTime UTC), and `labels` (Vec String, defaults to empty), in addition to common fields.
- 3.2. WHEN the `status` field is serialised THEN it SHALL be one of: `pending`, `active`, `completed`, `cancelled` (lowercase strings).
- 3.3. WHEN the `priority` field is serialised THEN it SHALL be one of: `none`, `low`, `medium`, `high`, `critical` (lowercase strings).

---

### Requirement 4 — Contacts Collection

**User Story:** As a connector author, I want a shared Contacts schema with structured name, communication channels, and addresses so that contact data from different sources merges cleanly.

#### Acceptance Criteria

- 4.1. WHEN the Contacts collection is defined THEN it SHALL include the fields `name` (ContactName, required), `emails` (Vec EmailAddress, defaults to empty), `phones` (Vec PhoneNumber, defaults to empty), `addresses` (Vec PostalAddress, defaults to empty), and `organisation` (Option String), in addition to common fields.
- 4.2. WHEN the `name` field is defined THEN it SHALL be a nested object with `given` (String, required), `family` (String, required), and `display` (String, required).
- 4.3. WHEN an `EmailAddress` is defined THEN it SHALL include `address` (String, required), `type` (Option String, serialised as `"type"` in JSON), and `primary` (Option bool).
- 4.4. WHEN a `PhoneNumber` is defined THEN it SHALL include `number` (String, required) and `type` (Option String, serialised as `"type"` in JSON).
- 4.5. WHEN a `PostalAddress` is defined THEN it SHALL include optional fields `street`, `city`, `state`, `postcode`, and `country`.

---

### Requirement 5 — Notes Collection

**User Story:** As a plugin author, I want a shared Notes schema so that text notes from different sources can be stored with consistent structure.

#### Acceptance Criteria

- 5.1. WHEN the Notes collection is defined THEN it SHALL include the fields `title` (String, required), `body` (String, required), and `tags` (Vec String, defaults to empty), in addition to common fields.
- 5.2. WHEN the `tags` field is empty THEN it SHALL be omitted from serialisation.
- 5.3. WHEN the `body` field contains content THEN it SHALL accept both plain text and markdown.

---

### Requirement 6 — Emails Collection

**User Story:** As a connector author, I want a shared Emails schema so that messages from different email providers can be stored and queried uniformly.

#### Acceptance Criteria

- 6.1. WHEN the Emails collection is defined THEN it SHALL include the fields `from` (String, required), `to` (Vec String, required), `cc` (Vec String, defaults to empty), `bcc` (Vec String, defaults to empty), `subject` (String, required), `body_text` (String, required), `body_html` (Option String), `thread_id` (Option String), `labels` (Vec String, defaults to empty), and `attachments` (Vec EmailAttachment, defaults to empty), in addition to common fields.
- 6.2. WHEN an `EmailAttachment` is defined THEN it SHALL include `file_id` (String, required), `filename` (String, required), `mime_type` (String, required), and `size` (u64, required).
- 6.3. WHEN the `cc`, `bcc`, or `labels` fields are empty THEN they SHALL be omitted from serialisation.

---

### Requirement 7 — Credentials Collection

**User Story:** As a plugin author, I want a shared Credentials schema for storing tokens, keys, and identity documents so that credential management is consistent across the system.

#### Acceptance Criteria

- 7.1. WHEN the Credentials collection is defined THEN it SHALL include the fields `type` (CredentialType enum, required), `issuer` (String, required), `issued_date` (String ISO 8601, required), `expiry_date` (Option String ISO 8601), and `claims` (serde_json::Value, required), in addition to common fields except `ext`.
- 7.2. WHEN the `type` field is serialised THEN it SHALL be one of: `oauth_token`, `api_key`, `identity_document`, `passkey` (snake_case strings). The Rust field name SHALL be `credential_type` with `#[serde(rename = "type")]`.
- 7.3. WHEN the `claims` field is written THEN Core SHALL treat it as an opaque JSON value and SHALL NOT validate its contents.

---

### Requirement 8 — Extensions Convention

**User Story:** As a plugin author, I want to attach custom fields to CDM records under my plugin's namespace so that my data coexists with other plugins without conflicts.

#### Acceptance Criteria

- 8.1. WHEN a plugin writes to the `ext` field THEN the key SHALL be the plugin's reverse-domain ID from its `manifest.toml` (e.g. `ext.com.life-engine.github`).
- 8.2. WHEN a plugin writes to the `ext` field THEN the value under its namespace SHALL be a JSON object.
- 8.3. WHEN a plugin attempts to read or write another plugin's extension namespace THEN the system SHALL restrict access to the plugin's own namespace only.
- 8.4. WHEN Core performs a write operation on a record with existing extension data THEN it SHALL merge extension namespaces, not replace the entire `ext` field.
- 8.5. WHEN the `ext` field is omitted entirely THEN the record SHALL be considered valid.
- 8.6. WHEN extension data is stored THEN Core SHALL treat it as opaque and SHALL NOT validate it against any schema.
- 8.7. WHEN the prefix `org.life-engine.*` is used as an extension namespace THEN it SHALL be reserved for first-party extensions only.

---

### Requirement 9 — Plugin-Scoped Collections

**User Story:** As a plugin author, I want to define private collections for data that does not fit the 6 CDM types so that I can store plugin-specific structured data.

#### Acceptance Criteria

- 9.1. WHEN a plugin declares a private collection THEN the collection name SHALL be prefixed with the plugin's reverse-domain ID (e.g. `com.life-engine.todos.custom_views`).
- 9.2. WHEN a plugin declares a private collection THEN it SHALL be declared in the plugin's `manifest.toml`.
- 9.3. WHEN a plugin-scoped collection is stored THEN the adapter SHALL store it like any other collection.
- 9.4. WHEN a plugin attempts to access another plugin's private collection THEN the system SHALL deny access unless the owning plugin explicitly grants a capability.

---

### Requirement 10 — Schema Versioning

**User Story:** As a Core developer, I want schema changes to be additive-only within a major version so that existing plugins do not break on minor SDK updates.

#### Acceptance Criteria

- 10.1. WHEN a minor SDK release adds new fields to a CDM schema THEN those fields SHALL be optional.
- 10.2. WHEN CDM schemas are compared between minor releases THEN no required fields SHALL have been removed and no field types SHALL have been changed.
- 10.3. WHEN a major SDK version introduces breaking changes THEN the schema versioning rules defined in the schema-versioning-rules document SHALL be followed.

---

### Requirement 11 — Rust Struct Definitions

**User Story:** As a Rust plugin author, I want canonical types available as importable structs with serde derives so that I can serialise and deserialise records without manual schema definitions.

#### Acceptance Criteria

- 11.1. WHEN a Rust struct is defined for a CDM collection THEN it SHALL have `Serialize`, `Deserialize`, `Clone`, and `Debug` derives.
- 11.2. WHEN a struct field is required THEN the Rust type SHALL be a non-optional type (e.g. `String`, not `Option<String>`).
- 11.3. WHEN a struct field is optional THEN the Rust type SHALL be `Option<T>` and serialise with `#[serde(skip_serializing_if = "Option::is_none")]`.
- 11.4. WHEN a Vec field defaults to empty THEN it SHALL use `#[serde(default)]` and `#[serde(skip_serializing_if = "Vec::is_empty")]` where specified.

---

### Requirement 12 — JSON Schema Files

**User Story:** As a Core developer, I want JSON Schema files for all CDM collections so that records can be validated programmatically and documentation can be generated.

#### Acceptance Criteria

- 12.1. WHEN the repository is built THEN JSON Schema files for all 6 collections SHALL exist in `docs/schemas/`.
- 12.2. WHEN a JSON Schema file is loaded THEN it SHALL conform to JSON Schema Draft 2020-12.
- 12.3. WHEN a field has enum constraints (e.g. Task `status`, Credential `type`) THEN the JSON Schema SHALL define the allowed values in an `enum` array.
- 12.4. WHEN test fixtures for a collection are validated against its JSON Schema THEN valid fixtures SHALL pass and invalid fixtures SHALL fail with descriptive errors.

---

### Requirement 13 — Implementor Guidance

**User Story:** As a connector author, I want clear guidance on mapping external data to CDM fields so that I preserve data fidelity during sync operations.

#### Acceptance Criteria

- 13.1. WHEN a connector sets the `source` field THEN it SHALL use a short, stable identifier for the external system (e.g. `"imap"`, `"caldav"`, `"google-calendar"`) that remains consistent across syncs.
- 13.2. WHEN a connector sets the `source_id` field THEN it SHALL use the external system's unique identifier for the record, enabling deduplication via the `(source, source_id)` pair.
- 13.3. WHEN the external system has fields that do not map to any CDM field THEN the connector SHALL store them in the `ext` namespace under the connector's plugin ID.
- 13.4. WHEN a connector stores timestamps THEN it SHALL normalise them to UTC before writing.
- 13.5. WHEN a connector needs to store data types with no CDM equivalent THEN it SHALL use plugin-scoped collections rather than overloading extension fields.
