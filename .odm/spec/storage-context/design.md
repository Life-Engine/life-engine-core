<!--
domain: storage-context
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Storage Context Design

## Purpose

This document describes the technical design for `StorageContext`, the single gateway between callers (plugins and the workflow engine) and the `StorageRouter`. It covers the struct layout, permission enforcement flow, collection scoping, schema validation, base field management, extension field isolation, the fluent query builder, host function signatures, audit event emission, the watch-to-event-bus bridge, and credential encryption.

## Caller Identity

Every `StorageContext` instance is constructed with a `CallerIdentity` that determines the access path:

```rust
pub enum CallerIdentity {
    Plugin {
        plugin_id: String,
        capabilities: Vec<String>,
        declared_collections: Vec<CollectionDeclaration>,
    },
    System,
}
```

- **`Plugin`** — Created by the plugin runtime when a WASM module invokes a host function. Capabilities and declared collections are read from the plugin's `manifest.toml`.
- **`System`** — Used by the workflow engine for internal orchestration. Bypasses plugin capability checks.

## StorageContext Struct

```rust
pub struct StorageContext {
    identity: CallerIdentity,
    router: Arc<StorageRouter>,
    schema_registry: Arc<SchemaRegistry>,
    event_bus: Arc<EventBus>,
    crypto: Arc<CryptoService>,
}
```

- **`identity`** — The caller's identity, determining permission scope
- **`router`** — The `StorageRouter` that dispatches operations to the appropriate adapter
- **`schema_registry`** — Provides JSON Schema lookup by collection name
- **`event_bus`** — The in-memory event bus for audit and change events
- **`crypto`** — Cryptographic service for credential field encryption

## Permission Enforcement Flow

Permission checks run before any operation reaches the router. The flow is:

1. Extract the operation type (doc read, doc write, doc delete, blob read, blob write, blob delete)
2. If `CallerIdentity::System`, skip to step 4
3. Check that the caller's capabilities list includes the required capability string. If missing, return `StorageError::CapabilityDenied`
4. Proceed to collection scoping

The capability strings map to operations as follows:

- **`storage:doc:read`** — `get`, `list`, `count`, `query`
- **`storage:doc:write`** — `create`, `update`, `partial_update`, `batch_create`, `batch_update`
- **`storage:doc:delete`** — `delete`, `batch_delete`
- **`storage:blob:read`** — `retrieve`, `exists`, `list`, `metadata`
- **`storage:blob:write`** — `store`, `copy`
- **`storage:blob:delete`** — `delete`

```rust
fn check_capability(&self, required: &str) -> Result<(), StorageError> {
    match &self.identity {
        CallerIdentity::System => Ok(()),
        CallerIdentity::Plugin { capabilities, .. } => {
            if capabilities.contains(&required.to_string()) {
                Ok(())
            } else {
                Err(StorageError::CapabilityDenied {
                    required: required.to_string(),
                })
            }
        }
    }
}
```

## Collection Scoping

After permission checks pass, `StorageContext` validates that the caller has access to the target collection:

- **Shared collections** — The caller's `declared_collections` must include an entry for this collection with a matching access level. A `read` declaration allows read operations only. A `write` declaration allows write and delete operations. A `read-write` declaration allows all operations.
- **Plugin-scoped collections** — The collection name must be prefixed with `{plugin_id}.`. Only the owning plugin can access these. The prefix is validated against the caller's `plugin_id`.
- **System callers** — All collections are accessible without declaration.

```rust
pub struct CollectionDeclaration {
    pub name: String,
    pub access: AccessLevel,
}

pub enum AccessLevel {
    Read,
    Write,
    ReadWrite,
}
```

## Schema Validation

Schema validation applies only to write operations (`create`, `update`, `partial_update`, and their batch variants):

1. Look up the collection's JSON Schema in the `SchemaRegistry`
2. If no schema is registered, skip validation
3. Validate the payload against the schema (JSON Schema draft 2020-12)
4. If validation fails, return `StorageError::ValidationFailed` with field-level details
5. Extra fields not in the schema are accepted by default (permissive mode). Strict mode rejects extra fields when enabled for the collection.

