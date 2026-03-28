# Architecture Migration Plan

<!--
plan: architecture-migration
created: 2026-03-28
status: active
specs:
  - pipeline-message
  - cdm-specification
  - workflow-engine-contract
  - schema-and-validation
  - schema-versioning-rules
  - document-storage-adapter
  - blob-storage-adapter
  - storage-router
  - storage-context
  - encryption-and-audit
  - plugin-manifest
  - plugin-actions
  - host-functions
  - plugin-system
  - pipeline-executor
  - control-flow
  - event-bus
  - trigger-system
  - scheduler
  - transport-layer
-->

This plan migrates Life Engine Core from its current monolithic architecture to the new four-layer pipeline architecture defined in the `.odm/doc/design/` documentation. The migration is organised into 10 phases, ordered by dependency: foundation types first, then storage, then plugins, then workflow engine, then transports, and finally integration.

Each phase can be developed and tested independently. Later phases depend on earlier phases but not vice versa. Within each phase, work packages are ordered by internal dependency.

---

## Phase 1 — Foundation Types and Shared Contracts

Establish the shared type definitions that every other layer depends on. These types form the vocabulary of the entire system — PipelineMessage, CDM structs, WorkflowRequest/WorkflowResponse, and Identity. Nothing else can be built until these are stable.

> depends: none
> spec: .odm/spec/pipeline-message, .odm/spec/cdm-specification, .odm/spec/workflow-engine-contract

### 1.1 — PipelineMessage Core Types

- [x] Define `PipelineMessage` struct with `payload: serde_json::Value` and `metadata: PipelineMetadata` in `packages/types/src/pipeline.rs`. The payload is the step's primary data. Metadata carries contextual information about the request, identity, execution trace, and plugin-writable hints. Implement `Clone`, `Debug`, `Serialize`, `Deserialize`.
  <!-- files: packages/types/src/pipeline.rs -->
  <!-- purpose: Establish the universal data envelope that all workflow steps consume and produce -->
  <!-- requirements: pipeline-message 1.1, 1.2 -->

- [x] Define `PipelineMetadata` struct with fields: `request_id: String`, `trigger_type: TriggerType` enum (Endpoint, Event, Schedule), `identity: Option<IdentitySummary>`, `params: HashMap<String, String>`, `query: HashMap<String, String>`, `traces: Vec<StepTrace>`, `status_hint: Option<WorkflowStatus>`, `warnings: Vec<String>`, `extra: HashMap<String, Value>`. Enforce write-permission boundaries: plugins may modify `payload`, `status_hint`, `warnings`, `extra`; all other fields are read-only (enforced by SDK, not by the struct itself).
  <!-- files: packages/types/src/pipeline.rs -->
  <!-- purpose: Carry full execution context without requiring plugins to manage it -->
  <!-- requirements: pipeline-message 1.2, 2.1, 2.2, 3.1 -->

- [x] Define `StepTrace` struct with `plugin_id: String`, `action: String`, `duration_ms: u64`, `status: StepStatus` enum (Completed, Skipped, Failed). The executor appends a trace after every step. The final WorkflowResponse includes the full trace list.
  <!-- files: packages/types/src/pipeline.rs -->
  <!-- purpose: Enable execution tracing and debugging across workflow steps -->
  <!-- requirements: pipeline-message 4.1, 4.2 -->

- [x] Define `IdentitySummary` struct with `subject: String` and `issuer: String`. This is the minimal identity projection carried in PipelineMetadata — not the full Identity from auth middleware, just enough for plugins to know who initiated the request.
  <!-- files: packages/types/src/pipeline.rs -->
  <!-- purpose: Provide identity context to plugins without leaking auth implementation details -->
  <!-- requirements: pipeline-message 2.3 -->

- [x] Implement JSON serialisation round-trip for PipelineMessage. Verify that serialising to JSON and deserialising back produces an identical struct. This is critical because PipelineMessage crosses the WASM boundary as JSON bytes.
  <!-- files: packages/types/src/pipeline.rs, packages/types/tests/pipeline_tests.rs -->
  <!-- purpose: Guarantee lossless serialisation at the WASM boundary -->
  <!-- requirements: pipeline-message 5.1, 5.2 -->

- [x] Write unit tests: construct a PipelineMessage with all fields populated, serialise to JSON, deserialise back, assert equality. Test with empty payload, nested payload, and large payload (1MB+ JSON). Test that StepTrace accumulation works correctly across multiple appends.
  <!-- files: packages/types/tests/pipeline_tests.rs -->
  <!-- purpose: Validate PipelineMessage contract stability -->
  <!-- requirements: pipeline-message 1.1, 1.2, 4.1, 5.1 -->

### 1.2 — CDM Rust Structs

- [x] Define the 6 canonical collection Rust structs in `packages/types/src/`: `CalendarEvent` (title, start, end, recurrence, attendees, location, description, timezone, all_day, status, reminders, ext), `Task` (title, description, status enum, priority enum, due_date, completed_at, tags, assignee, parent_id, ext), `Contact` (name struct with given/family/prefix/suffix/middle, emails, phones, addresses with type enums, organization, title, birthday, photo_url, notes, groups, ext), `Note` (title, body, format enum, tags, pinned, ext), `Email` (from/to/cc/bcc as EmailAddress objects, subject, body_text, body_html, date, message_id, in_reply_to, attachments as EmailAttachment, read, starred, ext), `Credential` (credential_type enum, name, service, encrypted, expires_at, claims). All structs share common fields: `id: Uuid`, `source: String`, `source_id: Option<String>`, `created_at: DateTime<Utc>`, `updated_at: DateTime<Utc>`. Credentials intentionally omit `ext` — the `claims` field serves that purpose.
  <!-- files: packages/types/src/events.rs, packages/types/src/tasks.rs, packages/types/src/contacts.rs, packages/types/src/notes.rs, packages/types/src/emails.rs, packages/types/src/credentials.rs -->
  <!-- purpose: Establish the canonical data model as Rust structs — the authoritative type definitions -->
  <!-- requirements: cdm-specification 1.1, 2.1, 3.1, 4.1, 5.1, 6.1, 7.1 -->

- [x] Define nested types for CDM structs: `Recurrence` (frequency, interval, until, count, by_day) with `from_rrule`/`to_rrule` helpers, `Attendee` (name, email, status enum: accepted/declined/tentative/needs-action), `Reminder` (minutes_before, method enum: notification/email), `EventStatus` enum, `TaskStatus` enum (Pending/InProgress/Completed/Cancelled) with Default impl, `TaskPriority` enum (Low/Medium/High/Urgent) with Default impl, `ContactName`, `ContactEmail`, `ContactPhone`, `ContactAddress` with type enums (home/work/other), `NoteFormat` enum (plain/markdown/html), `EmailAddress` (name optional, address required), `EmailAttachment` (filename, mime_type, size_bytes, content_id), `CredentialType` enum.
  <!-- files: packages/types/src/events.rs, packages/types/src/tasks.rs, packages/types/src/contacts.rs, packages/types/src/notes.rs, packages/types/src/emails.rs, packages/types/src/credentials.rs -->
  <!-- purpose: Define all nested/enum types that CDM structs depend on -->
  <!-- requirements: cdm-specification 2.1, 3.1, 4.1, 5.1, 6.1, 7.1 -->

- [x] Implement the extension convention: `ext: Option<HashMap<String, HashMap<String, Value>>>` on all CDM structs except Credential. The outer key is the plugin_id, the inner key is the field name. Extensions use merge-not-replace semantics on update. Add helper methods `get_ext(plugin_id, field)` and `set_ext(plugin_id, field, value)`.
  <!-- files: packages/types/src/extensions.rs -->
  <!-- purpose: Enable plugin-specific extension fields with namespace isolation -->
  <!-- requirements: cdm-specification 8.1, 8.2, 8.3 -->

- [x] Write unit tests for all CDM structs: construct each struct, serialise to JSON, deserialise back, assert equality. Test extension merge behaviour. Test enum default values. Test Recurrence from_rrule/to_rrule round-trip.
  <!-- files: packages/types/tests/ -->
  <!-- purpose: Validate CDM type stability and serialisation correctness -->
  <!-- requirements: cdm-specification 1.1 through 8.3 -->

### 1.3 — CDM JSON Schemas

