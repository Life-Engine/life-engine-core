<!--
domain: core
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Implementation Plan — Migration Format

## Task Overview

This plan implements the data migration system. Work starts with the manifest validation logic for `manifest.toml` entries, then builds the WASM transform runner, quarantine and logging tables, backup mechanism, the migration execution engine, and canonical migration support. Each task targets 1-3 files and produces a testable outcome.

**Progress:** 0 / 11 tasks complete

## Steering Document Compliance

- Migrations are declared in `manifest.toml` (not a separate JSON manifest)
- Transforms run inside the WASM sandbox — no dual-runtime (JS/Rust), WASM only
- Schema validation uses the `StorageBackend` trait
- No JavaScript or TypeScript — Core is Rust/WASM only
- Error types implement EngineError trait (code, severity, source_module)

## Atomic Task Requirements

- **File Scope:** 1-3 related files maximum
- **Time Boxing:** 15-30 minutes per task
- **Single Purpose:** one testable outcome per task
- **Specific Files:** exact file paths specified
- **Agent-Friendly:** clear input/output, minimal context switching

---

## 1.1 — Migration Manifest Validation
> spec: ./brief.md

- [ ] Implement manifest.toml migration entry parsing and validation
  <!-- file: packages/workflow-engine/src/migration/manifest.rs -->
  <!-- purpose: Parse [[migrations]] array from manifest.toml; validate from/to version fields, semver range format, and chain contiguity -->
  <!-- requirements: 1.1, 1.2, 1.3, 1.5, 1.6 -->
  <!-- leverage: none -->

---

## 1.2 — Migration Entry Overlap Detection
> spec: ./brief.md
> depends: 1.1

- [ ] Add overlap detection for migration `from` ranges
  <!-- file: packages/workflow-engine/src/migration/manifest.rs -->
  <!-- purpose: Validate that no two migration entries have overlapping from ranges; reject plugin update on overlap -->
  <!-- requirements: 1.4 -->
  <!-- leverage: manifest parsing from WP 1.1 -->

---

## 1.3 — WASM Export Validation
> spec: ./brief.md
> depends: 1.1

- [ ] Validate transform export names exist in plugin.wasm
  <!-- file: packages/workflow-engine/src/migration/validate.rs -->
  <!-- purpose: Load plugin.wasm and verify the declared transform function name exists as an export; reject plugin if missing -->
  <!-- requirements: 8.1, 8.2, 8.3 -->
  <!-- leverage: Extism WASM loading -->

---

## 2.1 — WASM Transform Runner
> spec: ./brief.md
> depends: 1.1

- [ ] Implement the WASM migration transform executor
  <!-- file: packages/workflow-engine/src/migration/runner.rs -->
  <!-- purpose: Load the plugin WASM module, call the exported migrate function with serialized JSON record, handle Ok/Err results, enforce sandbox (no host functions) -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.4, 2.5 -->
  <!-- leverage: Extism WASM runtime -->

---

## 3.1 — Quarantine Table
> spec: ./brief.md

- [ ] Create the quarantine table schema and CRUD operations
  <!-- file: packages/storage-sqlite/src/migration/quarantine.rs -->
  <!-- purpose: Define quarantine table with columns (id, record_data, plugin_id, collection, from_version, to_version, error_message, timestamp); implement insert and retry operations -->
  <!-- requirements: 5.1, 5.2, 5.3 -->
  <!-- leverage: packages/storage-sqlite StorageBackend -->

---

## 3.2 — Migration Log Table
> spec: ./brief.md

- [ ] Create the migration log table schema and logging operations
  <!-- file: packages/storage-sqlite/src/migration/log.rs -->
  <!-- purpose: Define migration_log table with columns (id, plugin_id, collection, from_version, to_version, records_migrated, records_quarantined, duration_ms, backup_path, timestamp); implement insert and failure logging -->
  <!-- requirements: 6.1, 6.2 -->
  <!-- leverage: packages/storage-sqlite StorageBackend -->

---

## 4.1 — Backup Mechanism
> spec: ./brief.md

- [ ] Implement pre-migration SQLite backup
  <!-- file: packages/storage-sqlite/src/migration/backup.rs -->
  <!-- purpose: Copy database to {data_dir}/backups/pre-migration-{timestamp}.db before migration; record backup path in migration log; implement restore function -->
  <!-- requirements: 7.1, 7.2, 7.3 -->
  <!-- leverage: packages/storage-sqlite -->

---

## 4.2 — Migration Execution Engine
> spec: ./brief.md
> depends: 2.1, 3.1, 3.2, 4.1

- [ ] Implement the core migration execution loop
  <!-- file: packages/workflow-engine/src/migration/engine.rs -->
  <!-- purpose: Begin SQLite transaction, iterate eligible records, apply WASM transform chain in ascending version order, update version column on success, quarantine on failure, validate output against StorageBackend schema, commit or rollback -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4, 9.1, 9.2 -->
  <!-- leverage: WASM runner from WP 2.1, quarantine from WP 3.1, log from WP 3.2, backup from WP 4.1 -->

---

## 5.1 — Canonical Migration Path
> spec: ./brief.md
> depends: 4.2

- [ ] Set up canonical migration file structure and startup trigger
  <!-- file: packages/types/src/migrations/mod.rs -->
  <!-- purpose: Create packages/types/migrations/ directory structure with subdirectories per collection; add startup check that compares record version to current schema version; wire canonical WASM transforms through the same execution engine -->
  <!-- requirements: 3.1, 3.2, 3.3 -->
  <!-- leverage: migration engine from WP 4.2 -->

---

## 5.2 — Version Column Update
> spec: ./brief.md
> depends: 4.2

- [ ] Implement post-transform version stamping
  <!-- file: packages/storage-sqlite/src/migration/version.rs -->
  <!-- purpose: After successful transform, update record's version column in plugin_data to migration's to version within the same SQLite transaction; prevent re-migration on subsequent startups -->
  <!-- requirements: 4.4 -->
  <!-- leverage: migration engine from WP 4.2 -->

---

## 6.1 — Integration Testing
> spec: ./brief.md
> depends: 5.1, 5.2

- [ ] Verify end-to-end migration behaviour
  <!-- file: packages/workflow-engine/src/tests/migration_test.rs -->
  <!-- purpose: Test a WASM migration that renames a field and adds a default; test quarantine by providing a transform that fails on specific records; test chain migration (v1 to v2 to v3) in a single run; verify schema validation on transform output -->
  <!-- requirements: 2.1, 2.2, 2.3, 4.1, 4.3, 5.1, 5.2, 9.1 -->
  <!-- leverage: packages/test-utils -->
