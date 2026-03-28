<!--
domain: schema-and-validation
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Requirements Document — Schema and Validation

## Introduction

Schema and validation governs how Life Engine ensures data integrity at the write boundary. All schemas use JSON Schema draft 2020-12. Plugins declare schemas in `manifest.toml` alongside collection definitions, either referencing CDM schemas via the `cdm:` prefix or providing custom schema files. Validation runs on every write operation (`create`, `update`, `partial_update`, and batch variants). Read operations bypass validation entirely.

The system is permissive by default — unknown fields pass through unless the collection enables strict mode. Extension fields use namespaced keys (`ext.{plugin_id}.{field_name}`) and are stored inline in the document. Each plugin can only write to its own namespace. Index declarations are advisory hints honoured by capable adapters.

## Alignment with Product Vision

- **Parse, Don't Validate** — Schema validation at the write boundary ensures only valid data reaches storage; downstream code trusts the types
- **Single Source of Truth** — CDM schemas shipped with the SDK define canonical field shapes; plugins reference them rather than duplicating definitions
- **Principle of Least Privilege** — Extension namespace isolation prevents plugins from modifying each other's data
- **Open/Closed Principle** — Index hints decouple plugin intent from adapter implementation; new adapters can support or ignore indexing without plugin changes
- **The Pit of Success** — Permissive defaults let plugin authors start fast; strict mode is available when rigour is needed
- **Defence in Depth** — System-managed fields are protected from caller tampering, ensuring trustworthy timestamps and identifiers

## Requirements

### Requirement 1 — Schema Declaration and Resolution

**User Story:** As a plugin author, I want to declare a JSON Schema for my collection in `manifest.toml` so that the system validates my data on write without requiring custom validation code.

#### Acceptance Criteria

- 1.1. WHEN a plugin declares `schema = "schemas/foo.json"` for a collection THEN the system SHALL resolve the path relative to the plugin's root directory and load the JSON Schema file.
- 1.2. WHEN a plugin declares `schema = "cdm:contacts"` for a collection THEN the system SHALL resolve the reference to the SDK-shipped JSON Schema for the `contacts` CDM type.
- 1.3. WHEN a plugin omits the `schema` field for a collection THEN the system SHALL treat the collection as schemaless and skip all validation on write.
- 1.4. WHEN a schema file cannot be loaded or parsed THEN the system SHALL reject plugin installation with a clear error identifying the collection and file path.

### Requirement 2 — Write-Time Validation Sequence

**User Story:** As a plugin author, I want the system to validate my data before storing it so that malformed records are rejected with clear error messages.

#### Acceptance Criteria

- 2.1. WHEN a write operation is submitted THEN the system SHALL first check that `created_at` and `updated_at` are not set by the caller, silently overwriting any caller-provided values.
- 2.2. WHEN a write operation is submitted for a collection with a declared schema THEN the system SHALL validate the document body against the JSON Schema after base field checks.
- 2.3. WHEN a write operation is submitted for a collection with a declared `extension_schema` THEN the system SHALL validate extension fields against the extension schema after body validation.
- 2.4. WHEN validation fails THEN the system SHALL reject the write with `StorageError::ValidationFailed` and return an error message identifying the specific field and constraint that failed.
- 2.5. WHEN a write operation is submitted for a schemaless collection THEN the system SHALL skip validation entirely and proceed to storage.

### Requirement 3 — Strict Mode

**User Story:** As a plugin author, I want to opt into strict validation so that only fields defined in my schema are accepted, preventing data drift.

#### Acceptance Criteria

- 3.1. WHEN a collection declares `strict = true` and a write includes fields not defined in the schema THEN the system SHALL reject the write with `StorageError::ValidationFailed`.
- 3.2. WHEN a collection declares `strict = false` (or omits `strict`) and a write includes fields not defined in the schema THEN the system SHALL accept and store the extra fields.

### Requirement 4 — System-Managed Base Fields

**User Story:** As a user, I want system-managed fields to be trustworthy so that I can rely on timestamps and identifiers for auditing and ordering.

#### Acceptance Criteria

- 4.1. WHEN a create operation is submitted without an `id` THEN `StorageContext` SHALL generate a unique identifier and set it on the document.
- 4.2. WHEN a create operation is submitted with a caller-provided `id` THEN `StorageContext` SHALL use the provided value.
- 4.3. WHEN any write operation is submitted THEN `StorageContext` SHALL set `updated_at` to the current timestamp, overwriting any caller-provided value.
- 4.4. WHEN a create operation is submitted THEN `StorageContext` SHALL set `created_at` to the current timestamp, overwriting any caller-provided value.
- 4.5. WHEN an update operation is submitted that attempts to change `id` or `created_at` THEN the system SHALL reject the write with an error indicating these fields are immutable.

### Requirement 5 — Extension Field Namespacing

**User Story:** As a plugin author, I want to add custom fields to shared collections so that my plugin can extend CDM records without conflicting with other plugins.

#### Acceptance Criteria

- 5.1. WHEN a plugin writes to a collection THEN the system SHALL allow the plugin to set fields under its own `ext.{plugin_id}` namespace.
- 5.2. WHEN a plugin attempts to write to another plugin's `ext.{other_plugin_id}` namespace THEN the system SHALL reject the write with `StorageError::CapabilityDenied`.
- 5.3. WHEN a plugin reads a document that contains extension fields from other plugins THEN the system SHALL include all extension fields in the returned document.
- 5.4. WHEN a plugin declares `extension_schema` and `extensions` in its manifest THEN the system SHALL validate extension field values against the declared schema on write.
- 5.5. WHEN a plugin declares `extension_indexes` in its manifest THEN the system SHALL pass the extension field paths as index hints to the adapter.

### Requirement 6 — Index Hints

**User Story:** As a plugin author, I want to declare index hints so that adapters can optimise queries on fields I frequently filter by.

#### Acceptance Criteria

- 6.1. WHEN a plugin declares `indexes` for a collection THEN the system SHALL include those field paths in the `CollectionDescriptor` passed to the adapter during `migrate`.
- 6.2. WHEN the active adapter reports `AdapterCapabilities.indexing = true` THEN the adapter SHALL create indexes for the declared field paths.
- 6.3. WHEN the active adapter reports `AdapterCapabilities.indexing = false` THEN the adapter SHALL silently ignore index hints without error.
- 6.4. WHEN a CDM schema ships default index hints THEN those hints SHALL be merged with any plugin-declared indexes, with plugin declarations taking precedence on conflict.

### Requirement 7 — Schema Evolution

**User Story:** As a maintainer, I want schema evolution rules enforced so that plugin updates do not silently break existing data.

#### Acceptance Criteria

- 7.1. WHEN a schema update adds a new optional field THEN the system SHALL accept the updated schema without requiring a migration.
- 7.2. WHEN a schema update removes a field or changes a field's type THEN the system SHALL treat this as a breaking change and require a new major SDK version.
- 7.3. WHEN a plugin updates its schema within the same major SDK version THEN the system SHALL verify the new schema is a superset of the previous schema and reject non-additive changes with a clear error.
