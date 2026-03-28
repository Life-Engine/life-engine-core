---
title: Blob Adapter Implementation Guide
type: adr
created: 2026-03-28
status: draft
tags:
  - architecture
  - adapter-sdk
  - blob-storage
  - core
---

# Blob Adapter Implementation Guide

## Overview

This document is the guide for implementing a `BlobStorageAdapter`. Blob adapters handle binary content storage — files, images, attachments — with self-contained metadata. Unlike document adapters, blob adapters manage their own metadata internally (no dependency on the document adapter).

For the full trait definition, see [[architecture/core/design/data-layer/blob-storage-trait]].

## Trait at a Glance

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

## Key Types

### ByteStream

```rust
pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>;
```

A streaming byte interface. Adapters that support streaming (capability `streaming: true`) process data in chunks without loading the entire blob into memory. Adapters that do not support streaming can collect the stream into a buffer internally.

### BlobInput

```rust
pub struct BlobInput {
    pub filename: String,
    pub content_type: String,
}
```

Caller-provided metadata for a store operation. The adapter stores this alongside the content.

### BlobMeta

```rust
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

The complete metadata record for a stored blob. The adapter computes and manages `size_bytes`, `checksum`, `created_at`, and `updated_at`.

## Implementation Guide by Method

### `store`

Accept a byte stream, a storage key, and input metadata. Persist the content and record metadata.

- Compute `size_bytes` from the stream as it is written
- Compute `checksum` as SHA-256 hash of the content
- Set `created_at` to now (or update `updated_at` if the key already exists)
- If a blob already exists at the key, overwrite it and update metadata accordingly
- Store metadata in your internal database (not in the document adapter)

### `retrieve`

Return a byte stream for the content at the given key.

- Return `StorageError::NotFound` if the key does not exist
- Stream the content — do not load the entire blob into memory if the backend supports streaming
- The caller (typically `StorageContext`) handles any chunking or buffering

### `delete`

Remove a blob and its metadata by key.

- Delete both the content and the metadata record
- Return `StorageError::NotFound` if the key does not exist

### `exists`

Check whether a blob exists at the given key. Return `true` or `false`.

- Should be a lightweight operation — check metadata, not content

### `copy`

Duplicate a blob from `source_key` to `dest_key`.

- If your backend supports server-side copy (capability `server_side_copy: true`), use it
- If not, fall back to retrieve-then-store internally
- Copy both content and metadata (the destination gets a new `created_at`)
- Return `StorageError::NotFound` if the source key does not exist

### `list`

List blob metadata entries matching a key prefix.

- Return all `BlobMeta` entries whose key starts with the given prefix
- Return an empty list if no blobs match
- Results should be sorted by key for deterministic output

### `metadata`

Return metadata for a single blob without retrieving the content.

- Return `StorageError::NotFound` if the key does not exist

### `health`

Return a `HealthReport`. Recommended health checks:

- Storage path accessible (directory exists, read/write permissions for filesystem adapters)
- Internal metadata database accessible
- Encryption active (if declared)
- Sufficient disk space (configurable warning threshold triggers `Degraded`)

### `capabilities`

```rust
pub struct BlobAdapterCapabilities {
    pub encryption: bool,
    pub server_side_copy: bool,
    pub streaming: bool,
}
```

- **encryption** — Adapter encrypts blobs at rest
- **server_side_copy** — Adapter can duplicate a blob without downloading it (e.g., S3 CopyObject)
- **streaming** — Adapter supports streaming reads and writes

For capabilities not supported:

- No `server_side_copy` → `StorageContext` falls back to retrieve-then-store
- No `streaming` → Operations work but large files consume more memory

## Internal Metadata Storage

The blob adapter is fully self-contained. It manages its own metadata database, separate from the document adapter.

For the v1 filesystem adapter, this is a SQLite database alongside the blob directory:

```
data/
  blobs/
    connector-email/
      attachments/
        invoice-2026-03.pdf
    connector-calendar/
      ics/
        meeting.ics
  blobs.db    # internal metadata database
```

Other adapters may use different mechanisms:

- S3 adapter — Object metadata (custom headers) or a sidecar DynamoDB table
- Postgres adapter — A `blob_metadata` table in the same database, with content in `bytea` or large objects

The internal database is an implementation detail. `StorageContext` interacts with the adapter through the trait interface only.

## Error Model

Blob operations return the same `StorageError` enum as document operations. The most common variants:

- `NotFound` — Blob does not exist at the given key
- `Timeout` — Operation exceeded the configured timeout (relevant for large files)
- `BackendError` — Storage-level failure (disk full, permission denied, connection lost)

Set `retryable: true` on `BackendError` for transient failures (e.g., temporary disk I/O error, network timeout for cloud backends). Set `retryable: false` for permanent failures (e.g., permission denied, invalid configuration).

## Change Events

The blob adapter does not emit change events directly. `StorageContext` handles event emission on the write path:

- `system.blob.stored` — After a successful `store`
- `system.blob.deleted` — After a successful `delete`

This is simpler than the document adapter's dual-mode approach (native watch vs write-path emission). All blob operations flow through `StorageContext`, so write-path emission is sufficient.

## Conformance Testing

The Adapter SDK ships a blob conformance test suite: `life_engine_adapter_tests::blob`. Run it against your implementation:

```rust
#[cfg(test)]
mod tests {
    use life_engine_adapter_tests::blob::run_conformance_suite;
    use crate::MyBlobAdapter;

    #[tokio::test]
    async fn conformance() {
        let adapter = MyBlobAdapter::new_test_instance().await;
        run_conformance_suite(&adapter).await;
    }
}
```

The suite tests:

- Store and retrieve (round-trip content integrity)
- Overwrite (store to existing key replaces content and updates metadata)
- Delete (content and metadata removed)
- Exists (true for existing, false for missing)
- Copy (content and metadata duplicated, new timestamps)
- List (prefix matching, empty results)
- Metadata (correct size, checksum, content type)
- Not-found errors for retrieve, delete, metadata
- Health check (returns `Healthy` after init)
- Capability-aware tests (server-side copy only if declared)

## Minimal Implementation

A minimal blob adapter needs:

- `store`, `retrieve`, `delete`, `exists` — Core operations
- `list`, `metadata` — Metadata access
- `copy` — Can be implemented as retrieve-then-store
- `health` — Storage accessible
- Report `server_side_copy: false` if not supported natively

The filesystem adapter (v1 built-in) is a good reference implementation. It stores blobs as files in a directory tree and metadata in a SQLite sidecar database.
