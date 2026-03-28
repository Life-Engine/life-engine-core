<!--
domain: storage-context
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Implementation Plan — Storage Context

## Task Overview

This plan implements `StorageContext` as the single gateway between callers and `StorageRouter`. Work begins with the core struct and caller identity types, then builds permission enforcement, collection scoping, schema validation, base field management, extension field isolation, the fluent query builder, host function bindings, audit event emission, the watch-to-event-bus bridge, and credential encryption. Each task targets a narrow slice of functionality that can be tested in isolation.

**Progress:** 0 / 22 tasks complete

## Atomic Task Requirements

- **File Scope:** 1-3 related files maximum
- **Time Boxing:** 15-30 minutes per task
- **Single Purpose:** one testable outcome per task
- **Specific Files:** exact file paths specified
- **Agent-Friendly:** clear input/output, minimal context switching

---

## 1.1 — Core Struct and Caller Identity

> spec: ./brief.md

- [ ] Define CallerIdentity enum and CollectionDeclaration types
  <!-- file: packages/types/src/storage_context.rs -->
  <!-- purpose: Define CallerIdentity (Plugin, System), CollectionDeclaration, and AccessLevel types -->
  <!-- requirements: 1.8, 2.1 -->

- [ ] Define StorageError enum
  <!-- file: packages/types/src/storage_error.rs -->
  <!-- purpose: Define CapabilityDenied, CollectionAccessDenied, ExtensionNamespaceDenied, ValidationFailed, DocumentNotFound, ImmutableFieldViolation, AdapterError, EncryptionError variants -->
  <!-- requirements: 1.7, 2.3, 3.3, 5.4 -->

- [ ] Define StorageContext struct with constructor
  <!-- file: packages/storage-context/src/lib.rs -->
  <!-- purpose: Define StorageContext struct holding CallerIdentity, StorageRouter, SchemaRegistry, EventBus, and CryptoService references; implement new() constructor -->
  <!-- requirements: 1.8 -->

## 1.2 — Permission Enforcement

> spec: ./brief.md

- [ ] Implement capability check method
  <!-- file: packages/storage-context/src/permissions.rs -->
  <!-- purpose: Implement check_capability() that verifies the caller's capabilities list contains the required capability string, returning CapabilityDenied on failure; System callers bypass the check -->
  <!-- requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8 -->

- [ ] Add unit tests for permission enforcement
  <!-- file: packages/storage-context/src/permissions.rs -->
  <!-- purpose: Test all six capability strings, test CapabilityDenied for missing capabilities, test System identity bypass -->
  <!-- requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8 -->

## 1.3 — Collection Scoping

> spec: ./brief.md

- [ ] Implement collection scope validation
  <!-- file: packages/storage-context/src/scoping.rs -->
  <!-- purpose: Implement check_collection_access() that validates shared collection declarations and access levels, plugin-scoped namespace ownership, and System identity bypass -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.4 -->

- [ ] Add unit tests for collection scoping
  <!-- file: packages/storage-context/src/scoping.rs -->
  <!-- purpose: Test shared collection read/write/read-write access, plugin-scoped namespace validation, undeclared collection rejection, System identity bypass -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.4 -->

## 2.1 — Schema Validation

> spec: ./brief.md

- [ ] Implement schema validation for write operations
  <!-- file: packages/storage-context/src/validation.rs -->
  <!-- purpose: Implement validate_write() that looks up JSON Schema from SchemaRegistry, validates payload against draft 2020-12, returns ValidationFailed with field-level errors on failure, skips validation when no schema is registered -->
  <!-- requirements: 3.1, 3.2, 3.3, 3.4, 3.5 -->

- [ ] Add unit tests for schema validation
  <!-- file: packages/storage-context/src/validation.rs -->
  <!-- purpose: Test validation pass, validation fail with field details, missing schema skip, extra fields accepted in permissive mode, strict mode rejection -->
  <!-- requirements: 3.1, 3.2, 3.3, 3.4, 3.5 -->

## 2.2 — System-Managed Base Fields

> spec: ./brief.md

- [ ] Implement base field injection and immutability enforcement
  <!-- file: packages/storage-context/src/base_fields.rs -->
  <!-- purpose: Implement inject_base_fields() that generates UUIDv7 id on create if missing, sets created_at and updated_at timestamps, enforces immutability of id and created_at on update -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7 -->

- [ ] Add unit tests for base field management
  <!-- file: packages/storage-context/src/base_fields.rs -->
  <!-- purpose: Test id generation, caller-provided id preservation, created_at/updated_at overwrite, immutability enforcement on update -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7 -->

