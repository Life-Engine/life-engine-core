---
title: Storage Context Specification
type: reference
created: 2026-03-28
status: active
tags:
  - storage
  - api
  - permissions
  - spec
---

# Storage Context Specification

## Overview

`StorageContext` is the API surface between callers and `StorageRouter`. It enforces permissions, validates schemas, scopes collections, emits audit events, and bridges storage changes to the event bus. Plugins never interact with adapters directly — all storage access flows through `StorageContext`.

## Access Paths

There are two distinct access paths:

- **Plugin access (via host functions)** — Scoped to the calling plugin's identity. Permission checks are enforced against the plugin's declared capabilities before any operation reaches the adapter.
- **Workflow engine access (internal)** — Uses a system-level identity. Bypasses plugin-scoped permission checks. Schema validation still applies.

## Permission Enforcement

`StorageContext` checks capabilities before any operation reaches the adapter. Capabilities are per-collection and must be declared in the plugin's `manifest.toml`. A missing capability results in `StorageError::CapabilityDenied` immediately — the operation never reaches the router.

The required capabilities by operation:

- **storage:doc:read** — `get`, `list`, `count`, `query`
- **storage:doc:write** — `create`, `update`, `partial_update`, `batch_create`, `batch_update`
- **storage:doc:delete** — `delete`, `batch_delete`
- **storage:blob:read** — `retrieve`, `exists`, `list`, `metadata`
- **storage:blob:write** — `store`, `copy`
- **storage:blob:delete** — `delete`

## Collection Scoping

Collections are scoped in two ways:

- **Shared collections** — Declared with an access level (`read`, `write`, or `read-write`). Multiple plugins can share the same collection. Access level determines which operations are permitted.
- **Plugin-scoped collections** — Namespaced as `{plugin_id}.collection_name` to prevent collisions between plugins. Only the owning plugin can access these.

Plugins cannot query collections they have not declared. System-level callers (workflow engine) can access any collection without declaration.

## Schema Validation

Schema validation applies to write operations only (`create`, `update`, `partial_update`, and their batch variants):

- Collections with a declared schema are validated against JSON Schema (draft 2020-12)
- Collections without a declared schema skip validation entirely
- Permissive by default — extra fields are accepted and stored unless strict mode is enabled
- Read operations are never validated

See [[schema-and-validation]] for full validation rules.

## System-Managed Base Fields

Every document in every collection carries these fields, managed by `StorageContext`:

- **`id`** — Generated on create if not provided by the caller. Immutable after creation.
- **`created_at`** — Set on create. Immutable after creation.
- **`updated_at`** — Set on every write operation.

Callers must not set `created_at` or `updated_at` directly. `StorageContext` overwrites any caller-provided values for these fields.

## Extension Field Handling

Extension fields are stored under `ext.{plugin_id}.{field_name}` within the document JSON.

Rules:

- Each plugin can only read and write fields in its own `ext.{plugin_id}` namespace
- Other plugins can read extension fields but cannot modify them
- Extension fields can have schema validation if `extension_schema` is declared in the manifest
- Extension indexes are declared via `extension_indexes` in the manifest

See [[schema-and-validation]] for extension schema declaration.

## Query Building

`StorageContext` exposes a fluent API that produces a backend-agnostic `QueryDescriptor`:

```rust
ctx.collection("contacts")
   .filter(|f| f.field("address.city").eq("London"))
   .sort("updated_at", Desc)
   .limit(20)
   .cursor(prev_cursor)
   .exec()
```

The `QueryDescriptor` is passed to the adapter's `list` or `count` method. See [[document-storage-adapter]] for the full `QueryDescriptor` definition.

## Host Functions

These host functions are exposed to plugin WASM modules via the plugin runtime.

Document operations:

- `storage_doc_get` — Retrieve a single document by ID
- `storage_doc_list` — List documents matching a query
- `storage_doc_count` — Count documents matching a query
- `storage_doc_create` — Create a new document
- `storage_doc_update` — Replace an entire document
- `storage_doc_partial_update` — Merge a partial patch into a document
- `storage_doc_delete` — Delete a single document
- `storage_doc_batch_create` — Create multiple documents in one call
- `storage_doc_batch_update` — Update multiple documents in one call
- `storage_doc_batch_delete` — Delete multiple documents in one call

Blob operations:

- `storage_blob_store` — Store a blob
- `storage_blob_retrieve` — Retrieve a blob by key
- `storage_blob_delete` — Delete a blob by key
- `storage_blob_exists` — Check if a blob exists
- `storage_blob_list` — List blobs by prefix
- `storage_blob_metadata` — Retrieve metadata for a blob

The following adapter operations are not exposed to plugins: `transaction`, `watch`, `migrate`, `health`, `copy`, `query`.

## Audit Event Emission

`StorageContext` emits audit events for write operations only, via the event bus. Read operations are not audited.

Events emitted:

- **`system.storage.created`** — Payload: `{ collection, id }`
- **`system.storage.updated`** — Payload: `{ collection, id, changed_fields }`
- **`system.storage.deleted`** — Payload: `{ collection, id }`
- **`system.blob.stored`** — Payload: `{ key }`
- **`system.blob.deleted`** — Payload: `{ key }`

Every audit event includes the originating plugin ID or `"system"` for workflow engine operations. Audit events must not contain full document payloads.

See [[encryption-and-audit]] for audit log storage details.

## Watch-to-Event-Bus Bridge

`StorageContext` bridges adapter-level change notifications to the event bus:

1. At startup, subscribe to the adapter's `watch` stream for each collection
2. Translate backend `ChangeEvent` values to `system.storage.*` events on the event bus
3. If the adapter supports native watch (reported via capabilities), use the adapter's stream directly
4. If the adapter does not support native watch, emit events on the write path instead
5. The bridge must not produce duplicate events — if events are emitted on the write path, the watch stream must not re-emit them

See [[event-bus]] for event bus semantics.

## Credential Encryption

The `credentials` collection receives special treatment. Sensitive fields are encrypted individually with a derived key before reaching the adapter. This encryption is independent of any adapter-level encryption (e.g., SQLCipher).

See [[encryption-and-audit]] for key derivation and encryption details.
