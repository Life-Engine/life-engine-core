<!--
domain: storage-context
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Requirements Document — Storage Context

## Introduction

`StorageContext` is the single gateway between callers (plugins and the workflow engine) and the `StorageRouter`. It enforces capability-based permissions, validates schemas, scopes collections, manages system base fields, isolates extension namespaces, exposes host functions for WASM plugins, emits audit events, bridges adapter change notifications to the event bus, and encrypts credential fields. Every storage operation passes through `StorageContext` before reaching any adapter.

## Alignment with Product Vision

- **Principle of Least Privilege** — Deny-by-default capability checks ensure plugins can only perform operations they have explicitly declared
- **Defence in Depth** — Field-level credential encryption layers on top of any adapter-level encryption
- **Parse, Don't Validate** — JSON Schema validation at the write boundary ensures only valid data reaches adapters
- **The Pit of Success** — The fluent query builder and host functions make the correct access path the easiest path for plugin authors
- **Single Source of Truth** — System-managed base fields ensure consistent metadata across all documents

## Requirements

### Requirement 1 — Permission Enforcement

**User Story:** As a plugin author, I want the system to check my declared capabilities before any storage operation so that I receive clear errors when my manifest is missing a required permission.

#### Acceptance Criteria

- 1.1. WHEN a plugin calls a document read operation (`get`, `list`, `count`, `query`) THEN `StorageContext` SHALL verify the plugin has the `storage:doc:read` capability before forwarding the operation to the router.
- 1.2. WHEN a plugin calls a document write operation (`create`, `update`, `partial_update`, `batch_create`, `batch_update`) THEN `StorageContext` SHALL verify the plugin has the `storage:doc:write` capability before forwarding the operation to the router.
- 1.3. WHEN a plugin calls a document delete operation (`delete`, `batch_delete`) THEN `StorageContext` SHALL verify the plugin has the `storage:doc:delete` capability before forwarding the operation to the router.
- 1.4. WHEN a plugin calls a blob read operation (`retrieve`, `exists`, `list`, `metadata`) THEN `StorageContext` SHALL verify the plugin has the `storage:blob:read` capability before forwarding the operation to the router.
- 1.5. WHEN a plugin calls a blob write operation (`store`, `copy`) THEN `StorageContext` SHALL verify the plugin has the `storage:blob:write` capability before forwarding the operation to the router.
- 1.6. WHEN a plugin calls a blob delete operation (`delete`) THEN `StorageContext` SHALL verify the plugin has the `storage:blob:delete` capability before forwarding the operation to the router.
- 1.7. WHEN a capability check fails THEN `StorageContext` SHALL return `StorageError::CapabilityDenied` immediately without forwarding the operation to the router.
- 1.8. WHEN the workflow engine calls any storage operation using a system-level identity THEN `StorageContext` SHALL bypass plugin capability checks and forward the operation directly.

### Requirement 2 — Collection Scoping

**User Story:** As a plugin author, I want my storage access scoped to declared collections so that I cannot accidentally read or write to collections I have not registered for.

#### Acceptance Criteria

- 2.1. WHEN a plugin accesses a shared collection THEN `StorageContext` SHALL verify the plugin has declared that collection with a matching access level (`read`, `write`, or `read-write`) before forwarding the operation.
- 2.2. WHEN a plugin accesses a plugin-scoped collection THEN `StorageContext` SHALL namespace it as `{plugin_id}.collection_name` and verify the calling plugin owns that namespace.
- 2.3. WHEN a plugin attempts to access a collection it has not declared THEN `StorageContext` SHALL reject the request with an access denied error.
- 2.4. WHEN the workflow engine accesses any collection THEN `StorageContext` SHALL allow the operation without requiring a collection declaration.

### Requirement 3 — Schema Validation

**User Story:** As a plugin author, I want write payloads validated against a declared schema so that malformed data is rejected before reaching the adapter.

#### Acceptance Criteria

- 3.1. WHEN a write operation targets a collection with a declared JSON Schema THEN `StorageContext` SHALL validate the payload against that schema (JSON Schema draft 2020-12) before forwarding.
- 3.2. WHEN a write operation targets a collection without a declared schema THEN `StorageContext` SHALL skip validation and forward the operation.
- 3.3. WHEN schema validation fails THEN `StorageContext` SHALL reject the write and return an error identifying the specific field and constraint that failed.
- 3.4. WHEN a payload includes extra fields not defined in the schema THEN `StorageContext` SHALL accept and store them unless strict mode is enabled for that collection.
- 3.5. WHEN a read operation is performed THEN `StorageContext` SHALL never validate the returned data against any schema.