```rust
fn validate_write(&self, collection: &str, payload: &serde_json::Value) -> Result<(), StorageError> {
    if let Some(schema) = self.schema_registry.get(collection) {
        let result = schema.validate(payload);
        if !result.is_valid() {
            return Err(StorageError::ValidationFailed {
                errors: result.errors().collect(),
            });
        }
    }
    Ok(())
}
```

## System-Managed Base Fields

Every document carries three system-managed fields. `StorageContext` injects or overwrites these before the operation reaches the router:

- **`id`** — On create: generated (UUIDv7) if not provided by the caller, preserved if provided. On update: immutable, rejected if the caller attempts to change it.
- **`created_at`** — On create: set to current UTC timestamp (RFC 3339), overwriting any caller-provided value. On update: immutable, stripped from the update payload.
- **`updated_at`** — On every write: set to current UTC timestamp (RFC 3339), overwriting any caller-provided value.

```rust
fn inject_base_fields(&self, payload: &mut serde_json::Value, is_create: bool) {
    let now = Utc::now().to_rfc3339();

    if is_create {
        if payload.get("id").is_none() {
            payload["id"] = serde_json::Value::String(uuid7().to_string());
        }
        payload["created_at"] = serde_json::Value::String(now.clone());
    }

    payload["updated_at"] = serde_json::Value::String(now);
}
```

## Extension Field Handling

Extension fields live under `ext.{plugin_id}.{field_name}` in the document JSON. `StorageContext` enforces namespace isolation:

