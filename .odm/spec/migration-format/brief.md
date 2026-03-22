<!--
domain: core
status: draft
tier: 1
updated: 2026-03-23
-->

# Migration Format Spec

## Overview

Defines the contract for data migration when canonical or plugin-owned schemas evolve. When a field is renamed, a type changes, or a new required field is added, existing records must be transformed to match the new schema version. Migrations are declared in `manifest.toml`, transform functions run inside the WASM sandbox, and the system is Rust/WASM only — no JavaScript or dual-runtime support. This spec covers migration manifests, the WASM transform API, execution semantics, quarantine handling, rollback policy, and validation rules.

## Goals

- **Schema evolution without data loss** — Allow schemas to change across versions while preserving all existing user data.
- **WASM-sandboxed transforms** — Migration scripts run inside the plugin's WASM sandbox with no access to host resources.
- **Graceful failure handling** — Quarantine individual records that fail migration rather than blocking the entire batch.
- **Auditability** — Log every migration run with counts, duration, and outcomes for debugging.
- **Safe rollback** — Create automatic pre-migration backups so users can recover if a migration produces undesirable results.

## User Stories

- As a plugin author, I want to declare migration transforms in my `manifest.toml` so that my users' data transforms automatically when they update my plugin.
- As a platform maintainer, I want canonical schema migrations to run on Core startup so that built-in collection schemas stay current.
- As a user, I want failed records to be quarantined rather than lost so that I can retry or manually fix them later.
- As a developer, I want a migration log so that I can diagnose issues when transforms produce unexpected results.
- As a user, I want a pre-migration backup so that I can restore my data if something goes wrong.

## Functional Requirements

- The system must validate migration entries in `manifest.toml` before executing any transforms.
- The system must support a pure `migrate(record)` transform function executed inside the WASM sandbox.
- The system must apply canonical schema migrations automatically on Core startup.
- The system must execute each migration run within a single SQLite transaction.
- The system must quarantine individual records that fail transformation with full error metadata.
- The system must record every migration run in a `migration_log` table.
- The system must create a SQLite backup before any migration begins.
- The system must enforce contiguous migration chains with non-overlapping version ranges.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