### Requirement 4 — System-Managed Base Fields

**User Story:** As a Core developer, I want system base fields managed transparently so that every document carries consistent metadata regardless of what the caller provides.

#### Acceptance Criteria

- 4.1. WHEN a document is created and the caller does not provide an `id` THEN `StorageContext` SHALL generate an `id` and attach it to the document.
- 4.2. WHEN a document is created and the caller provides an `id` THEN `StorageContext` SHALL use the caller-provided `id`.
- 4.3. WHEN a document is created THEN `StorageContext` SHALL set `created_at` to the current timestamp, overwriting any caller-provided value.
- 4.4. WHEN a document is created THEN `StorageContext` SHALL set `updated_at` to the current timestamp, overwriting any caller-provided value.
- 4.5. WHEN a document is updated THEN `StorageContext` SHALL set `updated_at` to the current timestamp, overwriting any caller-provided value.
- 4.6. WHEN any write operation is performed THEN `StorageContext` SHALL ensure the `id` field is immutable after creation — updates must not change the document's `id`.
- 4.7. WHEN any write operation is performed THEN `StorageContext` SHALL ensure the `created_at` field is immutable after creation — updates must not change the document's `created_at`.

### Requirement 5 — Extension Field Handling

**User Story:** As a plugin author, I want to store plugin-specific fields in a namespaced extension area so that my data coexists with other plugins without conflicts.

#### Acceptance Criteria

- 5.1. WHEN a plugin writes extension fields THEN `StorageContext` SHALL store them under `ext.{plugin_id}.{field_name}` within the document JSON.
- 5.2. WHEN a plugin writes extension fields THEN `StorageContext` SHALL only allow writes to the calling plugin's own `ext.{plugin_id}` namespace.
- 5.3. WHEN a plugin reads a document THEN `StorageContext` SHALL allow the plugin to read extension fields from any plugin's namespace.
- 5.4. WHEN a plugin attempts to modify another plugin's extension namespace THEN `StorageContext` SHALL reject the write with an access denied error.
- 5.5. WHEN a plugin declares an `extension_schema` in its manifest THEN `StorageContext` SHALL validate extension field writes against that schema.
- 5.6. WHEN a plugin declares `extension_indexes` in its manifest THEN `StorageContext` SHALL register those indexes with the adapter.

### Requirement 6 — Query Building

**User Story:** As a plugin author, I want a fluent query API so that I can build backend-agnostic queries without knowing the underlying storage engine.

#### Acceptance Criteria

- 6.1. WHEN a plugin builds a query using the fluent API THEN `StorageContext` SHALL produce a `QueryDescriptor` value containing the collection, filters, sort, limit, and cursor.
- 6.2. WHEN a plugin calls `.filter()` THEN `StorageContext` SHALL support field-level comparisons including equality, inequality, greater-than, less-than, and contains.
- 6.3. WHEN a plugin calls `.sort()` THEN `StorageContext` SHALL accept a field name and direction (ascending or descending).
- 6.4. WHEN a plugin calls `.limit()` THEN `StorageContext` SHALL cap the result set to the specified number of documents.
- 6.5. WHEN a plugin calls `.cursor()` THEN `StorageContext` SHALL support cursor-based pagination using the provided cursor value.
- 6.6. WHEN a plugin calls `.exec()` THEN `StorageContext` SHALL pass the `QueryDescriptor` to the adapter's `list` or `count` method.

### Requirement 7 — Host Functions

**User Story:** As a plugin author, I want host functions exposed to my WASM module so that I can perform storage operations from within the plugin runtime.

#### Acceptance Criteria

