<!--
domain: core
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Migration Format Specification

> Defines the contract for data migration when canonical or plugin-owned schemas evolve.

Related documents:

- [[Spec — Data Layer]] — Schema Evolution section
- [[Spec — Canonical Data Models]] — Schema Versioning section

---

## Purpose

Data schemas evolve over time. When a field is renamed, a type changes, or a new required field is added, existing records stored in the `plugin_data` table must be transformed to match the new schema version. Without a formal migration mechanism, schema changes would either break validation for existing records or require manual intervention.

Two tiers of migration exist:

- **Canonical schema migrations** — Platform-owned. Applied when the SDK ships a new major version that changes a canonical collection schema (tasks, events, contacts, emails, files, notes, credentials). These migrations are bundled with the SDK release and run automatically on Core startup.
- **Plugin schema migrations** — Plugin-owned. Applied when a plugin updates its version and declares migration transforms in `plugin.json`. These run when Core detects that a plugin's installed version differs from the version recorded for its data.

Both tiers share the same transform script contract and execution semantics.

---

## Migration Manifest Structure

Plugin authors declare migrations in their `plugin.json` manifest under a `migrations` array:

```json
{
  "id": "com.example.my-plugin",
  "version": "2.0.0",
  "migrations": [
    {
      "from": "1.x",
      "to": "2.0.0",
      "transform": "./migrations/v2.js",
      "description": "Rename 'dueDate' field to 'due_date', add 'priority' with default 'none'"
    }
  ]
}
```

Each migration entry has these fields:

- **from** (string, required) — Semver range matching the source version. Supports `x` wildcards for minor/patch (e.g., `1.x`, `1.0.x`). Records whose schema version matches this range are eligible for this migration.
- **to** (string, required) — The exact semver version that records will have after the transform runs.
- **transform** (string, required) — Relative path from the plugin root to the migration script file.
- **description** (string, optional) — Human-readable summary of what the migration does. Logged in the migration log.

### Version Matching Rules

- `from` uses simplified semver ranges: `1.x` matches any `1.*.*`, `1.0.x` matches any `1.0.*`, `1.0.0` matches exactly `1.0.0`
- `from` must be strictly less than `to` when comparing the minimum version in the range
- Multiple migration entries can exist for different source version ranges
- The `from` ranges of different entries must not overlap — each record must match at most one migration entry

---

## Transform Script API

Migration scripts implement a pure transformation function that receives one record and returns the transformed record.

### App Plugins (JavaScript)

The script must export a `migrate` function:

```javascript
export function migrate(record) {
  return {
    ...record,
    due_date: record.dueDate,
    priority: record.priority ?? "none",
  };
}
```

Contract:

- The function receives a deep clone of the record's JSON data — not the original
- The function must return the transformed record object
- The function must be pure: no side effects, no network access, no storage access, no `Date.now()` or other non-deterministic calls
- If the function throws, the migration is aborted for that individual record (the record is quarantined)
- The function runs synchronously — no `async`/`await`

### Core Plugins (Rust)

The transform function has the equivalent signature:

```rust
pub fn migrate(record: serde_json::Value) -> Result<serde_json::Value, String> {
    // Transform and return
}
```

- Returns `Ok(transformed)` on success
- Returns `Err(message)` to quarantine the record with the given error message
- Must be pure: no I/O, no global state mutation

---

## Canonical Schema Migrations

Canonical collection schemas are platform-owned and evolve with SDK releases. Their migrations differ from plugin migrations in how they are declared:

- Migration scripts are bundled with the SDK, not declared in plugin manifests
- Located at the well-known path: `packages/types/migrations/`
- Named by target version: `v{major}.{minor}.js` (for App-side) or `v{major}.{minor}.rs` (for Core-side)
- Applied automatically on Core startup when the `version` column in `plugin_data` for any canonical collection record does not match the current SDK schema version
- Migration metadata is hard-coded in the SDK release, not read from a manifest

Example file layout:

```
packages/types/migrations/
├── tasks/
│   └── v2.0.js
├── events/
│   └── v2.0.js
└── contacts/
    ├── v2.0.js
    └── v3.0.js
```

---

## Execution Semantics

### Transaction Boundaries

Each migration run (all records for one plugin/collection version transition) executes within a single SQLite transaction. If the transaction fails to commit, all changes are rolled back and the migration is marked as failed in the log.

### Execution Order

When multiple migration entries form a chain (e.g., v1 -> v2 -> v3), they execute in ascending order of `from` version. A record at v1 that needs to reach v3 will pass through both transforms sequentially within the same transaction.

### Quarantine

Individual records that fail migration are quarantined rather than blocking the entire batch:

- The failed record is moved to the `quarantine` table with metadata: original record data, plugin ID, collection name, source version, target version, error message, and timestamp
- The migration continues for remaining records
- Quarantined records can be retried later via the quarantine management API

### Migration Log

Every migration run is recorded in a `migration_log` table:

- **id** — Auto-increment primary key
- **plugin_id** — The plugin that owns the migrated collection
- **collection** — The collection name
- **from_version** — Source schema version
- **to_version** — Target schema version
- **records_migrated** — Count of successfully transformed records
- **records_quarantined** — Count of records that failed and were quarantined
- **duration_ms** — Wall-clock time for the migration run
- **timestamp** — ISO 8601 timestamp of when the migration completed

### Version Column Update

After a record is successfully transformed, its `version` column in the `plugin_data` table is updated to the migration's `to` version. This ensures the record is not re-migrated on subsequent startups.

---

## Rollback

Migrations are forward-only. There is no automatic rollback mechanism.

Before any migration begins, Core creates a SQLite backup of the database file at `{data_dir}/backups/pre-migration-{timestamp}.db`. The backup path is recorded in the migration log. Users can restore the backup manually if a migration produces undesirable results.

---

## Validation Rules

Core validates migration declarations before executing them:

- `from` version range must be strictly less than `to` version
- Transform script path must exist relative to the plugin root
- Transform script file must be under 1 MB
- Migration chain must be contiguous — no version gaps between consecutive entries (the `to` of one entry must match the `from` of the next in the chain)
- `from` ranges must not overlap between entries — each source version must map to exactly one migration path
- The final `to` version in the chain must match the plugin's current `version` field in the manifest

If any validation rule fails, the plugin update is rejected and the previous version remains active.

---

## Acceptance Criteria

- Migration manifest structure is formally defined and enforced by JSON Schema (`migration.schema.json`)
- Plugin manifest schema includes the optional `migrations` array field
- Transform script API contract is documented for both JS and Rust
- Canonical migration path (`packages/types/migrations/`) is defined
- Execution semantics specify transaction boundaries, ordering, quarantine, and logging
- Rollback policy is documented (forward-only with pre-migration backup)
- All validation rules are enumerated and testable
- The specification is consistent with the Data Layer spec's Schema Evolution section and the CDM spec's Schema Versioning policy
