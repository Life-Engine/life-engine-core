---
title: Host Functions Reference
type: adr
created: 2026-03-28
status: draft
tags:
  - architecture
  - plugin-sdk
  - host-functions
  - core
---

# Host Functions Reference

## Overview

Host functions are the capabilities that Core exports into each plugin's WASM sandbox. They are the only way a plugin can interact with the outside world — storage, events, configuration, and outbound HTTP.

Every host function requires a corresponding capability declaration in `manifest.toml`. Calling a host function without the required capability returns an error immediately.

The SDK provides typed wrappers around these raw imports. Plugin authors call SDK methods; the SDK handles serialisation, error mapping, and the raw WASM import calls.

## Document Storage

Requires: `storage_doc` capability with the appropriate operation (`read`, `write`, or `delete`).

All document operations are scoped to collections declared in the plugin's manifest. Attempting to access an undeclared collection returns `CapabilityDenied`.

### `storage_doc_get`

Retrieve a single document by collection and ID.

```rust
fn storage_doc_get(collection: &str, id: &str) -> Result<Document, StorageError>;
```

- Returns `StorageError::NotFound` if the document does not exist
- Returns `StorageError::CapabilityDenied` if the plugin lacks `read` access to the collection

SDK wrapper:

```rust
let contact = ctx.storage().doc("contacts").get("abc-123")?;
```

### `storage_doc_list`

Query documents in a collection. Accepts a JSON-encoded query descriptor.

```rust
fn storage_doc_list(collection: &str, query_json: &str) -> Result<DocumentList, StorageError>;
```

