<!--
domain: encryption-and-audit
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Requirements Document — Encryption and Audit

## Introduction

This document specifies the encryption-at-rest and audit logging requirements for Life Engine Core. Database encryption uses SQLCipher with Argon2id key derivation. Credential storage adds a second encryption layer per record via `packages/crypto`. Audit events are emitted through the event bus on every write operation and for all security-relevant actions, persisted locally with 90-day retention.

## Alignment with Product Vision

- **Defence in Depth** — Full-database encryption via SQLCipher plus per-credential encryption via `packages/crypto` provides layered protection at rest
- **Principle of Least Privilege** — Credential decryption requires explicit capability grants; access tokens never persist to disk
- **Single Source of Truth** — All audit events flow through one event bus with consistent event types
- **No Lock-In** — Audit data stays local with no telemetry or external reporting
- **The Pit of Success** — `packages/crypto` provides a single correct API surface for encryption; module developers cannot accidentally use raw primitives

## Requirements

### Requirement 1 — Master Passphrase and Key Derivation

**User Story:** As a user, I want to set a master passphrase that derives the encryption key for my database, so that my data is protected at rest.

#### Acceptance Criteria

- 1.1. WHEN Core starts for the first time THEN the system SHALL prompt for a master passphrase and derive a 32-byte encryption key using Argon2id (64 MB memory, 3 iterations, 4 parallelism) via `packages/crypto`.
- 1.2. WHEN the user provides the correct passphrase on subsequent starts THEN the system SHALL derive the same key and successfully unlock the SQLCipher database.
- 1.3. WHEN the user provides an incorrect passphrase THEN the system SHALL fail to open the database and return an authentication error without revealing whether the passphrase or database is at fault.
- 1.4. WHEN a low-resource device is detected THEN the Argon2id parameters SHALL be configurable via `config.toml` to reduce memory and iteration requirements.

### Requirement 2 — SQLCipher Database Encryption

**User Story:** As a user, I want my entire database encrypted at rest, so that my data is protected even if my device is stolen.

#### Acceptance Criteria

- 2.1. WHEN `packages/storage-sqlite` opens the database THEN it SHALL apply the Argon2id-derived key via SQLCipher's `PRAGMA key` to enable transparent encryption.
- 2.2. WHEN the database is opened successfully THEN the system SHALL enable WAL mode for concurrent read access during writes.
- 2.3. WHEN the database file is accessed without the correct key THEN the contents SHALL be unreadable — no plaintext data, table names, or schema information SHALL be recoverable.
- 2.4. WHEN a new database is created THEN SQLCipher SHALL use AES-256-CBC as the page-level cipher with HMAC page authentication enabled.

### Requirement 3 — Shared Crypto Crate

**User Story:** As a module developer, I want a shared crypto crate so that I can use consistent, audited encryption primitives across all modules.

#### Acceptance Criteria

- 3.1. WHEN the system builds THEN `packages/crypto` SHALL export an `encrypt(plaintext, key) -> ciphertext` function using AES-256-GCM with a random 96-bit nonce prepended to the output.
- 3.2. WHEN the system builds THEN `packages/crypto` SHALL export a `decrypt(ciphertext, key) -> plaintext` function that extracts the nonce and decrypts the payload.
- 3.3. WHEN the system builds THEN `packages/crypto` SHALL export a `derive_key(passphrase, salt) -> key` function implementing Argon2id with configurable parameters.
- 3.4. WHEN the system builds THEN `packages/crypto` SHALL export an `hmac_sign(data, key) -> tag` and `hmac_verify(data, key, tag) -> bool` function pair using HMAC-SHA256.
- 3.5. WHEN any module needs encryption THEN it SHALL depend on `packages/crypto` rather than importing raw cryptographic libraries directly.

### Requirement 4 — Credential Storage Encryption

**User Story:** As a user, I want my credentials individually encrypted so that a database-level compromise does not expose all secrets at once.

