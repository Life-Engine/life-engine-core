<!--
domain: document-storage-adapter
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Document Storage Adapter — Technical Design

## Purpose

This document defines the technical design for the `DocumentStorageAdapter` trait and its supporting types. The trait is the single abstraction boundary between Core's storage layer and any concrete database backend. All upstream consumers — StorageContext, the workflow engine, and plugin-facing APIs — operate through this trait.

## Crate Location

All types defined here live in the `le-storage` crate under `crates/le-storage/src/`. The trait and its supporting types are public; concrete adapter implementations live in separate crates (e.g., `le-storage-sqlite`).

## Trait Definition

The core trait that every storage backend must implement:

```rust
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::Stream;
use serde_json::Value;
use std::pin::Pin;

#[async_trait]
pub trait DocumentStorageAdapter: Send + Sync {
    async fn get(&self, collection: &str, id: &str) -> Result<Document, StorageError>;
    async fn create(&self, collection: &str, doc: Document) -> Result<String, StorageError>;
    async fn update(&self, collection: &str, id: &str, doc: Document) -> Result<(), StorageError>;
    async fn partial_update(&self, collection: &str, id: &str, patch: Value) -> Result<(), StorageError>;
    async fn delete(&self, collection: &str, id: &str) -> Result<(), StorageError>;
    async fn list(&self, collection: &str, query: QueryDescriptor) -> Result<DocumentList, StorageError>;
    async fn count(&self, collection: &str, query: QueryDescriptor) -> Result<u64, StorageError>;
    async fn batch_create(&self, collection: &str, docs: Vec<Document>) -> Result<Vec<String>, StorageError>;
    async fn batch_update(&self, collection: &str, updates: Vec<(String, Document)>) -> Result<(), StorageError>;
    async fn batch_delete(&self, collection: &str, ids: Vec<String>) -> Result<(), StorageError>;
    async fn transaction<F, R>(&self, f: F) -> Result<R, StorageError>
    where F: FnOnce(&dyn TransactionHandle) -> Result<R, StorageError> + Send;
    fn watch(&self, collection: &str) -> Pin<Box<dyn Stream<Item = ChangeEvent> + Send>>;
    async fn migrate(&self, descriptor: CollectionDescriptor) -> Result<(), StorageError>;
    async fn health(&self) -> HealthReport;
    fn capabilities(&self) -> AdapterCapabilities;
}
```

## Query Types

### QueryDescriptor

Describes a query against a collection. All fields except `pagination` are optional.

```rust
pub struct QueryDescriptor {
    pub filter: Option<FilterNode>,
    pub sort: Vec<SortField>,
    pub pagination: Pagination,
    pub fields: Option<Vec<String>>,
    pub text_search: Option<String>,
}
```

- **`filter`** -- Optional tree of filter conditions combined with And, Or, Not
- **`sort`** -- Ordered list of fields to sort by, each with a direction
- **`pagination`** -- Required limit and optional cursor for paged results
- **`fields`** -- Optional projection; when set, only these fields are returned
- **`text_search`** -- Optional full-text query; adapters without full-text support return `StorageError::UnsupportedOperation`

### FilterNode

A recursive expression tree for composing filter conditions:

```rust
pub enum FilterNode {
    Condition {
        field: String,
        operator: FilterOperator,
        value: Value,
    },
    And(Vec<FilterNode>),
    Or(Vec<FilterNode>),
    Not(Box<FilterNode>),
}
```

### FilterOperator

The set of comparison operators available in filter conditions:

```rust
pub enum FilterOperator {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
    In,
    NotIn,
    Contains,
    StartsWith,
    Exists,
}
```

Operator semantics:

- **`Eq`** / **`Ne`** -- Equality and inequality
- **`Gt`** / **`Gte`** / **`Lt`** / **`Lte`** -- Numeric and date comparisons
- **`In`** / **`NotIn`** -- Set membership against an array value
- **`Contains`** -- Substring match for strings, element presence for arrays
- **`StartsWith`** -- String prefix match
- **`Exists`** -- Field presence check (value is ignored)

### SortField and SortDirection

```rust
pub struct SortField {
    pub field: String,
    pub direction: SortDirection,
}

pub enum SortDirection {
    Asc,
    Desc,
}
```

### Pagination

Cursor-based pagination. The limit is always required.

```rust
pub struct Pagination {
    pub limit: u32,
    pub cursor: Option<String>,
}
```

### DocumentList

The return type for `list` operations:

```rust
pub struct DocumentList {
    pub documents: Vec<Document>,
    pub next_cursor: Option<String>,
    pub total_estimate: Option<u64>,
}
```

- **`next_cursor`** -- Opaque string for fetching the next page; `None` on the last page
- **`total_estimate`** -- Optional approximate count; adapters may omit this if counting is expensive

## Transaction Types

### TransactionHandle

A restricted set of operations available within a transaction scope:

```rust
pub trait TransactionHandle: Send + Sync {
    fn get(&self, collection: &str, id: &str) -> Result<Document, StorageError>;
    fn create(&self, collection: &str, doc: Document) -> Result<String, StorageError>;
    fn update(&self, collection: &str, id: &str, doc: Document) -> Result<(), StorageError>;
    fn delete(&self, collection: &str, id: &str) -> Result<(), StorageError>;
}
```

The handle is not exposed to plugins. Only StorageContext and the workflow engine use transactions directly.

## Change Events

### ChangeEvent and ChangeType

Emitted by the `watch` stream when documents change:

```rust
pub struct ChangeEvent {
    pub collection: String,
    pub document_id: String,
    pub change_type: ChangeType,
    pub timestamp: DateTime<Utc>,
}

pub enum ChangeType {
    Created,
    Updated,
    Deleted,
}
```

StorageContext translates these into event bus events.

## Schema Migration Types

### CollectionDescriptor

Describes a collection's structure for the `migrate` method:

```rust
pub struct CollectionDescriptor {
    pub name: String,
    pub fields: Vec<FieldDescriptor>,
    pub indexes: Vec<String>,
}
```

- **`name`** -- The collection name
- **`fields`** -- Field definitions guiding storage layout
- **`indexes`** -- Field paths to index (hints from plugin manifests)

### FieldDescriptor

```rust
pub struct FieldDescriptor {
    pub name: String,
    pub field_type: FieldType,
    pub required: bool,
}
```

### FieldType

```rust
pub enum FieldType {
    String,
    Integer,
    Float,
    Boolean,
    DateTime,
    Json,
    Array(Box<FieldType>),
}
```

Type semantics:

- **`String`** -- UTF-8 text
- **`Integer`** -- 64-bit signed integer
- **`Float`** -- 64-bit floating point
- **`Boolean`** -- True or false
- **`DateTime`** -- ISO 8601 timestamp
- **`Json`** -- Arbitrary nested JSON stored as-is
- **`Array(inner)`** -- Ordered list of elements of the inner type

## Health Reporting

### HealthReport, HealthStatus, HealthCheck

```rust
pub struct HealthReport {
    pub status: HealthStatus,
    pub checks: Vec<HealthCheck>,
}

pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

pub struct HealthCheck {
    pub name: String,
    pub status: HealthStatus,
    pub message: Option<String>,
}
```

Each adapter defines its own set of health checks (e.g., "connection", "disk_space", "encryption"). The overall status reflects the worst individual check.

## Capability Negotiation

### AdapterCapabilities

```rust
pub struct AdapterCapabilities {
    pub encryption: bool,
    pub indexing: bool,
    pub full_text_search: bool,
    pub watch: bool,
    pub transactions: bool,
}
```

Fallback behaviour by capability:

- **`encryption: false`** -- If `storage.toml` requires encryption, the engine refuses to start. Application-level encryption in StorageContext operates independently.
- **`indexing: false`** -- Index hints from manifests are silently ignored. Queries still work but may be slower.
- **`full_text_search: false`** -- Queries with `text_search` return `StorageError::UnsupportedOperation`.
- **`watch: false`** -- The `watch` method returns an empty stream. StorageContext emits events on the write path instead.
- **`transactions: false`** -- The `transaction` method returns `StorageError::UnsupportedOperation`. Batch operations must still be atomic via adapter-internal mechanisms.

## Error Handling

### StorageError

```rust
pub enum StorageError {
    NotFound { collection: String, id: String },
    AlreadyExists { collection: String, id: String },
    ValidationFailed { message: String, field: Option<String> },
    CapabilityDenied { capability: String, plugin_id: String },
    SchemaConflict { collection: String, message: String },
    Timeout { operation: String, duration_ms: u64 },
    ConnectionFailed { message: String },
    UnsupportedOperation { operation: String },
    Internal { message: String },
}
```

### Workflow Status Mapping

When a storage error occurs during workflow execution, it maps to a workflow fault state:

- **`NotFound`** -- Faulted, retryable: false
- **`AlreadyExists`** -- Faulted, retryable: false
- **`ValidationFailed`** -- Faulted, retryable: false
- **`CapabilityDenied`** -- Faulted, retryable: false
- **`SchemaConflict`** -- Faulted, retryable: false
- **`Timeout`** -- Faulted, retryable: true
- **`ConnectionFailed`** -- Faulted, retryable: true
- **`UnsupportedOperation`** -- Faulted, retryable: false
- **`Internal`** -- Faulted, retryable: true

## Design Conventions

- All adapter methods are async (via `async_trait`) and the trait requires `Send + Sync`
- The `Document` type is defined in `le-storage` and wraps a `serde_json::Value` with an `id` field
- Cursors are opaque strings; their internal encoding is adapter-specific
- The `watch` method returns a pinned boxed stream for object safety
- `TransactionHandle` methods are synchronous (non-async) because they execute within an already-open transaction context
- All timestamps use `chrono::DateTime<Utc>` and are serialized as ISO 8601 / RFC 3339
