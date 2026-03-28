<!--
domain: blob-storage-adapter
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Requirements Document — Blob Storage Adapter

## Introduction

The blob storage adapter provides binary object storage for Life Engine. It is defined by a `BlobStorageAdapter` trait in `packages/traits` with supporting types in `packages/types`. The adapter is self-contained — it manages its own metadata alongside blob data without depending on the document adapter. Plugins interact with blobs through `StorageContext` in `packages/plugin-sdk`, which validates keys, enforces scoping, and emits change events.

The adapter uses `ByteStream` for streaming I/O so that large files are handled without buffering entire contents in memory. Each implementation reports its capabilities (encryption, server-side copy, streaming) so the engine can enforce configuration policy at startup.

## Alignment with Product Vision

- **Defence in Depth** — Adapters report encryption capability; the engine refuses to start if encryption is required but not supported
- **Parse, Don't Validate** — `StorageContext` validates blob key format and input before passing to the adapter; the adapter trusts its inputs
- **Open/Closed Principle** — The `BlobStorageAdapter` trait allows new backends (filesystem, S3, etc.) without modifying plugins or `StorageContext`
- **Principle of Least Privilege** — Blob keys are scoped by `plugin_id`; `StorageContext` enforces that plugins can only access their own blobs. The `copy` operation is restricted to `StorageContext` and the workflow engine.
- **The Pit of Success** — Plugins use `StorageContext` for blob operations; the correct path is the only path

## Requirements

### Requirement 1 — BlobStorageAdapter Trait

**User Story:** As a module developer, I want a well-defined async trait for blob storage so that I can implement new backends without modifying callers.

#### Acceptance Criteria

- 1.1. WHEN the system initialises THEN `packages/traits` SHALL export a `BlobStorageAdapter` trait with the following async methods: `store`, `retrieve`, `delete`, `exists`, `copy`, `list`, `metadata`, and `health`, plus a sync `capabilities` method.
- 1.2. WHEN the trait is defined THEN it SHALL require `Send + Sync` bounds so that adapters can be shared across async tasks.
- 1.3. WHEN `packages/types` is compiled THEN it SHALL export `ByteStream`, `BlobInput`, `BlobMeta`, and `BlobAdapterCapabilities` types used by the trait.

### Requirement 2 — Store Operation

**User Story:** As a plugin author, I want to store binary files with metadata so that my plugin can persist attachments and exports.

#### Acceptance Criteria

- 2.1. WHEN `store` is called with a key, `ByteStream`, and `BlobInput` THEN the adapter SHALL write the blob data to storage at the given key.
- 2.2. WHEN `store` is called THEN the adapter SHALL compute a SHA-256 hex digest of the blob content and persist it in the blob's metadata as `checksum`.
- 2.3. WHEN `store` is called THEN the adapter SHALL record `size_bytes`, `created_at`, and `updated_at` in the blob's metadata.
- 2.4. WHEN `BlobInput.content_type` is `None` THEN the adapter SHALL attempt to detect the MIME type from the filename extension, defaulting to `application/octet-stream` if detection fails.
- 2.5. WHEN a blob already exists at the given key THEN the adapter SHALL overwrite both the blob data and its metadata, updating `updated_at` while preserving the original `created_at`.
- 2.6. WHEN a store operation fails partway through THEN the adapter SHALL ensure neither partial blob data nor orphaned metadata is persisted (atomic write semantics).

### Requirement 3 — Retrieve Operation

**User Story:** As a plugin author, I want to retrieve binary files as a stream so that my plugin can process large files without loading them entirely into memory.

#### Acceptance Criteria

- 3.1. WHEN `retrieve` is called with a valid key THEN the adapter SHALL return a `ByteStream` for streaming consumption of the blob data.
- 3.2. WHEN `retrieve` is called with a key that does not exist THEN the adapter SHALL return `StorageError::NotFound`.

### Requirement 4 — Delete Operation

**User Story:** As a plugin author, I want to delete blobs so that my plugin can clean up attachments that are no longer needed.

#### Acceptance Criteria

- 4.1. WHEN `delete` is called with a valid key THEN the adapter SHALL remove both the blob data and its associated metadata.
- 4.2. WHEN `delete` is called with a key that does not exist THEN the adapter SHALL return `StorageError::NotFound`.

### Requirement 5 — Exists Operation

**User Story:** As a plugin author, I want to check whether a blob exists without downloading it so that my plugin can make decisions efficiently.

#### Acceptance Criteria

- 5.1. WHEN `exists` is called with a key THEN the adapter SHALL return `true` if a blob is stored at that key, `false` otherwise.
- 5.2. WHEN `exists` is called THEN the adapter SHALL NOT retrieve the blob data.

### Requirement 6 — Copy Operation

**User Story:** As a workflow author, I want to copy blobs between keys efficiently so that large file operations do not require re-uploading data through the engine.

#### Acceptance Criteria

