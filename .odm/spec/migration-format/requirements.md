<!--
domain: core
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Migration Format Requirements

> spec: ./brief.md

## 1 — Migration Manifest Structure

- **1.1** — WHEN a plugin's `manifest.toml` contains a `[[migrations]]` array THEN the system SHALL validate that each entry has `from` (semver range), `to` (exact semver), and `transform` (relative path to WASM export name) fields.
- **1.2** — WHEN a `from` range is specified THEN the system SHALL support `x` wildcards for minor/patch (e.g., `1.x`, `1.0.x`) and exact versions (e.g., `1.0.0`).
- **1.3** — WHEN the `from` minimum version is not strictly less than `to` THEN the system SHALL reject the migration entry.
- **1.4** — WHEN multiple migration entries have overlapping `from` ranges THEN the system SHALL reject the plugin update with a validation error.
- **1.5** — WHEN migration entries form a chain THEN the system SHALL verify that the `to` of one entry matches the `from` of the next, with no version gaps.
- **1.6** — WHEN the final `to` version in the chain does not match the plugin's current `version` field THEN the system SHALL reject the plugin update.

## 2 — WASM Transform API

- **2.1** — WHEN a WASM migration function is executed THEN the system SHALL call the exported `migrate` function with a serialized JSON record as input.
- **2.2** — WHEN the `migrate` function returns successfully THEN the system SHALL use the returned serialized JSON as the transformed record.
- **2.3** — WHEN the `migrate` function returns an error THEN the system SHALL quarantine the individual record with the error message.
- **2.4** — WHEN a WASM migration function runs THEN the system SHALL enforce sandboxing with no access to host functions (no storage, no network, no filesystem).
- **2.5** — WHEN the transform is implemented in Rust THEN the function signature SHALL be `migrate(record: serde_json::Value) -> Result<serde_json::Value, String>`, compiled to WASM.

## 3 — Canonical Schema Migrations

- **3.1** — WHEN Core starts and detects that a canonical collection record's `version` does not match the current schema version THEN the system SHALL apply the appropriate migration transforms from `packages/types/migrations/`.
- **3.2** — WHEN canonical migration transforms are located THEN the system SHALL be compiled WASM modules following the naming convention `v{major}_{minor}.wasm` under the collection subdirectory.
- **3.3** — WHEN canonical migrations run THEN the system SHALL use the same WASM transform API and execution semantics as plugin migrations.

## 4 — Execution Semantics

- **4.1** — WHEN a migration run begins THEN the system SHALL execute all record transforms within a single SQLite transaction.
- **4.2** — WHEN the SQLite transaction fails to commit THEN the system SHALL roll back all changes and mark the migration as failed in the log.
- **4.3** — WHEN multiple migration entries form a chain (e.g., v1 to v2 to v3) THEN the system SHALL execute them in ascending `from` version order, passing each record through all applicable transforms sequentially.
- **4.4** — WHEN a record is successfully transformed THEN the system SHALL update its `version` column in the `plugin_data` table to the migration's `to` version.

## 5 — Quarantine

- **5.1** — WHEN an individual record fails migration THEN the system SHALL insert it into the `quarantine` table with: original record data, plugin ID, collection name, source version, target version, error message, and timestamp.
- **5.2** — WHEN a record is quarantined THEN the system SHALL continue processing remaining records in the batch.
- **5.3** — WHEN a user requests quarantine retry THEN the system SHALL re-run the WASM migration transform on the quarantined record and, on success, move it back to the main collection.

## 6 — Migration Log

- **6.1** — WHEN a migration run completes THEN the system SHALL insert a row into `migration_log` with: plugin_id, collection, from_version, to_version, records_migrated, records_quarantined, duration_ms, and timestamp.
- **6.2** — WHEN a migration run fails at the transaction level THEN the system SHALL still log the failure with zero records_migrated and the error context.

## 7 — Rollback and Backup

- **7.1** — WHEN any migration begins THEN the system SHALL create a SQLite backup at `{data_dir}/backups/pre-migration-{timestamp}.db` before executing transforms.
- **7.2** — WHEN a backup is created THEN the system SHALL record the backup file path in the migration log entry.
- **7.3** — WHEN a user restores a backup THEN the system SHALL replace the current database with the backup file, reverting all migrations applied after the backup timestamp.

## 8 — Validation Rules

- **8.1** — WHEN a transform export name is declared in `manifest.toml` THEN the system SHALL verify the export exists in the plugin's `plugin.wasm`.
- **8.2** — WHEN a `plugin.wasm` file exceeds 10 MB THEN the system SHALL log a warning but continue loading.
- **8.3** — WHEN any validation rule fails THEN the system SHALL reject the plugin update and keep the previous version active.

## 9 — Schema Validation

- **9.1** — WHEN a migration transform produces output THEN the system SHALL validate the output against the target schema version using the `StorageBackend` trait's validation logic.
- **9.2** — WHEN schema validation fails on the transformed record THEN the system SHALL quarantine the record with a validation error message.