The query descriptor supports filters, sort, pagination, field projection, and text search. See [[architecture/core/design/data-layer/document-storage-trait#QueryDescriptor]] for the full specification.

SDK wrapper with fluent query builder:

```rust
let results = ctx.storage().doc("contacts")
    .filter(|f| f.field("address.city").eq("London"))
    .sort("updated_at", Desc)
    .limit(20)
    .exec()?;
```

The `DocumentList` return type:

```rust
pub struct DocumentList {
    pub items: Vec<Document>,
    pub total: Option<u64>,
    pub cursor: Option<String>,
}
```

### `storage_doc_count`

Count documents matching a query without retrieving them.

```rust
fn storage_doc_count(collection: &str, query_json: &str) -> Result<u64, StorageError>;
```

SDK wrapper:

```rust
let count = ctx.storage().doc("contacts")
    .filter(|f| f.field("address.city").eq("London"))
    .count()?;
```

### `storage_doc_create`

Create a new document in a collection. Returns the assigned ID.

```rust
fn storage_doc_create(collection: &str, doc_json: &str) -> Result<String, StorageError>;
```

- `id` is optional — `StorageContext` generates one if not provided
- `created_at` and `updated_at` are set by the system
- The document is validated against the collection's schema (if one exists)
- Returns `StorageError::AlreadyExists` if a document with the same ID exists

SDK wrapper:

```rust
let id = ctx.storage().doc("emails").create(json!({
    "subject": "Hello",
    "from": "alice@example.com",
    "body": "..."
}))?;
```

### `storage_doc_update`

Full replacement of a document by collection and ID.

```rust
fn storage_doc_update(collection: &str, id: &str, doc_json: &str) -> Result<(), StorageError>;
```

- Replaces the entire document body
- `updated_at` is set by the system
- Returns `StorageError::NotFound` if the document does not exist

### `storage_doc_partial_update`

Patch specific fields without replacing the entire document.

```rust
fn storage_doc_partial_update(collection: &str, id: &str, patch_json: &str) -> Result<(), StorageError>;
```

- Only the fields in the patch are modified
- `updated_at` is set by the system
- If the adapter supports native partial update, it is applied directly; otherwise `StorageContext` does a read-modify-write

SDK wrapper:

```rust
ctx.storage().doc("tasks").partial_update("task-456", json!({
    "status": "done",
    "completed_at": "2026-03-28T15:00:00Z"
}))?;
```

### `storage_doc_delete`

Remove a document by collection and ID.

```rust
fn storage_doc_delete(collection: &str, id: &str) -> Result<(), StorageError>;
```

Requires `delete` capability. Returns `StorageError::NotFound` if the document does not exist.

### `storage_doc_batch_create`

Insert multiple documents atomically. Returns assigned IDs in input order.

```rust
fn storage_doc_batch_create(collection: &str, docs_json: &str) -> Result<Vec<String>, StorageError>;
```

Wrapped in a transaction internally by `StorageContext`. Useful for sync connectors that pull many records at once.

### `storage_doc_batch_update`

Replace multiple documents atomically.

```rust
fn storage_doc_batch_update(collection: &str, updates_json: &str) -> Result<(), StorageError>;
```

Each entry is a `(id, document)` pair. All-or-nothing within a transaction.

### `storage_doc_batch_delete`

Remove multiple documents atomically.

```rust
fn storage_doc_batch_delete(collection: &str, ids_json: &str) -> Result<(), StorageError>;
```

## Blob Storage

Requires: `storage_blob` capability with the appropriate operation (`read`, `write`, or `delete`).

Blob keys are automatically scoped to the plugin's ID prefix. A plugin storing under key `attachments/invoice.pdf` actually writes to `{plugin_id}/attachments/invoice.pdf`. The SDK handles this transparently.

### `storage_blob_store`

Store binary content under a key.

```rust
fn storage_blob_store(key: &str, bytes: &[u8]) -> Result<(), StorageError>;
```

- Overwrites if a blob already exists at the key
- Metadata (filename, content type) is passed as part of the byte payload header
- `StorageContext` emits a `system.blob.stored` event

SDK wrapper:

```rust
ctx.storage().blob().store("attachments/invoice.pdf", &bytes, BlobInput {
    filename: "invoice.pdf".into(),
    content_type: "application/pdf".into(),
})?;
```

### `storage_blob_retrieve`

Retrieve binary content by key.

```rust
fn storage_blob_retrieve(key: &str) -> Result<Vec<u8>, StorageError>;
```

Returns the full content as bytes. For WASM plugins, streaming is not available — the entire blob is loaded into the plugin's linear memory. Large file handling should be done at the workflow level, not within a single plugin step.

### `storage_blob_delete`

Remove a blob by key.

```rust
fn storage_blob_delete(key: &str) -> Result<(), StorageError>;
```

### `storage_blob_exists`

Check whether a blob exists without retrieving it.

```rust
fn storage_blob_exists(key: &str) -> Result<bool, StorageError>;
```

### `storage_blob_list`

List blob metadata entries matching a key prefix.

```rust
fn storage_blob_list(prefix: &str) -> Result<Vec<BlobMeta>, StorageError>;
```

The prefix is relative to the plugin's namespace. Calling `list("attachments/")` lists all blobs under `{plugin_id}/attachments/`.

### `storage_blob_metadata`

Get metadata for a single blob without retrieving the content.

```rust
fn storage_blob_metadata(key: &str) -> Result<BlobMeta, StorageError>;
```

## Events

### `emit_event`

Publish an event to the event bus.

Requires: `events_emit` capability.

```rust
fn emit_event(name: &str, payload: Option<Value>) -> Result<(), PluginError>;
```

- The event name must be declared in the plugin's manifest under `[events.emit].names`
- The `source` is set automatically to the plugin's ID
- The `depth` counter is inherited from the current workflow's event context (for loop prevention)
- Emitting an undeclared event returns an error

SDK wrapper:

```rust
ctx.events().emit("connector-email.fetch.completed", Some(json!({
    "count": 15,
    "folder": "INBOX"
})))?;
```

## Configuration

### `config_read`

Read the plugin's runtime configuration.

Requires: `config_read` capability.

```rust
fn config_read() -> Result<Value, PluginError>;
```

Returns the JSON value from the user's Core config under `[plugins.{plugin_id}]`. The value has already been validated against the plugin's config schema at startup.

SDK wrapper:

```rust
let config: EmailConfig = ctx.config().read()?;
```

The SDK deserialises the JSON into a typed struct defined by the plugin.

## HTTP Outbound

### `http_request`

Make an outbound HTTP request.

Requires: `http_outbound` capability.

```rust
fn http_request(request_json: &str) -> Result<String, PluginError>;
```

The request and response are JSON-encoded:

```rust
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

pub struct HttpResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
}
```

SDK wrapper:

```rust
let response = ctx.http().get("https://api.example.com/data")
    .header("Authorization", &format!("Bearer {}", token))
    .send()?;
```

Outbound HTTP is subject to Extism's host-level network policies. Core can restrict allowed domains in its configuration.

## Functions Not Exposed to Plugins

The following operations are internal to Core and not available via host functions:

- `transaction` — Plugins do not get direct transaction access. Batch operations are transactional internally.
- `watch` — Change watching is handled by `StorageContext` and the event bus.
- `migrate` — Schema migration is Core's responsibility at startup.
- `health` — Adapter health checks are a system concern.
- `copy` (blob) — Blob copying is a system-level operation.

## Error Handling

All host functions return a `Result`. Errors are mapped to SDK-specific error types:

```rust
pub enum PluginError {
    Storage(StorageError),
    CapabilityDenied { capability: String },
    EventRejected { name: String, reason: String },
    ConfigNotAvailable,
    HttpFailed { status: u16, body: String },
    Serialisation(String),
}
```

Plugins should handle errors and decide whether to propagate them (causing the step to fail) or handle them gracefully (returning a valid `PipelineMessage` with warnings).
