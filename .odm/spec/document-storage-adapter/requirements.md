<!--
domain: document-storage-adapter
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Requirements Document — Document Storage Adapter

## Introduction

The Document Storage Adapter defines the contract that all storage backends must satisfy. It is an async Rust trait with methods for single-document CRUD, batch operations, transactions, querying, change watching, schema migration, health checks, and capability negotiation. All upstream consumers (StorageContext, workflow engine) depend exclusively on this trait, making storage backends interchangeable.

## Alignment with Product Vision

- **Open/Closed Principle** — New storage backends implement the trait without modifying any upstream code
- **Parse, Don't Validate** — Typed data structures (FilterNode, FieldType, CollectionDescriptor) enforce correctness at the boundary; downstream code trusts the types
- **Defence in Depth** — Capability negotiation prevents startup with an adapter that lacks required features (e.g., encryption)
- **Single Source of Truth** — One trait definition governs all storage interactions
- **Fail-Safe Defaults** — Missing capabilities degrade gracefully (empty watch stream, ignored index hints) rather than crashing

## Requirements

### Requirement 1 — Single Document CRUD

**User Story:** As a Core developer, I want get, create, update, partial_update, and delete operations on individual documents so that StorageContext can serve plugin requests against any backend.

#### Acceptance Criteria

- 1.1. WHEN `get` is called with a collection and ID THEN the adapter SHALL return the matching `Document` or `StorageError::NotFound` if it does not exist.
- 1.2. WHEN `create` is called with a collection and Document THEN the adapter SHALL insert the document and return its ID, or return `StorageError::AlreadyExists` if a document with the same ID already exists.
- 1.3. WHEN `update` is called with a collection, ID, and Document THEN the adapter SHALL replace the entire document (except system-managed base fields) or return `StorageError::NotFound` if it does not exist.
- 1.4. WHEN `partial_update` is called with a collection, ID, and a JSON patch THEN the adapter SHALL merge only the provided fields into the existing document, leaving other fields unchanged, or return `StorageError::NotFound` if it does not exist.
- 1.5. WHEN `delete` is called with a collection and ID THEN the adapter SHALL remove the document or return `StorageError::NotFound` if it does not exist.

### Requirement 2 — Query and Count

**User Story:** As a plugin author, I want to list and count documents matching filter, sort, and pagination criteria so that I can retrieve specific subsets of data efficiently.

#### Acceptance Criteria

- 2.1. WHEN `list` is called with a `QueryDescriptor` THEN the adapter SHALL return a `DocumentList` containing documents matching the filter, sorted according to the sort fields, and bounded by the pagination limit.
- 2.2. WHEN the `QueryDescriptor` includes a `cursor` THEN the adapter SHALL return results starting after the cursor position.
- 2.3. WHEN the `QueryDescriptor` includes a `fields` projection THEN the adapter SHALL return only the specified fields in each document.
- 2.4. WHEN `count` is called with a `QueryDescriptor` THEN the adapter SHALL return the number of matching documents using the same filter logic as `list`.
- 2.5. WHEN the `QueryDescriptor` includes `text_search` and the adapter does not support full-text search THEN the adapter SHALL return `StorageError::UnsupportedOperation`.

### Requirement 3 — Filter Expressions

**User Story:** As a Core developer, I want a composable filter tree so that queries can express arbitrary conditions without raw SQL or backend-specific syntax.

#### Acceptance Criteria

- 3.1. WHEN a `FilterNode::Condition` specifies `Eq`, `Ne`, `Gt`, `Gte`, `Lt`, or `Lte` THEN the adapter SHALL apply the corresponding comparison to the specified field.
- 3.2. WHEN a `FilterNode::Condition` specifies `In` or `NotIn` THEN the adapter SHALL match documents where the field value is (or is not) in the provided array.
- 3.3. WHEN a `FilterNode::Condition` specifies `Contains` THEN the adapter SHALL match documents where the string field contains the substring or the array field contains the element.
- 3.4. WHEN a `FilterNode::Condition` specifies `StartsWith` THEN the adapter SHALL match documents where the string field starts with the given prefix.
- 3.5. WHEN a `FilterNode::Condition` specifies `Exists` THEN the adapter SHALL match documents where the field is present, regardless of value.
- 3.6. WHEN a `FilterNode::And` is provided THEN the adapter SHALL combine all child nodes with logical AND.
- 3.7. WHEN a `FilterNode::Or` is provided THEN the adapter SHALL combine all child nodes with logical OR.
- 3.8. WHEN a `FilterNode::Not` is provided THEN the adapter SHALL negate the child node.

### Requirement 4 — Batch Operations

**User Story:** As a Core developer, I want atomic batch create, update, and delete operations so that bulk data changes never leave the system in a partially written state.

#### Acceptance Criteria

- 4.1. WHEN `batch_create` is called with a vector of documents THEN the adapter SHALL insert all documents and return their IDs, or fail atomically with no partial writes if any document fails.
- 4.2. WHEN `batch_update` is called with a vector of (ID, Document) pairs THEN the adapter SHALL replace all specified documents atomically, or fail with no partial writes.
- 4.3. WHEN `batch_delete` is called with a vector of IDs THEN the adapter SHALL remove all specified documents atomically, or fail with no partial writes.

