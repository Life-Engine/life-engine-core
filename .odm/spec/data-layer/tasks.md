<!--
domain: data-layer
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Implementation Plan — Data Layer

## Task Overview

This plan implements the Core data layer from the ground up. Work begins with the database schema and SQLCipher setup, then builds the StorageAdapter trait and SQLite implementation, followed by schema validation, the query engine, audit logging, and data export. Each task targets a narrow slice of functionality that can be tested in isolation.

**Progress:** 0 / 16 tasks complete

## Steering Document Compliance

- Universal document model follows Single Source of Truth — one table shape for all plugins
- SQLCipher encryption with Argon2id follows Defence in Depth
- Schema validation at the boundary follows Parse, Don't Validate
- StorageAdapter trait follows Open/Closed Principle — new backends without code changes
- Plugin data scoped by plugin_id follows Principle of Least Privilege

## Atomic Task Requirements

- **File Scope:** 1-3 related files maximum
- **Time Boxing:** 15-30 minutes per task
- **Single Purpose:** one testable outcome per task
- **Specific Files:** exact file paths specified
- **Agent-Friendly:** clear input/output, minimal context switching

---

## 1.1 — Database Schema and Encryption

> spec: ./brief.md

- [ ] Create plugin_data table DDL with indexes
  <!-- file: apps/core/src/sqlite_storage.rs -->
  <!-- purpose: Define CREATE TABLE plugin_data with id, plugin_id, collection, data, version, created_at, updated_at and composite index -->
  <!-- requirements: 1.1 -->
  <!-- leverage: existing apps/core/src/sqlite_storage.rs -->

- [ ] Create audit_log table DDL with timestamp index
  <!-- file: apps/core/src/sqlite_storage.rs -->
  <!-- purpose: Define CREATE TABLE audit_log with id, timestamp, event_type, plugin_id, details, created_at and timestamp index -->
  <!-- requirements: 6.1 -->
  <!-- leverage: existing sqlite_storage.rs -->

- [ ] Configure SQLCipher encryption and WAL mode on database open
  <!-- file: apps/core/src/sqlite_storage.rs -->
  <!-- purpose: Open database with SQLCipher PRAGMA key from Argon2id-derived key, enable WAL mode -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4 -->
  <!-- leverage: existing rusqlite dependency in Cargo.toml -->

## 1.2 — StorageAdapter Trait

> spec: ./brief.md

- [ ] Define StorageAdapter trait with CRUD methods
  <!-- file: apps/core/src/storage.rs -->
  <!-- purpose: Define async trait with get(), set(), query(), delete(), list() methods and Record type -->
  <!-- requirements: 1.1, 1.2, 1.3, 1.5, 1.6 -->
  <!-- leverage: existing apps/core/src/storage.rs -->

- [ ] Implement StorageAdapter for SQLite backend
  <!-- file: apps/core/src/sqlite_storage.rs -->
  <!-- purpose: Implement all trait methods using rusqlite, including optimistic concurrency via version column -->
  <!-- requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6 -->
  <!-- leverage: existing sqlite_storage.rs -->

## 2.1 — Plugin Data Isolation

> spec: ./brief.md

- [ ] Add plugin_id scoping to all storage operations
  <!-- file: apps/core/src/sqlite_storage.rs -->
  <!-- purpose: Ensure all queries include WHERE plugin_id = ? clause, preventing cross-plugin data access -->
  <!-- requirements: 2.1, 2.2 -->
  <!-- leverage: existing sqlite_storage.rs implementation -->

- [ ] Add capability check for canonical collection access
  <!-- file: apps/core/src/storage.rs -->
  <!-- purpose: Verify plugin has declared storage:read or storage:write capability before allowing canonical collection access -->
  <!-- requirements: 2.3 -->
  <!-- leverage: existing capability enforcement infrastructure -->

## 2.2 — Schema Validation

> spec: ./brief.md

- [ ] Implement canonical collection schema validation
  <!-- file: apps/core/src/schema_registry.rs -->
  <!-- purpose: Load SDK-defined JSON Schemas for canonical collections and validate records before writes -->
  <!-- requirements: 3.1, 3.3 -->
  <!-- leverage: existing apps/core/src/schema_registry.rs -->

- [ ] Implement private collection schema validation from plugin manifests
  <!-- file: apps/core/src/schema_registry.rs -->
  <!-- purpose: Load JSON Schema from plugin manifest collections.private definitions and validate records before writes -->
  <!-- requirements: 3.2, 3.3 -->
  <!-- leverage: existing schema_registry.rs -->

- [ ] Support extensions object on canonical collection records
  <!-- file: apps/core/src/schema_registry.rs -->
  <!-- purpose: Allow arbitrary extensions namespace on canonical records without validation of extension contents -->
  <!-- requirements: 3.4 -->
  <!-- leverage: existing schema_registry.rs -->

## 3.1 — Query Engine

> spec: ./brief.md

- [ ] Implement query filter translation to SQL WHERE clauses
  <!-- file: apps/core/src/sqlite_storage.rs -->
  <!-- purpose: Translate equality, $gte/$lte, $contains, $and/$or filter syntax to json_extract-based SQL -->
  <!-- requirements: 7.1, 7.2, 7.3, 7.4 -->
  <!-- leverage: existing sqlite_storage.rs -->

- [ ] Implement sorting and pagination for list queries
  <!-- file: apps/core/src/sqlite_storage.rs -->
  <!-- purpose: Add ORDER BY, LIMIT, OFFSET support and include total count in paginated responses -->
  <!-- requirements: 8.1, 8.2, 8.3, 8.4 -->
  <!-- leverage: existing sqlite_storage.rs -->

## 3.2 — Credential Storage

> spec: ./brief.md

- [ ] Implement per-credential encryption within the credentials collection
  <!-- file: apps/core/src/credential_store.rs -->
  <!-- purpose: Encrypt each credential individually with a derived key before storage, decrypt on read -->
  <!-- requirements: 5.1, 5.2, 5.3 -->
  <!-- leverage: existing apps/core/src/credential_store.rs -->

## 4.1 — Audit Logging

> spec: ./brief.md

- [ ] Implement audit log write functions for security events
  <!-- file: apps/core/src/sqlite_storage.rs -->
  <!-- purpose: Write structured audit entries for auth, credential, plugin, and permission events -->
  <!-- requirements: 6.1, 6.2, 6.3, 6.5 -->
  <!-- leverage: existing sqlite_storage.rs audit_log table -->

- [ ] Implement 90-day audit log retention cleanup
  <!-- file: apps/core/src/sqlite_storage.rs -->
  <!-- purpose: Delete audit_log entries older than 90 days during daily scheduled cleanup -->
  <!-- requirements: 6.4 -->
  <!-- leverage: existing background scheduler -->

## 4.2 — Data Export

> spec: ./brief.md

- [ ] Implement full and per-service data export in standard formats
  <!-- file: apps/core/src/routes/export.rs -->
  <!-- purpose: Export full database as .tar.gz, email as .mbox, calendar as .ics, contacts as .vcf -->
  <!-- requirements: 9.1, 9.2, 9.3, 9.4, 9.5 -->
  <!-- leverage: existing apps/core/src/routes/ directory -->
