<!--
domain: document-storage-adapter
updated: 2026-03-28
-->

# Document Storage Adapter Spec

## Overview

This spec defines the `DocumentStorageAdapter` trait that all storage backends must implement. The trait provides a uniform interface for CRUD operations, batch operations, transactions, change watching, schema migration, health reporting, and capability negotiation. Adapters are pluggable — Core selects a concrete adapter at startup based on configuration, and all upstream code (StorageContext, workflow engine, plugins) interacts solely through this trait.

## Goals

- Uniform storage interface — a single async trait covers all document operations, enabling backend-agnostic code throughout Core
- Batch and transactional safety — batch operations are atomic, and transactions provide multi-step consistency guarantees
- Capability negotiation — adapters declare what they support; Core gracefully degrades or refuses startup for missing required features
- Safe schema migration — additive changes apply automatically; breaking changes are rejected rather than silently corrupting data
- Change observation — adapters that support native change detection emit events via a watch stream; others rely on StorageContext write-path emission

## User Stories

- As a Core developer, I want a single trait defining all storage operations so that I can swap backends without changing upstream code.
- As a plugin author, I want CRUD, list, and count operations exposed through StorageContext so that I can persist and query data without knowing the underlying database.
- As a Core developer, I want batch operations to be atomic so that partial writes never leave data in an inconsistent state.
- As a Core developer, I want transactions so that multi-step operations either fully commit or fully roll back.
- As a workflow engine developer, I want storage errors to map to workflow fault states so that workflows handle failures correctly.
- As a Core developer, I want adapters to report health so that the system can detect degraded or failing storage.
- As a Core developer, I want adapters to declare capabilities so that Core can disable unsupported features or refuse to start when required features are missing.

## Functional Requirements

- The system must define a `DocumentStorageAdapter` async trait with get, create, update, partial_update, delete, list, count, batch_create, batch_update, batch_delete, transaction, watch, migrate, health, and capabilities methods.
- The system must define supporting data structures: `QueryDescriptor`, `FilterNode`, `FilterOperator`, `SortField`, `Pagination`, `DocumentList`, `TransactionHandle`, `ChangeEvent`, `CollectionDescriptor`, `FieldDescriptor`, `FieldType`, `HealthReport`, `AdapterCapabilities`, and `StorageError`.
- All batch operations must be atomic — all succeed or all fail with no partial writes.
- The `migrate` method must be idempotent, additive, and safe (rejecting breaking changes with `SchemaConflict`).
- Storage errors must map to workflow fault states with appropriate retryable flags.
- Adapters must declare capabilities; Core must enforce fallback behaviour for unsupported capabilities.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