### Requirement 5 — Transactions

**User Story:** As a workflow engine developer, I want to execute multiple storage operations within a transaction so that multi-step workflows either fully commit or fully roll back.

#### Acceptance Criteria

- 5.1. WHEN `transaction` is called with a closure THEN the adapter SHALL execute all operations within the closure in a single transaction scope.
- 5.2. WHEN the transaction closure returns `Ok` THEN the adapter SHALL commit all changes.
- 5.3. WHEN the transaction closure returns `Err` or panics THEN the adapter SHALL roll back all changes.
- 5.4. WHEN the adapter does not support transactions (capability `transactions: false`) THEN `transaction` SHALL return `StorageError::UnsupportedOperation`.
- 5.5. WHEN a `TransactionHandle` is used within a transaction THEN it SHALL provide get, create, update, and delete operations that participate in the enclosing transaction.

### Requirement 6 — Change Watching

**User Story:** As a Core developer, I want to observe document changes in real time so that the event bus can notify subscribers of storage mutations.

#### Acceptance Criteria

- 6.1. WHEN `watch` is called on a collection and the adapter supports native change detection THEN the adapter SHALL return a stream that emits `ChangeEvent` values as documents are created, updated, or deleted.
- 6.2. WHEN the adapter does not support watch (capability `watch: false`) THEN `watch` SHALL return an empty stream, and StorageContext shall emit events on the write path instead.
- 6.3. WHEN a `ChangeEvent` is emitted THEN it SHALL contain the collection name, document ID, change type (Created, Updated, or Deleted), and a UTC timestamp.

### Requirement 7 — Schema Migration

**User Story:** As a Core developer, I want adapters to create or update collection structures from a descriptor so that new collections and schema changes are applied safely at startup.

#### Acceptance Criteria

- 7.1. WHEN `migrate` is called for a collection that does not yet exist THEN the adapter SHALL create the collection's storage structure based on the `CollectionDescriptor`.
- 7.2. WHEN `migrate` is called with a descriptor matching the current state THEN the adapter SHALL make no changes (idempotent).
- 7.3. WHEN `migrate` is called with additive changes (new fields, new indexes) THEN the adapter SHALL apply them without disrupting existing data.
- 7.4. WHEN `migrate` is called with breaking changes (field removal, type changes) THEN the adapter SHALL return `StorageError::SchemaConflict` without applying destructive changes.

### Requirement 8 — Health Reporting

**User Story:** As a Core developer, I want adapters to report their health so that the system can detect and respond to storage failures or degradation.

#### Acceptance Criteria

- 8.1. WHEN `health` is called THEN the adapter SHALL return a `HealthReport` with an overall status of Healthy, Degraded, or Unhealthy.
- 8.2. WHEN the adapter runs individual checks (e.g., connection, disk space, encryption) THEN each check SHALL appear in the `checks` vector with its own status and optional message.

### Requirement 9 — Capability Negotiation

**User Story:** As a Core developer, I want adapters to declare their capabilities so that Core can enforce fallback behaviour and refuse startup when required features are missing.

#### Acceptance Criteria

- 9.1. WHEN `capabilities` is called THEN the adapter SHALL return an `AdapterCapabilities` struct declaring support for encryption, indexing, full_text_search, watch, and transactions.
- 9.2. WHEN `encryption` is `false` and `storage.toml` requires encryption THEN the engine SHALL refuse to start.
- 9.3. WHEN `indexing` is `false` THEN index hints from plugin manifests SHALL be silently ignored.
- 9.4. WHEN `full_text_search` is `false` and a query includes `text_search` THEN the adapter SHALL return `StorageError::UnsupportedOperation`.
- 9.5. WHEN `watch` is `false` THEN the `watch` method SHALL return an empty stream.
- 9.6. WHEN `transactions` is `false` THEN the `transaction` method SHALL return `StorageError::UnsupportedOperation`, but batch operations SHALL still be atomic via adapter-internal mechanisms.

### Requirement 10 — Error Handling and Workflow Mapping

**User Story:** As a workflow engine developer, I want storage errors to carry enough information for correct fault handling so that workflows can decide whether to retry or halt.

#### Acceptance Criteria

- 10.1. WHEN a `NotFound`, `AlreadyExists`, `ValidationFailed`, `CapabilityDenied`, `SchemaConflict`, or `UnsupportedOperation` error occurs during workflow execution THEN the error SHALL map to workflow `Faulted` with a retryable flag of `false`.
- 10.2. WHEN a `Timeout`, `ConnectionFailed`, or `Internal` error occurs during workflow execution THEN the error SHALL map to workflow `Faulted` with a retryable flag of `true`.
- 10.3. WHEN a `StorageError` is returned THEN it SHALL contain contextual fields (collection, id, message, operation, or field as appropriate) sufficient to diagnose the failure.
