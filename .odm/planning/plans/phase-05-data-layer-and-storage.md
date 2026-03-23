<!--
project: life-engine-core
phase: 5
specs: data-layer
updated: 2026-03-23
-->

# Phase 5 — Data Layer and Storage

## Plan Overview

This phase implements the complete data layer: the `StorageBackend` implementation for SQLite/SQLCipher in `packages/storage-sqlite`, the `StorageContext` query builder integration, schema validation, plugin data isolation, per-credential encryption, audit logging, and data export. The universal document model uses a single `plugin_data` table with JSON data columns — no dynamic DDL, no plugin-specific tables.

This phase depends on Phase 2 (types), Phase 3 (traits, crypto), and Phase 4 (plugin SDK). Phase 6 (auth), Phase 9 (startup), and Phase 10 (migration) depend on the storage backend implemented here.

> spec: .odm/spec/data-layer/brief.md

Progress: 0 / 16 work packages complete

---

## 5.1 — SQLite Schema DDL
> spec: .odm/spec/data-layer/brief.md

- [x] Create plugin_data table DDL with composite index
  <!-- file: packages/storage-sqlite/src/schema.rs -->
  <!-- purpose: Define CREATE TABLE plugin_data with columns: id (TEXT PRIMARY KEY — UUID as text), plugin_id (TEXT NOT NULL — owning plugin identifier), collection (TEXT NOT NULL — collection name like "events", "tasks", or private collection name), data (TEXT NOT NULL — JSON-serialized record), version (INTEGER NOT NULL DEFAULT 1 — schema version for migrations and optimistic concurrency), created_at (TEXT NOT NULL — ISO 8601 timestamp), updated_at (TEXT NOT NULL — ISO 8601 timestamp). Create composite index idx_plugin_collection ON plugin_data(plugin_id, collection) for efficient per-plugin queries. Create index idx_collection ON plugin_data(collection) for cross-plugin canonical collection queries. Define the DDL as a const &str for use during database initialization. -->
  <!-- requirements: 2.1 -->
  <!-- leverage: none -->

- [x] Create audit_log table DDL with timestamp index
  <!-- file: packages/storage-sqlite/src/schema.rs -->
  <!-- purpose: Define CREATE TABLE audit_log with columns: id (TEXT PRIMARY KEY — UUID), timestamp (TEXT NOT NULL — ISO 8601), event_type (TEXT NOT NULL — one of: auth_success, auth_failure, credential_access, credential_modify, plugin_load, plugin_error, permission_change, data_export), plugin_id (TEXT — nullable, only set for plugin-related events), details (TEXT NOT NULL — JSON-serialized event details), created_at (TEXT NOT NULL). Create index idx_audit_timestamp ON audit_log(timestamp) for time-range queries and retention cleanup. Define daily rotation constant: 90 days retention. -->
  <!-- requirements: 7.1 -->
  <!-- leverage: schema.rs from previous task -->

---

## 5.2 — SQLCipher Database Initialization
> spec: .odm/spec/data-layer/brief.md

- [x] Implement database open with SQLCipher encryption and WAL mode
  <!-- file: packages/storage-sqlite/src/lib.rs -->
  <!-- purpose: Implement pub async fn init(config: toml::Value, key: [u8; 32]) -> Result<SqliteStorage> that: (1) extracts database path from config, (2) opens SQLCipher database using rusqlite with bundled-sqlcipher feature, (3) sets PRAGMA key using the 32-byte key derived from Argon2id (convert to hex string for SQLCipher), (4) sets PRAGMA journal_mode = WAL for concurrent read performance, (5) sets PRAGMA foreign_keys = ON, (6) runs schema DDL from schema.rs to create tables if they don't exist, (7) returns SqliteStorage struct wrapping the connection. Handle initialization errors with clear messages: wrong key (unable to decrypt), missing file (create new), permission denied. The key parameter comes from packages/crypto::derive_key() called by Core during startup — this crate does not call derive_key itself. -->
  <!-- requirements: 5.1, 5.2, 5.3, 5.4 -->
  <!-- leverage: packages/crypto for key format -->

---

