<!--
domain: data-layer
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Requirements Document — Data Layer

## Introduction

The data layer provides all persistent storage for Life Engine. It is defined by a `StorageBackend` trait in `packages/traits` and a `StorageContext` fluent query builder in `packages/plugin-sdk`. The current implementation lives in `packages/storage-sqlite`, using SQLite/SQLCipher for encrypted storage. Shared encryption primitives live in `packages/crypto`.

The data model uses a universal `plugin_data` table for all plugin data and an `audit_log` table for security events. Plugins interact with storage exclusively through `StorageContext` — they never import database crates directly and never execute DDL. The backend is swappable; implementing `StorageBackend` for a different database engine requires no changes to any plugin.

## Alignment with Product Vision

- **Defence in Depth** — Full-database encryption via SQLCipher plus per-credential encryption (using `packages/crypto`) provides layered protection at rest
- **Parse, Don't Validate** — Schema validation at the application boundary ensures only valid data reaches the database; downstream code trusts the types
- **Single Source of Truth** — One `plugin_data` table with a consistent envelope eliminates per-plugin schema drift
- **Open/Closed Principle** — The `StorageBackend` trait allows future backends without modifying plugin code
- **Principle of Least Privilege** — Plugin data is scoped by `plugin_id` and `collection`; cross-plugin access is denied by default
- **The Pit of Success** — `StorageContext` makes the correct path (using the query builder) the easiest path for plugin authors

## Requirements

### Requirement 1 — StorageBackend Trait and StorageContext

**User Story:** As a plugin author, I want to read and write data through a fluent query builder so that my plugin can persist state without importing any database crate.

#### Acceptance Criteria

- 1.1. WHEN the system initialises THEN `packages/traits` SHALL export a `StorageBackend` trait with two methods: `execute(StorageQuery) -> Vec<PipelineMessage>` and `mutate(StorageMutation) -> ()`.
- 1.2. WHEN the system initialises THEN `packages/plugin-sdk` SHALL export a `StorageContext` struct providing a fluent query builder API.
- 1.3. WHEN a plugin calls `storage.query("collection").where_eq("field", "value").order_by("field").limit(n).execute()` THEN the `StorageContext` SHALL produce a `StorageQuery` value and delegate to the active `StorageBackend`.
- 1.4. WHEN a plugin calls `storage.insert("collection", &message).execute()` THEN the `StorageContext` SHALL produce a `StorageMutation` value and delegate to the active `StorageBackend`.
- 1.5. WHEN a plugin calls `storage.update("collection", id).set("field", value).execute()` THEN the `StorageContext` SHALL produce a `StorageMutation` value and delegate to the active `StorageBackend`.
- 1.6. WHEN a plugin calls `storage.delete("collection", id).execute()` THEN the `StorageContext` SHALL produce a `StorageMutation` value and delegate to the active `StorageBackend`.

### Requirement 2 — Document Model and CRUD

**User Story:** As a plugin author, I want to create, read, update, and delete records in a collection, so that my plugin can persist and manage structured data.

#### Acceptance Criteria

- 2.1. WHEN a mutation inserts a new record THEN the `StorageBackend` implementation SHALL create a row in `plugin_data` with `id`, `plugin_id`, `collection`, `data` (JSON), `version` set to 1, and RFC 3339 `created_at`/`updated_at` timestamps.
- 2.2. WHEN a query requests a record by id THEN the `StorageBackend` implementation SHALL return the matching record as a `PipelineMessage`, scoped to the calling plugin's `plugin_id`.
- 2.3. WHEN a mutation updates a record with the correct `version` THEN the `StorageBackend` implementation SHALL update the record, increment `version` by 1, and update `updated_at`.
- 2.4. WHEN a mutation updates a record with a stale `version` THEN the `StorageBackend` implementation SHALL reject the update with a conflict error and return the current version.
- 2.5. WHEN a mutation deletes a record by id THEN the `StorageBackend` implementation SHALL remove the row from `plugin_data` and return success.
- 2.6. WHEN a query lists a collection THEN the `StorageBackend` implementation SHALL return all records matching the `plugin_id` and `collection`, scoped to the calling plugin.

### Requirement 3 — Plugin Data Isolation

**User Story:** As a user, I want each plugin's data isolated from other plugins, so that a misbehaving plugin cannot access or corrupt another plugin's data.

#### Acceptance Criteria

- 3.1. WHEN a plugin queries `plugin_data` via `StorageContext` THEN the system SHALL automatically scope the query to the calling plugin's `plugin_id`.
- 3.2. WHEN a plugin attempts to read or write a private collection owned by another plugin THEN the system SHALL reject the request with an access denied error.
- 3.3. WHEN a plugin accesses a canonical collection THEN the system SHALL allow access only if the plugin has declared the corresponding `storage:read` or `storage:write` capability.

### Requirement 4 — Schema Validation

**User Story:** As a plugin author, I want the system to validate my data before storage, so that malformed records are rejected with clear error messages.

#### Acceptance Criteria

