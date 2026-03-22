<!--
domain: data-layer
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Data Layer

## Purpose

This spec defines the storage model, schema, encryption, data access patterns, and query conventions for Core's data layer. All data is stored in a single SQLite database encrypted with SQLCipher. The data model uses a universal document envelope — plugins never run DDL statements.

## Document Model

All plugin data is stored in a single table with a fixed envelope. Plugin-specific fields live in a JSON column.

```sql
CREATE TABLE plugin_data (
    id          TEXT PRIMARY KEY,
    plugin_id   TEXT NOT NULL,
    collection  TEXT NOT NULL,
    data        TEXT NOT NULL,  -- JSON
    version     INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE INDEX idx_plugin_collection
    ON plugin_data(plugin_id, collection);
```

Benefits of this approach:

- **No dynamic DDL** — Plugins never run `CREATE TABLE`. No SQL injection surface, no migration headaches.
- **Trivial sync** — One table shape means every plugin's data syncs the same way.
- **Automatic plugin isolation** — Queries scoped by `plugin_id` + `collection`. The host enforces this at runtime.
- **Queryable JSON** — SQLite's `json_extract` handles queries at personal scale. Indexable if needed:

```sql
CREATE INDEX idx_todos_done
    ON plugin_data(json_extract(data, '$.done'))
    WHERE collection = 'todos';
```

## Universal Table Definitions

The following DDL statements are the canonical definitions for all Core tables. The formal JSON Schema definitions live in `docs/schemas/plugin-data.schema.json` and `docs/schemas/audit-log.schema.json`.

### plugin_data (canonical)

```sql
CREATE TABLE IF NOT EXISTS plugin_data (
    id          TEXT PRIMARY KEY,
    plugin_id   TEXT NOT NULL,
    collection  TEXT NOT NULL,
    data        TEXT NOT NULL,  -- JSON
    version     INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL,  -- RFC 3339
    updated_at  TEXT NOT NULL   -- RFC 3339
);

CREATE INDEX IF NOT EXISTS idx_plugin_collection
    ON plugin_data(plugin_id, collection);
```

### audit_log (canonical)

```sql
CREATE TABLE IF NOT EXISTS audit_log (
    id          TEXT PRIMARY KEY,
    timestamp   TEXT NOT NULL,  -- RFC 3339
    event_type  TEXT NOT NULL,
    plugin_id   TEXT,
    details     TEXT,           -- JSON
    created_at  TEXT NOT NULL   -- RFC 3339
);

CREATE INDEX IF NOT EXISTS idx_audit_timestamp
    ON audit_log(timestamp);
```

### Index Strategy

- **Composite index** `idx_plugin_collection` on `(plugin_id, collection)` supports the most common query pattern: listing records scoped to a specific plugin and collection.
- **Timestamp index** `idx_audit_timestamp` on `(timestamp)` supports retention cleanup queries and chronological audit review.
- Additional JSON-extract indexes can be created per-collection when query performance requires it (see the example in Document Model above).

### Conventions

- **Optimistic concurrency** — The `version` column starts at 1 and increments on every update. Updates that provide a stale version are rejected. This prevents lost-update anomalies without database-level locking.
- **Timestamp format** — All timestamp columns use RFC 3339 format (e.g., `2026-03-21T14:30:00+00:00`). This is the standard format used by `chrono::DateTime<Utc>::to_rfc3339()` in Rust and `Date.toISOString()` in JavaScript.
- **JSON data column** — The `data` column in `plugin_data` and the `details` column in `audit_log` store serialised JSON as TEXT. SQLite's `json_extract()` function enables querying into these columns without parsing. Schema validation is performed at the application layer before writes, not via database constraints.

## Two Tiers of Collections

### Canonical Collections (platform-owned)

Defined by the SDK, not by any plugin. These are the shared data types for universal personal data. Any plugin can declare read or write access. No plugin owns them.

- `events` — Calendar events
- `tasks` — To-dos, reminders
- `contacts` — People
- `notes` — Freeform text
- `emails` — Email messages
- `files` — File metadata
- `credentials` — Unified credential store (identity documents, OAuth tokens, API keys). Access scoped by type and plugin capabilities.

Canonical schemas are available in both SDKs: `plugin-sdk-rs` (Core WASM plugins) and `plugin-sdk-js` (App UI plugins). Using canonical collections requires no schema definition from the plugin author — the SDK defines them. This makes canonical the path of least resistance.

### Private Collections (plugin-owned)

For data that only makes sense within a single plugin. A Pomodoro plugin has `pomodoro_sessions`. A habit tracker has `habit_streaks`.

Private collections are namespaced automatically — `com.example.pomodoro/pomodoro_sessions` can never collide with another plugin's collection. Plugin authors define the schema via JSON Schema in their manifest.

## Extensions on Canonical Data

Plugins that need custom fields on canonical records use a namespaced `extensions` object. Core fields remain standardised and readable by all plugins. Plugin-specific fields live inside the extensions namespace and are ignored by plugins that do not understand them.

```json
{
  "title": "Team standup",
  "start": "2026-03-14T09:00:00Z",
  "end": "2026-03-14T09:15:00Z",
  "extensions": {
    "com.example.schedule-planner": {
      "priority": "high",
      "block_color": "#ff6b6b"
    }
  }
}
```

