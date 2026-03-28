<!--
domain: schema-and-validation
status: draft
tier: 1
updated: 2026-03-28
-->

# Schema and Validation Spec

## Overview

This spec defines how Life Engine validates data on write, manages JSON Schemas for collections, enforces extension field namespacing, handles index hints, and governs schema evolution. All schemas use JSON Schema draft 2020-12. Plugins declare schemas in their `manifest.toml` alongside collection definitions. The system distinguishes between CDM recommended schemas (shipped with the SDK, referenced via the `cdm:` prefix) and plugin collection schemas (declared per-collection in the manifest).

Validation applies only to write operations. Read operations are never validated. The default behaviour is permissive — extra fields are accepted and stored unless the collection opts into strict mode.

## Goals

- Consistent schema format — all collection schemas use JSON Schema draft 2020-12, whether CDM or plugin-defined
- Write-time validation — reject invalid data at the boundary before it reaches storage, following the Parse Don't Validate principle
- Permissive by default, strict by opt-in — unknown fields are accepted unless `strict = true`, giving plugin authors flexibility without sacrificing rigour when needed
- Extension field isolation — each plugin owns its `ext.{plugin_id}` namespace and cannot modify another plugin's extensions
- Index hints as adapter-level concerns — plugins declare desired indexes; adapters honour them if capable
- Additive-only evolution — schema changes within a major SDK version must be backward-compatible

## User Stories

- As a plugin author, I want to declare a JSON Schema for my collection so that the system rejects malformed writes with clear error messages.
- As a plugin author, I want to reference CDM schemas via a `cdm:` prefix so that I do not need to duplicate standard schema definitions.
- As a plugin author, I want to add extension fields to shared collections so that my plugin can store additional data alongside CDM records without conflicting with other plugins.
- As a plugin author, I want to declare index hints in my manifest so that adapters can optimise queries on frequently filtered fields.
- As a user, I want the system to protect system-managed fields (`id`, `created_at`, `updated_at`) so that timestamps and identifiers remain trustworthy.
- As a maintainer, I want schema evolution rules enforced so that plugin updates do not silently break existing data.

## Functional Requirements

- The system must validate write operations (`create`, `update`, `partial_update`, and batch variants) against the declared JSON Schema for the target collection.
- The system must resolve `cdm:` prefixed schema references to SDK-shipped schema files.
- The system must reject writes that attempt to set `created_at` or `updated_at`, and silently overwrite any caller-provided values for these fields.
- The system must allow callers to optionally provide `id` on create but generate one if omitted.
- The system must enforce extension field namespace isolation — writes to another plugin's `ext.{plugin_id}` namespace result in `StorageError::CapabilityDenied`.
- The system must validate extension fields against `extension_schema` when declared.
- The system must reject unknown fields on write when `strict = true` with `StorageError::ValidationFailed`.
- The system must skip validation entirely for schemaless collections (no `schema` field in manifest).
- The system must pass index hints to the adapter via `CollectionDescriptor` during `migrate`, and adapters without indexing capability must silently ignore them.
- The system must enforce additive-only schema changes within a major SDK version.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
