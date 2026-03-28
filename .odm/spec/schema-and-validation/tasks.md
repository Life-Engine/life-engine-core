<!--
domain: schema-and-validation
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Implementation Plan — Schema and Validation

## Task Overview

This plan implements schema loading, write-time validation, extension field enforcement, index hint propagation, and schema evolution checks. Work begins with the `SchemaRegistry` and schema resolution, then builds the validation pipeline, extension namespace enforcement, strict mode, and finally schema evolution checking. Each task targets a narrow slice of functionality that can be tested in isolation.

**Progress:** 0 / 14 tasks complete

## Steering Document Compliance

- Schema validation at the write boundary follows Parse, Don't Validate
- CDM schema references via `cdm:` prefix follow Single Source of Truth
- Extension namespace isolation follows Principle of Least Privilege
- Index hints decoupled from adapter implementation follow Open/Closed Principle
- Permissive defaults with strict opt-in follow The Pit of Success
- Additive-only evolution within a major version follows Defence in Depth

## Atomic Task Requirements

- **File Scope:** 1-3 related files maximum
- **Time Boxing:** 15-30 minutes per task
- **Single Purpose:** one testable outcome per task
- **Specific Files:** exact file paths specified
- **Agent-Friendly:** clear input/output, minimal context switching

---

## 1.1 — Schema Registry and Resolution

> spec: ./brief.md

- [ ] Define SchemaRegistry struct with resolve and get methods
  <!-- file: packages/storage/src/schema_registry.rs -->
  <!-- purpose: Create SchemaRegistry with HashMap<(PluginId, CollectionName), CompiledSchema> and resolve() method that handles cdm: prefix, relative paths, and schemaless collections -->
  <!-- requirements: 1.1, 1.2, 1.3 -->

- [ ] Define CollectionDeclaration struct
  <!-- file: packages/types/src/collection.rs -->
  <!-- purpose: Define CollectionDeclaration with schema, access, indexes, strict, extensions, extension_schema, and extension_indexes fields parsed from manifest.toml -->
  <!-- requirements: 1.1, 1.2, 3.1, 3.2 -->

- [ ] Add extension schema support to SchemaRegistry
  <!-- file: packages/storage/src/schema_registry.rs -->
  <!-- purpose: Add resolve_extension_schema() and get_extension_schema() methods to SchemaRegistry for loading and caching extension_schema files -->
  <!-- requirements: 5.4 -->

## 1.2 — Schema Loading and Error Handling

> spec: ./brief.md

- [ ] Implement schema file loading and compilation
  <!-- file: packages/storage/src/schema_loader.rs -->
  <!-- purpose: Implement load_and_compile() that reads a JSON Schema file, parses it with the jsonschema crate, and returns a CompiledSchema or SchemaError::NotFound / SchemaError::ParseFailed -->
  <!-- requirements: 1.4 -->

- [ ] Define SchemaError type
  <!-- file: packages/types/src/errors.rs -->
  <!-- purpose: Add SchemaError enum with NotFound, ParseFailed, and BreakingChange variants to the existing error types -->
  <!-- requirements: 1.4, 7.3 -->

## 2.1 — Write-Time Validation Pipeline

> spec: ./brief.md

- [ ] Implement base field enforcement for create operations
  <!-- file: packages/storage/src/validation.rs -->
  <!-- purpose: Implement enforce_base_fields_create() that sets created_at and updated_at to current timestamp, generates id if missing, and overwrites any caller-provided timestamp values -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4 -->

- [ ] Implement base field enforcement for update operations
  <!-- file: packages/storage/src/validation.rs -->
  <!-- purpose: Implement enforce_base_fields_update() that sets updated_at to current timestamp and rejects attempts to change id or created_at with StorageError::ValidationFailed -->
  <!-- requirements: 4.3, 4.5 -->

- [ ] Implement validate_write orchestrator function
  <!-- file: packages/storage/src/validation.rs -->
  <!-- purpose: Implement validate_write() that runs the four-step validation sequence: base field check, body schema validation, extension schema validation, strict mode check -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.4, 2.5 -->

## 2.2 — Strict Mode

> spec: ./brief.md

- [ ] Implement strict mode unknown field rejection
  <!-- file: packages/storage/src/validation.rs -->
  <!-- purpose: Implement reject_unknown_fields() that walks document keys and rejects any field not in schema properties, exempting system fields and ext.* fields -->
  <!-- requirements: 3.1, 3.2 -->

## 3.1 — Extension Field Namespace Enforcement

> spec: ./brief.md

- [ ] Implement extension namespace enforcement
  <!-- file: packages/storage/src/extensions.rs -->
  <!-- purpose: Implement enforce_extension_namespace() that checks all ext.* keys in a document and returns StorageError::CapabilityDenied if any belong to a different plugin -->
  <!-- requirements: 5.1, 5.2, 5.3 -->

## 3.2 — Index Hint Propagation

> spec: ./brief.md

- [ ] Build CollectionDescriptor from manifest declarations
  <!-- file: packages/storage/src/descriptor.rs -->
  <!-- purpose: Implement build_collection_descriptor() that merges CDM default indexes with plugin-declared indexes and extension_indexes into a CollectionDescriptor for the adapter -->
  <!-- requirements: 6.1, 6.4 -->

- [ ] Pass index hints to adapter during migrate
  <!-- file: packages/storage/src/migrate.rs -->
  <!-- purpose: Implement migrate logic that passes CollectionDescriptor to the adapter, respecting AdapterCapabilities.indexing to create or skip indexes -->
  <!-- requirements: 6.1, 6.2, 6.3 -->

## 4.1 — Schema Evolution

> spec: ./brief.md

- [ ] Implement schema evolution compatibility check
  <!-- file: packages/storage/src/schema_evolution.rs -->
  <!-- purpose: Implement check_compatibility() that compares old and new compiled schemas, allowing additive changes and rejecting field removals, type changes, and new required fields with SchemaError::BreakingChange -->
  <!-- requirements: 7.1, 7.2, 7.3 -->

- [ ] Integrate evolution check into plugin registration
  <!-- file: packages/storage/src/schema_registry.rs -->
  <!-- purpose: Update SchemaRegistry.resolve() to check for an existing schema and run check_compatibility() before replacing it, rejecting non-additive changes -->
  <!-- requirements: 7.3 -->