- [x] Create JSON Schema (Draft 2020-12) files for all 6 CDM collections: `schemas/events.schema.json`, `schemas/tasks.schema.json`, `schemas/contacts.schema.json`, `schemas/notes.schema.json`, `schemas/emails.schema.json`, `schemas/credentials.schema.json`. Each schema must include `$id` metadata, `uuid` format on `id` fields, accurate `$defs` for nested types, and `required` arrays matching the Rust struct's non-Optional fields. These schemas are the reference for validation — the Rust structs are authoritative for type definitions.
  <!-- files: schemas/*.schema.json -->
  <!-- purpose: Provide JSON Schema definitions for runtime validation and plugin interop -->
  <!-- requirements: cdm-specification 12.1, 12.2 -->

- [x] Write schema validation tests: for each CDM collection, construct valid and invalid JSON payloads and validate them against the schema files. Test that required fields are enforced, enum values are constrained, and optional fields are truly optional.
  <!-- files: packages/types/tests/schema_validation_tests.rs -->
  <!-- purpose: Verify JSON schemas match Rust struct definitions -->
  <!-- requirements: cdm-specification 12.1, 12.2 -->

### 1.4 — Workflow Engine Contract Types

- [x] Define `WorkflowRequest` struct with fields: `workflow: String` (e.g. "collection.list"), `identity: Identity`, `params: HashMap<String, String>` (path params), `query: HashMap<String, String>` (query string / GraphQL args), `body: Option<Value>` (parsed request body), `meta: RequestMeta` (request id, timestamp, source binding). Define `RequestMeta` with `request_id: String`, `timestamp: DateTime<Utc>`, `source_binding: String`.
  <!-- files: packages/types/src/workflow.rs -->
  <!-- purpose: Establish the input contract between transport handlers and the workflow engine -->
  <!-- requirements: workflow-engine-contract 1.1, 1.2 -->

- [x] Define `WorkflowResponse` struct with fields: `status: WorkflowStatus`, `data: Option<Value>`, `errors: Vec<WorkflowError>`, `meta: ResponseMeta`. Define `ResponseMeta` with `request_id: String`, `duration_ms: u64`, `traces: Vec<StepTrace>`. Define `WorkflowError` with `code: String`, `message: String`, `detail: Option<Value>`.
  <!-- files: packages/types/src/workflow.rs -->
  <!-- purpose: Establish the output contract from the workflow engine back to transport handlers -->
  <!-- requirements: workflow-engine-contract 2.1, 2.2 -->

- [x] Define `WorkflowStatus` enum with variants: `Ok`, `Created`, `NotFound`, `Denied`, `Invalid`, `Error`. Each variant must carry distinct semantics across multiple protocols (Rule A). Document each variant's intended meaning. Include `impl WorkflowStatus` with `is_success(&self) -> bool` and `http_status_code(&self) -> u16` helpers.
  <!-- files: packages/types/src/workflow.rs -->
  <!-- purpose: Provide a minimal, protocol-agnostic status vocabulary -->
  <!-- requirements: workflow-engine-contract 3.1, 3.2 -->

- [x] Define `Identity` struct with fields: `subject: String`, `issuer: String`, `claims: HashMap<String, Value>`. This is the full identity from auth middleware, passed as `Extension<Identity>` in Axum and included in every WorkflowRequest. Define `TriggerContext` enum with variants: `Endpoint(WorkflowRequest)`, `Event { name: String, payload: Option<Value>, source: String }`, `Schedule { workflow_id: String }`.
  <!-- files: packages/types/src/identity.rs, packages/types/src/trigger.rs -->
  <!-- purpose: Establish identity and trigger context types used throughout the system -->
  <!-- requirements: workflow-engine-contract 4.1, pipeline-executor 2.1 -->

- [x] Write unit tests for all contract types: serialisation round-trips, WorkflowStatus helper methods, TriggerContext variant construction.
  <!-- files: packages/types/tests/workflow_tests.rs -->
  <!-- purpose: Validate contract type stability -->
  <!-- requirements: workflow-engine-contract 1.1 through 4.1 -->

---

## Phase 2 — Schema Infrastructure

Build the schema validation engine and versioning rules. The data layer depends on this to validate documents on write. Plugin manifests reference schemas that this infrastructure resolves.

> depends: 1.3
> spec: .odm/spec/schema-and-validation, .odm/spec/schema-versioning-rules

### 2.1 — Schema Registry

- [x] Implement `SchemaRegistry` in `packages/traits/src/schema.rs`: a registry that loads JSON Schema files at startup and exposes `validate(collection: &str, document: &Value) -> Result<(), ValidationErrors>`. The registry holds compiled schemas in a `HashMap<String, CompiledSchema>`. Use the `jsonschema` crate for Draft 2020-12 validation. Schemas are loaded from two sources: CDM schema files bundled with Core, and plugin manifest-declared schemas discovered at startup.
  <!-- files: packages/traits/src/schema.rs -->
  <!-- purpose: Centralise schema validation so StorageContext and plugin system can reference it -->
  <!-- requirements: schema-and-validation 1.1, 1.2, 1.3 -->

- [x] Implement schema loading from plugin manifests: when a plugin declares `collections.{name}.schema = "path/to/schema.json"` in its manifest, the registry loads that schema file relative to the plugin directory. If the schema file is missing or invalid JSON Schema, fail at startup with a clear error.
  <!-- files: packages/traits/src/schema.rs -->
  <!-- purpose: Enable plugins to declare and enforce custom schemas -->
  <!-- requirements: schema-and-validation 2.1, 2.2 -->

- [x] Implement validation behaviour: validation applies to write operations only (create, update, partial_update). Read operations never validate. If a collection has no registered schema, writes pass through without validation. If a collection has a schema and `strict: true` in its declaration, additional properties beyond those in the schema are rejected. If `strict: false` (default), additional properties are allowed.
  <!-- files: packages/traits/src/schema.rs -->
  <!-- purpose: Define when and how validation is applied -->
  <!-- requirements: schema-and-validation 3.1, 3.2, 3.3 -->

- [x] Implement system-managed base field handling: the fields `id`, `created_at`, and `updated_at` are managed by StorageContext, not by callers. The schema registry strips these fields before validation (they are always valid because the system controls them) and injects them after validation. This prevents plugins from setting invalid IDs or timestamps.
  <!-- files: packages/traits/src/schema.rs -->
  <!-- purpose: Ensure system-managed fields are always correct -->
  <!-- requirements: schema-and-validation 4.1, 4.2 -->

- [x] Write unit tests: validate a valid CDM document against its schema, validate an invalid document and check error messages, test strict vs non-strict mode, test schema-less collection pass-through, test system field stripping.
  <!-- files: packages/traits/tests/schema_tests.rs -->
  <!-- purpose: Verify schema validation behaviour comprehensively -->
  <!-- requirements: schema-and-validation 1.1 through 4.2 -->

### 2.2 — Index Hints

- [x] Implement index hint parsing from plugin manifest collection declarations. Index hints are suggestions to storage adapters, not requirements. The schema registry parses `indexes: [{ fields: ["email"], unique: true }]` from manifest collection blocks and stores them alongside the schema. Adapters query index hints when creating collections and decide whether to honour them based on their capabilities.
  <!-- files: packages/traits/src/schema.rs -->
  <!-- purpose: Allow plugins to suggest optimal indexes without coupling to adapter internals -->
  <!-- requirements: schema-and-validation 5.1, 5.2 -->

### 2.3 — Schema Versioning Rules

- [x] Implement schema compatibility checker in `packages/traits/src/schema_versioning.rs`: given two schema versions (old and new), classify the change as non-breaking (additive) or breaking. Non-breaking changes: adding optional fields, adding new enum values, relaxing constraints (e.g. removing `required` from a field), adding new `$defs`. Breaking changes: removing fields, renaming fields, changing field types, adding required fields, removing enum values. The checker operates on the JSON Schema AST, comparing property sets, required arrays, and type declarations.
  <!-- files: packages/traits/src/schema_versioning.rs -->
  <!-- purpose: Enforce the additive-only rule within major SDK versions -->
  <!-- requirements: schema-versioning-rules 1.1, 2.1, 2.2, 3.1 -->

- [x] Implement deprecation tracking: when a schema field is deprecated, the schema must include a `deprecated: true` annotation on that field's definition. The compatibility checker warns (but does not reject) when deprecated fields are removed in a new major version. Deprecation notices must appear in CHANGELOG entries.
  <!-- files: packages/traits/src/schema_versioning.rs -->
  <!-- purpose: Enable graceful schema evolution with advance deprecation notice -->
  <!-- requirements: schema-versioning-rules 4.1, 4.2 -->

- [x] Write unit tests: classify known non-breaking changes as compatible, classify known breaking changes as incompatible, test edge cases (enum value addition, required field removal, type widening).
  <!-- files: packages/traits/tests/schema_versioning_tests.rs -->
  <!-- purpose: Verify schema compatibility classification accuracy -->
  <!-- requirements: schema-versioning-rules 1.1 through 4.2 -->

---

## Phase 3 — Storage Adapter Traits

Define the abstract storage contracts. These traits are implemented by concrete adapters (SQLite, filesystem) in Phase 4. The traits must be stable before any adapter code is written.

> depends: 1.1, 1.4
> spec: .odm/spec/document-storage-adapter, .odm/spec/blob-storage-adapter

### 3.1 — Document Storage Error Types

- [x] Define `StorageError` enum in `packages/traits/src/storage.rs` with variants: `NotFound { collection, id }`, `Conflict { collection, id, expected_version, actual_version }`, `ValidationFailed { collection, errors: Vec<String> }`, `PermissionDenied { collection, capability }`, `AdapterError { source: Box<dyn Error> }`, `Timeout { operation, duration }`, `TransactionFailed { reason }`, `UnsupportedOperation { operation, adapter }`. Each variant maps to a `WorkflowStatus`: NotFound → NotFound, Conflict → Invalid, ValidationFailed → Invalid, PermissionDenied → Denied, AdapterError → Error, Timeout → Error, TransactionFailed → Error, UnsupportedOperation → Error. Include `is_retryable(&self) -> bool` method.
  <!-- files: packages/traits/src/storage.rs -->
  <!-- purpose: Define a comprehensive error model that maps cleanly to workflow statuses -->
  <!-- requirements: document-storage-adapter 10.1, 10.2, 10.3 -->

### 3.2 — Document Storage Query Types

- [x] Define `QueryDescriptor` struct with: `filters: Option<FilterNode>`, `sort: Vec<SortField>`, `pagination: Pagination`, `fields: Option<Vec<String>>` (projection), `text_search: Option<String>`. Define `FilterNode` enum with variants: `And(Vec<FilterNode>)`, `Or(Vec<FilterNode>)`, `Not(Box<FilterNode>)`, `Comparison { field: String, operator: FilterOperator, value: Value }`. Define `FilterOperator` enum: `Eq`, `Ne`, `Gt`, `Gte`, `Lt`, `Lte`, `In`, `Contains`, `StartsWith`, `Exists`. Define `SortField` with `field: String` and `direction: SortDirection` (Asc, Desc). Define `Pagination` with `offset: u64` and `limit: u64`.
  <!-- files: packages/traits/src/storage.rs -->
  <!-- purpose: Establish a backend-agnostic query model -->
  <!-- requirements: document-storage-adapter 2.1, 3.1, 3.2 -->

- [x] Define `DocumentList` struct with: `items: Vec<Value>`, `total: u64`, `offset: u64`, `limit: u64`. This is the return type for list/query operations.
  <!-- files: packages/traits/src/storage.rs -->
  <!-- purpose: Standardise list operation return shape -->
  <!-- requirements: document-storage-adapter 2.2 -->

### 3.3 — Document Storage Adapter Trait

- [x] Define the `DocumentStorageAdapter` async trait with methods: `get(collection: &str, id: &str) -> Result<Value, StorageError>`, `create(collection: &str, document: Value) -> Result<Value, StorageError>`, `update(collection: &str, id: &str, document: Value) -> Result<Value, StorageError>`, `partial_update(collection: &str, id: &str, patch: Value) -> Result<Value, StorageError>`, `delete(collection: &str, id: &str) -> Result<(), StorageError>`, `list(collection: &str, query: QueryDescriptor) -> Result<DocumentList, StorageError>`, `count(collection: &str, filters: Option<FilterNode>) -> Result<u64, StorageError>`, `batch_create(collection: &str, documents: Vec<Value>) -> Result<Vec<Value>, StorageError>`, `batch_update(collection: &str, updates: Vec<(String, Value)>) -> Result<Vec<Value>, StorageError>`, `batch_delete(collection: &str, ids: Vec<String>) -> Result<u64, StorageError>`, `begin_transaction() -> Result<TransactionHandle, StorageError>`, `watch(collection: &str) -> Result<Receiver<ChangeEvent>, StorageError>`, `migrate(descriptor: CollectionDescriptor) -> Result<(), StorageError>`, `health() -> Result<HealthReport, StorageError>`, `capabilities() -> AdapterCapabilities`.
  <!-- files: packages/traits/src/storage.rs -->
  <!-- purpose: Define the complete document storage contract that all adapters implement -->
  <!-- requirements: document-storage-adapter 1.1, 2.1, 4.1, 5.1, 6.1, 7.1, 8.1, 9.1 -->

- [x] Define supporting types: `TransactionHandle` trait with `commit()` and `rollback()` methods, `ChangeEvent` struct with `change_type: ChangeType` (Created, Updated, Deleted), `collection: String`, `document_id: String`, `document: Option<Value>`, `timestamp: DateTime<Utc>`. Define `CollectionDescriptor` with `name: String`, `fields: Vec<FieldDescriptor>`, `indexes: Vec<IndexDescriptor>`. Define `FieldDescriptor` with `name: String`, `field_type: FieldType`, `required: bool`. Define `FieldType` enum: `String`, `Integer`, `Float`, `Boolean`, `DateTime`, `Uuid`, `Json`, `Array`, `Object`. Define `HealthReport` with `status: HealthStatus` (Healthy, Degraded, Unhealthy), `checks: Vec<HealthCheck>`, `latency_ms: u64`. Define `AdapterCapabilities` with `transactions: bool`, `text_search: bool`, `change_watching: bool`, `batch_operations: bool`, `partial_update: bool`.
  <!-- files: packages/traits/src/storage.rs -->
  <!-- purpose: Define all supporting types for the document storage trait -->
  <!-- requirements: document-storage-adapter 5.1, 6.1, 7.1, 8.1, 9.1 -->

- [x] Write trait-level documentation with usage examples and implement a mock adapter for testing. The mock stores documents in a `HashMap<String, HashMap<String, Value>>` (collection → id → document) and implements all trait methods.
  <!-- files: packages/test-utils/src/storage.rs -->
  <!-- purpose: Provide a test double for all modules that depend on document storage -->
  <!-- requirements: document-storage-adapter 1.1 -->

### 3.4 — Blob Storage Types

- [x] Define blob storage types in `packages/traits/src/blob.rs`: `ByteStream` (wrapping `tokio::io::AsyncRead + Send` for streaming without memory buffering), `BlobInput` struct with `key: String`, `data: ByteStream`, `content_type: String`, `metadata: HashMap<String, String>`, `BlobMeta` struct with `key: String`, `content_type: String`, `size_bytes: u64`, `checksum_sha256: String`, `created_at: DateTime<Utc>`, `metadata: HashMap<String, String>`. Define `BlobAdapterCapabilities` with `encryption: bool`, `server_side_copy: bool`, `streaming: bool`.
  <!-- files: packages/traits/src/blob.rs -->
  <!-- purpose: Establish blob storage types that support streaming I/O -->
  <!-- requirements: blob-storage-adapter 1.1, 1.2 -->

### 3.5 — Blob Storage Adapter Trait

- [x] Define the `BlobStorageAdapter` async trait with methods: `store(input: BlobInput) -> Result<BlobMeta, StorageError>` (compute SHA-256 during streaming write, return metadata with checksum), `retrieve(key: &str) -> Result<(ByteStream, BlobMeta), StorageError>`, `delete(key: &str) -> Result<(), StorageError>`, `exists(key: &str) -> Result<bool, StorageError>`, `copy(source: &str, destination: &str) -> Result<BlobMeta, StorageError>`, `list(prefix: &str) -> Result<Vec<BlobMeta>, StorageError>`, `metadata(key: &str) -> Result<BlobMeta, StorageError>`, `health() -> Result<HealthReport, StorageError>`, `capabilities() -> BlobAdapterCapabilities`.
  <!-- files: packages/traits/src/blob.rs -->
  <!-- purpose: Define the complete blob storage contract -->
  <!-- requirements: blob-storage-adapter 2.1, 3.1, 4.1, 5.1, 6.1, 7.1, 8.1, 9.1, 10.1 -->

- [x] Implement blob key format validation: keys must match pattern `{plugin_id}/{context}/{filename}`. Validate at the trait boundary (in StorageContext, not in the adapter). Keys with `..`, leading `/`, or empty segments are rejected.
  <!-- files: packages/traits/src/blob.rs -->
  <!-- purpose: Enforce consistent and safe blob key naming -->
  <!-- requirements: blob-storage-adapter 11.1 -->

- [x] Implement a mock blob adapter for testing that stores blobs in memory using `HashMap<String, Vec<u8>>`.
  <!-- files: packages/test-utils/src/blob.rs -->
  <!-- purpose: Provide a test double for blob storage consumers -->
  <!-- requirements: blob-storage-adapter 2.1 -->

---

## Phase 4 — Storage Layer Implementation

Implement the concrete storage adapters (SQLite/SQLCipher, filesystem), the StorageRouter that dispatches operations, and the StorageContext API that plugins use.

> depends: 2.1, 3.1 through 3.5
> spec: .odm/spec/storage-router, .odm/spec/storage-context

### 4.1 — SQLite Document Adapter

- [x] Implement `SqliteDocumentAdapter` in `packages/storage-sqlite/` implementing the `DocumentStorageAdapter` trait. Use `rusqlite` with `bundled-sqlcipher` feature for encrypted storage. Store documents as JSON in a `data` column alongside `id`, `collection`, `created_at`, `updated_at` columns. Implement `get`, `create`, `update`, `partial_update`, and `delete` methods using parameterised SQL (no string interpolation — prevent SQL injection).
  <!-- files: packages/storage-sqlite/src/document.rs -->
  <!-- purpose: Provide the default document storage backend -->
  <!-- requirements: document-storage-adapter 1.1, storage-context 1.1 -->

- [x] Implement query translation: convert `QueryDescriptor` into SQL WHERE clauses with bind parameters. Map `FilterNode` tree to SQL predicates. Support `json_extract()` for nested field access. Implement sorting, pagination (LIMIT/OFFSET), and field projection (select specific JSON paths). Implement `count` as `SELECT COUNT(*)`.
  <!-- files: packages/storage-sqlite/src/query.rs -->
  <!-- purpose: Translate the abstract query model into SQLite-specific SQL -->
  <!-- requirements: document-storage-adapter 2.1, 3.1, 3.2 -->

- [x] Implement batch operations: `batch_create` uses a single transaction with multiple INSERTs, `batch_update` uses a single transaction with multiple UPDATEs, `batch_delete` uses `DELETE ... WHERE id IN (...)`. All batch operations are atomic — if any individual operation fails, the entire batch rolls back.
  <!-- files: packages/storage-sqlite/src/batch.rs -->
  <!-- purpose: Provide efficient atomic batch operations -->
  <!-- requirements: document-storage-adapter 4.1 -->

- [x] Implement transaction support: `begin_transaction` returns a `SqliteTransactionHandle` wrapping a rusqlite transaction. Operations within the handle run against the transaction. `commit()` finalises; `rollback()` aborts. Dropping the handle without committing automatically rolls back.
  <!-- files: packages/storage-sqlite/src/transaction.rs -->
  <!-- purpose: Support multi-step atomic operations -->
  <!-- requirements: document-storage-adapter 5.1 -->

- [x] Implement change watching: use a polling-based approach — after each write operation, emit a `ChangeEvent` to subscribers via a Tokio broadcast channel. The `watch` method returns a receiver. This is a fallback implementation since SQLite does not have native change notifications.
  <!-- files: packages/storage-sqlite/src/watch.rs -->
  <!-- purpose: Enable reactive patterns via change event streaming -->
  <!-- requirements: document-storage-adapter 6.1 -->

- [x] Implement schema migration: `migrate(descriptor)` creates the collection table if it does not exist, adds columns for indexed fields, creates indexes from the descriptor's index list. Migrations are idempotent — running the same descriptor twice is a no-op. Breaking changes (removing columns) are rejected.
  <!-- files: packages/storage-sqlite/src/migration.rs -->
  <!-- purpose: Support schema evolution without data loss -->
  <!-- requirements: document-storage-adapter 7.1 -->

- [x] Implement health check: verify database connectivity by executing `SELECT 1`, report database file size, encryption status, and WAL mode status.
  <!-- files: packages/storage-sqlite/src/health.rs -->
  <!-- purpose: Enable health monitoring of the storage backend -->
  <!-- requirements: document-storage-adapter 8.1 -->

- [x] Write comprehensive tests: CRUD operations, query with filters/sort/pagination, batch atomicity (partial failure rolls back), transaction commit/rollback, change event emission, migration idempotency, health check.
  <!-- files: packages/storage-sqlite/tests/ -->
  <!-- purpose: Verify SQLite adapter correctness -->
  <!-- requirements: document-storage-adapter 1.1 through 10.3 -->

### 4.2 — Filesystem Blob Adapter

- [x] Implement `FilesystemBlobAdapter` in `packages/storage-sqlite/src/blob_fs.rs` (or a new crate) implementing the `BlobStorageAdapter` trait. Store blobs as files under a configured base directory. Use atomic write-then-rename strategy: write to a temporary file, then rename to the final path. This prevents partial writes from leaving corrupt files. Store metadata in sidecar `.meta.json` files alongside each blob.
  <!-- files: packages/storage-sqlite/src/blob_fs.rs -->
  <!-- purpose: Provide the default blob storage backend -->
  <!-- requirements: blob-storage-adapter 2.1, 3.1 -->

- [x] Implement streaming: `store` reads from `ByteStream` and writes to disk in chunks (64KB default), computing SHA-256 incrementally during the write. `retrieve` returns a `ByteStream` wrapping a `tokio::fs::File`. No full-file buffering in memory.
  <!-- files: packages/storage-sqlite/src/blob_fs.rs -->
  <!-- purpose: Handle large files without memory pressure -->
  <!-- requirements: blob-storage-adapter 2.1, 3.1 -->

- [x] Implement `delete` (remove both blob file and sidecar), `exists` (check file existence), `copy` (filesystem copy with new sidecar), `list` (directory walk matching prefix), `metadata` (read sidecar file).
  <!-- files: packages/storage-sqlite/src/blob_fs.rs -->
  <!-- purpose: Complete the blob adapter implementation -->
  <!-- requirements: blob-storage-adapter 4.1, 5.1, 6.1, 7.1, 8.1 -->

- [x] Implement MIME type detection: use the `infer` crate to detect content type from file magic bytes when `content_type` is not provided by the caller.
  <!-- files: packages/storage-sqlite/src/blob_fs.rs -->
  <!-- purpose: Auto-detect content types for blobs stored without explicit MIME type -->
  <!-- requirements: blob-storage-adapter 14.1 -->

- [x] Write tests: store and retrieve round-trip, SHA-256 checksum verification, atomic write (verify no partial files on failure), large file streaming (10MB+), list by prefix, copy operation, metadata sidecar correctness.
  <!-- files: packages/storage-sqlite/tests/blob_fs_tests.rs -->
  <!-- purpose: Verify filesystem blob adapter correctness -->
  <!-- requirements: blob-storage-adapter 2.1 through 14.1 -->

### 4.3 — Storage Router

- [x] Implement `StorageRouter` in `packages/traits/src/storage_router.rs`: holds references to the active `DocumentStorageAdapter` and `BlobStorageAdapter`. Routes document operations to the document adapter and blob operations to the blob adapter. The router is constructed at startup from `storage.toml` configuration, which specifies which adapter to use for each category.
  <!-- files: packages/traits/src/storage_router.rs -->
  <!-- purpose: Decouple storage consumers from specific adapter implementations -->
  <!-- requirements: storage-router 1.1, 1.2, 2.1 -->

- [x] Implement timeout enforcement: wrap every adapter call in `tokio::time::timeout`. Read timeout and write timeout are configured separately in `storage.toml`. If a timeout fires, return `StorageError::Timeout` with the operation name and configured duration. Default timeouts: 5s read, 10s write.
  <!-- files: packages/traits/src/storage_router.rs -->
  <!-- purpose: Prevent hung adapter operations from blocking the pipeline -->
  <!-- requirements: storage-router 3.1, 3.2 -->

- [x] Implement metrics emission: record operation latency, success/failure counts, and active operation count via structured log events. Use `tracing::instrument` for automatic span creation. Metrics are emitted for every operation routed through the router.
  <!-- files: packages/traits/src/storage_router.rs -->
  <!-- purpose: Enable operational monitoring of storage performance -->
  <!-- requirements: storage-router 4.1 -->

- [x] Implement startup sequence: load `storage.toml`, resolve adapter names to concrete implementations (static registry: "sqlite" → SqliteDocumentAdapter, "filesystem" → FilesystemBlobAdapter), check required capabilities declared in config against adapter capabilities, run health checks on both adapters, abort startup if any check fails.
  <!-- files: packages/traits/src/storage_router.rs -->
  <!-- purpose: Validate storage configuration at startup -->
  <!-- requirements: storage-router 5.1, 5.2, 5.3 -->

- [x] Implement health aggregation: `StorageRouter::health()` calls `health()` on both adapters and returns a combined `HealthReport`. Status is the worst of both (if either is Unhealthy, the aggregate is Unhealthy).
  <!-- files: packages/traits/src/storage_router.rs -->
  <!-- purpose: Single health check endpoint for all storage -->
  <!-- requirements: storage-router 6.1 -->

- [x] Write unit tests: routing to correct adapter, timeout enforcement (mock adapter with sleep), health aggregation, startup validation with capability mismatch.
  <!-- files: packages/traits/tests/storage_router_tests.rs -->
  <!-- purpose: Verify storage routing correctness -->
  <!-- requirements: storage-router 1.1 through 6.1 -->

### 4.4 — Storage Context

- [x] Implement `StorageContext` in `packages/traits/src/storage_context.rs`: the API surface that plugins and workflows use to interact with storage. StorageContext wraps the StorageRouter and adds permission checking, collection scoping, schema validation, and audit event emission. Two access paths: plugin access (scoped and permission-checked based on plugin manifest) and workflow engine access (system-level, bypasses plugin checks but still validates schemas and emits audit events).
  <!-- files: packages/traits/src/storage_context.rs -->
  <!-- purpose: Provide the single enforcement point for all storage access -->
  <!-- requirements: storage-context 1.1, 1.2, 2.1 -->

- [x] Implement permission enforcement: check that the calling plugin has declared the required capability (`storage:doc:read`, `storage:doc:write`, `storage:doc:delete`, `storage:blob:read`, `storage:blob:write`, `storage:blob:delete`) in its manifest. Check that the collection being accessed is declared in the plugin's `collections` manifest section. Reject with `StorageError::PermissionDenied` if either check fails.
  <!-- files: packages/traits/src/storage_context.rs -->
  <!-- purpose: Enforce deny-by-default storage access -->
  <!-- requirements: storage-context 2.1, 2.2, 2.3 -->

- [x] Implement collection scoping: shared collections are accessed by name directly. Plugin-scoped collections are prefixed with `{plugin_id}.` automatically. A plugin can only access its own scoped collections and shared collections declared in its manifest.
  <!-- files: packages/traits/src/storage_context.rs -->
  <!-- purpose: Isolate plugin data while allowing shared collections -->
  <!-- requirements: storage-context 3.1, 3.2 -->

- [x] Implement schema validation on write: before passing a document to the StorageRouter for create/update/partial_update, validate it against the collection's registered schema (from the SchemaRegistry). If validation fails, return `StorageError::ValidationFailed` without calling the adapter. System-managed base fields (id, created_at, updated_at) are injected after validation.
  <!-- files: packages/traits/src/storage_context.rs -->
  <!-- purpose: Validate data before it reaches the storage layer -->
  <!-- requirements: storage-context 4.1, 4.2, 4.3 -->

- [x] Implement extension field handling: when a plugin writes to a shared collection, only its own namespace (`ext.{plugin_id}.*`) is writable. Attempting to write to another plugin's namespace is silently ignored (the write succeeds, but the foreign namespace fields are stripped). Extension merging uses merge-not-replace: updating `ext.my_plugin.foo` does not affect `ext.other_plugin.bar`.
  <!-- files: packages/traits/src/storage_context.rs -->
  <!-- purpose: Enforce extension namespace isolation -->
  <!-- requirements: storage-context 5.1, 5.2 -->

- [x] Implement audit event emission: after every successful write operation, emit a system event via the event bus: `system.storage.created`, `system.storage.updated`, `system.storage.deleted` for documents; `system.blob.stored`, `system.blob.deleted` for blobs. Events include collection name, document ID, and the identity of the caller (if available).
  <!-- files: packages/traits/src/storage_context.rs -->
  <!-- purpose: Enable audit logging and reactive patterns based on storage changes -->
  <!-- requirements: storage-context 6.1, 6.2 -->

- [x] Implement watch-to-event-bus bridge: when a StorageContext consumer calls `watch(collection)`, the StorageContext subscribes to the adapter's ChangeEvent stream and re-emits each change as an event bus event. This bridges adapter-level change watching into the unified event model.
  <!-- files: packages/traits/src/storage_context.rs -->
  <!-- purpose: Unify change watching with the event bus -->
  <!-- requirements: storage-context 7.1 -->

- [x] Write comprehensive tests: permission checks (allowed/denied), collection scoping (plugin vs shared), schema validation (valid/invalid), extension namespace isolation, audit event emission, system-level access bypass.
  <!-- files: packages/traits/tests/storage_context_tests.rs -->
  <!-- purpose: Verify StorageContext enforcement behaviour -->
  <!-- requirements: storage-context 1.1 through 7.1 -->

---

## Phase 5 — Encryption and Audit

Layer encryption-at-rest and audit logging onto the storage infrastructure built in Phase 4.

> depends: 4.1, 4.4
> spec: .odm/spec/encryption-and-audit

### 5.1 — Shared Crypto Crate

- [x] Implement `packages/crypto/` with four primitives: `aes_256_gcm_encrypt(plaintext: &[u8], key: &[u8]) -> Result<Vec<u8>>` and `aes_256_gcm_decrypt(ciphertext: &[u8], key: &[u8]) -> Result<Vec<u8>>` using random nonces prepended to ciphertext; `argon2id_derive_key(passphrase: &str, salt: &[u8], params: Argon2Params) -> [u8; 32]` with configurable memory (default 64MB), iterations (default 3), and parallelism (default 4); `hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32]`. All functions are pure, stateless, and reusable across modules.
  <!-- files: packages/crypto/src/lib.rs, packages/crypto/src/aes.rs, packages/crypto/src/kdf.rs, packages/crypto/src/hmac.rs -->
  <!-- purpose: Provide shared crypto primitives used by all modules needing encryption -->
  <!-- requirements: encryption-and-audit 1.1, 2.1, 2.2, 2.3 -->

- [x] Write unit tests: encrypt-decrypt round-trip, incorrect key returns error, Argon2 parameter validation, HMAC produces consistent output for same inputs.
  <!-- files: packages/crypto/tests/ -->
  <!-- purpose: Verify crypto primitive correctness -->
  <!-- requirements: encryption-and-audit 1.1, 2.1 -->

### 5.2 — Database Encryption

- [x] Implement SQLCipher key derivation and database opening in `packages/storage-sqlite/`: derive the encryption key from a user-provided master passphrase using `argon2id_derive_key`, open the database with `PRAGMA key = x'...'`. The passphrase is provided at startup via environment variable or interactive prompt. The derived key is held in memory for the lifetime of the process. The raw passphrase is zeroized immediately after derivation.
  <!-- files: packages/storage-sqlite/src/encryption.rs -->
  <!-- purpose: Encrypt the entire database at rest -->
  <!-- requirements: encryption-and-audit 3.1, 3.2, 3.3 -->

- [x] Implement key rotation: `rekey(old_passphrase, new_passphrase)` derives a new key and calls `PRAGMA rekey`. This re-encrypts the entire database. The operation is atomic — if it fails, the database retains the old key.
  <!-- files: packages/storage-sqlite/src/encryption.rs -->
  <!-- purpose: Support passphrase changes without data loss -->
  <!-- requirements: encryption-and-audit 3.4 -->

### 5.3 — Per-Record Credential Encryption

- [x] Implement credential encryption in StorageContext: when writing to the `credentials` collection, encrypt the `encrypted` field of each credential document using AES-256-GCM with a key derived from the master passphrase via a separate HKDF context (so the credential key is distinct from the database key). Decrypt on read. This provides defence-in-depth — even if the database encryption is compromised, individual credentials remain encrypted.
  <!-- files: packages/traits/src/storage_context.rs, packages/crypto/src/credential.rs -->
  <!-- purpose: Add per-record encryption for sensitive credential data -->
  <!-- requirements: encryption-and-audit 4.1, 4.2 -->

### 5.4 — Audit Logging

- [x] Implement audit log persistence: subscribe to `system.storage.*` and `system.blob.*` events via the event bus. Write each event to an `audit_log` collection with fields: `event_type`, `collection`, `document_id`, `identity_subject`, `timestamp`, `detail: Value`. The audit log collection is internal (not accessible to plugins). Implement daily rotation (partition by date) and 90-day retention (delete entries older than 90 days on a daily schedule).
  <!-- files: apps/core/src/audit.rs -->
  <!-- purpose: Persist security-relevant events for local review -->
  <!-- requirements: encryption-and-audit 5.1, 5.2, 5.3, 5.4 -->

- [x] Implement security event logging: in addition to storage events, log auth attempts (success/failure), credential access, plugin installs/removals, permission changes, and connector auth/revocation as audit events.
  <!-- files: apps/core/src/audit.rs -->
  <!-- purpose: Comprehensive security audit trail -->
  <!-- requirements: encryption-and-audit 6.1, 6.2 -->

- [x] Write tests: audit event emission and persistence, retention enforcement (entries older than 90 days are removed), credential encryption round-trip.
  <!-- files: apps/core/tests/audit_tests.rs -->
  <!-- purpose: Verify audit system correctness -->
  <!-- requirements: encryption-and-audit 5.1 through 6.2 -->

---

## Phase 6 — Plugin SDK and Contracts

Define the developer-facing contracts for plugin authors: manifest format, action signatures, host function stubs. This phase produces the SDK crate that plugin developers depend on.

> depends: 1.1, 1.2, 4.4
> spec: .odm/spec/plugin-manifest, .odm/spec/plugin-actions, .odm/spec/host-functions

### 6.1 — Plugin Manifest Specification

- [x] Define manifest types in `packages/plugin-system/src/manifest.rs`: `PluginManifest` struct deserialised from `manifest.toml` with sections: `[plugin]` (id, name, version, description, author, license), `[actions]` (map of action_name → ActionDeclaration with description and optional timeout_ms), `[capabilities]` (storage_doc: Vec of read/write/delete, storage_blob: Vec of read/write/delete, http_outbound: Vec of domain strings, events_emit: Vec of event names, events_subscribe: Vec of event names, config_read: bool), `[collections]` (map of collection_name → CollectionDeclaration with schema path, access level: shared/private, extensions: bool, indexes, strict: bool), `[events]` (emit: Vec<String>, subscribe: Vec<String>), `[config]` (schema for runtime configuration validation as JSON Schema reference).
  <!-- files: packages/plugin-system/src/manifest.rs -->
  <!-- purpose: Define the complete manifest format that plugin authors use to declare their requirements -->
  <!-- requirements: plugin-manifest 1.1, 2.1, 3.1, 4.1, 5.1, 6.1 -->

- [x] Implement manifest validation at load time: verify plugin ID follows `[a-z0-9-]+` pattern, version follows semver, all event names in capabilities match events section, all collection names in capabilities match collections section, schema paths point to existing files (relative to plugin directory), no reserved collection names (audit_log, system.*), action timeout defaults to 30s if not specified. Return `Vec<ManifestError>` for all violations found (not just the first).
  <!-- files: packages/plugin-system/src/manifest.rs -->
  <!-- purpose: Catch manifest errors at load time with actionable error messages -->
  <!-- requirements: plugin-manifest 7.1, 7.2, 7.3 -->

- [x] Implement trust model: plugins have a trust level: `first_party` (shipped with Core, capabilities auto-granted) or `third_party` (requires explicit approval in Core config). The trust level is determined by checking if the plugin directory is under Core's built-in plugins path. Third-party plugins whose capabilities have not been approved are loaded but all capability-gated operations are denied at runtime.
  <!-- files: packages/plugin-system/src/manifest.rs -->
  <!-- purpose: Enforce deny-by-default with explicit approval for third-party plugins -->
  <!-- requirements: plugin-manifest 8.1, 8.2 -->

- [x] Write unit tests: parse a valid manifest, parse a manifest with missing required fields, validate event name consistency, validate collection name rules, trust level determination.
  <!-- files: packages/plugin-system/tests/manifest_tests.rs -->
  <!-- purpose: Verify manifest parsing and validation -->
  <!-- requirements: plugin-manifest 1.1 through 8.2 -->

### 6.2 — Plugin Actions and Lifecycle

- [x] Define the plugin action contract in `packages/plugin-sdk-rs/src/traits.rs`: the `#[plugin_action]` macro attribute marks a function as a workflow step. Every action receives `PipelineMessage` as input and returns `Result<PipelineMessage, PluginError>`. The SDK provides a `PluginContext` struct giving typed access to host functions: `ctx.storage().doc_read(...)`, `ctx.storage().doc_write(...)`, `ctx.events().emit(...)`, `ctx.config().read(...)`, `ctx.http().request(...)`.
  <!-- files: packages/plugin-sdk-rs/src/traits.rs, packages/plugin-sdk-rs/src/context.rs -->
  <!-- purpose: Define the standard action signature all plugins implement -->
  <!-- requirements: plugin-actions 1.1, 2.1 -->

- [x] Define optional lifecycle hooks: `init(ctx: &PluginContext) -> Result<(), PluginError>` called once after the plugin is instantiated (before any actions run), and `shutdown(ctx: &PluginContext) -> Result<(), PluginError>` called when Core is shutting down. Both are optional — if not implemented, no-op defaults are used.
  <!-- files: packages/plugin-sdk-rs/src/traits.rs -->
  <!-- purpose: Enable plugins to perform setup and teardown -->
  <!-- requirements: plugin-actions 3.1, 3.2 -->

- [x] Define error handling contract: actions return `Result<PipelineMessage, PluginError>`. A hard failure (`Err`) causes the executor to apply the step's `on_error` strategy. A soft warning is appended to `metadata.warnings` inside the returned `Ok(message)` — the workflow continues but the warning propagates to the final `WorkflowResponse.errors`.
  <!-- files: packages/plugin-sdk-rs/src/error.rs -->
  <!-- purpose: Distinguish between fatal errors and non-fatal warnings -->
  <!-- requirements: plugin-actions 4.1, 4.2 -->

- [x] Document the connector pattern: a connector is a plugin that follows a standard flow — read config → fetch from external API via `ctx.http()` → normalise to CDM types → write documents via `ctx.storage()` → emit completion event via `ctx.events()`. This pattern is documented, not enforced — connectors are regular plugins that follow a convention.
  <!-- files: packages/plugin-sdk-rs/src/lib.rs (module-level docs) -->
  <!-- purpose: Guide connector plugin authors toward a proven pattern -->
  <!-- requirements: plugin-actions 5.1 -->

- [x] Write unit tests: action execution with mock PluginContext, lifecycle hook invocation order, error vs warning distinction, PluginContext method routing.
  <!-- files: packages/plugin-sdk-rs/tests/ -->
  <!-- purpose: Verify plugin action contract -->
  <!-- requirements: plugin-actions 1.1 through 5.1 -->

### 6.3 — Host Functions

- [x] Define all host function signatures in `packages/plugin-sdk-rs/src/wasm_guest.rs` (guest-side stubs) and `packages/plugin-system/src/host_functions.rs` (host-side implementations). Document storage functions: `storage_doc_get(collection: &str, id: &str) -> Result<Value, PluginError>`, `storage_doc_list(collection: &str, query: QueryDescriptor) -> Result<DocumentList, PluginError>`, `storage_doc_count(collection: &str, filters: Option<FilterNode>) -> Result<u64, PluginError>`, `storage_doc_create(collection: &str, document: Value) -> Result<Value, PluginError>`, `storage_doc_update(collection: &str, id: &str, document: Value) -> Result<Value, PluginError>`, `storage_doc_partial_update(collection: &str, id: &str, patch: Value) -> Result<Value, PluginError>`, `storage_doc_batch_create(collection: &str, documents: Vec<Value>) -> Result<Vec<Value>, PluginError>`, `storage_doc_batch_update(collection: &str, updates: Vec<(String, Value)>) -> Result<Vec<Value>, PluginError>`, `storage_doc_delete(collection: &str, id: &str) -> Result<(), PluginError>`.
  <!-- files: packages/plugin-sdk-rs/src/wasm_guest.rs, packages/plugin-system/src/host_functions.rs -->
  <!-- purpose: Define the complete document storage host function interface -->
  <!-- requirements: host-functions 1.1, 2.1, 3.1 -->

- [x] Define blob storage host functions: `storage_blob_store(key: &str, data: &[u8], content_type: &str) -> Result<BlobMeta, PluginError>`, `storage_blob_retrieve(key: &str) -> Result<Vec<u8>, PluginError>`, `storage_blob_delete(key: &str) -> Result<(), PluginError>`. Blob keys are automatically prefixed with the calling plugin's ID to enforce isolation.
  <!-- files: packages/plugin-sdk-rs/src/wasm_guest.rs, packages/plugin-system/src/host_functions.rs -->
  <!-- purpose: Define blob storage host functions with automatic key scoping -->
  <!-- requirements: host-functions 4.1 -->

- [x] Define event host function: `emit_event(name: &str, payload: Option<Value>) -> Result<(), PluginError>`. The host-side implementation validates that the event name is declared in the plugin's manifest, sets the source to the plugin's ID, and publishes to the event bus broadcast channel.
  <!-- files: packages/plugin-sdk-rs/src/wasm_guest.rs, packages/plugin-system/src/host_functions.rs -->
  <!-- purpose: Enable plugins to emit events into the event bus -->
  <!-- requirements: host-functions 5.1 -->

- [x] Define config host function: `config_read(key: &str) -> Result<Option<Value>, PluginError>`. Reads from the plugin's configuration section in Core config. The key is scoped to the plugin — a plugin cannot read another plugin's config.
  <!-- files: packages/plugin-sdk-rs/src/wasm_guest.rs, packages/plugin-system/src/host_functions.rs -->
  <!-- purpose: Provide plugins with access to their own configuration -->
  <!-- requirements: host-functions 6.1 -->

- [x] Define HTTP outbound host function: `http_request(method: &str, url: &str, headers: HashMap<String, String>, body: Option<Vec<u8>>) -> Result<HttpResponse, PluginError>`. The host-side implementation validates the URL domain against the plugin's declared `http_outbound` domains. Requests to undeclared domains are rejected with `PluginError::PermissionDenied`.
  <!-- files: packages/plugin-sdk-rs/src/wasm_guest.rs, packages/plugin-system/src/host_functions.rs -->
  <!-- purpose: Enable controlled outbound HTTP access for plugins -->
  <!-- requirements: host-functions 7.1 -->

- [x] Write integration tests: call each host function from a test WASM plugin, verify correct routing through StorageContext, verify capability enforcement (call without required capability → error), verify blob key scoping.
  <!-- files: packages/plugin-system/tests/host_function_tests.rs -->
  <!-- purpose: Verify end-to-end host function behaviour -->
  <!-- requirements: host-functions 1.1 through 7.1 -->

---

## Phase 7 — Plugin System Runtime

Build the WASM runtime that loads, isolates, and manages plugin lifecycle. This phase implements the Extism integration and capability enforcement.

> depends: 6.1, 6.2, 6.3
> spec: .odm/spec/plugin-system

### 7.1 — Plugin Discovery and Loading

- [x] Implement plugin directory scanning in `packages/plugin-system/src/discovery.rs`: scan a configured directory (default `plugins/`) for subdirectories containing both `plugin.wasm` and `manifest.toml`. Return a list of `DiscoveredPlugin` structs with path, manifest, and wasm binary path. Log a warning for directories missing either file.
  <!-- files: packages/plugin-system/src/discovery.rs -->
  <!-- purpose: Find all installed plugins at startup -->
  <!-- requirements: plugin-system 1.1, 1.2 -->

- [x] Implement manifest loading and validation: read `manifest.toml`, deserialise into `PluginManifest`, run all validation checks defined in Phase 6 WP 6.1. If validation fails, the plugin is skipped (not loaded) and a clear error message is logged.
  <!-- files: packages/plugin-system/src/loader.rs -->
  <!-- purpose: Ensure only valid plugins are loaded -->
  <!-- requirements: plugin-system 1.3, 1.4 -->

### 7.2 — WASM Runtime Integration

- [x] Implement Extism plugin loading in `packages/plugin-system/src/runtime.rs`: create an `ExtismPlugin` instance from the `.wasm` binary. Register all host functions defined in Phase 6 WP 6.3 as Extism host functions. Configure resource limits: 64MB memory default (configurable per-plugin in manifest), 30-second execution timeout (configurable per-action in manifest).
  <!-- files: packages/plugin-system/src/runtime.rs -->
  <!-- purpose: Load WASM plugins into sandboxed execution environments -->
  <!-- requirements: plugin-system 2.1, 2.2 -->

- [x] Implement action invocation: `PluginInstance::call(action: &str, input: PipelineMessage) -> Result<PipelineMessage, PluginError>`. Serialise the PipelineMessage to JSON, pass to the WASM function matching the action name, deserialise the output JSON back to PipelineMessage. Handle WASM traps (memory violations, timeout) as `PluginError::Crash`.
  <!-- files: packages/plugin-system/src/runtime.rs -->
  <!-- purpose: Execute plugin actions with proper serialisation boundary -->
  <!-- requirements: plugin-system 2.3, 2.4 -->

### 7.3 — Capability Enforcement

- [x] Implement capability checking in `packages/plugin-system/src/capability.rs`: before executing any host function, check that the calling plugin's manifest declares the required capability. Maintain a `HashMap<PluginId, HashSet<Capability>>` built from approved manifests at startup. First-party plugins are auto-approved. Third-party plugins require explicit entries in Core config under `approved_plugins: { "plugin-id": ["storage:doc:read", "storage:doc:write", ...] }`.
  <!-- files: packages/plugin-system/src/capability.rs -->
  <!-- purpose: Enforce deny-by-default capability model at runtime -->
  <!-- requirements: plugin-system 3.1, 3.2, 3.3 -->

- [x] Implement collection access enforcement: verify that storage operations target only collections declared in the plugin's manifest. A plugin calling `storage_doc_read("events", ...)` must have `events` in its `[collections]` section. Plugin-scoped collections (`{plugin_id}.private_data`) are automatically allowed for the owning plugin.
  <!-- files: packages/plugin-system/src/capability.rs -->
  <!-- purpose: Restrict plugins to their declared data scope -->
  <!-- requirements: plugin-system 3.4, 3.5 -->

### 7.4 — Plugin Lifecycle Management

- [x] Implement `PluginLifecycleManager` in `packages/plugin-system/src/lifecycle.rs` managing the lifecycle: `Discover → Load → Init → Running → Stop → Unload`. On startup: discover all plugins, load and validate manifests, instantiate WASM modules, call `init` hook. On shutdown: call `shutdown` hook for all running plugins, unload WASM modules. Track plugin state in a `HashMap<PluginId, PluginState>`.
  <!-- files: packages/plugin-system/src/lifecycle.rs -->
  <!-- purpose: Manage plugin lifecycle from discovery to shutdown -->
  <!-- requirements: plugin-system 4.1, 4.2, 4.3 -->

- [x] Implement crash isolation: if a plugin action panics or traps in WASM, catch the error and return `PluginError::Crash`. The plugin instance remains loaded (Extism handles cleanup). The crash does not affect other plugins or Core. Log the crash with plugin ID, action name, and error detail.
  <!-- files: packages/plugin-system/src/runtime.rs -->
  <!-- purpose: Ensure plugin crashes are isolated and recoverable -->
  <!-- requirements: plugin-system 2.5 -->

- [x] Implement the `PluginSystemExecutor` in `packages/plugin-system/src/execute.rs`: the interface the workflow engine uses to call plugin actions. `execute(plugin_id: &str, action: &str, message: PipelineMessage) -> Result<PipelineMessage, PluginError>`. This method resolves the plugin instance, checks capabilities, calls the action, and returns the result.
  <!-- files: packages/plugin-system/src/execute.rs -->
  <!-- purpose: Provide the workflow engine with a single entry point for plugin execution -->
  <!-- requirements: plugin-system 5.1 -->

- [x] Write comprehensive tests: plugin discovery from directory, manifest validation rejection, WASM loading with host functions, action invocation round-trip, capability denial for unapproved operations, crash isolation (plugin trap does not affect caller), lifecycle hook ordering.
  <!-- files: packages/plugin-system/tests/ -->
  <!-- purpose: Verify plugin system correctness end-to-end -->
  <!-- requirements: plugin-system 1.1 through 5.1 -->

---

## Phase 8 — Workflow Engine

Build the pipeline executor, control flow primitives, event bus, trigger system, and scheduler. This is the central orchestration layer.

> depends: 7.4 (PluginSystemExecutor), 1.1 (PipelineMessage), 1.4 (TriggerContext, WorkflowResponse)
> spec: .odm/spec/pipeline-executor, .odm/spec/control-flow, .odm/spec/event-bus, .odm/spec/trigger-system, .odm/spec/scheduler

### 8.1 — Workflow Definition and YAML Loading

- [x] Define `WorkflowDefinition` struct in `packages/workflow-engine/src/definition.rs`: `id: String`, `description: Option<String>`, `mode: ExecutionMode` (Sync, Async), `trigger: TriggerDeclaration` (optional endpoint, event, and schedule fields), `steps: Vec<WorkflowStep>`. Define `WorkflowStep` enum with `Plugin { plugin_id, action, on_error: ErrorStrategy }` and `Condition(ConditionBlock)` variants. Define `ErrorStrategy` with variants `Halt`, `Retry { max_retries: u32, fallback: Option<Box<WorkflowStep>> }`, `Skip`. Default is `Halt`.
  <!-- files: packages/workflow-engine/src/definition.rs -->
  <!-- purpose: Define the workflow data model loaded from YAML -->
  <!-- requirements: pipeline-executor 1.1, control-flow 1.1 -->

- [x] Implement YAML workflow loader: scan a configured directory for `.yaml`/`.yml` files, deserialise each into `WorkflowDefinition`, build an immutable `HashMap<String, WorkflowDefinition>`. Reject startup if two files declare the same `id`. Validate: all plugin references resolve to loaded plugins, all event trigger names follow naming conventions, all cron expressions parse. Validate nesting depth limit: condition blocks contain flat step lists only (no nested conditions).
  <!-- files: packages/workflow-engine/src/loader.rs -->
  <!-- purpose: Load all workflow definitions at startup -->
  <!-- requirements: pipeline-executor 3.1, 3.2, control-flow 4.1 -->

### 8.2 — Pipeline Executor Core

- [x] Implement `WorkflowExecutor` in `packages/workflow-engine/src/executor/mod.rs` with two methods: `async execute(&self, trigger: TriggerContext) -> WorkflowResponse` (sync execution) and `fn spawn(&self, trigger: TriggerContext) -> JobId` (async execution). The executor checks the workflow's `mode` field to determine which path to take. Both methods build an initial `PipelineMessage` from `TriggerContext`.
  <!-- files: packages/workflow-engine/src/executor/mod.rs -->
  <!-- purpose: Provide the public API for workflow execution -->
  <!-- requirements: pipeline-executor 1.1, 1.2 -->

- [x] Implement initial PipelineMessage construction from TriggerContext: for `Endpoint`, the `WorkflowRequest.body` becomes the payload, params/query/identity go into metadata. For `Event`, the event payload becomes the PipelineMessage payload, event name and source go into metadata. For `Schedule`, the payload is empty, metadata has workflow_id and trigger_type "schedule".
  <!-- files: packages/workflow-engine/src/executor/message_builder.rs -->
  <!-- purpose: Build the correct initial message for each trigger type -->
  <!-- requirements: pipeline-executor 2.1, 2.2, 2.3 -->

- [x] Implement sequential step execution: iterate through `WorkflowDefinition.steps`, for each step: clone the current PipelineMessage (pre-step snapshot), call `PluginSystemExecutor::execute(plugin_id, action, message)`, if success replace current message with output and append StepTrace, if failure apply error strategy. Support `WorkflowStep::Condition` by evaluating the condition and recursively executing the appropriate branch.
  <!-- files: packages/workflow-engine/src/executor/runner.rs -->
  <!-- purpose: Execute workflow steps in sequence with error handling -->
  <!-- requirements: pipeline-executor 4.1, 4.2, 4.3, control-flow 1.1, 1.2 -->

- [x] Implement concurrency limiting: use a `tokio::sync::Semaphore` with configurable permits (default 32) to cap concurrent workflow tasks. Both sync and async workflows acquire a permit before execution. If all permits are held, the workflow queues until one is released.
  <!-- files: packages/workflow-engine/src/executor/mod.rs -->
  <!-- purpose: Prevent resource exhaustion from runaway workflows -->
  <!-- requirements: pipeline-executor 4.4 -->

### 8.3 — Control Flow

- [x] Implement condition evaluation in `packages/workflow-engine/src/executor/condition.rs`: `evaluate(condition: &ConditionBlock, message: &PipelineMessage) -> bool`. Resolve the `field` path (dot-separated) into the PipelineMessage payload using a `resolve_field` function. Apply the operator: `Equals` (exact match), `NotEquals`, `Exists` (field present, any value including null), `IsEmpty` (absent, null, empty string, or empty array). If the field path does not exist, the condition evaluates to false (else branch taken). This makes conditions safe by default.
  <!-- files: packages/workflow-engine/src/executor/condition.rs -->
  <!-- purpose: Evaluate conditional branches within workflows -->
  <!-- requirements: control-flow 2.1, 2.2, 2.3, 2.4, 2.5 -->

- [x] Implement branch execution: when a condition block is encountered, evaluate the condition, execute the `then` or `else` step list, and use the branch's final output as the current message for the step after the condition block (branch rejoining).
  <!-- files: packages/workflow-engine/src/executor/runner.rs -->
  <!-- purpose: Handle conditional branching with correct message flow -->
  <!-- requirements: control-flow 5.1, 5.2 -->

- [x] Implement retry strategy: when a step fails with `on_error: retry`, replay the pre-step PipelineMessage clone as input up to `max_retries` times with exponential backoff (base 1s, capped at 30s). If retries are exhausted and a `fallback` step is declared, execute the fallback with the same pre-step message. If the fallback itself fails, halt the workflow.
  <!-- files: packages/workflow-engine/src/executor/error_handler.rs -->
  <!-- purpose: Provide retry-based error recovery -->
  <!-- requirements: control-flow 7.1, 7.2, 7.3 -->

- [x] Implement skip strategy: when a step fails with `on_error: skip`, log the error, pass the pre-step PipelineMessage clone to the next step, and append the error to the response's `warnings` list. The workflow continues with `status: Ok` but the caller sees the degraded execution in `errors`.
  <!-- files: packages/workflow-engine/src/executor/error_handler.rs -->
  <!-- purpose: Enable graceful degradation for non-critical steps -->
  <!-- requirements: control-flow 8.1, 8.2 -->

- [x] Write unit tests: sequential execution with mock plugin, condition evaluation for all four operators, missing field defaults to else, branch rejoining, retry with backoff (mock failing then succeeding), skip with warning propagation, halt on error.
  <!-- files: packages/workflow-engine/tests/executor_tests.rs -->
  <!-- purpose: Verify executor and control flow correctness -->
  <!-- requirements: pipeline-executor 1.1 through 4.4, control-flow 1.1 through 8.2 -->

### 8.4 — Async Job Lifecycle

- [x] Implement `JobRegistry` in `packages/workflow-engine/src/executor/job_registry.rs`: `Arc<RwLock<HashMap<JobId, JobEntry>>>` where `JobEntry` has `status: JobStatus` (InProgress, Completed, Failed), `response: Option<WorkflowResponse>`, `created_at: Instant`. The executor writes to it when spawning/completing async workflows. Transport handlers read from it to poll job status via `GET /api/v1/jobs/:id`.
  <!-- files: packages/workflow-engine/src/executor/job_registry.rs -->
  <!-- purpose: Track async workflow execution state -->
  <!-- requirements: pipeline-executor 5.1, 5.2 -->

- [x] Implement TTL-based cleanup: spawn a background Tokio task that periodically sweeps the registry and removes `Completed`/`Failed` entries older than the configured TTL (default 1 hour). After TTL, the job ID still returns `Completed` with no data attached.
  <!-- files: packages/workflow-engine/src/executor/job_registry.rs -->
  <!-- purpose: Prevent unbounded memory growth from accumulated job records -->
  <!-- requirements: pipeline-executor 5.3 -->

### 8.5 — Event Bus

- [x] Implement the event bus in `packages/workflow-engine/src/event_bus.rs` using a Tokio broadcast channel. Define `Event` struct with `name: String`, `payload: Option<Value>`, `source: String`, `timestamp: DateTime<Utc>`, `depth: u8`. Implement `EventBus::emit(event: Event)` and `EventBus::subscribe() -> Receiver<Event>`. Events are delivered to all subscribers concurrently. No acknowledgement or retry — fire-and-forget.
  <!-- files: packages/workflow-engine/src/event_bus.rs -->
  <!-- purpose: Provide the internal pub/sub mechanism for plugin and system events -->
  <!-- requirements: event-bus 1.1, 1.2, 3.1, 3.2, 3.3 -->

- [x] Implement event naming enforcement: plugin events must be namespaced as `{plugin_id}.{action}.{outcome}`. System events use `system.*` prefix. Validate at emission time that the event name is declared in the emitting plugin's manifest. Reject undeclared events with a logged warning.
  <!-- files: packages/workflow-engine/src/event_bus.rs -->
  <!-- purpose: Enforce structured event naming -->
  <!-- requirements: event-bus 2.1, 2.2, 2.3 -->

- [x] Implement loop prevention via depth counter: every event carries a `depth` field starting at 0 for root events. When a workflow triggered by an event emits a new event, the child event's depth is `parent_depth + 1`. Events exceeding max depth (default 8, configurable) are dropped with a warning log.
  <!-- files: packages/workflow-engine/src/event_bus.rs -->
  <!-- purpose: Prevent infinite event cascades -->
  <!-- requirements: event-bus 5.1, 5.2, 5.3 -->

- [x] Implement system events: emit `system.startup` after Core initialisation, `system.plugin.loaded` / `system.plugin.failed` during plugin loading, `system.workflow.completed` / `system.workflow.failed` after workflow execution. These use the same event bus — no separate channel.
  <!-- files: packages/workflow-engine/src/event_bus.rs -->
  <!-- purpose: Enable reactive patterns based on system lifecycle -->
  <!-- requirements: event-bus 4.1, 4.2, 4.3, 4.4, 4.5 -->

### 8.6 — Trigger System

- [x] Implement the trigger registry in `packages/workflow-engine/src/triggers/mod.rs`: at startup, scan all loaded `WorkflowDefinition` entries and register triggers. Build three maps: endpoint triggers (`HashMap<(Method, Path), WorkflowId>` — one-to-one), event triggers (`HashMap<EventName, Vec<WorkflowId>>` — one-to-many), schedule triggers (forwarded to the Scheduler). Validate: no two workflows claim the same endpoint trigger, every endpoint trigger references a route that exists in the router config, event names follow naming conventions.
  <!-- files: packages/workflow-engine/src/triggers/mod.rs -->
  <!-- purpose: Resolve incoming signals to workflow executions -->
  <!-- requirements: trigger-system 1.1, 2.1, 2.2, 3.1, 4.1, 4.2 -->

- [x] Implement trigger resolution: `resolve_endpoint(method, path) -> Option<WorkflowId>`, `resolve_event(event_name) -> Vec<WorkflowId>`, schedule resolution handled by the Scheduler. For events, all matching workflows are spawned independently and concurrently. For endpoints, exactly one workflow is returned.
  <!-- files: packages/workflow-engine/src/triggers/resolver.rs -->
  <!-- purpose: Map signals to workflow IDs for execution -->
  <!-- requirements: trigger-system 5.1, 5.2, 5.3 -->

- [x] Wire event triggers to the event bus: subscribe to the broadcast channel and for each received event, look up matching workflows in the trigger map and spawn each via the executor.
  <!-- files: packages/workflow-engine/src/triggers/event_listener.rs -->
  <!-- purpose: Connect event bus to workflow execution -->
  <!-- requirements: trigger-system 3.1, 3.2 -->

### 8.7 — Scheduler

- [x] Implement `ScheduleEntry` and `ScheduleRegistry` in `packages/workflow-engine/src/scheduler/`: parse cron expressions from workflow trigger declarations, build an immutable registry at startup. Invalid cron expressions cause a startup failure with a clear error message.
  <!-- files: packages/workflow-engine/src/scheduler/types.rs, packages/workflow-engine/src/scheduler/registry.rs -->
  <!-- purpose: Register all schedule triggers at startup -->
  <!-- requirements: scheduler 1.1, 6.1, 6.2 -->

- [x] Implement the scheduler loop in `packages/workflow-engine/src/scheduler/mod.rs`: a single Tokio task that collects all schedule entries, computes next fire times from UTC now, sleeps until the earliest, fires all due workflows, recalculates, and repeats. Use the `cron` crate for expression parsing and next-fire calculation. All times are UTC — no timezone support in v1.
  <!-- files: packages/workflow-engine/src/scheduler/mod.rs -->
  <!-- purpose: Fire workflows on cron-based schedules -->
  <!-- requirements: scheduler 3.1, 3.2, 3.3, 2.1, 2.2 -->

- [x] Implement missed tick handling: if Core was offline when a tick was due, the scheduler calculates the next future fire time on restart — no catch-up, no persistence of last-run timestamps.
  <!-- files: packages/workflow-engine/src/scheduler/mod.rs -->
  <!-- purpose: Handle graceful recovery after downtime -->
  <!-- requirements: scheduler 4.1, 4.2 -->

- [x] Implement overlap prevention: before spawning a scheduled workflow, check the `JobRegistry` for an existing `InProgress` instance of the same workflow. If one exists, skip the tick and log at debug level.
  <!-- files: packages/workflow-engine/src/scheduler/mod.rs -->
  <!-- purpose: Prevent resource pile-up from overlapping scheduled executions -->
  <!-- requirements: scheduler 5.1, 5.2 -->

- [x] Write comprehensive tests: cron parsing and next-fire calculation, registry construction with valid/invalid expressions, overlap skip logic, PipelineMessage shape for schedule triggers, event bus integration, trigger resolution for all three types.
  <!-- files: packages/workflow-engine/tests/ -->
  <!-- purpose: Verify workflow engine orchestration correctness -->
  <!-- requirements: scheduler 1.1 through 7.4, trigger-system 1.1 through 5.3, event-bus 1.1 through 5.3 -->

---

## Phase 9 — Transport Layer

Build the HTTP entry point: listener configuration, route merging, REST and GraphQL handlers, auth middleware, and protocol translation.

> depends: 8.2 (WorkflowExecutor), 1.4 (WorkflowRequest/WorkflowResponse)
> spec: .odm/spec/transport-layer

### 9.1 — Transport Configuration

- [x] Define config structs in `packages/transport-rest/src/config/`: `ListenerConfig` with fields: `binding: String`, `port: u16`, `address: String`, `tls: Option<TlsConfig>`, `auth: AuthConfig`, `handlers: Vec<HandlerConfig>`. `HandlerConfig` with `handler_type: HandlerType` (Rest, GraphQL) and `routes: Vec<RouteConfig>`. `RouteConfig` with `method: HttpMethod`, `path: String`, `workflow: String`, `public: bool`. `TlsConfig` with `cert: PathBuf`, `key: PathBuf`. `AuthConfig` with `verify: AuthMode` (Token, None).
  <!-- files: packages/transport-rest/src/config/types.rs -->
  <!-- purpose: Define the listener configuration model -->
  <!-- requirements: transport-layer 1.1, 1.2, 1.3 -->

- [x] Implement config validation: verify port is in valid range, TLS cert/key files exist when TLS is configured, no duplicate routes (same method + path), REST routes start with `/api/`, GraphQL routes start with `/graphql`. Return all violations (not just the first) with human-readable error messages.
  <!-- files: packages/transport-rest/src/config/validation.rs -->
  <!-- purpose: Catch config errors at startup -->
  <!-- requirements: transport-layer 1.1, 4.1, 4.2, 4.3, 15.1 -->

- [x] Implement default config generation: on first run, generate a default `listeners.yaml` with generic CRUD routes (`collection.list`, `collection.get`, `collection.create`, `collection.update`, `collection.delete`), GraphQL endpoint, and public health check. Write to the config directory.
  <!-- files: packages/transport-rest/src/config/defaults.rs -->
  <!-- purpose: Provide a working out-of-the-box configuration -->
  <!-- requirements: transport-layer 2.1, 2.2, 2.3, 2.4 -->

- [x] Write unit tests: config parsing from YAML, validation errors for invalid configs, default generation produces parseable config.
  <!-- files: packages/transport-rest/src/config/tests.rs -->
  <!-- purpose: Verify config infrastructure -->
  <!-- requirements: transport-layer 1.1, 2.1 -->

### 9.2 — Router and Route Merging

- [x] Implement route merging in `packages/transport-rest/src/router/`: combine routes from the listener config with routes from plugin manifests into a single route list. Plugin manifest routes are additive — they cannot override config routes. Detect and reject conflicts (same method + path from two sources).
  <!-- files: packages/transport-rest/src/router/merge.rs -->
  <!-- purpose: Unify config and plugin routes into a single router -->
  <!-- requirements: transport-layer 3.1, 3.2, 3.4, 5.1 -->

- [x] Build the immutable Axum `Router` from the merged route list. Each route extracts path parameters (`:collection`, `:id`) into `HashMap<String, String>` and dispatches to the appropriate handler (REST or GraphQL) based on handler type. The router is built once at startup and never modified.
  <!-- files: packages/transport-rest/src/router/build.rs -->
  <!-- purpose: Construct the runtime router from validated routes -->
  <!-- requirements: transport-layer 5.1, 5.2, 5.3, 5.4 -->

- [x] Write unit tests: route merging, collision detection, path parameter extraction, namespace validation.
  <!-- files: packages/transport-rest/src/router/tests.rs -->
  <!-- purpose: Verify router correctness -->
  <!-- requirements: transport-layer 3.1, 4.1, 5.1 -->

### 9.3 — REST Handler

- [x] Implement REST request-to-WorkflowRequest translation: extract `workflow` from route config, `identity` from auth middleware extension, `params` from path parameters, `query` from URL query string, `body` from JSON request body, `meta` with generated request ID and timestamp.
  <!-- files: packages/transport-rest/src/handlers/rest.rs -->
  <!-- purpose: Translate HTTP requests into the workflow engine contract -->
  <!-- requirements: transport-layer 7.1 -->

- [x] Implement WorkflowResponse-to-HTTP translation: map `WorkflowStatus` to HTTP status codes (Ok→200, Created→201, NotFound→404, Denied→403, Invalid→400, Error→500). Wrap success data in `{ "data": ... }`, errors in `{ "error": { "code": "...", "message": "..." } }`.
  <!-- files: packages/transport-rest/src/handlers/rest.rs -->
  <!-- purpose: Translate workflow responses back to HTTP -->
  <!-- requirements: transport-layer 7.2, 7.3, 7.4 -->

- [x] Write unit tests: request translation with various HTTP methods, response translation for all status variants, JSON body parsing, path parameter extraction.
  <!-- files: packages/transport-rest/src/handlers/rest_tests.rs -->
  <!-- purpose: Verify REST handler correctness -->
  <!-- requirements: transport-layer 7.1, 7.2 -->

### 9.4 — GraphQL Handler

- [x] Implement GraphQL request-to-WorkflowRequest translation: parse the GraphQL query, set workflow to `graphql.query`, flatten query arguments (limit, offset, filters) into the `query` field, set body to the raw GraphQL query string.
  <!-- files: packages/transport-graphql/src/handler.rs -->
  <!-- purpose: Translate GraphQL requests into the workflow engine contract -->
  <!-- requirements: transport-layer 8.1 -->

- [x] Implement GraphQL schema generation at startup: for each plugin that declares a schema in its manifest, generate a GraphQL type with fields matching the schema's properties. Collections without a declared schema are not queryable via GraphQL. Generate query resolvers (list, get) and mutation resolvers (create, update, delete) for each typed collection.
  <!-- files: packages/transport-graphql/src/schema.rs -->
  <!-- purpose: Auto-generate GraphQL schema from plugin declarations -->
  <!-- requirements: transport-layer 9.1, 9.2, 9.3, 9.4 -->

- [x] Write unit tests: GraphQL query parsing, argument flattening, schema generation from mock manifests, response shape verification.
  <!-- files: packages/transport-graphql/tests/ -->
  <!-- purpose: Verify GraphQL handler correctness -->
  <!-- requirements: transport-layer 8.1, 9.1 -->

### 9.5 — Middleware Stack

- [x] Implement CORS middleware: permissive when bound to `127.0.0.1` (allow all origins), strict when bound to `0.0.0.0` (configured allowed origins only). Use `tower-http::CorsLayer`.
  <!-- files: packages/transport-rest/src/middleware/cors.rs -->
  <!-- purpose: Handle cross-origin requests appropriately based on exposure -->
  <!-- requirements: transport-layer 12.1, 12.2, 12.3 -->

- [x] Implement auth middleware: validate tokens via Pocket ID (OIDC). On success, insert `Extension<Identity>` into the request. On failure, return 401. Routes marked `public: true` bypass auth entirely. The middleware reads from the OIDC provider's JWKS endpoint to validate JWTs.
  <!-- files: packages/transport-rest/src/middleware/auth.rs -->
  <!-- purpose: Authenticate requests at the transport boundary -->
  <!-- requirements: transport-layer 13.1, 13.2, 13.3, 13.4, 14.1, 14.2 -->

- [x] Implement structured JSON logging middleware: log every request with method, path, status code, and duration as structured JSON. Use `tracing` with `tracing-subscriber`'s JSON formatter.
  <!-- files: packages/transport-rest/src/middleware/logging.rs -->
  <!-- purpose: Provide operational request visibility -->
  <!-- requirements: transport-layer 11.1, 11.2 -->

- [x] Implement error handling middleware: catch panics and unhandled errors, translate them into a consistent JSON error shape. Never expose internal error details to the client.
  <!-- files: packages/transport-rest/src/middleware/error.rs -->
  <!-- purpose: Ensure all errors produce well-formed responses -->
  <!-- requirements: transport-layer 11.1, 11.3 -->

### 9.6 — Listener and TLS

- [x] Implement listener socket binding: bind to the configured address and port, apply the middleware stack (TLS → CORS → Auth → Logging → Error), mount the Axum router. When TLS is configured, use `tokio-rustls` to terminate TLS. Log a startup warning when bound to `0.0.0.0`.
  <!-- files: packages/transport-rest/src/listener.rs -->
  <!-- purpose: Start the HTTP server -->
  <!-- requirements: transport-layer 15.1, 15.2, 15.3, 16.1, 16.2 -->

- [x] Write integration tests: listener startup with and without TLS, REST and GraphQL requests to the same listener, public route bypasses auth, authenticated route rejects invalid token.
  <!-- files: packages/transport-rest/tests/ -->
  <!-- purpose: Verify end-to-end transport behaviour -->
  <!-- requirements: transport-layer 10.1, 10.2, 15.1, 16.1 -->

### 9.7 — Transport Equivalence Verification

- [x] Write integration test proving REST and GraphQL produce identical results for the same workflow: issue a `collection.list` request via REST and GraphQL, compare the returned data arrays. Both should dispatch through the same system workflow and return the same records.
  <!-- files: packages/transport-rest/tests/transport_equivalence.rs -->
  <!-- purpose: Verify the design principle that both transports are interchangeable -->
  <!-- requirements: transport-layer 10.1, 10.2 -->

---

## Phase 10 — Integration, Migration, and Verification

Wire all layers together into the Core binary, create system workflows, generate default configuration, migrate existing functionality, and run end-to-end tests.

> depends: all previous phases
> spec: all specs (integration)

### 10.1 — Core Binary Startup Orchestration

- [x] Rewrite `apps/core/src/main.rs` startup sequence to follow the four-layer initialisation order: (1) Load configuration (config.yaml, listeners.yaml, storage.toml), (2) Initialise crypto and derive encryption key from passphrase, (3) Initialise storage — create StorageRouter with SQLite document adapter and filesystem blob adapter, run health checks, (4) Initialise schema registry — load CDM schemas and plugin-declared schemas, (5) Initialise StorageContext with references to StorageRouter, SchemaRegistry, and EventBus, (6) Discover and load plugins — scan directory, validate manifests, instantiate WASM modules with host functions, call init hooks, (7) Load workflow definitions from YAML, (8) Build trigger registry — register endpoint, event, and schedule triggers, (9) Build transport — merge routes, build Axum router, apply middleware stack, (10) Start listener, scheduler, and event bus. Each step depends on the previous and fails fast with a clear error message.
  <!-- files: apps/core/src/main.rs -->
  <!-- purpose: Wire all layers together in the correct initialisation order -->
  <!-- requirements: all specs — integration -->

- [x] Implement graceful shutdown: on SIGINT/SIGTERM, (1) stop accepting new requests, (2) wait for in-flight workflows to complete (with timeout), (3) call shutdown hooks on all plugins, (4) close storage connections, (5) exit. Use a Tokio signal handler and a shared shutdown channel.
  <!-- files: apps/core/src/shutdown.rs -->
  <!-- purpose: Clean shutdown without data loss -->
  <!-- requirements: plugin-system 4.3 -->

### 10.2 — System Workflows

- [x] Create default system workflow YAML files in `workflows/`: `collection.list.yaml` (pass-through to storage_doc_list), `collection.get.yaml` (pass-through to storage_doc_get), `collection.create.yaml` (pass-through to storage_doc_create with status_hint Created), `collection.update.yaml` (pass-through to storage_doc_update), `collection.delete.yaml` (pass-through to storage_doc_delete), `graphql.query.yaml` (GraphQL query resolution), `system.health.yaml` (aggregate storage and plugin health). These are real workflow definitions that the user can edit — not hardcoded behaviour.
  <!-- files: workflows/*.yaml -->
  <!-- purpose: Provide working out-of-the-box CRUD and system workflows -->
  <!-- requirements: workflow-engine-contract 5.1, transport-layer 2.1 -->

- [x] Create a built-in "pass-through" plugin (or system step handler) that implements the generic CRUD operations by calling StorageContext directly. This plugin is the default executor for system workflows. It is a first-party plugin with auto-granted capabilities.
  <!-- files: plugins/engine/system-crud/ or packages/workflow-engine/src/system_steps.rs -->
  <!-- purpose: Provide the default implementation for system workflows -->
  <!-- requirements: workflow-engine-contract 5.1 -->

### 10.3 — Default Configuration Generation

- [ ] Implement first-run configuration generation: when Core starts with no existing config directory, generate default files: `config.yaml` (core settings, auth, storage, plugin, network sections), `listeners.yaml` (default REST+GraphQL listener on localhost:8080), `storage.toml` (SQLite document adapter, filesystem blob adapter, default paths and timeouts). Write to the configured data directory. On subsequent starts, load existing config without overwriting.
  <!-- files: apps/core/src/config.rs -->
  <!-- purpose: Zero-configuration first run experience -->
  <!-- requirements: transport-layer 2.1, storage-router 2.1 -->

### 10.4 — Migration from Current Architecture

- [ ] Audit existing `apps/core/src/` modules and identify code that maps to the new architecture: (a) code that should move into `packages/` crates (storage, crypto, auth, plugin loading, workflow execution), (b) code that is replaced by the new architecture (monolithic route handlers, inline storage calls, direct plugin invocations), (c) code that remains in `apps/core/` (startup orchestration, config, shutdown). Create a migration checklist mapping each existing module to its destination.
  <!-- files: (analysis task — no file output) -->
  <!-- purpose: Plan the migration of existing code into the new crate structure -->
  <!-- requirements: all specs -->

- [ ] Migrate storage code: move `sqlite_storage.rs`, `pg_storage.rs`, and `storage_migration.rs` into `packages/storage-sqlite/`, refactoring them to implement the `DocumentStorageAdapter` trait. Preserve all existing tests and verify they pass against the new trait implementation.
  <!-- files: packages/storage-sqlite/ -->
  <!-- purpose: Migrate existing storage to the new adapter trait -->
  <!-- requirements: document-storage-adapter 1.1 -->

- [ ] Migrate crypto code: consolidate `crypto.rs`, `credential_store.rs`, `credential_bridge.rs`, and `rekey.rs` into `packages/crypto/`, ensuring all functions use AES-256-GCM (not XOR) and Argon2id key derivation. Remove any remaining legacy crypto patterns.
  <!-- files: packages/crypto/ -->
  <!-- purpose: Unify crypto under the shared crate -->
  <!-- requirements: encryption-and-audit 1.1, 2.1 -->

- [ ] Migrate auth code: move `auth/` module into `packages/auth/`, implementing the OIDC token validation used by the transport middleware. Preserve WebAuthn support as an optional auth mode.
  <!-- files: packages/auth/ -->
  <!-- purpose: Migrate auth to a standalone crate -->
  <!-- requirements: transport-layer 13.1 -->

- [ ] Migrate plugin loading: move `plugin_loader.rs`, `manifest.rs`, `wasm_runtime.rs`, `wasm_adapter.rs` into `packages/plugin-system/`, refactoring to use the new manifest types and lifecycle management. Ensure all existing plugins continue to load and function.
  <!-- files: packages/plugin-system/ -->
  <!-- purpose: Migrate plugin infrastructure to the new crate -->
  <!-- requirements: plugin-system 1.1 -->

- [ ] Migrate workflow engine: move `message_bus.rs` into `packages/workflow-engine/src/event_bus.rs`. Implement the pipeline executor and scheduler as new code (these do not exist in the current monolith). Route handlers in `apps/core/src/routes/` become thin dispatchers that build WorkflowRequests and call the executor.
  <!-- files: packages/workflow-engine/ -->
  <!-- purpose: Implement the workflow engine layer -->
  <!-- requirements: pipeline-executor 1.1, event-bus 1.1, scheduler 1.1 -->

### 10.5 — First-Party Plugin Migration

- [ ] Migrate all 10 first-party plugins to the new SDK: update each plugin to use `#[plugin_action]` signatures, `PipelineMessage` input/output, `PluginContext` for host function access, and `manifest.toml` for capability declarations. Verify each plugin compiles to `wasm32-wasip1` and loads correctly via the new plugin system. Plugins to migrate: `connector-email`, `connector-calendar`, `connector-contacts`, `connector-filesystem`, `webhook-sender`, `webhook-receiver`, `search-indexer`, `backup`, `api-caldav`, `api-carddav`.
  <!-- files: plugins/engine/*/ -->
  <!-- purpose: Migrate all existing plugins to the new architecture -->
  <!-- requirements: plugin-actions 1.1, plugin-manifest 1.1 -->

