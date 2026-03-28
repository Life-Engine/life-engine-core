---
title: Blob Storage Trait
type: adr
created: 2026-03-28
status: draft
tags:
  - architecture
  - data-layer
  - blob-storage
  - adapter
  - core
---

# Blob Storage Trait

## Overview

The blob storage trait defines the contract for binary content storage adapters. Unlike document storage, blob storage is fully self-contained — it stores both the binary content and its own metadata (filename, content type, size, checksum) in its own internal database.

Blob storage is a separate system from document storage. It is not a document collection. Plugins that need to associate files with documents store the blob key as a field on their document record. The relationship is a reference, not co-ownership.

V1 ships one built-in adapter: local filesystem with an internal SQLite database for metadata.

## Trait Definition

```rust
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

## Operations

- **store** — Accept a byte stream, a storage key, and input metadata. Persist the content and record the metadata internally. If a blob already exists at the key, it is overwritten.
- **retrieve** — Accept a storage key, return a byte stream. Streaming (not load-all-into-memory) so large files do not exhaust memory. Returns `StorageError::NotFound` if the key does not exist.
- **delete** — Remove a blob and its metadata by storage key. Returns `StorageError::NotFound` if the key does not exist.
- **exists** — Check whether a blob exists at the given key without retrieving it.
- **copy** — Duplicate a blob under a new key. Adapters that support server-side copy (e.g., S3) perform it without downloading. Adapters that do not support server-side copy fall back to retrieve-then-store internally.
- **list** — List blob metadata entries matching a key prefix. Returns an empty list if no blobs match.
- **metadata** — Return metadata for a single blob without retrieving the content. Returns `StorageError::NotFound` if the key does not exist.
- **health** — Self-check for the adapter.
- **capabilities** — Report what the adapter supports.

## Storage Keys

Keys are opaque strings provided by the caller. The recommended format is hierarchical:

```
{plugin_id}/{context}/{filename}
```

For example: `connector-email/attachments/invoice-2026-03.pdf`

`StorageContext` enforces that plugins can only store and retrieve blobs under their own plugin ID prefix. System-level callers have no prefix restriction.

## Blob Metadata

The adapter owns and manages all blob metadata internally. This metadata is not stored in document storage.

```rust
pub struct BlobInput {
    pub filename: String,
    pub content_type: String,
}

pub struct BlobMeta {
    pub key: String,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: u64,
    pub checksum: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

- `checksum` — SHA-256 hash of the content, computed by the adapter on store
- `size_bytes` — Computed by the adapter from the byte stream
- `created_at` / `updated_at` — Managed by the adapter

The adapter is responsible for keeping metadata in sync with the actual blob content. If a blob is overwritten via `store`, the adapter updates the metadata accordingly.

## Internal Metadata Storage

The blob adapter maintains its own internal database for metadata. For the v1 filesystem adapter, this is a SQLite database alongside the blob directory. The internal database is an implementation detail — other adapters may use different mechanisms (e.g., S3 object metadata, Postgres table).

This design keeps blob storage fully self-contained. No cross-backend coordination is needed between document storage and blob storage.

## Byte Streaming

The `store` and `retrieve` methods use byte streams rather than in-memory buffers:

```rust
pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>;
```

This allows large files to be stored and retrieved without loading the entire content into memory. The transport layer handles any chunking — blob storage receives and returns a continuous stream.

## Health Check

Blob adapter health checks:

- Storage path accessible (directory exists, read/write permissions for filesystem adapter)
- Internal metadata database accessible
- Encryption active (if capability declared)
- Sufficient disk space (warning threshold configurable, triggers `Degraded` status)

The `HealthReport` struct is shared with the document storage trait — see [[document-storage-trait#Health Check]].

## Adapter Capabilities

```rust
pub struct BlobAdapterCapabilities {
    pub encryption: bool,
    pub server_side_copy: bool,
    pub streaming: bool,
}
```

- **encryption** — Adapter encrypts blobs at rest
- **server_side_copy** — Adapter can duplicate a blob without downloading it
- **streaming** — Adapter supports streaming reads and writes (vs loading full blob into memory)

For capabilities an adapter does not support:

- No `server_side_copy` → `StorageContext` falls back to retrieve-then-store
- No `streaming` → Operations work but large files consume more memory

Required capabilities for blob storage are configurable in `storage.toml`. The default configuration does not require encryption for blob storage — filesystem encryption is typically handled at the OS or volume level.

## Change Events

`StorageContext` emits change events for blob operations via the [[architecture/core/design/workflow-engine-layer/event-bus|event bus]]:

- `system.blob.stored` — `{ key }`
- `system.blob.deleted` — `{ key }`

These events are emitted by `StorageContext` on the write path. The blob storage trait does not include a `watch` method — unlike document storage, blob changes are always observable from the write path since all operations flow through `StorageContext`.

## Error Handling

Blob operations return the same `StorageError` enum as document operations — see [[document-storage-trait#Error Model]]. The most common variants for blob operations:

- `NotFound` — Blob does not exist at the given key
- `Timeout` — Operation exceeded the configured timeout (relevant for large files)
- `BackendError` — Storage-level failure (disk full, permission denied, connection lost)