- 4.1. WHEN a record is written to a canonical collection THEN the system SHALL validate it against the SDK-defined JSON Schema for that collection.
- 4.2. WHEN a record is written to a private collection THEN the system SHALL validate it against the JSON Schema declared in the plugin's manifest.
- 4.3. WHEN validation fails THEN the system SHALL reject the write and return an error message identifying the specific field and constraint that failed.
- 4.4. WHEN a record includes an `extensions` object on a canonical collection THEN the system SHALL accept the extensions without validating their contents, preserving them alongside core fields.

### Requirement 5 — SQLCipher Encryption

**User Story:** As a user, I want my database encrypted at rest, so that my data is protected even if my device is stolen or compromised.

#### Acceptance Criteria

- 5.1. WHEN Core starts for the first time THEN the system SHALL prompt for a master passphrase and derive an encryption key using Argon2id (64 MB memory, 3 iterations, 4 parallelism, 32-byte output) via `packages/crypto`.
- 5.2. WHEN Core opens the database THEN `packages/storage-sqlite` SHALL use the derived key to unlock the SQLCipher-encrypted database file.
- 5.3. WHEN an incorrect passphrase is provided THEN the system SHALL fail to open the database and return an authentication error.
- 5.4. WHEN the database is opened successfully THEN the system SHALL enable WAL mode for concurrent read access during writes.

### Requirement 6 — Credential Storage

**User Story:** As a user, I want my credentials (OAuth tokens, API keys) individually encrypted, so that a database-level compromise does not expose all credentials at once.

#### Acceptance Criteria

- 6.1. WHEN a credential is stored THEN the system SHALL encrypt it individually using `packages/crypto` with a key derived from the master passphrase before writing to the `credentials` collection.
- 6.2. WHEN an OAuth refresh token is stored THEN the system SHALL encrypt it at rest; the corresponding access token SHALL be held in memory only and never persisted.
- 6.3. WHEN a plugin requests credential access THEN the system SHALL verify the plugin has the `credentials:read` capability scoped to the credential type before returning decrypted data.

### Requirement 7 — Audit Logging

**User Story:** As a user, I want security events logged locally, so that I can review what happened and when.

#### Acceptance Criteria

- 7.1. WHEN an authentication attempt occurs (success or failure) THEN the system SHALL write an entry to the `audit_log` table with `event_type`, `timestamp`, and `details`.
- 7.2. WHEN a credential is accessed, rotated, or revoked THEN the system SHALL write an audit entry with the `plugin_id` that performed the operation.
- 7.3. WHEN a plugin is installed, enabled, or disabled THEN the system SHALL write an audit entry recording the action and plugin_id.
- 7.4. WHEN audit log entries are older than 90 days THEN the system SHALL delete them during the daily retention cleanup.
- 7.5. WHEN audit entries are written THEN they SHALL be encrypted at rest within the SQLCipher database alongside all other data.

### Requirement 8 — Query Filters

**User Story:** As a plugin author, I want to filter records by field values via the `StorageContext` query builder, so that I can retrieve specific subsets of data without loading everything.

#### Acceptance Criteria

- 8.1. WHEN a query includes a `where_eq("field", "value")` clause THEN the system SHALL return only records where the JSON data field equals the specified value.
- 8.2. WHEN a query includes a `where_gte("field", n)` or `where_lte("field", n)` clause THEN the system SHALL return records matching the comparison.
- 8.3. WHEN a query includes a `where_contains("field", "text")` clause THEN the system SHALL return records where the field contains the substring (case-insensitive).
- 8.4. WHEN a query combines multiple `where_*` clauses THEN the system SHALL combine them with AND logic by default.

### Requirement 9 — Sorting and Pagination

**User Story:** As a plugin author, I want to sort and paginate query results via the `StorageContext` query builder, so that I can display large datasets efficiently.

#### Acceptance Criteria

- 9.1. WHEN a query specifies `.order_by("field")` THEN the system SHALL return results ordered by the specified field in ascending order by default.
- 9.2. WHEN a query specifies `.order_by_desc("field")` THEN the system SHALL return results in descending order.
- 9.3. WHEN a query specifies `.limit(n)` and `.offset(n)` THEN the system SHALL return the specified page of results.
- 9.4. WHEN `.limit()` exceeds 1000 THEN the system SHALL cap it at 1000 and return at most 1000 records.
- 9.5. WHEN paginated results are returned THEN the response SHALL include a `total` count of all matching records.

### Requirement 10 — Data Export

**User Story:** As a user, I want to export all my data in standard formats, so that I am never locked in to Life Engine.

#### Acceptance Criteria

- 10.1. WHEN a user requests a full export THEN the system SHALL package the database, files, config, and plugin data as a `.tar.gz` archive.
- 10.2. WHEN a user requests a per-service export THEN the system SHALL export only data from the specified connector's canonical and private collections.
- 10.3. WHEN email data is exported THEN the system SHALL produce `.eml` or `.mbox` format files.
- 10.4. WHEN calendar data is exported THEN the system SHALL produce `.ics` format files.
- 10.5. WHEN contact data is exported THEN the system SHALL produce `.vcf` format files.