### 10.6 — End-to-End Testing

- [ ] Write end-to-end integration tests that exercise the full request path: HTTP request → transport → workflow engine → plugin step → storage → response. Test scenarios: (a) REST CRUD operations on a CDM collection, (b) GraphQL query with filters, (c) event-triggered workflow that writes to storage, (d) scheduled workflow execution, (e) plugin with multiple capabilities accessing storage and HTTP, (f) error handling — plugin failure with retry and skip strategies, (g) concurrent workflow execution up to the concurrency limit.
  <!-- files: tests/integration/ -->
  <!-- purpose: Verify the complete four-layer pipeline end-to-end -->
  <!-- requirements: all specs — integration -->

- [ ] Write transport equivalence test: verify that REST and GraphQL return identical results for the same operations. Issue `collection.list`, `collection.create`, `collection.get`, `collection.update`, `collection.delete` via both transports and compare responses.
  <!-- files: tests/integration/transport_equivalence.rs -->
  <!-- purpose: Verify both transports are interchangeable -->
  <!-- requirements: transport-layer 10.1 -->

- [ ] Write security tests: (a) unauthenticated request to protected route returns 401, (b) plugin without storage capability is denied, (c) plugin cannot access undeclared collection, (d) blob key scoping prevents cross-plugin access, (e) extension namespace isolation prevents cross-plugin writes, (f) event depth limit prevents infinite loops.
  <!-- files: tests/integration/security_tests.rs -->
  <!-- purpose: Verify security properties of the architecture -->
  <!-- requirements: plugin-system 3.1, storage-context 2.1, event-bus 5.1 -->

