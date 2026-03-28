---
title: Adapter SDK Outline
type: adr
created: 2026-03-28
status: draft
tags:
  - architecture
  - adapter-sdk
  - core
---

# Adapter SDK Outline

## Scope

The Adapter SDK defines the contract for implementing storage adapters — the backend engines that persist data in Life Engine. Unlike workflow plugins (WASM modules), adapters are native Rust trait implementations compiled into Core. They operate at a different trust level: adapters need raw filesystem and network access that WASM sandboxing cannot provide.

The Adapter SDK provides the trait definitions, shared types, error model, and testing harness needed to implement a new document or blob storage adapter.

## What the SDK Provides

- **Trait definitions** — `DocumentStorageAdapter` and `BlobStorageAdapter` async traits with all required methods.
- **Shared types** — `Document`, `QueryDescriptor`, `FilterNode`, `BlobMeta`, `CollectionDescriptor`, `HealthReport`, `StorageError`, and all supporting enums and structs.
- **Capability structs** — `AdapterCapabilities` and `BlobAdapterCapabilities` for declaring what the adapter supports.
- **Test harness** — A conformance test suite that validates any adapter implementation against the full trait contract. Run it against your adapter to ensure correctness before registration.
- **Registration hook** — A function to register the adapter in Core's `AdapterRegistry` so it can be selected in `storage.toml`.

## What the SDK Does Not Provide

- **StorageContext** — That sits above adapters. Adapters never see permission checks, schema validation, or audit events.
- **StorageRouter** — Routing, timeouts, and metrics are handled above the adapter.
- **WASM runtime** — Adapters are not WASM. They are compiled Rust linked into the Core binary.
- **Migration strategy** — The trait defines the `migrate` interface. The adapter decides how to implement it for its backend.

## Adapter Types

Two independent traits, two independent adapters:

- **Document storage adapter** — Handles structured data (JSON documents in named collections). Must implement `DocumentStorageAdapter`. See [[document-adapter]].
- **Blob storage adapter** — Handles binary content with self-contained metadata. Must implement `BlobStorageAdapter`. See [[blob-adapter]].

Each adapter is selected independently in `storage.toml`. The document adapter and blob adapter can be different backends (e.g., Postgres for documents, S3 for blobs).

## Adapter Model

Adapters are native Rust code, not plugins. The trust boundary is different:

- Adapters have unrestricted access to the filesystem, network, and OS
- Adapters are compiled into the Core binary (v1 uses a static registry)
- Adapters are responsible for their own connection management, pooling, and error recovery
- Adapters trust what they receive from `StorageContext` — validation happens above them

Future versions may support external adapters loaded from shared libraries (`.so`/`.dylib`), but v1 uses compile-time registration only.

## Design Principles

- **Minimal contract** — The trait asks for the least possible. Optional capabilities (indexing, encryption, native watch, cursors, text search) are declared, not required. Fallback behaviour for missing capabilities is handled by `StorageContext` or `StorageRouter`.
- **Backend-agnostic types** — All types above the trait are backend-agnostic. A `QueryDescriptor` works identically whether the adapter is SQLite, Postgres, or a cloud API.
- **Idempotent lifecycle** — `migrate` runs every startup and is idempotent. `health` is safe to call at any time.
- **Fail fast** — Adapters return errors immediately. No retries at the adapter level. The workflow layer decides retry strategy.
- **Self-contained** — Each adapter manages its own state. No shared state between adapters. The blob adapter does not depend on the document adapter or vice versa.

## Components

- [[document-adapter]] — Full guide to implementing `DocumentStorageAdapter`
- [[blob-adapter]] — Full guide to implementing `BlobStorageAdapter`

## Registration

V1 uses a static registry. To add a new adapter:

1. Implement the trait
2. Add it to the `AdapterRegistry` in Core's initialisation code
3. Reference it by name in `storage.toml`

```rust
registry.register_document("postgres", Box::new(PostgresDocumentAdapter::new()));
registry.register_blob("s3", Box::new(S3BlobAdapter::new()));
```

The adapter name in the registry must match the `adapter` field in `storage.toml`:

```toml
[document]
adapter = "postgres"

[blob]
adapter = "s3"
```

## Startup Sequence

When Core starts, each adapter goes through:

1. **Initialisation** — The adapter receives its configuration from `storage.toml` (connection strings, paths, etc.)
2. **Capability validation** — Core checks that the adapter's reported capabilities meet the `require.capabilities` from config
3. **Migration** — `migrate` is called for every known collection. Additive changes are applied automatically.
4. **Health check** — `health` must return `Healthy` or Core refuses to start

See [[architecture/core/design/data-layer/storage-router#Startup Sequence]] for the full sequence.