- 6.1. WHEN `copy` is called and the adapter reports `server_side_copy: true` THEN the adapter SHALL copy the blob without transferring data through the engine.
- 6.2. WHEN `copy` is called and the adapter reports `server_side_copy: false` THEN the adapter SHALL fall back to retrieve-then-store to complete the copy.
- 6.3. WHEN `copy` is called with a source key that does not exist THEN the adapter SHALL return `StorageError::NotFound`.
- 6.4. WHEN `copy` completes THEN the destination blob SHALL have its own independent metadata with a new `created_at` timestamp.

### Requirement 7 — List Operation

**User Story:** As a plugin author, I want to list blobs by prefix so that my plugin can enumerate attachments in a collection.

#### Acceptance Criteria

- 7.1. WHEN `list` is called with a prefix THEN the adapter SHALL return `BlobMeta` for all blobs whose key starts with that prefix.
- 7.2. WHEN no blobs match the prefix THEN the adapter SHALL return an empty vector.
- 7.3. WHEN `list` is called THEN results SHALL NOT be paginated in v1.

### Requirement 8 — Metadata Operation

**User Story:** As a plugin author, I want to retrieve metadata for a single blob without downloading the blob data so that my plugin can display file information efficiently.

#### Acceptance Criteria

- 8.1. WHEN `metadata` is called with a valid key THEN the adapter SHALL return a `BlobMeta` struct containing `key`, `filename`, `content_type`, `size_bytes`, `checksum`, `created_at`, and `updated_at`.
- 8.2. WHEN `metadata` is called with a key that does not exist THEN the adapter SHALL return `StorageError::NotFound`.

### Requirement 9 — Health Reporting

**User Story:** As a maintainer, I want to query the blob adapter's health so that I can monitor operational status and diagnose issues.

#### Acceptance Criteria

- 9.1. WHEN `health` is called THEN the adapter SHALL return a `HealthReport` describing its current operational status.
- 9.2. WHEN the adapter cannot access its backing store THEN the `HealthReport` SHALL indicate a degraded or unavailable status.

### Requirement 10 — Capability Reporting and Enforcement

**User Story:** As a user, I want the engine to refuse to start if my storage adapter does not meet the required capabilities so that my data is always protected.

#### Acceptance Criteria

- 10.1. WHEN `capabilities` is called THEN the adapter SHALL return a `BlobAdapterCapabilities` struct with `encryption`, `server_side_copy`, and `streaming` fields.
- 10.2. WHEN `storage.toml` requires encryption and the adapter reports `encryption: false` THEN the engine SHALL refuse to start with a clear error message.
- 10.3. WHEN the adapter reports `streaming: false` THEN the adapter SHALL still accept and return `ByteStream` for API compatibility, buffering internally.

### Requirement 11 — Blob Key Format and Validation

**User Story:** As a plugin author, I want clear and consistent blob key formatting so that I can organise my blobs predictably.

#### Acceptance Criteria

- 11.1. WHEN a blob key is constructed THEN it SHALL follow the format `{plugin_id}/{context}/{filename}`.
- 11.2. WHEN a blob key contains characters outside ASCII alphanumeric, hyphens, underscores, forward slashes, and dots THEN `StorageContext` SHALL reject the key before it reaches the adapter.
- 11.3. WHEN a system blob is stored THEN the `plugin_id` segment SHALL be `"system"`.

### Requirement 12 — Change Events

**User Story:** As a maintainer, I want blob mutations to emit events so that downstream systems can react to changes.

#### Acceptance Criteria

- 12.1. WHEN a `store` operation succeeds THEN `StorageContext` SHALL emit a `system.blob.stored` event on the event bus.
- 12.2. WHEN a `delete` operation succeeds THEN `StorageContext` SHALL emit a `system.blob.deleted` event on the event bus.
- 12.3. WHEN the adapter is called directly (bypassing `StorageContext`) THEN no change events SHALL be emitted — events are the responsibility of `StorageContext`.

### Requirement 13 — Error Handling

**User Story:** As a plugin author, I want clear error types so that my plugin can handle failures gracefully.

#### Acceptance Criteria

- 13.1. WHEN a blob does not exist at the requested key THEN the adapter SHALL return `StorageError::NotFound`.
- 13.2. WHEN an operation exceeds the configured `blob_read_ms` or `blob_write_ms` timeout THEN the adapter SHALL return `StorageError::Timeout`.
- 13.3. WHEN a filesystem error, I/O failure, or streaming error occurs THEN the adapter SHALL return `StorageError::Internal` with a descriptive message.
- 13.4. WHEN an unsupported operation is requested THEN the adapter SHALL return `StorageError::UnsupportedOperation`.

### Requirement 14 — Internal Metadata Storage

**User Story:** As a module developer, I want the blob adapter to manage its own metadata so that it does not depend on the document adapter.

#### Acceptance Criteria

- 14.1. WHEN the blob adapter stores a blob THEN it SHALL persist metadata alongside the blob data using an adapter-specific mechanism.
- 14.2. WHEN the filesystem adapter is used THEN it SHALL store a `.meta.json` sidecar file next to each blob.
- 14.3. WHEN a store or delete operation completes THEN metadata and blob data SHALL be consistent — no orphaned metadata or data-without-metadata.