This removes the main reason plugin authors create private collections — "the canonical schema doesn't have the fields I need." With extensions, they can add fields without schema conflicts or name collisions.

## Schema Validation

Validation happens at the application layer, not in SQLite. Bad data is rejected before it hits the database.

- **Canonical collections** — Validated against SDK-defined schemas. These schemas are versioned with the SDK and shared across all plugins.
- **Private collections** — Validated against the JSON Schema declared in the plugin manifest:

```json
{
  "collections": {
    "canonical": ["events", "tasks"],
    "private": {
      "plans": {
        "schema": {
          "type": "object",
          "required": ["name", "event_ids"],
          "properties": {
            "name": { "type": "string" },
            "event_ids": { "type": "array", "items": { "type": "string" } }
          }
        }
      }
    }
  }
}
```

## Schema Evolution

Canonical schemas are versioned with the SDK:

- **Additive changes are non-breaking** — Adding new optional fields does not require a version bump.
- **Removals require a major SDK version bump** — Removing or changing existing fields triggers a major version increment with a 12-month support overlap so plugin authors can migrate.

Plugin migrations run on version update via transform scripts:

```json
{
  "version": "2.0.0",
  "migrations": [
    {
      "from": "1.x",
      "transform": "./migrations/v2.js"
    }
  ]
}
```

The migration script receives each record and returns the transformed version. The host runs it once on update.

## StorageAdapter Trait

Pluggable storage behind a trait. SQLite is the only supported backend for now. The trait exists so future backends can be added without changing plugin code.

```rust
#[async_trait]
pub trait StorageAdapter: Send + Sync {
    async fn get(&self, id: &str) -> Result<Record>;
    async fn set(&self, id: &str, data: &Record) -> Result<()>;
    async fn query(&self, filters: &QueryFilters) -> Result<Vec<Record>>;
    async fn delete(&self, id: &str) -> Result<()>;
    async fn list(&self, prefix: &str) -> Result<Vec<Record>>;
}
```

## SQLite Configuration

- **Driver** — `rusqlite` with SQLCipher extension
- **WAL mode** — Write-Ahead Logging for concurrent reads during writes. Sufficient for personal-scale data.
- **Single-writer** — SQLite's single-writer model is appropriate for personal use. No write contention issues at this scale.

## Encryption at Rest

- **Database** — SQLCipher provides transparent, full-database encryption. All data pages are encrypted.
- **Key derivation** — Argon2id with these parameters: 64 MB memory, 3 iterations, 4 parallelism, 32-byte output. Configurable for low-resource devices.
- **Master passphrase** — User provides at first launch. The derived key unlocks the database on every subsequent launch.
- **File-level encryption** — `age` for exports and backups.

## Credential Storage

Defence-in-depth: individual encryption even within the encrypted database.

- Each credential is encrypted separately with a key derived from the master passphrase
- OAuth refresh tokens are encrypted on disk; access tokens are held in memory only
- Automatic rotation before token expiry
- Credential access is scoped by type and plugin capabilities, logged in the audit log

## Data Export

- **Full export** — Database, files, config, and plugin data packaged as `.tar.gz`
- **Per-service export** — All data from a specific connector
- **Standard formats** — `.eml`/`.mbox` for email, `.ics` for calendar, `.vcf` for contacts
- **API access** — All data readable via the REST API

## Audit Logging

Security-relevant events are logged locally in structured JSON:

- Auth attempts (success and failure)
- Credential access (read, write, rotation)
- Plugin installs, enables, disables
- Permission changes
- Connector auth and revocation

Audit log management:

- Rotated daily
- Retained 90 days
- Encrypted at rest (within the SQLCipher database)
- No telemetry or external reporting

## Query Filter Syntax

The `/api/data/{collection}` endpoint supports the following filter operators in query payloads:

- **Equality** — `{ "field": "value" }` matches records where the field equals the value
- **Comparison** — `{ "field": { "$gte": 10 } }` and `{ "field": { "$lte": 100 } }` for greater/less than or equal
- **Text search** — `{ "field": { "$contains": "search term" } }` for substring matching
- **Logical operators** — `{ "$and": [...] }` and `{ "$or": [...] }` for combining conditions

## Sort and Pagination

List endpoints accept these query parameters:

- `limit` — Maximum number of records to return (default 50, max 1000)
- `offset` — Number of records to skip (default 0)
- `sort_by` — Field name to sort by (supports nested JSON fields via dot notation)
- `sort_dir` — Sort direction: `asc` or `desc` (default `asc`)

Paginated responses include a `total` count so clients can calculate page counts.

## Acceptance Criteria

- CRUD operations work on both canonical and private collections via the API
- Filters, sort, and pagination return correct results
- Encryption round-trip passes — data written to an encrypted database is readable after reopening with the correct passphrase
- Audit log entries are created for all security events (auth attempts, credential access, plugin installs, permission changes)
- Schema validation rejects malformed records with clear error messages
- Extensions on canonical data are stored and retrieved correctly without affecting core fields
- Private collection namespacing prevents cross-plugin data access
