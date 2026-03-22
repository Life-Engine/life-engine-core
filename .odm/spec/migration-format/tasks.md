<!--
domain: core
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Migration Format Tasks

> spec: ./brief.md

## 1.1 — Migration Manifest Schema
> spec: ./brief.md

Define the JSON Schema for migration manifest entries.

- Create `schemas/migration.schema.json` with `from`, `to`, `transform`, and `description` fields
- Add semver range validation pattern for `from` field
- Add exact semver validation pattern for `to` field

> estimate: 20 min

## 1.2 — Manifest Validation Logic
> spec: ./brief.md

Implement migration manifest validation in Core.

- Add validation that `from` minimum is strictly less than `to`
- Add validation that `from` ranges do not overlap between entries
- Add chain contiguity check (each `to` matches next `from`, final `to` matches plugin version)

> estimate: 25 min
> depends: 1.1

## 1.3 — Transform Script File Validation
> spec: ./brief.md

Validate transform script files before execution.

- Verify transform script path exists relative to plugin root
- Verify file size is under 1 MB
- Reject plugin update and keep previous version active on any validation failure

> estimate: 15 min
> depends: 1.2

## 2.1 — JS Transform Runner
> spec: ./brief.md

Implement the JavaScript migration script executor.

- Create a sandboxed JS execution environment that calls `migrate(record)` with a deep-cloned record
- Enforce synchronous execution with no access to network, storage, or non-deterministic APIs
- Catch thrown errors and route the record to quarantine with the error message

> estimate: 30 min
> depends: 1.2

## 2.2 — Rust Transform Runner
> spec: ./brief.md

Implement the Rust migration function executor.

- Create a Rust execution path that calls `migrate(record: serde_json::Value)` and handles `Result`
- Route `Ok(value)` to the transformed record output
- Route `Err(message)` to quarantine with the provided error message

> estimate: 25 min
> depends: 1.2

## 3.1 — Quarantine Table
> spec: ./brief.md

Create the quarantine table and CRUD operations.

- Define `quarantine` table schema with columns: id, record_data, plugin_id, collection, from_version, to_version, error_message, timestamp
- Implement insert operation for failed records
- Implement retry operation that re-runs the transform and moves the record back on success

> estimate: 25 min

## 3.2 — Migration Log Table
> spec: ./brief.md

Create the migration log table and logging operations.

- Define `migration_log` table schema with columns: id, plugin_id, collection, from_version, to_version, records_migrated, records_quarantined, duration_ms, backup_path, timestamp
- Implement insert operation for completed migration runs
- Implement failure logging with zero records_migrated and error context

> estimate: 20 min

## 4.1 — Backup Mechanism
> spec: ./brief.md

Implement pre-migration SQLite backup.

- Create backup logic that copies the database to `{data_dir}/backups/pre-migration-{timestamp}.db`
- Record the backup path in the migration log entry
- Add restore function that replaces the current database with a backup file

> estimate: 20 min

## 4.2 — Migration Execution Engine
> spec: ./brief.md

Implement the core migration execution loop.

- Begin a SQLite transaction for each migration run
- Iterate all eligible records, apply the transform chain in ascending version order
- Update each record's `version` column on success, quarantine on failure
- Commit the transaction or roll back on failure

> estimate: 30 min
> depends: 2.1, 2.2, 3.1, 3.2, 4.1

## 5.1 — Canonical Migration Path
> spec: ./brief.md

Set up the canonical migration file structure and startup trigger.

- Create `packages/types/migrations/` directory structure with subdirectories per collection
- Add startup check that compares record `version` to current SDK schema version
- Wire canonical migration scripts through the same execution engine as plugin migrations

> estimate: 25 min
> depends: 4.2

## 5.2 — Version Column Update
> spec: ./brief.md

Implement post-transform version stamping.

- After successful transform, update the record's `version` column in `plugin_data` to the migration's `to` version
- Ensure version update is within the same SQLite transaction as the data transform
- Prevent re-migration of already-updated records on subsequent startups

> estimate: 15 min
> depends: 4.2

## 6.1 — Integration Testing
> spec: ./brief.md

Verify end-to-end migration behaviour.

- Test a JS migration that renames a field and adds a default value, verifying all records transform correctly
- Test quarantine by providing a script that fails on specific records, verifying failed records are quarantined and successful records complete
- Test chain migration (v1 to v2 to v3) in a single run, verifying records pass through both transforms

> estimate: 30 min
> depends: 5.1, 5.2
