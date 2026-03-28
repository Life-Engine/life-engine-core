---
title: Document Storage Adapter Specification
type: reference
created: 2026-03-28
status: active
tags:
  - storage
  - adapter
  - document
  - spec
---

# Document Storage Adapter Specification

## Trait Definition

Every document storage adapter must implement the following trait:

```rust
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

## Operations

### get

Retrieve a single document by ID from the specified collection. Returns `StorageError::NotFound` if the document does not exist.

### create

Insert a new document into the collection. The `Document` must have an `id` field (set by `StorageContext`). Returns the document ID on success. Returns `StorageError::AlreadyExists` if a document with the same ID already exists.

### update

Replace an entire document by ID. The adapter must overwrite all fields except system-managed base fields. Returns `StorageError::NotFound` if the document does not exist.

### partial_update

Merge a JSON patch (`serde_json::Value`) into an existing document. Only the fields present in the patch are modified; other fields remain unchanged. Returns `StorageError::NotFound` if the document does not exist.

### delete

Remove a single document by ID. Returns `StorageError::NotFound` if the document does not exist.

### list

Return documents matching the given `QueryDescriptor`. Results are returned as a `DocumentList` containing the matched documents and pagination metadata.

### count

Return the number of documents matching the given `QueryDescriptor`. Uses the same filter logic as `list` but returns only the count.

### batch_create

Insert multiple documents in a single call. Returns a vector of IDs for the created documents. If any document fails validation or conflicts, the entire batch must fail atomically — no partial writes.

### batch_update

Replace multiple documents in a single call. Each entry is a tuple of `(id, Document)`. Atomic — all succeed or all fail.

### batch_delete

Delete multiple documents by ID in a single call. Atomic — all succeed or all fail.

### transaction

Execute a closure within a transaction. The closure receives a `TransactionHandle` for performing operations within the transaction scope. If the closure returns `Ok`, the transaction commits. If it returns `Err` or panics, the transaction rolls back.

### watch

Return a stream of `ChangeEvent` values for the specified collection. If the adapter supports native change detection, this stream emits events as changes occur. If not, the stream may be empty (the write-path emission in `StorageContext` handles event delivery instead).

### migrate

Create or update the collection's storage structure based on the `CollectionDescriptor`. Must be idempotent — calling `migrate` multiple times with the same descriptor produces no additional effect. Additive changes (new fields, new indexes) are applied automatically. Breaking changes (removing fields, changing types) return `StorageError::SchemaConflict`.

### health

Return a `HealthReport` describing the adapter's current operational status.

### capabilities

Return the adapter's `AdapterCapabilities` struct describing which optional features are supported.

## QueryDescriptor

```rust
pub struct QueryDescriptor {
    pub filter: Option<FilterNode>,
    pub sort: Vec<SortField>,
    pub pagination: Pagination,
    pub fields: Option<Vec<String>>,
    pub text_search: Option<String>,
}
```

- **`filter`** — An optional tree of filter conditions
- **`sort`** — Ordered list of sort fields
- **`pagination`** — Limit and cursor for paginated results
- **`fields`** — Optional field projection. When set, only the listed fields are returned. When `None`, all fields are returned.
- **`text_search`** — Optional full-text search query string. Adapters without text search capability must return `StorageError::UnsupportedOperation`.

### FilterNode

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

- **`Eq`** — Field equals value
- **`Ne`** — Field does not equal value
- **`Gt`**, **`Gte`**, **`Lt`**, **`Lte`** — Numeric/date comparisons
- **`In`** — Field value is one of the provided array values
- **`NotIn`** — Field value is not in the provided array
- **`Contains`** — String field contains substring, or array field contains element
- **`StartsWith`** — String field starts with prefix
- **`Exists`** — Field exists (value is ignored)

### SortField

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

```rust
pub struct Pagination {
    pub limit: u32,
    pub cursor: Option<String>,
}
```

- **`limit`** — Maximum number of documents to return. Required.
- **`cursor`** — Opaque cursor string from a previous `DocumentList` response. When provided, results begin after the cursor position.

### DocumentList

```rust
pub struct DocumentList {
    pub documents: Vec<Document>,
    pub next_cursor: Option<String>,
    pub total_estimate: Option<u64>,
}
```

- **`documents`** — The matched documents for this page
- **`next_cursor`** — Cursor for the next page, or `None` if this is the last page
- **`total_estimate`** — Optional approximate total count. Adapters may omit this if counting is expensive.

## TransactionHandle

```rust
pub trait TransactionHandle: Send + Sync {
    fn get(&self, collection: &str, id: &str) -> Result<Document, StorageError>;
    fn create(&self, collection: &str, doc: Document) -> Result<String, StorageError>;
    fn update(&self, collection: &str, id: &str, doc: Document) -> Result<(), StorageError>;
    fn delete(&self, collection: &str, id: &str) -> Result<(), StorageError>;
}
```

`TransactionHandle` provides a subset of operations available within a transaction scope. All operations through the handle participate in the enclosing transaction. The handle is not exposed to plugins — only `StorageContext` and the workflow engine use transactions.

## ChangeEvent

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

`ChangeEvent` represents a single change detected by the adapter's watch mechanism. `StorageContext` translates these into event bus events. See [[storage-context]].

## CollectionDescriptor

```rust
pub struct CollectionDescriptor {
    pub name: String,
    pub fields: Vec<FieldDescriptor>,
    pub indexes: Vec<String>,
}
```

- **`name`** — The collection name
- **`fields`** — Field definitions from the schema, used to guide storage layout
- **`indexes`** — Field paths to index (hints from manifest)

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

- **`String`** — UTF-8 text
- **`Integer`** — 64-bit signed integer
- **`Float`** — 64-bit floating point
- **`Boolean`** — True or false
- **`DateTime`** — ISO 8601 timestamp
- **`Json`** — Arbitrary nested JSON (stored as-is)
- **`Array(inner)`** — Ordered list of elements of the inner type

## HealthReport

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

- **`status`** — Overall adapter health. `Healthy` means fully operational. `Degraded` means functional but with issues (e.g., slow responses). `Unhealthy` means unable to serve requests.
- **`checks`** — Individual check results. Each adapter defines its own checks (e.g., "connection", "disk_space", "encryption").
- **`message`** — Optional human-readable detail for a check.

## AdapterCapabilities

```rust
pub struct AdapterCapabilities {
    pub encryption: bool,
    pub indexing: bool,
    pub full_text_search: bool,
    pub watch: bool,
    pub transactions: bool,
}
```

Capability fallback behaviour:

- **`encryption`** — If `false` and encryption is required in `storage.toml`, the engine refuses to start. Application-level encryption in `StorageContext` operates independently.
- **`indexing`** — If `false`, index hints from manifests are silently ignored. Queries still work but may be slower.
- **`full_text_search`** — If `false`, queries with `text_search` set return `StorageError::UnsupportedOperation`.
- **`watch`** — If `false`, the `watch` method returns an empty stream. `StorageContext` emits events on the write path instead.
- **`transactions`** — If `false`, the `transaction` method returns `StorageError::UnsupportedOperation`. Batch operations must still be atomic via adapter-internal mechanisms.

## StorageError

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

### Workflow status mapping

When a storage error occurs during workflow execution, it maps to a workflow status:

- **`NotFound`** — Maps to workflow `Faulted` with a retryable flag of `false`
- **`AlreadyExists`** — Maps to workflow `Faulted` with a retryable flag of `false`
- **`ValidationFailed`** — Maps to workflow `Faulted` with a retryable flag of `false`
- **`CapabilityDenied`** — Maps to workflow `Faulted` with a retryable flag of `false`
- **`SchemaConflict`** — Maps to workflow `Faulted` with a retryable flag of `false`
- **`Timeout`** — Maps to workflow `Faulted` with a retryable flag of `true`
- **`ConnectionFailed`** — Maps to workflow `Faulted` with a retryable flag of `true`
- **`UnsupportedOperation`** — Maps to workflow `Faulted` with a retryable flag of `false`
- **`Internal`** — Maps to workflow `Faulted` with a retryable flag of `true`

See [[workflow-engine-contract]] for workflow status definitions.

## Migration Behaviour

The `migrate` method must be:

- **Idempotent** — Calling with the same `CollectionDescriptor` multiple times has no additional effect
- **Additive** — New fields and indexes are created without disrupting existing data
- **Safe** — Breaking changes (field removal, type changes) return `StorageError::SchemaConflict` instead of applying destructive changes

On first call for a collection, `migrate` creates the collection's storage structure. On subsequent calls, it reconciles the current state with the descriptor and applies only additive changes.
