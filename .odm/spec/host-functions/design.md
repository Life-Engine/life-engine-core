<!--
domain: host-functions
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Host Functions — Design

## Purpose

This document defines the technical design for all host functions that Core exports to WASM plugins via Extism. Host functions are registered during plugin loading and gated by the plugin's approved capabilities. They are the only mechanism through which plugins interact with storage, events, configuration, and the network.

All host function implementations live in `packages/plugin-system/src/host_functions/`. Each domain has its own module: `storage.rs`, `events.rs`, `config.rs`, `http.rs`, and `logging.rs`. The `mod.rs` file re-exports all host functions and provides the registration entry point.

## Host Function Registration

During plugin loading, Core reads the plugin's approved capabilities and registers only the host functions that match. This is handled in `packages/plugin-system/src/injection.rs`:

```rust
pub fn register_host_functions(
    builder: &mut PluginBuilder,
    plugin_id: &str,
    capabilities: &CapabilitySet,
    manifest: &PluginManifest,
    ctx: HostContext,
) {
    if capabilities.has("storage:doc:read") {
        builder.host_fn("storage_doc_get", make_doc_get(ctx.clone(), plugin_id, manifest));
        builder.host_fn("storage_doc_list", make_doc_list(ctx.clone(), plugin_id, manifest));
        builder.host_fn("storage_doc_count", make_doc_count(ctx.clone(), plugin_id, manifest));
    }
    if capabilities.has("storage:doc:write") {
        builder.host_fn("storage_doc_create", make_doc_create(ctx.clone(), plugin_id, manifest));
        builder.host_fn("storage_doc_update", make_doc_update(ctx.clone(), plugin_id, manifest));
        builder.host_fn("storage_doc_partial_update", make_doc_partial_update(ctx.clone(), plugin_id, manifest));
        builder.host_fn("storage_doc_batch_create", make_doc_batch_create(ctx.clone(), plugin_id, manifest));
        builder.host_fn("storage_doc_batch_update", make_doc_batch_update(ctx.clone(), plugin_id, manifest));
    }
    if capabilities.has("storage:doc:delete") {
        builder.host_fn("storage_doc_delete", make_doc_delete(ctx.clone(), plugin_id, manifest));
        builder.host_fn("storage_doc_batch_delete", make_doc_batch_delete(ctx.clone(), plugin_id, manifest));
    }
    if capabilities.has("storage:blob:read") {
        builder.host_fn("storage_blob_retrieve", make_blob_retrieve(ctx.clone(), plugin_id));
        builder.host_fn("storage_blob_exists", make_blob_exists(ctx.clone(), plugin_id));
        builder.host_fn("storage_blob_list", make_blob_list(ctx.clone(), plugin_id));
        builder.host_fn("storage_blob_metadata", make_blob_metadata(ctx.clone(), plugin_id));
    }
    if capabilities.has("storage:blob:write") {
        builder.host_fn("storage_blob_store", make_blob_store(ctx.clone(), plugin_id));
    }
    if capabilities.has("storage:blob:delete") {
        builder.host_fn("storage_blob_delete", make_blob_delete(ctx.clone(), plugin_id));
    }
    if capabilities.has("events:emit") {
        builder.host_fn("emit_event", make_emit_event(ctx.clone(), plugin_id, manifest));
    }
    if capabilities.has("config:read") {
        builder.host_fn("config_read", make_config_read(ctx.clone(), plugin_id));
    }
    if capabilities.has("http:outbound") {
        builder.host_fn("http_request", make_http_request(ctx.clone(), plugin_id));
    }
}
```

Functions not matching any approved capability are never registered, so the WASM runtime has no way to call them.

## HostContext

All host functions receive a shared `HostContext` that provides access to Core services:

```rust
#[derive(Clone)]
pub struct HostContext {
    pub storage: Arc<dyn StorageBackend>,
    pub event_bus: Arc<EventBus>,
    pub config_store: Arc<ConfigStore>,
    pub http_client: Arc<HttpClient>,
}
```

Each `make_*` factory function closes over the `HostContext`, the calling `plugin_id`, and (where needed) the `PluginManifest`, producing a closure that Extism can call.

## PluginError Type

All host functions return `Result<T, PluginError>`. The error type is defined in `packages/plugin-system/src/error.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginError {
    CapabilityDenied { capability: String, detail: String },
    NotFound { resource: String, id: String },
    ValidationError { message: String, field: Option<String> },
    StorageError { message: String },
    NetworkError { message: String },
    InternalError { message: String },
}
```

When returned across the WASM boundary, `PluginError` is serialised as JSON. The plugin SDK (`packages/plugin-sdk-rs`) deserialises it into a typed Rust enum that actions can match on.

## Document Storage Design

### Collection Scoping

Every document storage host function validates that the requested collection appears in the plugin's manifest `[collections]` section before proceeding. The validation is a simple set lookup:

```rust
fn validate_collection(manifest: &PluginManifest, collection: &str) -> Result<(), PluginError> {
    if !manifest.collections.contains(collection) {
        return Err(PluginError::CapabilityDenied {
            capability: "storage:doc".into(),
            detail: format!("collection '{}' not declared in manifest", collection),
        });
    }
    Ok(())
}
```

### Function Signatures

Document storage functions and their required capabilities:

Read operations (require `storage:doc:read`):

- `storage_doc_get(collection: &str, id: &str) -> Result<Document>` — Retrieve a single document by ID
- `storage_doc_list(collection: &str, query_json: &str) -> Result<DocumentList>` — Query documents with filters, sorting, and pagination
- `storage_doc_count(collection: &str, query_json: &str) -> Result<u64>` — Count documents matching a query

Write operations (require `storage:doc:write`):

