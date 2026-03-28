---
title: Host Functions Specification
type: reference
created: 2026-03-28
status: active
tags:
  - spec
  - plugin
  - host-functions
  - wasm
---

# Host Functions Specification

This document is the complete reference for all host functions that Core exports to WASM plugins via Extism. Each host function requires a corresponding capability declaration in the plugin's [[plugin-manifest|manifest]]. Calling a host function without the required capability returns a `CapabilityDenied` error.

## Document Storage

All document storage functions are scoped to collections declared in the plugin's manifest. Accessing an undeclared collection returns `CapabilityDenied`.

### Read Operations

Require `storage:doc:read`.

- `storage_doc_get(collection: &str, id: &str) -> Result<Document>`
  Retrieve a single document by ID. Returns `NotFound` if the document does not exist.

- `storage_doc_list(collection: &str, query_json: &str) -> Result<DocumentList>`
  Query documents in a collection. The `query_json` parameter is a JSON-encoded query object supporting filters, sorting, and pagination.

- `storage_doc_count(collection: &str, query_json: &str) -> Result<u64>`
  Count documents matching a query. Accepts the same query format as `storage_doc_list`.

### Write Operations

Require `storage:doc:write`.

- `storage_doc_create(collection: &str, doc_json: &str) -> Result<String>`
  Create a new document. Returns the assigned document ID.

- `storage_doc_update(collection: &str, id: &str, doc_json: &str) -> Result<()>`
  Replace a document entirely. Returns `NotFound` if the document does not exist.

- `storage_doc_partial_update(collection: &str, id: &str, patch_json: &str) -> Result<()>`
  Apply a partial update to an existing document. The `patch_json` is a JSON object whose fields are merged into the existing document.

- `storage_doc_batch_create(collection: &str, docs_json: &str) -> Result<Vec<String>>`
  Create multiple documents in a single call. Returns a list of assigned IDs in the same order as the input.

- `storage_doc_batch_update(collection: &str, updates_json: &str) -> Result<()>`
  Update multiple documents in a single call. The `updates_json` is a JSON array of `{ "id": "...", "doc": {...} }` objects.

### Delete Operations

Require `storage:doc:delete`.

- `storage_doc_delete(collection: &str, id: &str) -> Result<()>`
  Delete a single document by ID. Returns `NotFound` if the document does not exist.

- `storage_doc_batch_delete(collection: &str, ids_json: &str) -> Result<()>`
  Delete multiple documents by ID. The `ids_json` is a JSON array of document ID strings.

## Blob Storage

Blob keys are automatically prefixed with the calling plugin's ID. A plugin can only access its own blobs.

### Read Operations

Require `storage:blob:read`.

- `storage_blob_retrieve(key: &str) -> Result<Vec<u8>>`
  Retrieve a blob by key. Returns `NotFound` if the key does not exist.

- `storage_blob_exists(key: &str) -> Result<bool>`
  Check whether a blob exists without retrieving its contents.

- `storage_blob_list(prefix: &str) -> Result<Vec<BlobMeta>>`
  List blobs whose keys match the given prefix. Returns metadata only, not blob contents.

- `storage_blob_metadata(key: &str) -> Result<BlobMeta>`
  Retrieve metadata for a single blob (size, content type, created timestamp).

### Write Operations

Require `storage:blob:write`.

- `storage_blob_store(key: &str, bytes: &[u8]) -> Result<()>`
  Store a blob. Overwrites if the key already exists.

### Delete Operations

Require `storage:blob:delete`.

- `storage_blob_delete(key: &str) -> Result<()>`
  Delete a blob by key. Returns `NotFound` if the key does not exist.

## Events

Require `events:emit`.

- `emit_event(name: &str, payload: Option<Value>) -> Result<()>`
  Emit an event to the [[event-bus]]. The event name must be declared in the plugin's manifest `[events.emit]` section. Emitting an undeclared event returns `CapabilityDenied`. Core automatically sets the event's `source` to the calling plugin's ID and `depth` from the current execution context.

## Configuration

Require `config:read`.

- `config_read() -> Result<Value>`
  Return the plugin's runtime configuration as a JSON value. The configuration is validated against the schema declared in the manifest's `[config]` section at load time.

## HTTP Outbound

Require `http:outbound`.

- `http_request(request_json: &str) -> Result<String>`
  Make an outbound HTTP request. The `request_json` parameter is a JSON object with the following fields:

  - **method** — HTTP method (`GET`, `POST`, `PUT`, `DELETE`, `PATCH`)
  - **url** — Full URL including scheme
  - **headers** — Optional object of header name-value pairs
  - **body** — Optional request body as a string

  Returns a JSON-encoded response object containing `status`, `headers`, and `body`.

## Functions Not Exposed to Plugins

The following Core internals are not available as host functions:

- `transaction` — Internal transaction management
- `watch` — Change-stream subscriptions
- `migrate` — Schema migration operations
- `health` — System health checks
- `copy` — Internal document copy operations

These are reserved for Core's internal use and are never callable from WASM.

## Error Handling

All host functions return `Result<T, PluginError>` on failure. Error types include:

- **CapabilityDenied** — The plugin lacks the required capability or tried to access an undeclared collection.
- **NotFound** — The requested document or blob does not exist.
- **ValidationError** — The provided data failed schema validation.
- **StorageError** — An underlying storage operation failed.
- **NetworkError** — An outbound HTTP request failed (timeout, DNS failure, connection refused).
- **InternalError** — An unexpected error within the host function implementation.

The plugin SDK surfaces these as typed errors that actions can match on and handle appropriately.
