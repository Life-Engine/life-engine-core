<!--
domain: data-layer
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Requirements Document — Data Layer

## Introduction

The data layer provides all persistent storage for Life Engine. It uses a single SQLite database encrypted with SQLCipher, a universal `plugin_data` table for all plugin data, and an `audit_log` table for security events. Plugins interact with storage through a `StorageAdapter` trait and never execute DDL. The query engine supports JSON field filtering, sorting, and cursor-based pagination.

## Alignment with Product Vision

- **Defence in Depth** — Full-database encryption via SQLCipher plus per-credential encryption provides layered protection at rest
- **Parse, Don't Validate** — Schema validation at the application boundary ensures only valid data reaches the database; downstream code trusts the types
- **Single Source of Truth** — One `plugin_data` table with a consistent envelope eliminates per-plugin schema drift
- **Open/Closed Principle** — The `StorageAdapter` trait allows future backends without modifying plugin code
- **Principle of Least Privilege** — Plugin data is scoped by `plugin_id` and `collection`; cross-plugin access is denied by default

## Requirements

### Requirement 1 — Document Model and CRUD

**User Story:** As a plugin author, I want to create, read, update, and delete records in a collection, so that my plugin can persist and manage structured data.

#### Acceptance Criteria

- 1.1. WHEN a plugin calls `set()` with a new record THEN the system SHALL insert a row into `plugin_data` with `id`, `plugin_id`, `collection`, `data` (JSON), `version` set to 1, and RFC 3339 `created_at`/`updated_at` timestamps.
- 1.2. WHEN a plugin calls `get()` with a valid id THEN the system SHALL return the matching record scoped to the calling plugin's `plugin_id`.
- 1.3. WHEN a plugin calls `set()` with an existing id and the correct `version` THEN the system SHALL update the record, increment `version` by 1, and update `updated_at`.
- 1.4. WHEN a plugin calls `set()` with a stale `version` THEN the system SHALL reject the update with a conflict error and return the current version.
- 1.5. WHEN a plugin calls `delete()` with a valid id THEN the system SHALL remove the row from `plugin_data` and return success.
- 1.6. WHEN a plugin calls `list()` for a collection THEN the system SHALL return all records matching the `plugin_id` and `collection`, scoped to the calling plugin.

### Requirement 2 — Plugin Data Isolation

**User Story:** As a user, I want each plugin's data isolated from other plugins, so that a misbehaving plugin cannot access or corrupt another plugin's data.

#### Acceptance Criteria

- 2.1. WHEN a plugin queries `plugin_data` THEN the system SHALL automatically scope the query to the calling plugin's `plugin_id`.
- 2.2. WHEN a plugin attempts to read or write a private collection owned by another plugin THEN the system SHALL reject the request with an access denied error.
- 2.3. WHEN a plugin accesses a canonical collection THEN the system SHALL allow access only if the plugin has declared the corresponding `storage:read` or `storage:write` capability.

### Requirement 3 — Schema Validation

**User Story:** As a plugin author, I want the system to validate my data before storage, so that malformed records are rejected with clear error messages.

#### Acceptance Criteria

- 3.1. WHEN a record is written to a canonical collection THEN the system SHALL validate it against the SDK-defined JSON Schema for that collection.
- 3.2. WHEN a record is written to a private collection THEN the system SHALL validate it against the JSON Schema declared in the plugin's manifest.
- 3.3. WHEN validation fails THEN the system SHALL reject the write and return an error message identifying the specific field and constraint that failed.
- 3.4. WHEN a record includes an `extensions` object on a canonical collection THEN the system SHALL accept the extensions without validating their contents, preserving them alongside core fields.

### Requirement 4 — SQLCipher Encryption

**User Story:** As a user, I want my database encrypted at rest, so that my data is protected even if my device is stolen or compromised.

#### Acceptance Criteria

