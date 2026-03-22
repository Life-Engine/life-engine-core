<!--
domain: data-layer
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Implementation Plan — Data Layer

## Task Overview

This plan implements the Core data layer across the new crate architecture. Work begins with the `StorageBackend` trait and supporting types in `packages/traits` and `packages/types`, then builds the `StorageContext` query builder in `packages/plugin-sdk`, followed by the SQLite/SQLCipher implementation in `packages/storage-sqlite`. Schema validation, audit logging, credential storage, and data export follow. Each task targets a narrow slice of functionality that can be tested in isolation.

**Progress:** 0 / 18 tasks complete

## Steering Document Compliance

- Universal document model follows Single Source of Truth — one table shape for all plugins
- `StorageBackend` trait in `packages/traits` follows Open/Closed Principle — new backends without plugin code changes
- `StorageContext` in `packages/plugin-sdk` follows The Pit of Success — the easy path is the correct path
- SQLCipher encryption with Argon2id (via `packages/crypto`) follows Defence in Depth
- Schema validation at the boundary follows Parse, Don't Validate
- Plugin data scoped by plugin_id follows Principle of Least Privilege

## Atomic Task Requirements

- **File Scope:** 1-3 related files maximum
- **Time Boxing:** 15-30 minutes per task
- **Single Purpose:** one testable outcome per task
- **Specific Files:** exact file paths specified
- **Agent-Friendly:** clear input/output, minimal context switching

---

## 1.1 — StorageBackend Trait and Query Types

> spec: ./brief.md

- [ ] Define StorageQuery and StorageMutation types
  <!-- file: packages/types/src/storage.rs -->
  <!-- purpose: Define StorageQuery (collection, filters, sort, limit, offset) and StorageMutation (Insert, Update, Delete variants) data structures -->
  <!-- requirements: 1.1, 1.2 -->
  <!-- leverage: existing packages/types/src/ -->

- [ ] Define StorageBackend trait with execute and mutate methods
  <!-- file: packages/traits/src/storage.rs -->
  <!-- purpose: Define async trait with execute(StorageQuery) -> Result<Vec<PipelineMessage>> and mutate(StorageMutation) -> Result<()> -->
  <!-- requirements: 1.1, 1.2 -->
  <!-- leverage: existing packages/traits/src/ -->

- [ ] Re-export storage types from packages/types lib.rs and packages/traits lib.rs
  <!-- file: packages/types/src/lib.rs, packages/traits/src/lib.rs -->
  <!-- purpose: Make StorageQuery, StorageMutation, and StorageBackend available as public API -->
  <!-- requirements: 1.1 -->
  <!-- leverage: existing lib.rs files -->

## 1.2 — StorageContext Query Builder

> spec: ./brief.md

- [ ] Implement StorageContext fluent query builder
  <!-- file: packages/plugin-sdk/src/storage_context.rs -->
  <!-- purpose: Build fluent API with query(), insert(), update(), delete() methods that produce StorageQuery/StorageMutation values -->
  <!-- requirements: 1.2, 1.3, 1.4, 1.5, 1.6 -->
  <!-- leverage: existing packages/plugin-sdk/src/ -->

- [ ] Add query filter methods to StorageContext
  <!-- file: packages/plugin-sdk/src/storage_context.rs -->
  <!-- purpose: Add where_eq(), where_gte(), where_lte(), where_contains() fluent methods that populate StorageQuery filters -->
  <!-- requirements: 8.1, 8.2, 8.3, 8.4 -->
  <!-- leverage: StorageContext from previous task -->

- [ ] Add sort and pagination methods to StorageContext
  <!-- file: packages/plugin-sdk/src/storage_context.rs -->
  <!-- purpose: Add order_by(), order_by_desc(), limit(), offset() fluent methods with 1000 limit cap -->
  <!-- requirements: 9.1, 9.2, 9.3, 9.4, 9.5 -->
  <!-- leverage: StorageContext from previous task -->

## 2.1 — SQLite Schema and Encryption

> spec: ./brief.md

- [ ] Create plugin_data table DDL with indexes
  <!-- file: packages/storage-sqlite/src/schema.rs -->
  <!-- purpose: Define CREATE TABLE plugin_data with id, plugin_id, collection, data, version, created_at, updated_at and composite index -->
  <!-- requirements: 2.1 -->
  <!-- leverage: existing packages/storage-sqlite/src/ -->

- [ ] Create audit_log table DDL with timestamp index
  <!-- file: packages/storage-sqlite/src/schema.rs -->
  <!-- purpose: Define CREATE TABLE audit_log with id, timestamp, event_type, plugin_id, details, created_at and timestamp index -->
  <!-- requirements: 7.1 -->
  <!-- leverage: schema.rs from previous task -->

