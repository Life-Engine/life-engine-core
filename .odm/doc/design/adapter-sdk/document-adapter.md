---
title: Document Adapter Implementation Guide
type: adr
created: 2026-03-28
status: draft
tags:
  - architecture
  - adapter-sdk
  - document-storage
  - core
---

# Document Adapter Implementation Guide

## Overview

This document is the guide for implementing a `DocumentStorageAdapter`. It walks through every trait method, explains what Core expects, documents the capability system, and describes how to use the conformance test suite.

For the full trait definition, see [[architecture/core/design/data-layer/document-storage-trait]].

## Trait at a Glance

```rust
#[async_trait]
pub trait DocumentStorageAdapter: Send + Sync {
    // Single-document CRUD
    async fn get(&self, collection: &str, id: &str) -> Result<Document, StorageError>;
    async fn create(&self, collection: &str, doc: Document) -> Result<String, StorageError>;
    async fn update(&self, collection: &str, id: &str, doc: Document) -> Result<(), StorageError>;
    async fn partial_update(&self, collection: &str, id: &str, patch: Value) -> Result<(), StorageError>;
    async fn delete(&self, collection: &str, id: &str) -> Result<(), StorageError>;

    // Query
    async fn list(&self, collection: &str, query: QueryDescriptor) -> Result<DocumentList, StorageError>;
    async fn count(&self, collection: &str, query: QueryDescriptor) -> Result<u64, StorageError>;

    // Batch (all-or-nothing)
    async fn batch_create(&self, collection: &str, docs: Vec<Document>) -> Result<Vec<String>, StorageError>;
    async fn batch_update(&self, collection: &str, updates: Vec<(String, Document)>) -> Result<(), StorageError>;
    async fn batch_delete(&self, collection: &str, ids: Vec<String>) -> Result<(), StorageError>;

    // Transaction
    async fn transaction<F, R>(&self, f: F) -> Result<R, StorageError>
    where F: FnOnce(&dyn TransactionHandle) -> Result<R, StorageError> + Send;

    // Change watching
    fn watch(&self, collection: &str) -> Pin<Box<dyn Stream<Item = ChangeEvent> + Send>>;

    // Lifecycle
    async fn migrate(&self, descriptor: CollectionDescriptor) -> Result<(), StorageError>;
    async fn health(&self) -> HealthReport;
    fn capabilities(&self) -> AdapterCapabilities;
}
```

## Implementation Guide by Method

### `get`

Retrieve a single document by collection name and ID.

- Return the full document as a `Document` (a wrapper around `serde_json::Value` with guaranteed `id`, `created_at`, `updated_at` fields)
- Return `StorageError::NotFound` if the document does not exist
- Return `StorageError::CollectionNotFound` if the collection has not been migrated

### `create`

Insert a new document. The document arrives with `id`, `created_at`, and `updated_at` already set by `StorageContext`.

- Store the document as-is. Do not modify system fields.
- Return the document's `id` on success
- Return `StorageError::AlreadyExists` if a document with the same `id` already exists in the collection

### `update`

Full replacement. The incoming document is the complete new state.

- Replace the entire stored document with the new one
- The document arrives with `updated_at` already set by `StorageContext`
- Return `StorageError::NotFound` if the document does not exist

### `partial_update`

Apply a JSON patch to specific fields. Only implement this if the backend supports native partial updates efficiently.

- If your adapter reports `partial_update: true` in capabilities, Core calls this directly
- If your adapter reports `partial_update: false`, Core never calls this — `StorageContext` handles read-modify-write and calls `update` instead
- The `patch` is a flat JSON value with only the fields to change
- Merge the patch into the existing document

### `delete`

Remove a document by collection and ID.

- Return `StorageError::NotFound` if the document does not exist

### `list`

Execute a query described by `QueryDescriptor` against a collection.

The `QueryDescriptor` contains:

- **filters** — A tree of `FilterNode` (conditions joined by `And`/`Or`). Support all `FilterOperator` variants: `Eq`, `Neq`, `Gt`, `Gte`, `Lt`, `Lte`, `Contains`, `StartsWith`, `Exists`, `In`. Dot-notation fields (e.g., `address.city`) must resolve into nested JSON.
- **sort** — Ordered list of `SortField` entries. Apply in order.
- **pagination** — Either `Offset { offset, limit }` or `Cursor { cursor, limit }`. If your adapter does not support cursor pagination (capability `cursor_pagination: false`), you can encode the offset into an opaque cursor string as a fallback.
- **fields** — Optional field projection. If your adapter cannot do projection at the storage level (capability `field_projection: false`), return the full document — `StorageContext` strips fields.
- **text_search** — Optional full-text query. If your adapter supports text search (capability `text_search: true`), use its native implementation. If not, `StorageContext` falls back to `Contains` matching.

Return a `DocumentList`:

```rust
pub struct DocumentList {
    pub items: Vec<Document>,
    pub total: Option<u64>,
    pub cursor: Option<String>,
}
```

- `total` is optional. Return it if the backend can provide a count without extra cost.
- `cursor` is set when cursor pagination is active. It is an opaque string the caller passes back for the next page.

### `count`

Count documents matching a query without retrieving them. Uses the same `QueryDescriptor` as `list` but ignores sort, pagination, and field projection.

### `batch_create`, `batch_update`, `batch_delete`

Atomic multi-document operations. Wrap them in a transaction (if your backend supports it) so they are all-or-nothing.

- `batch_create` returns IDs in the same order as input
- If any operation in the batch fails, roll back the entire batch
- Adapters that do not support transactions execute sequentially and attempt manual rollback on failure

