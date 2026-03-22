---
title: "Core — Data Layer"
tags: [life-engine, core, data, storage, encryption, schema]
created: 2026-03-14
updated: 2026-03-23
---

# Data Layer

The data layer provides persistent storage behind an abstract `StorageBackend` trait. SQLite/SQLCipher is the current implementation. The backend is swappable without changing any plugin or module code.

Plugins never interact with the database directly. All storage access goes through a `StorageContext` query builder provided by the plugin SDK.

## StorageBackend Trait

Defined in `packages/traits`. Each storage implementation (SQLite, Postgres, etc.) implements this trait:

```rust
trait StorageBackend: Send + Sync {
    async fn execute(&self, query: StorageQuery) -> Result<Vec<PipelineMessage>>;
    async fn mutate(&self, op: StorageMutation) -> Result<()>;
}
```

The trait is intentionally minimal. `StorageQuery` and `StorageMutation` are data structures produced by the query builder — the backend translates them to native queries for its engine.

## StorageContext Query Builder

Plugins interact with storage through a fluent query builder API provided by the plugin SDK:

```rust
// Read
let contacts = storage
    .query("contacts")
    .where_eq("city", "Sydney")
    .order_by("name")
    .limit(10)
    .execute()
    .await?;

// Write
storage
    .insert("contacts", &contact_message)
    .execute()
    .await?;

// Update
storage
    .update("contacts", id)
    .set("phone", new_phone)
    .execute()
    .await?;

// Delete
storage
    .delete("contacts", id)
    .execute()
    .await?;
```

The query builder produces `StorageQuery` / `StorageMutation` values. The active `StorageBackend` translates these to native queries (SQL for SQLite/Postgres, scan+filter for key-value stores). Plugins never import database crates directly.

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

Benefits:

- **No dynamic DDL** — Plugins never run `CREATE TABLE`. No SQL injection surface, no migration headaches.
- **Plugin isolation is automatic** — Queries scoped by `plugin_id` + `collection`. The host enforces this.
- **Queryable JSON** — SQLite's `json_extract` handles queries at personal scale.

## Two Tiers of Collections

### Canonical Collections (platform-owned)

Defined by the SDK, not by any plugin. These are the shared data types for universal personal data. Any plugin can declare read or write access.

- `events` — Calendar events
- `tasks` — To-dos, reminders
- `contacts` — People
- `notes` — Freeform text
- `emails` — Email messages
- `files` — File metadata
- `credentials` — Identity documents, OAuth tokens, API keys

Using canonical collections requires no schema definition — the SDK already defines them. This makes canonical the path of least resistance for plugin authors.

### Private Collections (plugin-owned)

For data that only makes sense within a single plugin. Namespaced automatically to prevent collisions. Plugin authors define the schema via JSON Schema in their manifest.

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

## Schema Validation

Validation happens at the application layer, not in the database.

Canonical collections are validated against SDK-defined schemas. Private collections are validated against the JSON Schema declared in the plugin manifest. Bad data is rejected before it reaches storage.

Validation level is configurable per workflow — see Workflow.md.

## Schema Evolution

Canonical schemas are versioned with the SDK:

- Adding fields is non-breaking
- Removing or changing fields requires a major SDK version bump with a migration path

See Schema Versioning Rules.md for the full versioning policy.

## Encryption at Rest

- **Database** — SQLCipher (transparent, full-database encryption)
- **Key derivation** — Argon2id (64 MB memory, 3 iterations, 4 parallelism). Configurable for low-resource devices.
- **Master passphrase** — User provides at first launch. Derived key unlocks the database.
- **Shared crypto crate** — Encryption primitives (AES-256-GCM, key derivation, HMAC) live in `packages/crypto`, shared across modules.

## Credential Storage

- Each credential encrypted separately with a key derived from the master passphrase
- OAuth refresh tokens encrypted on disk, access tokens in memory only
- Automatic rotation before token expiry
- Defence-in-depth: individual encryption even within the encrypted database

## Data Export

- **Full export** — Database, files, config, plugin data as `.tar.gz`
- **Per-service export** — All data from a specific connector
- **Standard formats** — `.eml`/`.mbox` for email, `.ics` for calendar, `.vcf` for contacts
- **API access** — All data readable via configured transports

## Audit Logging

Security-relevant events logged locally in structured JSON:

- Auth attempts, credential access, plugin installs, permission changes, connector auth/revocation
- Rotated daily, retained 90 days, encrypted at rest
- No telemetry or external reporting
