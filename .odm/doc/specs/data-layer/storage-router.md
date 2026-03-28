---
title: Storage Router Specification
type: reference
created: 2026-03-28
status: active
tags:
  - storage
  - routing
  - configuration
  - spec
---

# Storage Router Specification

## Routing Model

`StorageRouter` routes operations by type:

- **Document operations** — Routed to the configured document adapter
- **Blob operations** — Routed to the configured blob adapter

There is no per-collection routing in v1. All document operations go to a single document adapter, and all blob operations go to a single blob adapter.

## Configuration

Storage is configured via `storage.toml` at the engine root. The file declares which adapter to use for each operation type, along with adapter-specific settings and timeout values.

```toml
[document]
adapter = "sqlite"
path = "./data/life-engine.db"
encryption_key_file = "./data/key"

[document.require]
capabilities = ["encryption"]

[blob]
adapter = "filesystem"
path = "./data/blobs"

[blob.require]
capabilities = []

[timeouts]
document_read_ms = 5000
document_write_ms = 10000
blob_read_ms = 30000
blob_write_ms = 60000
```

Configuration fields for each adapter section:

- **`adapter`** — The adapter name as registered in the adapter registry. Required.
- **`path`** — Adapter-specific configuration (varies by adapter). Passed through to the adapter's `init` method.
- **`require.capabilities`** — A list of capability names the adapter must report. If the adapter does not report a required capability, the engine refuses to start.

Timeout fields:

- **`document_read_ms`** — Maximum duration for document read operations (`get`, `list`, `count`)
- **`document_write_ms`** — Maximum duration for document write operations (`create`, `update`, `partial_update`, `delete`, batch variants, `migrate`)
- **`blob_read_ms`** — Maximum duration for blob read operations (`retrieve`, `exists`, `list`, `metadata`)
- **`blob_write_ms`** — Maximum duration for blob write operations (`store`, `copy`, `delete`)

## Timeout Enforcement

Every adapter call is wrapped with the configured timeout for its class. If the adapter exceeds the timeout, the router returns `StorageError::Timeout`. The underlying adapter operation may still complete — the router does not cancel in-flight adapter work, but the caller receives the error immediately.

## Metrics

The router emits a structured log entry for every operation with the following fields:

- **`operation`** — The adapter method called (e.g., `get`, `create`, `list`)
- **`collection`** or **`key`** — The target collection name (documents) or blob key (blobs)
- **`duration_ms`** — Wall-clock time for the operation
- **`status`** — `ok` or the error variant name
- **`adapter`** — The adapter name that handled the operation

## Adapter Registry

In v1, the adapter registry is static — adapters are compiled into the binary.

```rust
pub struct AdapterRegistry {
    document_adapters: HashMap<String, Box<dyn DocumentStorageAdapter>>,
    blob_adapters: HashMap<String, Box<dyn BlobStorageAdapter>>,
}
```

Built-in adapters:

- **`"sqlite"`** — SQLite/SQLCipher document adapter. See [[document-storage-adapter]].
- **`"filesystem"`** — Local filesystem blob adapter. See [[blob-storage-adapter]].

## Startup Sequence

The router follows this sequence at engine startup:

1. Read and parse `storage.toml`
2. Look up the named document and blob adapters in the registry
3. Initialise each adapter with its configuration section
4. Validate that each adapter's reported capabilities meet the `require.capabilities` list
5. Run `migrate` on the document adapter for all declared collections (see [[schema-and-validation]])
6. Run `health` on both adapters
7. If either adapter is unhealthy or missing a required capability, refuse to start and log the reason
8. Register the router as available for `StorageContext` to use

If `storage.toml` is missing or unparseable, the engine refuses to start.

## Health Aggregation

The router aggregates health from both adapters into a single status:

- Both adapters report `Healthy` — Router reports `Healthy`
- Either adapter reports `Degraded` — Router reports `Degraded`
- Either adapter reports `Unhealthy` — Router reports `Unhealthy`

See [[document-storage-adapter]] and [[blob-storage-adapter]] for `HealthReport` details.