### 10.7 — Admin Panel Integration

- [ ] Update the existing admin panel (apps/admin/) to work with the new architecture: update API client to use the new transport endpoints, update config types to match the new configuration structure (listeners.yaml, storage.toml), add plugin management UI showing loaded plugins with their manifest declarations and capability approval status.
  <!-- files: apps/admin/src/ -->
  <!-- purpose: Ensure the admin UI works with the new architecture -->
  <!-- requirements: transport-layer 17.1 -->

### 10.8 — Documentation and Verification

- [ ] Update ARCHITECTURE.md to reflect the final implemented state. Verify all crate boundaries, public APIs, and dependency directions match the documentation. Run `cargo doc --workspace` and verify all public types have documentation.
  <!-- files: ARCHITECTURE.md -->
  <!-- purpose: Keep documentation in sync with implementation -->
  <!-- requirements: all specs -->

- [ ] Run the complete test suite: `cargo test --workspace`, verify all tests pass. Run `cargo clippy --workspace` with no warnings. Run `cargo build --release` to verify release builds succeed.
  <!-- files: (verification task — no file output) -->
  <!-- purpose: Final verification that the migration is complete and correct -->
  <!-- requirements: all specs -->

---

## Summary

- **Phases:** 10
- **Work Packages:** 54
- **Specs Covered:** 20 (pipeline-message, cdm-specification, workflow-engine-contract, schema-and-validation, schema-versioning-rules, document-storage-adapter, blob-storage-adapter, storage-router, storage-context, encryption-and-audit, plugin-manifest, plugin-actions, host-functions, plugin-system, pipeline-executor, control-flow, event-bus, trigger-system, scheduler, transport-layer)

Phase dependency chain (longest path): Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 7 → Phase 8 → Phase 9 → Phase 10

Parallel opportunities:
- Phase 2 (Schema) and Phase 3 (Storage Traits) can run in parallel after Phase 1
- Phase 5 (Encryption) and Phase 6 (Plugin SDK) can run in parallel after Phase 4
- Phase 8 (Workflow Engine) and Phase 9 (Transport) can start as soon as Phase 7 completes
