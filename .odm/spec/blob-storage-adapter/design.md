<!--
domain: blob-storage-adapter
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Blob Storage Adapter — Technical Design

## Purpose

This document describes the technical design for the blob storage adapter layer. The adapter provides streaming binary object storage behind an async trait. The trait lives in `packages/traits`, supporting types live in `packages/types`, and the filesystem implementation lives in `packages/storage-fs` (or alongside the SQLite adapter in `packages/storage-sqlite` if co-located). Plugins interact with blobs exclusively through `StorageContext` in `packages/plugin-sdk`.

## Crate Layout

The blob storage adapter spans four crates:

- **`packages/types`** — `ByteStream`, `BlobInput`, `BlobMeta`, `BlobAdapterCapabilities` type definitions
- **`packages/traits`** — `BlobStorageAdapter` trait definition
- **`packages/plugin-sdk`** — `StorageContext` blob methods (key validation, event emission, delegation to adapter)
- **`packages/storage-fs`** — Filesystem implementation of `BlobStorageAdapter`

## Core Types

### ByteStream

```rust
use bytes::Bytes;
use futures::Stream;
use std::pin::Pin;

pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, StorageError>> + Send>>;
```

`ByteStream` is an asynchronous stream of byte chunks. Adapters produce and consume this type for all blob I/O. This ensures large files are never buffered entirely in memory.

### BlobInput

```rust
pub struct BlobInput {
    pub filename: Option<String>,
    pub content_type: Option<String>,
}
```

Provided by the caller on `store`. Both fields are optional:

- **`filename`** — Original filename, stored in metadata for display purposes
- **`content_type`** — MIME type. If `None`, the adapter detects from the filename extension or defaults to `application/octet-stream`

### BlobMeta

```rust
use chrono::{DateTime, Utc};

pub struct BlobMeta {
    pub key: String,
    pub filename: Option<String>,
    pub content_type: String,
    pub size_bytes: u64,
    pub checksum: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

Returned by `metadata`, `list`, and used internally by the adapter:

- **`key`** — Full storage key in `{plugin_id}/{context}/{filename}` format
- **`filename`** — Original filename from `BlobInput`, if provided
- **`content_type`** — Resolved MIME type (never `None` in output)
- **`size_bytes`** — Total byte count of the blob
- **`checksum`** — SHA-256 hex digest, computed on store
- **`created_at`** — Timestamp of first store
- **`updated_at`** — Timestamp of most recent store

### BlobAdapterCapabilities

```rust
pub struct BlobAdapterCapabilities {
    pub encryption: bool,
    pub server_side_copy: bool,
    pub streaming: bool,
}
```

Reported by each adapter via the sync `capabilities()` method:

- **`encryption`** — Adapter encrypts blobs at rest. If `false` and encryption is required in `storage.toml`, the engine refuses to start.
- **`server_side_copy`** — `copy` avoids transferring data through the engine. If `false`, the adapter falls back to retrieve-then-store.
- **`streaming`** — Adapter supports true streaming without internal buffering. If `false`, the adapter still accepts/returns `ByteStream` for API compatibility.

## BlobStorageAdapter Trait

```rust
use async_trait::async_trait;

#[async_trait]
pub trait BlobStorageAdapter: Send + Sync {
    async fn store(&self, key: &str, data: ByteStream, meta: BlobInput) -> Result<(), StorageError>;
    async fn retrieve(&self, key: &str) -> Result<ByteStream, StorageError>;
    async fn delete(&self, key: &str) -> Result<(), StorageError>;
    async fn exists(&self, key: &str) -> Result<bool, StorageError>;
    async fn copy(&self, source_key: &str, dest_key: &str) -> Result<(), StorageError>;
    async fn list(&self, prefix: &str) -> Result<Vec<BlobMeta>, StorageError>;
    async fn metadata(&self, key: &str) -> Result<BlobMeta, StorageError>;
    async fn health(&self) -> HealthReport;
    fn capabilities(&self) -> BlobAdapterCapabilities;
}
```

All methods except `capabilities` and `health` return `Result<T, StorageError>`. The `health` method returns `HealthReport` directly (health checks do not fail with `StorageError`). The `capabilities` method is synchronous because capability information is static for the lifetime of the adapter.

## Storage Key Format

Keys follow the pattern `{plugin_id}/{context}/{filename}`:

- `connector-google/attachments/meeting-notes.pdf`
- `system/exports/2026-03-28-backup.zip`
- `plugin-tasks/cache/thumbnail-abc123.png`

Allowed characters: ASCII alphanumeric (`a-z`, `A-Z`, `0-9`), hyphens (`-`), underscores (`_`), forward slashes (`/`), and dots (`.`).

`StorageContext` validates key format before delegating to the adapter. The adapter may assume keys are well-formed.

### Key Validation Logic

```rust
fn validate_blob_key(key: &str) -> Result<(), StorageError> {
    let parts: Vec<&str> = key.split('/').collect();
    if parts.len() < 3 {
        return Err(StorageError::InvalidKey("key must have at least 3 segments: plugin_id/context/filename".into()));
    }
    for ch in key.chars() {
        if !ch.is_ascii_alphanumeric() && ch != '-' && ch != '_' && ch != '/' && ch != '.' {
            return Err(StorageError::InvalidKey(format!("invalid character in key: {ch}")));
        }
    }
    Ok(())
}
```

## Filesystem Adapter Design

The filesystem adapter maps blob keys to file paths under a configurable root directory.

### Directory Structure

```
{root}/
  connector-google/
    attachments/
      meeting-notes.pdf
      meeting-notes.pdf.meta.json
  system/
    exports/
      2026-03-28-backup.zip
      2026-03-28-backup.zip.meta.json