- 7.1. WHEN a WASM plugin calls `storage_doc_get` THEN the host SHALL retrieve a single document by ID through `StorageContext`.
- 7.2. WHEN a WASM plugin calls `storage_doc_list` THEN the host SHALL list documents matching the provided query through `StorageContext`.
- 7.3. WHEN a WASM plugin calls `storage_doc_count` THEN the host SHALL return the count of documents matching the provided query through `StorageContext`.
- 7.4. WHEN a WASM plugin calls `storage_doc_create` THEN the host SHALL create a new document through `StorageContext`.
- 7.5. WHEN a WASM plugin calls `storage_doc_update` THEN the host SHALL replace an entire document through `StorageContext`.
- 7.6. WHEN a WASM plugin calls `storage_doc_partial_update` THEN the host SHALL merge a partial patch into a document through `StorageContext`.
- 7.7. WHEN a WASM plugin calls `storage_doc_delete` THEN the host SHALL delete a single document through `StorageContext`.
- 7.8. WHEN a WASM plugin calls `storage_doc_batch_create` THEN the host SHALL create multiple documents in one call through `StorageContext`.
- 7.9. WHEN a WASM plugin calls `storage_doc_batch_update` THEN the host SHALL update multiple documents in one call through `StorageContext`.
- 7.10. WHEN a WASM plugin calls `storage_doc_batch_delete` THEN the host SHALL delete multiple documents in one call through `StorageContext`.
- 7.11. WHEN a WASM plugin calls `storage_blob_store` THEN the host SHALL store a blob through `StorageContext`.
- 7.12. WHEN a WASM plugin calls `storage_blob_retrieve` THEN the host SHALL retrieve a blob by key through `StorageContext`.
- 7.13. WHEN a WASM plugin calls `storage_blob_delete` THEN the host SHALL delete a blob by key through `StorageContext`.
- 7.14. WHEN a WASM plugin calls `storage_blob_exists` THEN the host SHALL check blob existence through `StorageContext`.
- 7.15. WHEN a WASM plugin calls `storage_blob_list` THEN the host SHALL list blobs by prefix through `StorageContext`.
- 7.16. WHEN a WASM plugin calls `storage_blob_metadata` THEN the host SHALL retrieve blob metadata through `StorageContext`.
- 7.17. WHEN a WASM plugin attempts to call `transaction`, `watch`, `migrate`, `health`, `copy`, or `query` adapter operations THEN the host SHALL not expose these functions.

### Requirement 8 — Audit Event Emission

**User Story:** As a Core developer, I want audit events emitted for every write operation so that security-sensitive changes are traceable and recoverable.

#### Acceptance Criteria

- 8.1. WHEN a document is created THEN `StorageContext` SHALL emit a `system.storage.created` event with payload `{ collection, id }` to the event bus.
- 8.2. WHEN a document is updated THEN `StorageContext` SHALL emit a `system.storage.updated` event with payload `{ collection, id, changed_fields }` to the event bus.
- 8.3. WHEN a document is deleted THEN `StorageContext` SHALL emit a `system.storage.deleted` event with payload `{ collection, id }` to the event bus.
- 8.4. WHEN a blob is stored THEN `StorageContext` SHALL emit a `system.blob.stored` event with payload `{ key }` to the event bus.
- 8.5. WHEN a blob is deleted THEN `StorageContext` SHALL emit a `system.blob.deleted` event with payload `{ key }` to the event bus.
- 8.6. WHEN any audit event is emitted THEN the event SHALL include the originating plugin ID, or `"system"` for workflow engine operations.
- 8.7. WHEN any audit event is emitted THEN the event SHALL not contain the full document payload.
- 8.8. WHEN a read operation is performed THEN `StorageContext` SHALL not emit any audit event.

### Requirement 9 — Watch-to-Event-Bus Bridge

**User Story:** As a Core developer, I want adapter-level change notifications bridged to the event bus so that downstream consumers receive real-time storage change events.

#### Acceptance Criteria

- 9.1. WHEN Core starts THEN `StorageContext` SHALL subscribe to the adapter's `watch` stream for each collection.
- 9.2. WHEN the adapter reports a `ChangeEvent` THEN `StorageContext` SHALL translate it to the corresponding `system.storage.*` event on the event bus.
- 9.3. WHEN the adapter reports native watch support via its capabilities THEN `StorageContext` SHALL use the adapter's watch stream directly.
- 9.4. WHEN the adapter does not support native watch THEN `StorageContext` SHALL emit events on the write path instead.
- 9.5. WHEN events are emitted on the write path THEN the watch stream SHALL not re-emit those same events, preventing duplicate events.

### Requirement 10 — Credential Encryption

**User Story:** As a user, I want my stored credentials encrypted at the field level so that sensitive data is protected independently of any adapter-level encryption.

#### Acceptance Criteria

- 10.1. WHEN a write operation targets the `credentials` collection THEN `StorageContext` SHALL encrypt sensitive fields individually with a derived key before passing data to the adapter.
- 10.2. WHEN a read operation targets the `credentials` collection THEN `StorageContext` SHALL decrypt the sensitive fields after retrieving data from the adapter.
- 10.3. WHEN credential encryption is applied THEN it SHALL operate independently of any adapter-level encryption (e.g., SQLCipher).