### `transaction`

Accept a closure that receives a `TransactionHandle` and execute it within a transaction.

```rust
#[async_trait]
pub trait TransactionHandle: Send + Sync {
    async fn get(&self, collection: &str, id: &str) -> Result<Document, StorageError>;
    async fn create(&self, collection: &str, doc: Document) -> Result<String, StorageError>;
    async fn update(&self, collection: &str, id: &str, doc: Document) -> Result<(), StorageError>;
    async fn partial_update(&self, collection: &str, id: &str, patch: Value) -> Result<(), StorageError>;
    async fn delete(&self, collection: &str, id: &str) -> Result<(), StorageError>;
}
```

- If the closure returns `Ok`, commit
- If the closure returns `Err` or panics, roll back
- Transactions can span multiple collections
- If your backend does not support transactions, execute operations sequentially and report `transactions: false` in capabilities

Plugins do not call `transaction` directly — `StorageContext` uses it internally for batch operations.

### `watch`

Return a stream of `ChangeEvent` values for a collection.

```rust
pub struct ChangeEvent {
    pub collection: String,
    pub id: String,
    pub operation: ChangeOperation,
    pub changed_fields: Option<Vec<String>>,
    pub timestamp: DateTime<Utc>,
}
```

- If your backend supports native change detection (e.g., Postgres LISTEN/NOTIFY), push events through this stream
- If not, return an empty stream and report `native_watch: false` — `StorageContext` emits events on the write path instead

### `migrate`

Ensure the adapter's storage is ready for a given collection. Called at startup for every known collection.

Receives a `CollectionDescriptor`:

```rust
pub struct CollectionDescriptor {
    pub name: String,
    pub owner: CollectionOwner,
    pub fields: Vec<FieldDescriptor>,
    pub indexes: Vec<String>,
    pub schema_version: String,
}
```

Migration behaviour:

1. If the collection does not exist, create it
2. If the collection exists, compare the current descriptor with the stored one
3. Apply additive changes automatically (new fields, new indexes)
4. Return `StorageError::SchemaConflict` for breaking changes (removed fields, changed types)
5. Store the current descriptor for future comparison

Migration is idempotent. If nothing changed, it is a no-op.

The `fields` list is for storage optimisation, not enforcement. A minimal adapter can ignore field descriptors and store everything as JSON blobs.

### `health`

Return a `HealthReport`:

```rust
pub struct HealthReport {
    pub status: HealthStatus,       // Healthy, Degraded, Unhealthy
    pub backend_name: String,
    pub checks: Vec<HealthCheck>,
}
```

Recommended health checks:

- Connection alive (execute a trivial query)
- Storage accessible (read/write permissions)
- Encryption active (if declared)
- All expected collections exist

At startup, the adapter must return `Healthy`. `Degraded` is a runtime-only state.

### `capabilities`

Return an `AdapterCapabilities` struct:

```rust
pub struct AdapterCapabilities {
    pub encryption: bool,
    pub indexing: bool,
    pub transactions: bool,
    pub native_watch: bool,
    pub cursor_pagination: bool,
    pub partial_update: bool,
    pub text_search: bool,
    pub field_projection: bool,
}
```

Report honestly. Core's `StorageContext` and `StorageRouter` provide fallback behaviour for any capability you report as `false`. Lying about capabilities leads to runtime failures.

## Error Model

All methods return `StorageError`:

```rust
pub enum StorageError {
    NotFound { collection: String, id: String },
    AlreadyExists { collection: String, id: String },
    ValidationFailed { errors: Vec<FieldError> },
    CapabilityDenied { capability: String, plugin_id: String },
    CollectionNotFound { collection: String },
    SchemaConflict { collection: String, message: String },
    Timeout { operation: String, duration_ms: u64 },
    BackendError { message: String, code: Option<String>, retryable: bool },
    TransactionFailed { reason: String },
}
```

Use the correct variant. The `retryable` flag on `BackendError` is important — the workflow layer checks it to decide whether to retry. Set `retryable: true` for transient failures (connection lost, lock contention). Set `retryable: false` for permanent failures (corrupt data, unsupported operation).

## Conformance Testing

The Adapter SDK ships a conformance test suite: `life_engine_adapter_tests::document`. Run it against your adapter implementation:

```rust
#[cfg(test)]
mod tests {
    use life_engine_adapter_tests::document::run_conformance_suite;
    use crate::MyDocumentAdapter;

    #[tokio::test]
    async fn conformance() {
        let adapter = MyDocumentAdapter::new_test_instance().await;
        run_conformance_suite(&adapter).await;
    }
}
```

The suite tests:

- CRUD operations (create, get, update, partial_update, delete)
- Not-found and already-exists error cases
- Query filters (every `FilterOperator` variant)
- Sort (single and multi-field)
- Pagination (offset and cursor)
- Batch operations (create, update, delete with rollback verification)
- Transaction commit and rollback
- Migration (create collection, additive change, breaking change rejection)
- Health check (returns `Healthy` after init)
- Capability-aware tests (only runs tests for declared capabilities)

Passing the conformance suite is required before an adapter can be registered in Core.

## Minimal vs Full Implementation

A minimal adapter needs to handle:

- CRUD operations
- `list` with offset pagination and basic filters
- `count`
- `migrate` (create collections, store descriptors)
- `health` (connection alive)
- Report all optional capabilities as `false`

`StorageContext` fills in the gaps: read-modify-write for partial updates, write-path event emission for watch, offset-encoded cursors, full-document retrieval with post-query field stripping.

A full adapter provides native support for all capabilities, giving better performance but requiring more implementation effort.