- `storage_doc_create(collection: &str, doc_json: &str) -> Result<String>` — Create a document, return assigned ID
- `storage_doc_update(collection: &str, id: &str, doc_json: &str) -> Result<()>` — Full document replacement
- `storage_doc_partial_update(collection: &str, id: &str, patch_json: &str) -> Result<()>` — Merge patch fields into existing document
- `storage_doc_batch_create(collection: &str, docs_json: &str) -> Result<Vec<String>>` — Create multiple documents, return IDs in order
- `storage_doc_batch_update(collection: &str, updates_json: &str) -> Result<()>` — Update multiple documents from `[{ "id": "...", "doc": {...} }]`

Delete operations (require `storage:doc:delete`):

- `storage_doc_delete(collection: &str, id: &str) -> Result<()>` — Delete a single document
- `storage_doc_batch_delete(collection: &str, ids_json: &str) -> Result<()>` — Delete multiple documents from `["id1", "id2", ...]`

### Query JSON Format

The `query_json` parameter accepted by `storage_doc_list` and `storage_doc_count` supports the following structure:

```json
{
  "filters": [
    { "field": "status", "op": "eq", "value": "active" },
    { "field": "created_at", "op": "gte", "value": "2026-01-01T00:00:00Z" }
  ],
  "sort": [
    { "field": "created_at", "direction": "desc" }
  ],
  "limit": 50,
  "offset": 0
}
```

The `StorageBackend` translates this into native queries for the underlying engine.

## Blob Storage Design

### Key Prefixing

All blob keys are automatically prefixed with the calling plugin's ID to enforce namespace isolation:

```rust
fn scoped_key(plugin_id: &str, key: &str) -> String {
    format!("{}/{}", plugin_id, key)
}
```

This happens transparently inside each blob host function. The plugin never sees the prefix.

### BlobMeta Type

The `storage_blob_list` and `storage_blob_metadata` functions return blob metadata:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobMeta {
    pub key: String,
    pub size: u64,
    pub content_type: Option<String>,
    pub created_at: String,  // RFC 3339
}
```

The `key` field in `BlobMeta` returns the unprefixed key (the plugin's view), not the internal prefixed key.

### Function Signatures

Read operations (require `storage:blob:read`):

- `storage_blob_retrieve(key: &str) -> Result<Vec<u8>>` — Retrieve blob bytes
- `storage_blob_exists(key: &str) -> Result<bool>` — Check blob existence
- `storage_blob_list(prefix: &str) -> Result<Vec<BlobMeta>>` — List blobs matching prefix
- `storage_blob_metadata(key: &str) -> Result<BlobMeta>` — Retrieve blob metadata

Write operations (require `storage:blob:write`):

- `storage_blob_store(key: &str, bytes: &[u8]) -> Result<()>` — Store blob, overwrite if exists

Delete operations (require `storage:blob:delete`):

- `storage_blob_delete(key: &str) -> Result<()>` — Delete blob by key

## Event Emission Design

The `emit_event` host function validates the event name against the plugin's manifest `[events.emit]` section, then publishes to the event bus:

```rust
fn emit_event_impl(
    plugin_id: &str,
    manifest: &PluginManifest,
    event_bus: &EventBus,
    name: &str,
    payload: Option<Value>,
    depth: u32,
) -> Result<(), PluginError> {
    if !manifest.events_emit.contains(name) {
        return Err(PluginError::CapabilityDenied {
            capability: "events:emit".into(),
            detail: format!("event '{}' not declared in manifest [events.emit]", name),
        });
    }
    let event = Event {
        name: name.to_string(),
        source: plugin_id.to_string(),
        payload,
        depth,
        timestamp: Utc::now(),
    };
    event_bus.publish(event)?;
    Ok(())
}
```

Core automatically sets `source` to the calling plugin's ID and `depth` from the current execution context. The plugin cannot override these fields.

## Configuration Design

The `config_read` host function returns the plugin's runtime configuration as a JSON value. Configuration is validated against the manifest's `[config]` schema at load time, so the value is guaranteed to conform to the declared schema:

```rust
fn config_read_impl(
    plugin_id: &str,
    config_store: &ConfigStore,
) -> Result<Value, PluginError> {
    config_store
        .get_plugin_config(plugin_id)
        .map_err(|e| PluginError::InternalError { message: e.to_string() })
}
```

## HTTP Outbound Design

The `http_request` host function accepts a JSON request object and returns a JSON response:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct HttpRequestPayload {
    pub method: String,   // GET, POST, PUT, DELETE, PATCH
    pub url: String,      // Full URL including scheme
    pub headers: Option<HashMap<String, String>>,
    pub body: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HttpResponsePayload {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
}
```

The host function deserialises the request JSON, executes the HTTP call via the shared `HttpClient`, and returns the response serialised as JSON. Failures (timeout, DNS, connection refused) return `NetworkError`.

## Excluded Functions

The following Core internals are never registered as host functions:

- `transaction` — Internal transaction management
- `watch` — Change-stream subscriptions
- `migrate` — Schema migration operations
- `health` — System health checks
- `copy` — Internal document copy operations

These are reserved for Core-internal use. The `register_host_functions` function simply does not register them, so the WASM runtime has no binding for these names.

## File Layout

All host function implementation files live under `packages/plugin-system/src/host_functions/`:

- `mod.rs` — Re-exports and registration entry point
- `storage.rs` — Document storage and blob storage host functions
- `events.rs` — Event emission host function
- `config.rs` — Configuration reading host function
- `http.rs` — HTTP outbound host function
- `logging.rs` — Logging host function (supplementary, not specified in this domain)

Supporting files:

- `packages/plugin-system/src/error.rs` — `PluginError` enum definition
- `packages/plugin-system/src/injection.rs` — Host function registration logic
- `packages/plugin-system/src/capability.rs` — Capability types and checking