- [ ] Configure SQLCipher encryption and WAL mode on database open
  <!-- file: packages/storage-sqlite/src/lib.rs -->
  <!-- purpose: Open database with SQLCipher PRAGMA key from Argon2id-derived key (via packages/crypto), enable WAL mode -->
  <!-- requirements: 5.1, 5.2, 5.3, 5.4 -->
  <!-- leverage: packages/crypto for key derivation -->

## 2.2 — StorageBackend Implementation for SQLite

> spec: ./brief.md

- [ ] Implement StorageBackend::execute for SQLite
  <!-- file: packages/storage-sqlite/src/backend.rs -->
  <!-- purpose: Translate StorageQuery to SQL using json_extract, apply filters, sorting, pagination, return Vec<PipelineMessage> -->
  <!-- requirements: 2.2, 2.6, 8.1, 8.2, 8.3, 8.4, 9.1, 9.2, 9.3, 9.4, 9.5 -->
  <!-- leverage: rusqlite dependency, schema.rs -->

- [ ] Implement StorageBackend::mutate for SQLite
  <!-- file: packages/storage-sqlite/src/backend.rs -->
  <!-- purpose: Translate StorageMutation to INSERT/UPDATE/DELETE SQL with optimistic concurrency via version column -->
  <!-- requirements: 2.1, 2.3, 2.4, 2.5 -->
  <!-- leverage: rusqlite dependency, schema.rs -->

## 3.1 — Plugin Data Isolation

> spec: ./brief.md

- [ ] Add plugin_id scoping to all storage operations
  <!-- file: packages/storage-sqlite/src/backend.rs -->
  <!-- purpose: Ensure all queries and mutations include WHERE plugin_id = ? clause, preventing cross-plugin data access -->
  <!-- requirements: 3.1, 3.2 -->
  <!-- leverage: backend.rs implementation -->

- [ ] Add capability check for canonical collection access
  <!-- file: packages/plugin-sdk/src/storage_context.rs -->
  <!-- purpose: Verify plugin has declared storage:read or storage:write capability before allowing canonical collection access -->
  <!-- requirements: 3.3 -->
  <!-- leverage: existing capability enforcement in plugin-sdk -->

## 3.2 — Schema Validation

> spec: ./brief.md

- [ ] Implement canonical collection schema validation
  <!-- file: packages/storage-sqlite/src/validation.rs -->
  <!-- purpose: Load SDK-defined JSON Schemas for canonical collections and validate records before writes -->
  <!-- requirements: 4.1, 4.3 -->
  <!-- leverage: packages/types for canonical schemas -->

- [ ] Implement private collection schema validation from plugin manifests
  <!-- file: packages/storage-sqlite/src/validation.rs -->
  <!-- purpose: Load JSON Schema from plugin manifest collections.private definitions and validate records before writes -->
  <!-- requirements: 4.2, 4.3 -->
  <!-- leverage: validation.rs from previous task -->

- [ ] Support extensions object on canonical collection records
  <!-- file: packages/storage-sqlite/src/validation.rs -->
  <!-- purpose: Allow arbitrary extensions namespace on canonical records without validation of extension contents -->
  <!-- requirements: 4.4 -->
  <!-- leverage: validation.rs from previous task -->

## 4.1 — Credential Storage and Audit Logging

> spec: ./brief.md

- [ ] Implement per-credential encryption within the credentials collection
  <!-- file: packages/storage-sqlite/src/credentials.rs -->
  <!-- purpose: Encrypt each credential individually using packages/crypto with a derived key before storage, decrypt on read -->
  <!-- requirements: 6.1, 6.2, 6.3 -->
  <!-- leverage: packages/crypto for AES-256-GCM encryption primitives -->

- [ ] Implement audit log write functions and 90-day retention cleanup
  <!-- file: packages/storage-sqlite/src/audit.rs -->
  <!-- purpose: Write structured audit entries for auth, credential, plugin, and permission events; delete entries older than 90 days -->
  <!-- requirements: 7.1, 7.2, 7.3, 7.4, 7.5 -->
  <!-- leverage: audit_log table from schema.rs -->

## 4.2 — Data Export

> spec: ./brief.md

- [ ] Implement full and per-service data export in standard formats
  <!-- file: packages/storage-sqlite/src/export.rs -->
  <!-- purpose: Export full database as .tar.gz, email as .mbox, calendar as .ics, contacts as .vcf -->
  <!-- requirements: 10.1, 10.2, 10.3, 10.4, 10.5 -->
  <!-- leverage: packages/storage-sqlite as data source -->