## 5.3 — StorageBackend::execute for SQLite
> depends: 5.1, 5.2
> spec: .odm/spec/data-layer/brief.md

- [x] Implement StorageBackend::execute to translate StorageQuery to SQL
  <!-- file: packages/storage-sqlite/src/backend.rs -->
  <!-- purpose: Implement the execute method on SqliteStorage. Translation logic: (1) start with SELECT id, plugin_id, collection, data, version, created_at, updated_at FROM plugin_data, (2) add WHERE plugin_id = ? for plugin scoping (always present), (3) add WHERE collection = ? from query.collection, (4) for each QueryFilter, translate to SQL: Eq -> json_extract(data, '$.field') = ?, Gte -> json_extract(data, '$.field') >= ?, Lte -> json_extract(data, '$.field') <= ?, Contains -> json_extract(data, '$.field') LIKE '%?%', NotEq -> json_extract(data, '$.field') != ?, (5) apply ORDER BY json_extract(data, '$.field') ASC/DESC from sort fields, (6) apply LIMIT and OFFSET with maximum 1000 limit cap, (7) deserialize each row into a PipelineMessage by parsing the data JSON column into the appropriate CdmType based on collection name, wrapping in TypedPayload::Cdm, and constructing MessageMetadata from the row's metadata. Use parameterized queries throughout — never interpolate user values into SQL strings. Return Vec<PipelineMessage>. -->
  <!-- requirements: 2.2, 2.6, 8.1, 8.2, 8.3, 8.4, 9.1, 9.2, 9.3, 9.4, 9.5 -->
  <!-- leverage: rusqlite, schema.rs -->

---

## 5.4 — StorageBackend::mutate for SQLite
> depends: 5.1, 5.2
> spec: .odm/spec/data-layer/brief.md

- [x] Implement StorageBackend::mutate to translate StorageMutation to SQL
  <!-- file: packages/storage-sqlite/src/backend.rs -->
  <!-- purpose: Implement the mutate method on SqliteStorage. For StorageMutation::Insert: INSERT INTO plugin_data (id, plugin_id, collection, data, version, created_at, updated_at) VALUES (?, ?, ?, ?, 1, ?, ?). Generate UUID for id, serialize PipelineMessage payload to JSON for data column, set version to 1, set timestamps to now. For StorageMutation::Update: UPDATE plugin_data SET data = ?, version = version + 1, updated_at = ? WHERE id = ? AND plugin_id = ? AND version = ?. The WHERE version = ? clause implements optimistic concurrency — if the version has changed since the caller read it, the update affects 0 rows and returns a ConcurrencyConflict error. For StorageMutation::Delete: DELETE FROM plugin_data WHERE id = ? AND plugin_id = ?. The plugin_id check ensures a plugin can only delete its own data. All mutations use parameterized queries. Wrap each mutation in a transaction. -->
  <!-- requirements: 2.1, 2.3, 2.4, 2.5 -->
  <!-- leverage: rusqlite, schema.rs -->

---

## 5.5 — Plugin Data Isolation
> depends: 5.3, 5.4
> spec: .odm/spec/data-layer/brief.md

- [x] Enforce plugin_id scoping on all storage operations
  <!-- file: packages/storage-sqlite/src/backend.rs -->
  <!-- purpose: Audit all execute and mutate code paths to ensure every SQL query includes WHERE plugin_id = ? with the requesting plugin's ID. For canonical collections (events, tasks, contacts, notes, emails, files, credentials): allow read access across all plugins (canonical data is shared), but write access only for the owning plugin. For private collections: enforce strict plugin_id isolation on both reads and writes — a plugin cannot read or write another plugin's private collection data. Add a test that creates data with plugin_id "plugin-a", then attempts to read/update/delete it with plugin_id "plugin-b" and verifies the operation fails or returns empty results. -->
  <!-- requirements: 3.1, 3.2 -->
  <!-- leverage: backend.rs implementation -->

