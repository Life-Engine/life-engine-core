<!--
domain: canonical-data-models
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Requirements Document — Canonical Data Models

## Introduction

Life Engine defines 7 canonical collection schemas (Events, Tasks, Contacts, Notes, Emails, Files, Credentials) as the shared data language across all plugins and connectors. These schemas are published as Rust structs in `packages/types` and JSON Schema files in `.odm/doc/schemas/`. The plugin SDK (`packages/plugin-sdk`) re-exports all types so plugin authors have a single dependency.

This spec also covers the `PipelineMessage` envelope — the standard data format flowing through workflow pipelines. `PipelineMessage` wraps canonical and custom payloads with metadata (correlation ID, source, timestamp, auth context), giving every workflow step a uniform input/output contract.

This document specifies the requirements for schema definitions, the `PipelineMessage` envelope, extension handling, versioning, and private collection support.

## Alignment with Product Vision

- **Interoperability** — A single shared schema enables any plugin or connector to read/write canonical data without per-integration mapping.
- **Uniform pipeline contract** — `PipelineMessage` gives every workflow step the same input/output shape, so plugins compose without custom adapters.
- **Developer experience** — Plugin authors import canonical types and `PipelineMessage` from `plugin-sdk` rather than defining their own schemas or envelope formats.
- **Extensibility** — The namespaced extensions convention lets plugins attach custom fields without conflicting with each other. `TypedPayload::Custom` lets plugins carry non-canonical data through pipelines.
- **Stability** — Additive-only versioning within a major release guarantees backward compatibility.

## Requirements

### Requirement 1 — Rust Struct Definitions

**User Story:** As a Rust plugin author, I want canonical types available as importable structs with serde derives, so that I can serialize and deserialize records without manual schema definitions.

#### Acceptance Criteria

- 1.1. WHEN a Rust plugin imports `plugin-sdk` THEN all 7 canonical collection types SHALL be available as public structs with `Serialize`, `Deserialize`, `Clone`, and `Debug` derives.
- 1.2. WHEN a struct field is marked as required in the schema THEN the Rust type SHALL be a non-optional type (e.g., `String`, not `Option<String>`).
- 1.3. WHEN a struct field is marked as optional in the schema THEN the Rust type SHALL be `Option<T>` and serialize with `#[serde(skip_serializing_if = "Option::is_none")]`.
- 1.4. WHEN the Contacts collection has a nested `name` object THEN the Rust definition SHALL use a separate `ContactName` struct with `given`, `family`, and `display` fields.

---

### Requirement 2 — PipelineMessage Envelope

**User Story:** As a plugin author, I want a standard envelope type for pipeline data, so that my plugin can process any workflow step without custom input/output wiring.

#### Acceptance Criteria

- 2.1. WHEN `packages/types` is compiled THEN a public `PipelineMessage` struct SHALL exist with two fields: `metadata: MessageMetadata` and `payload: TypedPayload`.
- 2.2. WHEN a `MessageMetadata` struct is constructed THEN it SHALL contain at minimum: `correlation_id` (string), `source` (string), `timestamp` (ISO 8601 datetime), and `auth_context` (struct or optional struct for unauthenticated flows).
- 2.3. WHEN a `TypedPayload` enum is constructed THEN it SHALL have two variants: `Cdm(CdmType)` for canonical collection data and `Custom(SchemaValidated<Value>)` for plugin-defined data validated against a JSON Schema.
- 2.4. WHEN a plugin imports `plugin-sdk` THEN `PipelineMessage`, `MessageMetadata`, and `TypedPayload` SHALL be re-exported and available as public types.
- 2.5. WHEN a plugin step receives input THEN the input SHALL be a `PipelineMessage`, and WHEN a plugin step produces output THEN the output SHALL be a `PipelineMessage`.

---

### Requirement 3 — JSON Schema Files

**User Story:** As a Core developer, I want JSON Schema files for all canonical collections, so that records can be validated programmatically and documentation can be generated.

#### Acceptance Criteria

- 3.1. WHEN the monorepo is built THEN JSON Schema files for all 7 collections SHALL exist at `.odm/doc/schemas/{collection}.schema.json`.
- 3.2. WHEN a JSON Schema file is loaded THEN it SHALL conform to JSON Schema Draft 2020-12.
- 3.3. WHEN test fixtures for a collection are validated against the schema THEN valid fixtures SHALL pass and invalid fixtures SHALL fail with descriptive errors.
- 3.4. WHEN a field has enum constraints (e.g., Task `status`) THEN the JSON Schema SHALL define the allowed values in an `enum` array.

---

### Requirement 4 — Extensions Convention

**User Story:** As a plugin author, I want to attach custom fields to canonical records under my plugin's namespace, so that my data coexists with other plugins without conflicts.

#### Acceptance Criteria

- 4.1. WHEN a plugin writes to the `extensions` field THEN the key SHALL be the plugin's reverse-domain ID (e.g., `com.life-engine.github`).
- 4.2. WHEN a plugin attempts to write to another plugin's extension namespace THEN Core SHALL reject the write with an error.
- 4.3. WHEN Core performs a sync or merge operation THEN it SHALL preserve all extension data from all namespaces.
- 4.4. WHEN the `extensions` field is omitted entirely THEN the record SHALL be considered valid.
- 4.5. WHEN the Credentials collection is written THEN the record SHALL NOT include an `extensions` field; the `claims` object carries type-specific data instead.

---

### Requirement 5 — Schema Versioning

**User Story:** As a Core developer, I want schema changes to be additive-only within a major version, so that existing plugins do not break on minor SDK updates.

#### Acceptance Criteria

- 5.1. WHEN a minor SDK release adds new fields to a canonical schema THEN those fields SHALL be optional.
- 5.2. WHEN canonical schemas are compared between minor releases THEN no required fields SHALL have been removed and no field types SHALL have been changed.
- 5.3. WHEN a major SDK version introduces breaking changes THEN the previous major version SHALL continue to receive security fixes for 12 months.

---

### Requirement 6 — Private Collections

**User Story:** As a plugin author, I want to define private collections for data that does not fit the 7 canonical types, so that I can store plugin-specific structured data.

#### Acceptance Criteria

- 6.1. WHEN a plugin declares a private collection in its `manifest.toml` THEN the collection name SHALL be prefixed with the plugin ID (e.g., `com.life-engine.todos.checklists`).
- 6.2. WHEN a plugin provides a JSON Schema for its private collection in `manifest.toml` THEN Core SHALL validate records against that schema on write.
- 6.3. WHEN a plugin attempts to access another plugin's private collection THEN Core SHALL deny the access unless the owning plugin explicitly exposes it.

---

### Requirement 7 — Plugin SDK Re-exports

**User Story:** As a plugin author, I want a single dependency (`plugin-sdk`) that gives me all canonical types and the `PipelineMessage` envelope, so that I do not need to depend on internal crates.

#### Acceptance Criteria

- 7.1. WHEN a plugin depends on `plugin-sdk` THEN all 7 canonical collection types SHALL be importable from `plugin-sdk`.
- 7.2. WHEN a plugin depends on `plugin-sdk` THEN `PipelineMessage`, `MessageMetadata`, and `TypedPayload` SHALL be importable from `plugin-sdk`.
- 7.3. WHEN canonical types or `PipelineMessage` are updated in `packages/types` THEN the re-exports in `plugin-sdk` SHALL reflect those changes without manual synchronization.
