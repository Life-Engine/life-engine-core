---
title: "ADR-010: Native Rust Storage Adapters behind Trait Abstractions"
type: adr
created: 2026-03-28
status: active
---

# ADR-010: Native Rust Storage Adapters behind Trait Abstractions

## Status

Accepted

## Context

Life Engine's data layer must support multiple storage backends — SQLite for self-hosters who want a single-file database, Postgres for power users with existing infrastructure, local filesystem for binary files, S3 for cloud blob storage. The abstraction must allow backend swaps without changing any code above the adapter.

A key design question is whether storage adapters should be WASM plugins (like workflow plugins) or native Rust code compiled into Core. WASM would provide sandboxing and language-agnosticism, but storage adapters need raw filesystem and network access that WASM sandboxing cannot efficiently provide. They also operate at a different trust level — an adapter manages all user data, so it is inherently trusted.

Additionally, the system stores two fundamentally different kinds of data — structured documents (JSON in collections) and binary blobs (files, images, attachments). These have different access patterns, query requirements, and backend characteristics. The question is whether to unify them behind a single abstraction or separate them.

## Decision

Storage adapters are native Rust trait implementations compiled into Core. They are not WASM plugins. Two independent traits define the adapter contract:

- `DocumentStorageAdapter` — Handles structured data (JSON documents in named collections). Supports CRUD, queries with filtering/sorting/pagination, batch operations, transactions, collection migration, and health checks.
- `BlobStorageAdapter` — Handles binary content with self-contained metadata. Supports upload, download, delete, list, and metadata retrieval. Blobs are self-contained — the blob adapter manages its own internal metadata without depending on the document adapter.

Each adapter is selected independently in `storage.toml`. The document adapter and blob adapter can be different backends (e.g., SQLite for documents, S3 for blobs).

V1 ships two built-in adapters:

- **SQLite/SQLCipher** as the document storage adapter
- **Local filesystem** as the blob storage adapter

Adapters declare their capabilities through `AdapterCapabilities` and `BlobAdapterCapabilities` structs. Optional capabilities (indexing, encryption, native watch, cursors, full-text search) are declared, not required. `StorageContext` and `StorageRouter` provide fallback behaviour for missing capabilities.

V1 uses a static registry — adapters are registered in Core's initialisation code and selected by name in config. Future versions may support external adapters loaded from shared libraries.

Above the adapters, two mediating components handle cross-cutting concerns:

- `StorageContext` — Enforces permissions, validates schemas, scopes collections, and emits audit events. This is the API surface that plugins interact with via host functions.
- `StorageRouter` — Routes operations to the active adapter, enforces timeouts, and emits metrics.

## Consequences

Positive consequences:

- Adapters have full access to filesystem and network APIs. No WASM overhead on the storage hot path.
- The trait abstraction means switching backends is a config change, not a code change. Everything above the adapter works identically regardless of which backend is active.
- Separating document and blob traits means each can evolve independently and use the most appropriate backend for its data type.
- The capability declaration model means v1 adapters can implement a pragmatic subset while the trait signatures accommodate future needs.
- `StorageContext` provides a single enforcement point for permissions, validation, and auditing — adapters trust what they receive and focus purely on storage mechanics.

Negative consequences:

- Native adapters must be written in Rust and compiled into Core. Third-party adapters cannot be distributed as standalone packages in v1 — they require recompiling Core.
- Two independent traits mean two independent adapters to implement, test, and maintain for each new backend. A Postgres adapter requires a document implementation and (optionally) a separate blob implementation.
- The `StorageRouter` adds a layer of indirection between `StorageContext` and the adapter. This is a small performance cost paid for timeout enforcement and metrics.
- Cross-adapter transactions are not supported in v1. A workflow that writes to both document and blob storage cannot do so atomically.