- [x] Add capability check for canonical collection storage access
  <!-- file: packages/plugin-sdk/src/storage.rs -->
  <!-- purpose: Before executing any StorageContext operation, verify the calling plugin has the required capability: storage:read for query/read operations, storage:write for insert/update/delete operations. If the capability is not in the plugin's approved set, return a CapabilityViolation error with CAP_002 code and Severity::Fatal. The capability set is passed to StorageContext at construction time. Add tests: read with storage:read succeeds, read without storage:read returns CAP_002 error, write with storage:write succeeds, write without storage:write returns CAP_002 error. -->
  <!-- requirements: 3.3 -->
  <!-- leverage: capability types from Phase 3 -->

---

## 5.6 — Canonical Collection Schema Validation
> depends: 5.3, 5.4
> spec: .odm/spec/data-layer/brief.md

- [x] Implement schema validation for canonical collections on write
  <!-- file: packages/storage-sqlite/src/validation.rs -->
  <!-- purpose: Load the 7 JSON Schema files from a bundled location (embedded in the binary using include_str! or loaded from a schemas/ directory). Before any Insert or Update mutation on a canonical collection, validate the data field against the corresponding JSON Schema using the jsonschema crate. If validation fails, return a StorageValidationError with the collection name, the failing field path, and the constraint that was violated. Validation errors have Severity::Fatal — invalid data never enters the database. Map collection name strings ("events", "tasks", "contacts", "notes", "emails", "files", "credentials") to their corresponding schemas. Return a clear error if a write targets an unknown collection that is neither canonical nor a declared private collection. -->
  <!-- requirements: 4.1, 4.3 -->
  <!-- leverage: JSON schemas from Phase 2 -->

---

## 5.7 — Private Collection Schema Validation
> depends: 5.6
> spec: .odm/spec/data-layer/brief.md

- [ ] Implement schema validation for plugin private collections
  <!-- file: packages/storage-sqlite/src/validation.rs -->
  <!-- purpose: Accept a registry of private collection schemas loaded from plugin manifests. Each plugin can declare private collections in its manifest.toml under [collections.private] with a JSON Schema definition. Before any write to a private collection, look up the schema by the composite key (plugin_id, collection_name) and validate. If no schema is registered for the collection, reject the write with an error. Private collections are namespaced by plugin_id — plugin "com.example.weather" can write to "com.example.weather:forecasts" but not to "com.example.maps:locations". Add tests: write with valid schema passes, write violating schema fails, write to unregistered collection fails, cross-plugin write rejected. -->
  <!-- requirements: 4.2, 4.3 -->
  <!-- leverage: validation.rs from previous WP -->

---

## 5.8 — Extensions Object Support
> depends: 5.6
> spec: .odm/spec/data-layer/brief.md

- [ ] Allow extensions field on canonical collection records without validation
  <!-- file: packages/storage-sqlite/src/validation.rs -->
  <!-- purpose: When validating canonical collection records, allow an "extensions" field containing arbitrary nested JSON without validating its internal structure against the canonical schema. The extensions field is an object where keys are reverse-domain plugin namespaces and values are arbitrary JSON. The canonical schema's additionalProperties: false should NOT reject the extensions field — add it as an explicitly allowed optional property in the validation logic (or use a schema that permits it). Verify that writing a canonical record with extensions works, reading it back preserves extensions, and updating without specifying extensions preserves existing extension data (merge semantics). Credentials collection has no extensions field — verify writes with extensions on Credentials are rejected. -->
  <!-- requirements: 4.4 -->
  <!-- leverage: validation.rs from previous WP -->

---

## 5.9 — Per-Credential Encryption
> spec: .odm/spec/data-layer/brief.md

- [ ] Implement individual credential encryption within the credentials collection
  <!-- file: packages/storage-sqlite/src/credentials.rs -->
  <!-- purpose: Override the default storage flow for the "credentials" collection to add per-credential encryption. On Insert/Update: before writing to the database, encrypt the credential's sensitive fields (the "claims" object containing tokens, keys, documents) using packages/crypto::encrypt() with a derived key specific to the credential (derive from master key + credential ID as salt via packages/crypto::derive_key()). Store the encrypted blob in the data column with an "encrypted": true flag and a "nonce" field. On Read: decrypt the claims field using packages/crypto::decrypt() before returning the PipelineMessage. If decryption fails (wrong key, corrupted data), return a clear error with the credential ID but never return partially-decrypted data. This provides defence-in-depth: even if SQLCipher encryption is compromised, individual credentials remain encrypted. Add tests: encrypt/store/read round-trip preserves data, read with wrong key fails, credential without encryption flag reads normally. -->
  <!-- requirements: 6.1, 6.2, 6.3 -->
  <!-- leverage: packages/crypto for AES-256-GCM -->

