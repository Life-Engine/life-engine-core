---
title: "Engine — Data Layer"
tags: [life-engine, engine, data, sqlite, encryption, schema]
created: 2026-03-14
---

# Data Layer

SQLite is the storage backend. All plugin data is stored in a document model with a fixed envelope. Canonical collections provide the shared data language of the ecosystem. All data is encrypted at rest.

The data layer implements several [[03 - Projects/Life Engine/Design/Principles|Design Principles]]: *Parse, Don't Validate* (typed canonical collections with schema validation at the boundary — what passes validation is guaranteed to conform), *Single Source of Truth* (canonical schemas defined once in `packages/types/`, consumed everywhere), *Defence in Depth* (SQLCipher encryption, individual credential encryption, audit logging), and *Fail-Fast with Defined States* (bad data is rejected before it hits SQLite).

## Document Model

One universal table shape for all plugin data. Plugin-specific fields live in a JSON column.

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
- **Sync is trivial** — One table shape means every plugin's data syncs the same way. See [[03 - Projects/Life Engine/Design/Core/Client Interface#Sync Protocol (PowerSync)]].
- **Plugin isolation is automatic** — Queries scoped by `plugin_id` + `collection`. The host enforces this.
- **Queryable JSON** — SQLite's `json_extract` handles queries at personal scale. Indexable if needed:

```sql
CREATE INDEX idx_todos_done
    ON plugin_data(json_extract(data, '$.done'))
    WHERE collection = 'todos';
```

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

Canonical schemas live in both SDKs: `plugin-sdk-rs` (Core WASM plugins) and `plugin-sdk-js` (App UI plugins). Using canonical collections requires no schema definition — the SDK already defines them. This makes canonical the path of least resistance for plugin authors.

Connector plugins write to canonical collections. Other plugins consume and extend them. This is the interoperability layer that makes the ecosystem composable.

### Private Collections (plugin-owned)

For data that only makes sense within a single plugin. A Pomodoro plugin has `pomodoro_sessions`. A habit tracker has `habit_streaks`.

Private collections are namespaced automatically — `com.example.pomodoro/pomodoro_sessions` can never collide with another plugin's collection. Plugin authors define the schema via JSON Schema in their manifest.

### Extensions on Canonical Data

Plugins that need custom fields on canonical records use a namespaced `extensions` object:

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

- Core fields are standardised and readable by all plugins
- Plugin-specific fields live in a namespaced `extensions` object
- Other plugins ignore extensions they don't understand
- No schema conflicts, no field name collisions

This removes the main reason plugin authors create private collections — "the canonical schema doesn't have the fields I need."

## Schema Validation

Validation happens at the application layer, not in SQLite.

Canonical collections are validated against SDK-defined schemas. Private collections are validated against the JSON Schema declared in the plugin manifest:

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

Bad data is rejected before it hits SQLite.

## Schema Evolution

Canonical schemas are versioned with the SDK:

- Adding fields is non-breaking
- Removing or changing fields requires a major SDK version bump with a migration path
- Gives plugin authors confidence that canonical collections won't break their plugin

Plugin migrations on version update:

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

## Cross-Plugin Data Sharing

Two plugins that need shared data both declare the same canonical collection in their manifests. The host validates that capabilities are compatible at install time.

- A "Calendar View" plugin and a "Schedule Planner" plugin both read/write `events`
- A "Pomodoro Timer" plugin reads `tasks` (canonical) and writes `pomodoro_sessions` (private)
- The email connector writes to `emails` (canonical), an "Email Viewer" plugin reads from it

This is the simplest form of plugin-to-plugin communication — shared data collections. No special mechanism needed.

## SQLite Configuration

- **Core** — `rusqlite` with SQLCipher extension
- **WAL mode** — Write-Ahead Logging for concurrent reads during writes. Sufficient for personal-scale data.
- **Single-writer** — SQLite's single-writer model is fine for personal use.

## Storage Abstraction

Pluggable storage behind a trait. SQLite is the only supported backend for now.

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

The trait exists so future backends (Postgres, S3, etc.) can be added without changing plugin code.

## Encryption at Rest

- **Database** — SQLCipher (transparent, full-database encryption)
- **Key derivation** — Argon2id (64 MB memory, 3 iterations, 4 parallelism). Configurable for low-resource devices.
- **Master passphrase** — User provides at first launch. Derived key unlocks the database.
- **File-level encryption** — `age` for exports and backups.

## Credential Storage

- Each credential encrypted separately with a key derived from the master passphrase
- OAuth refresh tokens encrypted on disk, access tokens in memory only
- Automatic rotation before token expiry
- Defence-in-depth: individual encryption even within the encrypted database

## Data Export

- **Full export** — Database, files, config, plugin data as `.tar.gz`
- **Per-service export** — All data from a specific connector
- **Standard formats** — `.eml`/`.mbox` for email, `.ics` for calendar, `.vcf` for contacts
- **API access** — All data readable via REST

## Audit Logging

Security-relevant events logged locally in structured JSON:

- Auth attempts, credential access, plugin installs, permission changes, connector auth/revocation
- Rotated daily, retained 90 days, encrypted at rest
- No telemetry or external reporting