#### Acceptance Criteria

- 4.1. WHEN a credential (API key, password, or token) is stored THEN the system SHALL encrypt it individually using `packages/crypto` with a key derived from the master passphrase before writing to the `credentials` collection.
- 4.2. WHEN a credential is read THEN the system SHALL decrypt it in memory and never write the plaintext to disk or logs.
- 4.3. WHEN credential encryption is applied THEN it SHALL be independent of SQLCipher's database-level encryption — both layers operate simultaneously as defence in depth.

### Requirement 5 — OAuth Token Handling

**User Story:** As a user, I want OAuth tokens handled securely so that refresh tokens are encrypted at rest and access tokens never persist to disk.

#### Acceptance Criteria

- 5.1. WHEN an OAuth refresh token is received THEN the system SHALL encrypt it at rest using `packages/crypto` before persisting to the credentials collection.
- 5.2. WHEN an OAuth access token is received THEN the system SHALL hold it in memory only and SHALL NOT write it to disk, database, or logs.
- 5.3. WHEN an access token approaches expiry THEN the system SHALL automatically rotate it using the stored refresh token before the current token expires.
- 5.4. WHEN a refresh token is rotated by the provider THEN the system SHALL encrypt and persist the new refresh token and discard the old one.

### Requirement 6 — Storage Write Audit Events

**User Story:** As a user, I want every data modification logged so that I have a complete audit trail of changes to my data.

#### Acceptance Criteria

- 6.1. WHEN a record is created in any collection THEN `StorageContext` SHALL emit a `system.storage.created` event via the event bus containing the collection name, record id, and plugin id.
- 6.2. WHEN a record is updated in any collection THEN `StorageContext` SHALL emit a `system.storage.updated` event via the event bus containing the collection name, record id, and plugin id.
- 6.3. WHEN a record is deleted from any collection THEN `StorageContext` SHALL emit a `system.storage.deleted` event via the event bus containing the collection name, record id, and plugin id.
- 6.4. WHEN a blob is stored THEN the system SHALL emit a `system.blob.stored` event via the event bus containing the blob key and plugin id.
- 6.5. WHEN a blob is deleted THEN the system SHALL emit a `system.blob.deleted` event via the event bus containing the blob key and plugin id.
- 6.6. WHEN audit events are emitted THEN read operations SHALL NOT produce audit events.

### Requirement 7 — Security Event Audit Logging

**User Story:** As a user, I want security-relevant actions logged so that I can investigate incidents and verify correct system behaviour.

#### Acceptance Criteria

- 7.1. WHEN an authentication attempt occurs (success or failure) THEN the system SHALL write an entry to the `audit_log` table with `event_type`, `timestamp`, and `details`.
- 7.2. WHEN a credential is accessed, rotated, or revoked THEN the system SHALL write an audit entry with the `plugin_id` that performed the operation.
- 7.3. WHEN a plugin is installed, enabled, or disabled THEN the system SHALL write an audit entry recording the action and plugin id.
- 7.4. WHEN a permission is granted or revoked THEN the system SHALL write an audit entry recording the permission, target plugin, and granting principal.
- 7.5. WHEN a connector is authorised or its authorisation is revoked THEN the system SHALL write an audit entry recording the connector id and action.

### Requirement 8 — Audit Log Retention and Privacy

**User Story:** As a user, I want audit logs automatically cleaned up and kept private so that old entries do not accumulate and no data leaves my device.

#### Acceptance Criteria

- 8.1. WHEN the daily retention job runs THEN the system SHALL delete all audit log entries older than 90 days.
- 8.2. WHEN audit log entries are written THEN they SHALL be encrypted at rest within the SQLCipher database alongside all other data.
- 8.3. WHEN the system operates THEN it SHALL NOT send any audit data, telemetry, or usage metrics to any external service.
- 8.4. WHEN the retention period is configurable THEN the system SHALL accept a `audit_retention_days` setting in `config.toml` with a default of 90.