- 4.1. WHEN Core starts for the first time THEN the system SHALL prompt for a master passphrase and derive an encryption key using Argon2id (64 MB memory, 3 iterations, 4 parallelism, 32-byte output).
- 4.2. WHEN Core opens the database THEN the system SHALL use the derived key to unlock the SQLCipher-encrypted database file.
- 4.3. WHEN an incorrect passphrase is provided THEN the system SHALL fail to open the database and return an authentication error.
- 4.4. WHEN the database is opened successfully THEN the system SHALL enable WAL mode for concurrent read access during writes.

### Requirement 5 — Credential Storage

**User Story:** As a user, I want my credentials (OAuth tokens, API keys) individually encrypted, so that a database-level compromise does not expose all credentials at once.

#### Acceptance Criteria

- 5.1. WHEN a credential is stored THEN the system SHALL encrypt it individually with a key derived from the master passphrase before writing to the `credentials` collection.
- 5.2. WHEN an OAuth refresh token is stored THEN the system SHALL encrypt it at rest; the corresponding access token SHALL be held in memory only and never persisted.
- 5.3. WHEN a plugin requests credential access THEN the system SHALL verify the plugin has the `credentials:read` capability scoped to the credential type before returning decrypted data.

### Requirement 6 — Audit Logging

**User Story:** As a user, I want security events logged locally, so that I can review what happened and when.

#### Acceptance Criteria

- 6.1. WHEN an authentication attempt occurs (success or failure) THEN the system SHALL write an entry to the `audit_log` table with `event_type`, `timestamp`, and `details`.
- 6.2. WHEN a credential is accessed, rotated, or revoked THEN the system SHALL write an audit entry with the `plugin_id` that performed the operation.
- 6.3. WHEN a plugin is installed, enabled, or disabled THEN the system SHALL write an audit entry recording the action and plugin_id.
- 6.4. WHEN audit log entries are older than 90 days THEN the system SHALL delete them during the daily retention cleanup.
- 6.5. WHEN audit entries are written THEN they SHALL be encrypted at rest within the SQLCipher database alongside all other data.

### Requirement 7 — Query Filters

**User Story:** As a plugin author, I want to filter records by field values, so that I can retrieve specific subsets of data without loading everything.

#### Acceptance Criteria

- 7.1. WHEN a query includes `{ "field": "value" }` THEN the system SHALL return only records where the JSON data field equals the specified value.
- 7.2. WHEN a query includes `{ "field": { "$gte": N } }` or `{ "field": { "$lte": N } }` THEN the system SHALL return records matching the comparison.
- 7.3. WHEN a query includes `{ "field": { "$contains": "text" } }` THEN the system SHALL return records where the field contains the substring (case-insensitive).
- 7.4. WHEN a query includes `{ "$and": [...] }` or `{ "$or": [...] }` THEN the system SHALL combine the enclosed conditions with the specified logical operator.

### Requirement 8 — Sorting and Pagination

**User Story:** As a plugin author, I want to sort and paginate query results, so that I can display large datasets efficiently.

#### Acceptance Criteria

- 8.1. WHEN a query specifies `sort_by` and `sort_dir` THEN the system SHALL return results ordered by the specified field in the specified direction.
- 8.2. WHEN a query specifies `limit` and `offset` THEN the system SHALL return the specified page of results.
- 8.3. WHEN `limit` exceeds 1000 THEN the system SHALL cap it at 1000 and return at most 1000 records.
- 8.4. WHEN paginated results are returned THEN the response SHALL include a `total` count of all matching records.

### Requirement 9 — Data Export

**User Story:** As a user, I want to export all my data in standard formats, so that I am never locked in to Life Engine.

#### Acceptance Criteria

- 9.1. WHEN a user requests a full export THEN the system SHALL package the database, files, config, and plugin data as a `.tar.gz` archive.
- 9.2. WHEN a user requests a per-service export THEN the system SHALL export only data from the specified connector's canonical and private collections.
- 9.3. WHEN email data is exported THEN the system SHALL produce `.eml` or `.mbox` format files.
- 9.4. WHEN calendar data is exported THEN the system SHALL produce `.ics` format files.
- 9.5. WHEN contact data is exported THEN the system SHALL produce `.vcf` format files.
