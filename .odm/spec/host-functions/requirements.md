<!--
domain: host-functions
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Host Functions — Requirements

## Requirement 1 — Document Storage Read Operations

**User Story:** As a plugin author, I want to retrieve documents from my declared collections so that I can read persisted data in my actions.

#### Acceptance Criteria

- 1.1. WHEN a plugin calls `storage_doc_get` with a valid collection and document ID, THEN the host SHALL return the matching document.
- 1.2. WHEN a plugin calls `storage_doc_get` with a document ID that does not exist, THEN the host SHALL return a `NotFound` error.
- 1.3. WHEN a plugin calls `storage_doc_list` with a collection and query JSON, THEN the host SHALL return a list of documents matching the filters, sorting, and pagination specified in the query.
- 1.4. WHEN a plugin calls `storage_doc_count` with a collection and query JSON, THEN the host SHALL return the count of documents matching the query.
- 1.5. WHEN a plugin calls any document read function on a collection not declared in its manifest, THEN the host SHALL return a `CapabilityDenied` error.
- 1.6. WHEN a plugin calls any document read function without the `storage:doc:read` capability, THEN the host SHALL return a `CapabilityDenied` error.

## Requirement 2 — Document Storage Write Operations

**User Story:** As a plugin author, I want to create and update documents in my declared collections so that I can persist and modify structured data.

#### Acceptance Criteria

- 2.1. WHEN a plugin calls `storage_doc_create` with a valid collection and document JSON, THEN the host SHALL create the document and return the assigned ID.
- 2.2. WHEN a plugin calls `storage_doc_update` with a valid collection, document ID, and document JSON, THEN the host SHALL replace the document entirely.
- 2.3. WHEN a plugin calls `storage_doc_update` with a document ID that does not exist, THEN the host SHALL return a `NotFound` error.
- 2.4. WHEN a plugin calls `storage_doc_partial_update` with a valid collection, document ID, and patch JSON, THEN the host SHALL merge the patch fields into the existing document.
- 2.5. WHEN a plugin calls `storage_doc_batch_create` with a collection and array of documents, THEN the host SHALL create all documents and return a list of assigned IDs in input order.
- 2.6. WHEN a plugin calls `storage_doc_batch_update` with a collection and array of update objects, THEN the host SHALL update all specified documents.
- 2.7. WHEN a plugin calls any document write function on a collection not declared in its manifest, THEN the host SHALL return a `CapabilityDenied` error.
- 2.8. WHEN a plugin calls any document write function without the `storage:doc:write` capability, THEN the host SHALL return a `CapabilityDenied` error.
- 2.9. WHEN a plugin provides document data that fails schema validation, THEN the host SHALL return a `ValidationError` without persisting the data.

## Requirement 3 — Document Storage Delete Operations

**User Story:** As a plugin author, I want to delete documents from my declared collections so that I can remove records that are no longer needed.

#### Acceptance Criteria

- 3.1. WHEN a plugin calls `storage_doc_delete` with a valid collection and document ID, THEN the host SHALL delete the document.
- 3.2. WHEN a plugin calls `storage_doc_delete` with a document ID that does not exist, THEN the host SHALL return a `NotFound` error.
- 3.3. WHEN a plugin calls `storage_doc_batch_delete` with a collection and array of document IDs, THEN the host SHALL delete all specified documents.
- 3.4. WHEN a plugin calls any document delete function on a collection not declared in its manifest, THEN the host SHALL return a `CapabilityDenied` error.
- 3.5. WHEN a plugin calls any document delete function without the `storage:doc:delete` capability, THEN the host SHALL return a `CapabilityDenied` error.

## Requirement 4 — Blob Storage Read Operations

**User Story:** As a plugin author, I want to retrieve and inspect binary blobs so that I can work with files and attachments stored through Core.

#### Acceptance Criteria

- 4.1. WHEN a plugin calls `storage_blob_retrieve` with a valid key, THEN the host SHALL return the blob bytes.
- 4.2. WHEN a plugin calls `storage_blob_retrieve` with a key that does not exist, THEN the host SHALL return a `NotFound` error.
- 4.3. WHEN a plugin calls `storage_blob_exists` with a key, THEN the host SHALL return `true` if the blob exists or `false` otherwise.
- 4.4. WHEN a plugin calls `storage_blob_list` with a prefix, THEN the host SHALL return metadata for all blobs whose keys match the prefix.
- 4.5. WHEN a plugin calls `storage_blob_metadata` with a valid key, THEN the host SHALL return the blob's size, content type, and created timestamp.
- 4.6. WHEN a plugin calls any blob read function, THEN the host SHALL automatically prefix the key with the calling plugin's ID so that the plugin can only access its own blobs.
- 4.7. WHEN a plugin calls any blob read function without the `storage:blob:read` capability, THEN the host SHALL return a `CapabilityDenied` error.