## 2.3 — Extension Field Handling

> spec: ./brief.md

- [ ] Implement extension namespace enforcement
  <!-- file: packages/storage-context/src/extensions.rs -->
  <!-- purpose: Implement enforce_extension_namespace() that blocks writes to foreign ext namespaces, merges caller's ext namespace with existing data, validates against extension_schema if declared -->
  <!-- requirements: 5.1, 5.2, 5.3, 5.4, 5.5 -->

- [ ] Implement extension index registration
  <!-- file: packages/storage-context/src/extensions.rs -->
  <!-- purpose: Implement register_extension_indexes() that reads extension_indexes from the manifest and registers them with the adapter at plugin load time -->
  <!-- requirements: 5.6 -->

- [ ] Add unit tests for extension field handling
  <!-- file: packages/storage-context/src/extensions.rs -->
  <!-- purpose: Test namespace isolation, foreign namespace rejection, extension schema validation, index registration -->
  <!-- requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.6 -->

## 3.1 — Query Builder

> spec: ./brief.md

- [ ] Implement QueryDescriptor types and fluent builder
  <!-- file: packages/storage-context/src/query.rs -->
  <!-- purpose: Define QueryDescriptor, Filter, FilterOp, Sort, SortDirection structs; implement fluent builder with collection(), filter(), sort(), limit(), cursor(), exec() methods -->
  <!-- requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6 -->

- [ ] Add unit tests for query builder
  <!-- file: packages/storage-context/src/query.rs -->
  <!-- purpose: Test fluent API produces correct QueryDescriptor values, test all FilterOp variants, test sort direction, test limit and cursor -->
  <!-- requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6 -->

## 3.2 — Host Functions

> spec: ./brief.md

- [ ] Implement document host function bindings
  <!-- file: packages/storage-context/src/host_functions.rs -->
  <!-- purpose: Register storage_doc_get, storage_doc_list, storage_doc_count, storage_doc_create, storage_doc_update, storage_doc_partial_update, storage_doc_delete, storage_doc_batch_create, storage_doc_batch_update, storage_doc_batch_delete with the WASM plugin runtime -->
  <!-- requirements: 7.1, 7.2, 7.3, 7.4, 7.5, 7.6, 7.7, 7.8, 7.9, 7.10, 7.17 -->

- [ ] Implement blob host function bindings
  <!-- file: packages/storage-context/src/host_functions.rs -->
  <!-- purpose: Register storage_blob_store, storage_blob_retrieve, storage_blob_delete, storage_blob_exists, storage_blob_list, storage_blob_metadata with the WASM plugin runtime -->
  <!-- requirements: 7.11, 7.12, 7.13, 7.14, 7.15, 7.16, 7.17 -->

## 4.1 — Audit Event Emission

> spec: ./brief.md

- [ ] Implement audit event emission for write operations
  <!-- file: packages/storage-context/src/audit.rs -->
  <!-- purpose: Implement emit_audit() that fires system.storage.created, system.storage.updated, system.storage.deleted, system.blob.stored, system.blob.deleted events to the event bus with origin and without full payloads -->
  <!-- requirements: 8.1, 8.2, 8.3, 8.4, 8.5, 8.6, 8.7, 8.8 -->

- [ ] Add unit tests for audit event emission
  <!-- file: packages/storage-context/src/audit.rs -->
  <!-- purpose: Test each event type is emitted with correct payload structure, test origin field for plugin and system callers, test that read operations produce no events, test that full payloads are excluded -->
  <!-- requirements: 8.1, 8.2, 8.3, 8.4, 8.5, 8.6, 8.7, 8.8 -->

## 4.2 — Watch-to-Event-Bus Bridge

> spec: ./brief.md

- [ ] Implement watch bridge with fallback to write-path emission
  <!-- file: packages/storage-context/src/watch_bridge.rs -->
  <!-- purpose: Implement start_watch_bridge() that subscribes to the adapter's watch stream when native watch is supported, translates ChangeEvent to system.storage.* events, falls back to write-path emission when not supported, prevents duplicate events -->
  <!-- requirements: 9.1, 9.2, 9.3, 9.4, 9.5 -->

## 4.3 — Credential Encryption

> spec: ./brief.md

- [ ] Implement field-level credential encryption and decryption
  <!-- file: packages/storage-context/src/credentials.rs -->
  <!-- purpose: Implement encrypt_credential_fields() and decrypt_credential_fields() that target the credentials collection, encrypting/decrypting sensitive fields (password, token, secret, api_key, private_key) using CryptoService with a derived key -->
  <!-- requirements: 10.1, 10.2, 10.3 -->