---

## 5.10 — Audit Logging
> depends: 5.1
> spec: .odm/spec/data-layer/brief.md

- [ ] Implement audit log write functions
  <!-- file: packages/storage-sqlite/src/audit.rs -->
  <!-- purpose: Define AuditEvent struct with event_type (AuditEventType enum: AuthSuccess, AuthFailure, CredentialAccess, CredentialModify, PluginLoad, PluginError, PermissionChange, DataExport), plugin_id (Option<String>), and details (serde_json::Value). Implement pub async fn log_event(db: &Connection, event: AuditEvent) -> Result<()> that inserts a row into the audit_log table with a generated UUID, current timestamp, and the event data. Implement pub async fn cleanup_old_entries(db: &Connection) -> Result<u64> that deletes entries older than 90 days and returns the count of deleted rows. The cleanup function should be called daily — expose it for the scheduler to invoke. Add daily rotation by date: audit entries are bucketed by day in the timestamp index. -->
  <!-- requirements: 7.1, 7.2, 7.3, 7.4, 7.5 -->
  <!-- leverage: audit_log table from schema.rs -->

---

## 5.11 — Audit Logging Integration
> depends: 5.10, 5.9
> spec: .odm/spec/data-layer/brief.md

- [ ] Wire audit logging into storage operations
  <!-- file: packages/storage-sqlite/src/backend.rs -->
  <!-- purpose: After credential reads, log AuditEventType::CredentialAccess with the credential ID and requesting plugin_id. After credential writes, log AuditEventType::CredentialModify with the credential ID, operation type (insert/update/delete), and requesting plugin_id. After plugin load events (called by the plugin system), log AuditEventType::PluginLoad. After permission changes (capability modifications), log AuditEventType::PermissionChange. Audit logging must not block the main operation — log failures are warned but do not cause the storage operation to fail. Add tests verifying audit entries are created for credential access and modification. -->
  <!-- requirements: 7.2, 7.3, 7.4 -->
  <!-- leverage: audit.rs from previous WP -->

---

## 5.12 — Data Export
> spec: .odm/spec/data-layer/brief.md

- [ ] Implement full database export as compressed archive
  <!-- file: packages/storage-sqlite/src/export.rs -->
  <!-- purpose: Implement pub async fn export_full(db: &Connection, output_path: &Path) -> Result<PathBuf> that: (1) queries all data from plugin_data table, (2) groups records by collection, (3) serializes each collection as a JSON array file (events.json, tasks.json, etc.), (4) packages all files into a .tar.gz archive at the output path, (5) logs AuditEventType::DataExport with record counts per collection, (6) returns the path to the created archive. Handle large datasets by streaming records rather than loading all into memory. -->
  <!-- requirements: 10.1, 10.2 -->
  <!-- leverage: packages/storage-sqlite as data source -->

- [ ] Implement per-service data export in standard formats
  <!-- file: packages/storage-sqlite/src/export.rs -->
  <!-- purpose: Implement format-specific export functions: export_emails(db, output_path) -> .mbox file (standard mbox format with From_ separator lines), export_calendar(db, output_path) -> .ics file (iCalendar format with VCALENDAR/VEVENT blocks), export_contacts(db, output_path) -> .vcf file (vCard 4.0 format). Each function queries the relevant collection, transforms CDM records to the standard format, and writes the output file. Use the dav-utils crate for iCal and vCard serialization where applicable. Log AuditEventType::DataExport for each export operation. -->
  <!-- requirements: 10.3, 10.4, 10.5 -->
  <!-- leverage: packages/dav-utils for format helpers -->
