---
title: Blob Storage Adapter Specification
type: reference
created: 2026-03-28
status: active
tags:
  - storage
  - adapter
  - blob
  - spec
---

# Blob Storage Adapter Specification

## Trait Definition

Every blob storage adapter must implement the following trait:

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

### store

Write a blob to storage at the given key. The `data` parameter is a `ByteStream` for streaming large files without loading them entirely into memory. The `meta` parameter provides metadata to store alongside the blob. If a blob already exists at the key, it is overwritten.

### retrieve

Read a blob from storage by key. Returns a `ByteStream` for streaming consumption. Returns `StorageError::NotFound` if no blob exists at the key.

### delete

Remove a blob and its associated metadata by key. Returns `StorageError::NotFound` if no blob exists at the key.

### exists

Check whether a blob exists at the given key. Returns `true` if present, `false` otherwise. This operation must not retrieve the blob data.

### copy

Copy a blob from `source_key` to `dest_key`. If the adapter supports server-side copy (reported via capabilities), the copy happens without downloading and re-uploading the data. If server-side copy is not supported, the adapter must fall back to retrieve-then-store. Returns `StorageError::NotFound` if the source key does not exist. The `copy` operation is not exposed to plugins — only `StorageContext` and the workflow engine use it.

### list

Return metadata for all blobs whose key starts with the given prefix. Returns an empty vector if no blobs match. Results are not paginated in v1.

### metadata

Return metadata for a single blob by key. Returns `StorageError::NotFound` if no blob exists at the key.

### health

Return a `HealthReport` describing the adapter's current operational status. See [[document-storage-adapter]] for `HealthReport`, `HealthStatus`, and `HealthCheck` definitions.

### capabilities

Return the adapter's `BlobAdapterCapabilities` struct describing which optional features are supported.

## Storage Keys

Blob keys follow the format: `{plugin_id}/{context}/{filename}`

- **`plugin_id`** — The plugin that owns the blob. For system blobs, use `"system"`.
- **`context`** — A grouping segment chosen by the caller (e.g., `"attachments"`, `"exports"`, `"cache"`).
- **`filename`** — The blob's filename, preserving the original extension where possible.

Examples:

- `connector-google/attachments/meeting-notes.pdf`
- `system/exports/2026-03-28-backup.zip`
- `plugin-tasks/cache/thumbnail-abc123.png`

Keys must contain only ASCII alphanumeric characters, hyphens, underscores, forward slashes, and dots. `StorageContext` validates key format before passing to the adapter.

## ByteStream

```rust
pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, StorageError>> + Send>>;
```

`ByteStream` is an asynchronous stream of byte chunks. Adapters must support streaming reads and writes without buffering entire blobs in memory. Errors during streaming are reported as `StorageError::Internal` with a descriptive message.

## BlobInput

```rust
pub struct BlobInput {
    pub filename: Option<String>,
    pub content_type: Option<String>,
}
```

- **`filename`** — The original filename, if known. Optional. Stored in metadata.
- **`content_type`** — MIME type of the blob (e.g., `"application/pdf"`, `"image/png"`). Optional. If not provided, the adapter may attempt to detect it from the filename extension or default to `"application/octet-stream"`.

## BlobMeta

```rust
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

- **`key`** — The full storage key
- **`filename`** — The original filename, if provided during store
- **`content_type`** — MIME type of the blob
- **`size_bytes`** — Total size in bytes
- **`checksum`** — SHA-256 hex digest of the blob content. Computed by the adapter on store.
- **`created_at`** — Timestamp when the blob was first stored
- **`updated_at`** — Timestamp of the last store operation for this key

## Internal Metadata Storage

The blob adapter is self-contained — it stores metadata alongside blob data without depending on the document adapter. The storage mechanism is adapter-specific:

- The filesystem adapter stores a `.meta.json` sidecar file next to each blob
- Other adapters may use a metadata index file, database table, or object metadata headers

The adapter must ensure metadata and blob data remain consistent. If a store operation fails partway through, neither the blob nor its metadata must be persisted (atomic write semantics).

## BlobAdapterCapabilities

```rust
pub struct BlobAdapterCapabilities {
    pub encryption: bool,
    pub server_side_copy: bool,
    pub streaming: bool,
}
```

Capability behaviour:

- **`encryption`** — If `true`, the adapter encrypts blobs at rest. If `false` and encryption is required in `storage.toml`, the engine refuses to start. Application-level encryption (e.g., for credentials) operates independently in `StorageContext`.
- **`server_side_copy`** — If `true`, the `copy` operation avoids transferring data through the engine. If `false`, the adapter falls back to retrieve-then-store for copy operations.
- **`streaming`** — If `true`, the adapter supports true streaming via `ByteStream` without buffering entire blobs. If `false`, the adapter buffers internally but still accepts and returns `ByteStream` for API compatibility.

## Change Events

The blob adapter does not emit change events directly. `StorageContext` emits blob-related events on the write path:

- **`system.blob.stored`** — Emitted after a successful `store` call
- **`system.blob.deleted`** — Emitted after a successful `delete` call

See [[storage-context]] for event details and [[event-bus]] for event bus semantics.

## Error Handling

Blob operations use the same `StorageError` enum as document operations. See [[document-storage-adapter]] for the full enum definition and workflow status mapping.

Blob-specific error scenarios:

- **`StorageError::NotFound`** — Blob does not exist at the requested key
- **`StorageError::Timeout`** — Operation exceeded the configured `blob_read_ms` or `blob_write_ms` timeout
- **`StorageError::Internal`** — Filesystem errors, I/O failures, or streaming errors
- **`StorageError::UnsupportedOperation`** — Operation not supported by this adapter (e.g., `copy` without server-side copy and streaming fallback disabled)
