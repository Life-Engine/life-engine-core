<!--
domain: core
status: draft
tier: 1
updated: 2026-03-22
-->

# Migration Format Spec

## Overview

Defines the contract for data migration when canonical or plugin-owned schemas evolve. When a field is renamed, a type changes, or a new required field is added, existing records must be transformed to match the new schema version. This spec covers migration manifests, transform script APIs for both JavaScript and Rust, execution semantics, quarantine handling, rollback policy, and validation rules.

## Goals

- **Schema evolution without data loss** — Allow schemas to change across versions while preserving all existing user data.
- **Dual-runtime transforms** — Support migration scripts in both JavaScript (App plugins) and Rust (Core plugins).
- **Graceful failure handling** — Quarantine individual records that fail migration rather than blocking the entire batch.
- **Auditability** — Log every migration run with counts, duration, and outcomes for debugging and compliance.
- **Safe rollback** — Create automatic pre-migration backups so users can recover if a migration produces undesirable results.

## User Stories

- As a plugin author, I want to declare migration scripts in my manifest so that my users' data transforms automatically when they update my plugin.
- As a platform maintainer, I want canonical schema migrations to run on Core startup so that built-in collection schemas stay current.
- As a user, I want failed records to be quarantined rather than lost so that I can retry or manually fix them later.
- As a developer, I want a migration log so that I can diagnose issues when transforms produce unexpected results.
- As a user, I want a pre-migration backup so that I can restore my data if something goes wrong.

## Functional Requirements

- The system must validate migration manifest entries before executing any transforms.
- The system must support a pure `migrate(record)` transform function in both JS and Rust.
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