## Requirement 5 — Blob Storage Write Operations

**User Story:** As a plugin author, I want to store binary blobs so that I can persist files and attachments.

#### Acceptance Criteria

- 5.1. WHEN a plugin calls `storage_blob_store` with a key and byte data, THEN the host SHALL store the blob, overwriting any existing blob at that key.
- 5.2. WHEN a plugin calls `storage_blob_store`, THEN the host SHALL automatically prefix the key with the calling plugin's ID.
- 5.3. WHEN a plugin calls `storage_blob_store` without the `storage:blob:write` capability, THEN the host SHALL return a `CapabilityDenied` error.

## Requirement 6 — Blob Storage Delete Operations

**User Story:** As a plugin author, I want to delete blobs so that I can clean up files and attachments I no longer need.

#### Acceptance Criteria

- 6.1. WHEN a plugin calls `storage_blob_delete` with a valid key, THEN the host SHALL delete the blob.
- 6.2. WHEN a plugin calls `storage_blob_delete` with a key that does not exist, THEN the host SHALL return a `NotFound` error.
- 6.3. WHEN a plugin calls `storage_blob_delete`, THEN the host SHALL automatically prefix the key with the calling plugin's ID.
- 6.4. WHEN a plugin calls `storage_blob_delete` without the `storage:blob:delete` capability, THEN the host SHALL return a `CapabilityDenied` error.

## Requirement 7 — Event Emission

**User Story:** As a plugin author, I want to emit events so that workflows and other plugins can react to actions my plugin performs.

#### Acceptance Criteria

- 7.1. WHEN a plugin calls `emit_event` with a declared event name and optional payload, THEN the host SHALL publish the event to the event bus.
- 7.2. WHEN a plugin calls `emit_event`, THEN the host SHALL automatically set the event's `source` to the calling plugin's ID and `depth` from the current execution context.
- 7.3. WHEN a plugin calls `emit_event` with an event name not declared in its manifest `[events.emit]` section, THEN the host SHALL return a `CapabilityDenied` error.
- 7.4. WHEN a plugin calls `emit_event` without the `events:emit` capability, THEN the host SHALL return a `CapabilityDenied` error.

## Requirement 8 — Configuration Reading

**User Story:** As a plugin author, I want to read my plugin's runtime configuration so that I can adapt behaviour based on user settings.

#### Acceptance Criteria

- 8.1. WHEN a plugin calls `config_read`, THEN the host SHALL return the plugin's runtime configuration as a JSON value.
- 8.2. WHEN a plugin's configuration has been validated against the manifest's `[config]` schema at load time, THEN the value returned by `config_read` SHALL conform to that schema.
- 8.3. WHEN a plugin calls `config_read` without the `config:read` capability, THEN the host SHALL return a `CapabilityDenied` error.

## Requirement 9 — HTTP Outbound

**User Story:** As a plugin author, I want to make outbound HTTP requests so that I can integrate with external APIs and services.

#### Acceptance Criteria

- 9.1. WHEN a plugin calls `http_request` with a valid JSON request object containing `method`, `url`, and optional `headers` and `body`, THEN the host SHALL execute the request and return a JSON response with `status`, `headers`, and `body`.
- 9.2. WHEN an outbound HTTP request fails due to timeout, DNS failure, or connection refused, THEN the host SHALL return a `NetworkError`.
- 9.3. WHEN a plugin calls `http_request` without the `http:outbound` capability, THEN the host SHALL return a `CapabilityDenied` error.

## Requirement 10 — Error Handling

**User Story:** As a plugin author, I want typed errors from host functions so that I can match on failure modes and handle them appropriately.

#### Acceptance Criteria

- 10.1. WHEN any host function fails, THEN it SHALL return a `Result<T, PluginError>` with one of the following variants: `CapabilityDenied`, `NotFound`, `ValidationError`, `StorageError`, `NetworkError`, `InternalError`.
- 10.2. WHEN a capability check fails, THEN the host SHALL return `CapabilityDenied` without executing the underlying operation.
- 10.3. WHEN the plugin SDK receives a `PluginError`, THEN it SHALL surface the error as a typed enum that actions can match on.

## Requirement 11 — Excluded Internal Functions

**User Story:** As a Core developer, I want certain internal operations excluded from the host function surface so that plugins cannot access transaction management, change streams, migrations, health checks, or internal copy operations.

#### Acceptance Criteria

- 11.1. WHEN a plugin attempts to call `transaction`, `watch`, `migrate`, `health`, or `copy`, THEN the WASM runtime SHALL have no such function registered and the call SHALL fail.
- 11.2. WHEN Core initialises the Extism runtime for a plugin, THEN it SHALL NOT register host functions for `transaction`, `watch`, `migrate`, `health`, or `copy`.