```

### Sidecar Metadata

Each blob has a `.meta.json` sidecar file stored alongside it:

```json
{
  "key": "connector-google/attachments/meeting-notes.pdf",
  "filename": "meeting-notes.pdf",
  "content_type": "application/pdf",
  "size_bytes": 245760,
  "checksum": "a1b2c3d4e5f6...",
  "created_at": "2026-03-28T10:00:00Z",
  "updated_at": "2026-03-28T10:00:00Z"
}
```

### Atomic Write Strategy

The filesystem adapter uses a write-to-temp-then-rename strategy:

1. Write blob data to a temporary file in the same directory (e.g., `.tmp-{uuid}`)
2. Compute SHA-256 checksum while streaming the data
3. Write the `.meta.json` sidecar to a temporary file
4. Rename the blob temp file to the final path
5. Rename the sidecar temp file to the final path

If any step fails, both temp files are cleaned up. The rename operations are atomic on POSIX filesystems, ensuring consistency.

### Copy Behaviour

The filesystem adapter reports `server_side_copy: false`. The `copy` method calls `retrieve` on the source key and pipes the `ByteStream` into `store` at the destination key.

### Health Check

The filesystem adapter's `health` method verifies:

- The root directory exists and is writable
- Disk space is above a configured threshold

### Capabilities

The filesystem adapter reports:

- **`encryption`** — `false` (filesystem adapter does not encrypt at rest; application-level encryption is handled by `StorageContext` if needed)
- **`server_side_copy`** — `false`
- **`streaming`** — `true`

## StorageContext Blob Integration

`StorageContext` in `packages/plugin-sdk` provides the public API for blob operations. It:

1. Validates the blob key format
2. Scopes the key to the calling plugin's `plugin_id` (prepends it if not already present)
3. Delegates to the active `BlobStorageAdapter`
4. Emits change events on the event bus after successful mutations

### Event Emission

After a successful `store`, `StorageContext` emits:

```rust
Event {
    topic: "system.blob.stored",
    payload: json!({
        "key": "connector-google/attachments/meeting-notes.pdf",
        "content_type": "application/pdf",
        "size_bytes": 245760
    }),
}
```

After a successful `delete`, `StorageContext` emits:

```rust
Event {
    topic: "system.blob.deleted",
    payload: json!({
        "key": "connector-google/attachments/meeting-notes.pdf"
    }),
}
```

### Copy Restriction

The `copy` method is not exposed to plugins via `StorageContext`. Only internal callers (`StorageContext` itself and the workflow engine) may invoke `copy` on the adapter directly.

## Error Handling

Blob operations use the shared `StorageError` enum (defined in `packages/types`, documented in the document-storage-adapter spec). Blob-specific scenarios:

- **`StorageError::NotFound`** — No blob at the requested key
- **`StorageError::Timeout`** — Operation exceeded `blob_read_ms` or `blob_write_ms` from configuration
- **`StorageError::Internal`** — Filesystem I/O error, streaming error, or unexpected failure
- **`StorageError::UnsupportedOperation`** — Requested operation not supported by this adapter

## MIME Type Detection

When `BlobInput.content_type` is `None`, the adapter resolves the MIME type using the following strategy:

1. If `BlobInput.filename` is present, use the file extension to look up the MIME type (e.g., `.pdf` maps to `application/pdf`)
2. If the extension is unknown or no filename is provided, default to `application/octet-stream`

A lightweight lookup (not a full `mime_guess` crate) is sufficient for v1. Common mappings:

- `.pdf` — `application/pdf`
- `.png` — `image/png`
- `.jpg` / `.jpeg` — `image/jpeg`
- `.zip` — `application/zip`
- `.json` — `application/json`
- `.txt` — `text/plain`

## Configuration

Blob adapter configuration lives in `storage.toml`:

```toml
[blob]
adapter = "filesystem"
root = "./data/blobs"
blob_read_ms = 30000
blob_write_ms = 60000
require_encryption = false
```

- **`adapter`** — Which blob adapter to use (only `"filesystem"` in v1)
- **`root`** — Root directory for the filesystem adapter
- **`blob_read_ms`** — Timeout for read operations in milliseconds
- **`blob_write_ms`** — Timeout for write operations in milliseconds
- **`require_encryption`** — If `true`, the engine refuses to start unless the adapter reports `encryption: true`
