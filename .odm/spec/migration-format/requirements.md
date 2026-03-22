<!--
domain: core
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Migration Format Requirements

> spec: ./brief.md

## 1 — Migration Manifest Structure

- **1.1** — WHEN a plugin manifest contains a `migrations` array THEN the system SHALL validate that each entry has `from` (semver range), `to` (exact semver), and `transform` (relative path) fields.
- **1.2** — WHEN a `from` range is specified THEN the system SHALL support `x` wildcards for minor/patch (e.g., `1.x`, `1.0.x`) and exact versions (e.g., `1.0.0`).
- **1.3** — WHEN the `from` minimum version is not strictly less than `to` THEN the system SHALL reject the migration entry.
- **1.4** — WHEN multiple migration entries have overlapping `from` ranges THEN the system SHALL reject the plugin update with a validation error.
- **1.5** — WHEN migration entries form a chain THEN the system SHALL verify that the `to` of one entry matches the `from` of the next, with no version gaps.
- **1.6** — WHEN the final `to` version in the chain does not match the plugin's current `version` field THEN the system SHALL reject the plugin update.

## 2 — Transform Script API (JavaScript)

- **2.1** — WHEN a JS migration script is executed THEN the system SHALL call its exported `migrate(record)` function with a deep clone of the record's JSON data.
- **2.2** — WHEN the `migrate` function returns a value THEN the system SHALL use the returned object as the transformed record.
- **2.3** — WHEN the `migrate` function throws an error THEN the system SHALL quarantine the individual record with the error message.
- **2.4** — WHEN a JS migration script runs THEN the system SHALL enforce that it executes synchronously with no access to network, storage, or non-deterministic APIs.

## 3 — Transform Script API (Rust)

- **3.1** — WHEN a Rust migration function is executed THEN the system SHALL call `migrate(record: serde_json::Value)` and accept `Result<serde_json::Value, String>`.
- **3.2** — WHEN the function returns `Ok(transformed)` THEN the system SHALL use the transformed value as the migrated record.
- **3.3** — WHEN the function returns `Err(message)` THEN the system SHALL quarantine the record with the provided error message.
- **3.4** — WHEN a Rust migration function runs THEN the system SHALL enforce purity with no I/O or global state mutation.

## 4 — Canonical Schema Migrations

- **4.1** — WHEN Core starts and detects that a canonical collection record's `version` does not match the current SDK schema version THEN the system SHALL apply the appropriate migration scripts from `packages/types/migrations/`.
- **4.2** — WHEN canonical migration scripts are located THEN the system SHALL follow the naming convention `v{major}.{minor}.js` or `v{major}.{minor}.rs` under the collection subdirectory.
- **4.3** — WHEN canonical migrations run THEN the system SHALL use the same transform script API and execution semantics as plugin migrations.

## 5 — Execution Semantics

- **5.1** — WHEN a migration run begins THEN the system SHALL execute all record transforms within a single SQLite transaction.
- **5.2** — WHEN the SQLite transaction fails to commit THEN the system SHALL roll back all changes and mark the migration as failed in the log.
- **5.3** — WHEN multiple migration entries form a chain (e.g., v1 to v2 to v3) THEN the system SHALL execute them in ascending `from` version order, passing each record through all applicable transforms sequentially.
- **5.4** — WHEN a record is successfully transformed THEN the system SHALL update its `version` column in the `plugin_data` table to the migration's `to` version.

## 6 — Quarantine

- **6.1** — WHEN an individual record fails migration THEN the system SHALL insert it into the `quarantine` table with: original record data, plugin ID, collection name, source version, target version, error message, and timestamp.
- **6.2** — WHEN a record is quarantined THEN the system SHALL continue processing remaining records in the batch.
- **6.3** — WHEN a user requests quarantine retry THEN the system SHALL re-run the migration transform on the quarantined record and, on success, move it back to the main collection.

## 7 — Migration Log

- **7.1** — WHEN a migration run completes THEN the system SHALL insert a row into `migration_log` with: plugin_id, collection, from_version, to_version, records_migrated, records_quarantined, duration_ms, and timestamp.
- **7.2** — WHEN a migration run fails at the transaction level THEN the system SHALL still log the failure with zero records_migrated and the error context.

## 8 — Rollback and Backup

- **8.1** — WHEN any migration begins THEN the system SHALL create a SQLite backup at `{data_dir}/backups/pre-migration-{timestamp}.db` before executing transforms.
- **8.2** — WHEN a backup is created THEN the system SHALL record the backup file path in the migration log entry.
- **8.3** — WHEN a user restores a backup THEN the system SHALL replace the current database with the backup file, reverting all migrations applied after the backup timestamp.

## 9 — Validation Rules

- **9.1** — WHEN a transform script path is declared THEN the system SHALL verify the file exists relative to the plugin root.
- **9.2** — WHEN a transform script file exceeds 1 MB THEN the system SHALL reject the migration entry.
- **9.3** — WHEN any validation rule fails THEN the system SHALL reject the plugin update and keep the previous version active.