- On write: strip any `ext.*` keys that do not belong to the calling plugin. Merge the caller's `ext.{plugin_id}` namespace with the existing document's extension data (preserving other plugins' namespaces).
- On read: return the full `ext` object including all plugin namespaces. All plugins can read all extension namespaces.

If the plugin declares an `extension_schema` in its manifest, `StorageContext` validates the plugin's extension fields against that schema before writing. If the plugin declares `extension_indexes`, `StorageContext` registers those indexes with the adapter at plugin load time.

```rust
fn enforce_extension_namespace(
    &self,
    payload: &mut serde_json::Value,
    plugin_id: &str,
) -> Result<(), StorageError> {
    if let Some(ext) = payload.get_mut("ext") {
        if let Some(obj) = ext.as_object_mut() {
            let foreign_keys: Vec<String> = obj
                .keys()
                .filter(|k| *k != plugin_id)
                .cloned()
                .collect();
            for key in foreign_keys {
                return Err(StorageError::ExtensionNamespaceDenied {
                    attempted: key,
                    owner: plugin_id.to_string(),
                });
            }
        }
    }
    Ok(())
}
```

## Query Builder

The fluent query builder produces a `QueryDescriptor` that is backend-agnostic:

```rust
pub struct QueryDescriptor {
    pub collection: String,
    pub filters: Vec<Filter>,
    pub sort: Option<Sort>,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

pub struct Filter {
    pub field: String,
    pub op: FilterOp,
    pub value: serde_json::Value,
}

pub enum FilterOp {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
    Contains,
}

pub struct Sort {
    pub field: String,
    pub direction: SortDirection,
}

pub enum SortDirection {
    Asc,
    Desc,
}
```

Usage from plugin code:

```rust
ctx.collection("contacts")
   .filter(|f| f.field("address.city").eq("London"))
   .sort("updated_at", Desc)
   .limit(20)
   .cursor(prev_cursor)
   .exec()
```

The `.exec()` call triggers the full `StorageContext` pipeline: permission check, collection scope check, and then delegation to the adapter's `list` or `count` method via the router.

## Host Functions

These functions are registered with the WASM plugin runtime. Each function deserializes arguments from the WASM module, constructs the appropriate `StorageContext` call, and serializes the result back.

Document host functions:

- `storage_doc_get(collection: &str, id: &str) -> Result<Document>`
- `storage_doc_list(query_json: &str) -> Result<Vec<Document>>`
- `storage_doc_count(query_json: &str) -> Result<u64>`
- `storage_doc_create(collection: &str, payload_json: &str) -> Result<Document>`
- `storage_doc_update(collection: &str, id: &str, payload_json: &str) -> Result<Document>`
- `storage_doc_partial_update(collection: &str, id: &str, patch_json: &str) -> Result<Document>`
- `storage_doc_delete(collection: &str, id: &str) -> Result<()>`
- `storage_doc_batch_create(collection: &str, payloads_json: &str) -> Result<Vec<Document>>`
- `storage_doc_batch_update(collection: &str, updates_json: &str) -> Result<Vec<Document>>`
- `storage_doc_batch_delete(collection: &str, ids_json: &str) -> Result<()>`

Blob host functions:

- `storage_blob_store(key: &str, data: &[u8], metadata_json: &str) -> Result<()>`
- `storage_blob_retrieve(key: &str) -> Result<Vec<u8>>`
- `storage_blob_delete(key: &str) -> Result<()>`
- `storage_blob_exists(key: &str) -> Result<bool>`
- `storage_blob_list(prefix: &str) -> Result<Vec<BlobEntry>>`
- `storage_blob_metadata(key: &str) -> Result<BlobMetadata>`

The following adapter operations are intentionally not exposed: `transaction`, `watch`, `migrate`, `health`, `copy`, `query`.

## Audit Event Emission

Write operations trigger audit events after the adapter confirms success. Events are emitted to the event bus asynchronously (fire-and-forget, not blocking the write response).

Event types and payloads:

- **`system.storage.created`** — `{ collection, id }`
- **`system.storage.updated`** — `{ collection, id, changed_fields }`
- **`system.storage.deleted`** — `{ collection, id }`
- **`system.blob.stored`** — `{ key }`
- **`system.blob.deleted`** — `{ key }`

Every event includes an `origin` field set to the caller's `plugin_id` or `"system"` for workflow engine operations. Audit events never contain full document payloads. Read operations produce no audit events.

```rust
fn emit_audit(&self, event_type: &str, payload: serde_json::Value) {
    let origin = match &self.identity {
        CallerIdentity::Plugin { plugin_id, .. } => plugin_id.clone(),
        CallerIdentity::System => "system".to_string(),
    };

    self.event_bus.emit(AuditEvent {
        event_type: event_type.to_string(),
        origin,
        payload,
        timestamp: Utc::now(),
    });
}
```

## Watch-to-Event-Bus Bridge

The bridge translates adapter-level change notifications into event bus events:

1. At startup, query the adapter's capabilities for native watch support
2. If native watch is supported, subscribe to the adapter's `watch` stream for each collection and translate `ChangeEvent` values to `system.storage.*` events
3. If native watch is not supported, emit events on the write path (inside the write methods) instead
4. The bridge must not produce duplicates. If write-path emission is active, the watch stream subscription is skipped entirely for that adapter

```rust
async fn start_watch_bridge(&self) {
    if self.router.adapter_supports_watch() {
        let mut stream = self.router.watch_all().await;
        while let Some(change) = stream.next().await {
            let event = translate_change_event(change);
            self.event_bus.emit(event);
        }
    }
    // If no native watch, events are emitted in the write path methods
}
```

## Credential Encryption

The `credentials` collection receives special treatment. Before a write reaches the adapter, `StorageContext` identifies sensitive fields and encrypts each one individually using a derived key from `CryptoService`. On read, the fields are decrypted after retrieval.

This encryption is independent of any adapter-level encryption (e.g., SQLCipher). The key derivation and encryption algorithm details are defined in the encryption-and-audit spec.

```rust
fn encrypt_credential_fields(
    &self,
    payload: &mut serde_json::Value,
) -> Result<(), StorageError> {
    let sensitive_fields = ["password", "token", "secret", "api_key", "private_key"];
    if let Some(obj) = payload.as_object_mut() {
        for field in &sensitive_fields {
            if let Some(value) = obj.get(*field) {
                let encrypted = self.crypto.encrypt_field(value)?;
                obj.insert(field.to_string(), serde_json::Value::String(encrypted));
            }
        }
    }
    Ok(())
}
```

## Error Types

`StorageContext` defines a dedicated error enum for all failure modes:

```rust
pub enum StorageError {
    CapabilityDenied { required: String },
    CollectionAccessDenied { collection: String },
    ExtensionNamespaceDenied { attempted: String, owner: String },
    ValidationFailed { errors: Vec<ValidationError> },
    DocumentNotFound { collection: String, id: String },
    ImmutableFieldViolation { field: String },
    AdapterError(Box<dyn std::error::Error + Send + Sync>),
    EncryptionError(String),
}
```
