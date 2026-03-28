<!--
domain: schema-versioning-rules
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Implementation Plan — Schema Versioning Rules

## Task Overview

This plan implements schema versioning enforcement, migration transforms, deprecation tooling, and JSON Schema file conventions for Life Engine. Work begins with the compatibility classification library, then builds the CI enforcement step, followed by the migration runtime, deprecation annotation infrastructure, and versioned schema file layout. Each task targets a narrow slice of functionality that can be tested in isolation.

**Progress:** 0 / 16 tasks complete

## Steering Document Compliance

- Additive-only rule follows Open/Closed Principle — schemas are open for extension, closed for modification within a major version
- CI enforcement follows Parse, Don't Validate — invalid schema changes are rejected at the boundary before they enter `main`
- 12-month maintenance window follows Principle of Least Surprise — plugin authors have a predictable, non-negotiable timeline
- Idempotent migration transforms follow Defence in Depth — partial runs and restarts are safe by design
- Deprecation annotations follow The Pit of Success — compiler warnings make the correct migration path the obvious one

## Atomic Task Requirements

- **File Scope:** 1-3 related files maximum
- **Time Boxing:** 15-30 minutes per task
- **Single Purpose:** one testable outcome per task
- **Specific Files:** exact file paths specified
- **Agent-Friendly:** clear input/output, minimal context switching

---

## 1.1 — Compatibility Classification Library

> spec: ./brief.md

- [ ] Define ChangeKind enum and SchemaChange struct
  <!-- file: packages/schema-compat/src/types.rs -->
  <!-- purpose: Define ChangeKind (Breaking, NonBreaking) enum and SchemaChange struct holding field name, change description, and classification -->
  <!-- requirements: 2.1-2.4, 3.1-3.7 -->

- [ ] Implement field-level diff detection
  <!-- file: packages/schema-compat/src/diff.rs -->
  <!-- purpose: Compare two JSON Schema objects and detect added, removed, renamed, and type-changed fields. Return Vec<SchemaChange> -->
  <!-- requirements: 2.1, 3.1, 3.2, 3.3, 3.4 -->

- [ ] Implement enum value diff detection
  <!-- file: packages/schema-compat/src/diff.rs -->
  <!-- purpose: Detect added and removed enum values within fields. Classify removals as breaking, additions as non-breaking -->
  <!-- requirements: 2.2, 3.5 -->

- [ ] Implement constraint diff detection
  <!-- file: packages/schema-compat/src/diff.rs -->
  <!-- purpose: Detect tightened and relaxed constraints (minLength, maxLength, minimum, maximum, pattern, format). Classify tightened as breaking, relaxed as non-breaking -->
  <!-- requirements: 2.4, 3.7, 4.3 -->

- [ ] Implement edge case classification for defaults and format additions
  <!-- file: packages/schema-compat/src/diff.rs -->
  <!-- purpose: Detect default value changes and format additions on existing fields. Classify as breaking per the conservative default rule -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4 -->

## 1.2 — Compatibility Classification Tests

> spec: ./brief.md

- [ ] Write unit tests for non-breaking change classification
  <!-- file: packages/schema-compat/src/tests/non_breaking.rs -->
  <!-- purpose: Test that optional field additions, new enum values, new collections, and relaxed constraints are classified as NonBreaking -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.4 -->

- [ ] Write unit tests for breaking change classification
  <!-- file: packages/schema-compat/src/tests/breaking.rs -->
  <!-- purpose: Test that field removal, rename, type change, required field addition, enum removal, semantic change, and tightened constraints are classified as Breaking -->
  <!-- requirements: 3.1-3.7, 4.1, 4.3 -->

## 2.1 — CI Enforcement Step

> spec: ./brief.md

- [ ] Implement schema-compat CLI entry point
  <!-- file: packages/schema-compat/src/main.rs -->
  <!-- purpose: CLI that accepts base and proposed schema paths, runs classify_change, checks SDK major version, and exits with appropriate code. Outputs human-readable report of all detected changes -->
  <!-- requirements: 6.1, 6.2, 6.3, 6.4 -->

- [ ] Implement migration presence verification
  <!-- file: packages/schema-compat/src/verify.rs -->
  <!-- purpose: When breaking changes are detected and major version is bumped, verify that migration entries exist in the manifest for each affected collection. Fail if any are missing -->
  <!-- requirements: 6.3, 8.1, 8.2 -->

## 2.2 — Migration Transform Runtime

> spec: ./brief.md

- [ ] Define MigrationEntry manifest struct and parser
  <!-- file: packages/types/src/migration.rs -->
  <!-- purpose: Define MigrationEntry struct with from (semver range), collection (String), and transform (PathBuf) fields. Implement deserialization from manifest JSON -->
  <!-- requirements: 8.1, 8.2 -->

- [ ] Implement migration executor
  <!-- file: packages/core/src/migration.rs -->
  <!-- purpose: On startup, detect version changes, load transform scripts, iterate records in affected collections, apply transforms, write back changed records, log results to audit_log -->
  <!-- requirements: 8.3, 8.4, 8.5, 8.6, 8.7 -->

- [ ] Write integration tests for migration executor
  <!-- file: packages/core/src/tests/migration_test.rs -->
  <!-- purpose: Test idempotency (double-run produces same result), partial failure (non-migratable records returned unchanged with warning), and audit log output -->
  <!-- requirements: 8.3, 8.4, 8.7 -->

## 3.1 — JSON Schema File Layout

> spec: ./brief.md

- [ ] Create versioned schema directory structure
  <!-- file: docs/schemas/v1/ (directory), docs/schemas/v1/tasks.schema.json, docs/schemas/v1/events.schema.json -->
  <!-- purpose: Move existing schema files into v1/ directory, update $id fields to include major version in URI, set $schema to draft/2020-12 -->
  <!-- requirements: 11.1, 11.3, 11.5 -->

- [ ] Create stable symlinks for unprefixed schema paths
  <!-- file: docs/schemas/tasks.schema.json (symlink), docs/schemas/events.schema.json (symlink) -->
  <!-- purpose: Create symlinks from unprefixed paths to the current stable major version directory (v1/) -->
  <!-- requirements: 11.4 -->

## 3.2 — Deprecation Tooling

> spec: ./brief.md

- [ ] Implement deprecation-before-removal check in schema-compat CLI
  <!-- file: packages/schema-compat/src/deprecation.rs -->
  <!-- purpose: When a field removal is detected in a major version bump, check git history for a #[deprecated] annotation or @deprecated JSDoc in the preceding major cycle. Fail if the field was never deprecated -->
  <!-- requirements: 10.1, 10.2, 10.3, 10.4 -->
