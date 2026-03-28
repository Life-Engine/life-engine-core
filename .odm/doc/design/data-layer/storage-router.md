---
title: Storage Router
type: adr
created: 2026-03-28
status: draft
tags:
  - architecture
  - data-layer
  - storage-router
  - core
---

# Storage Router

## Overview

The `StorageRouter` sits between `StorageContext` and the active adapters. It holds one document adapter and one blob adapter, routes operations to the correct one, enforces timeouts, and emits operation metrics.

## Routing Model

The router determines the target adapter based on the operation type, not the collection:

- **Document operations** (get, list, count, create, update, partial_update, delete, batch ops, transaction, watch, migrate) → document adapter
- **Blob operations** (store, retrieve, delete, exists, copy, list, metadata) → blob adapter

There is no per-collection routing in v1. All document collections go to the same adapter. All blobs go to the same adapter.

## Configuration

Active adapters are selected in `storage.toml`, read once at startup:

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

- `adapter` — Selects the adapter by name from the registry
- Adapter-specific settings (path, connection strings, etc.) are passed to the adapter at initialisation
- `require.capabilities` — List of capabilities the active adapter must report. Validated at startup.
- `timeouts` — Per-operation-class timeout limits

Switching adapters is a config change plus restart. Consistent with the immutable-at-runtime pattern used throughout Core.

## Timeout Enforcement

The router wraps every adapter call with a configurable timeout:

- If an operation exceeds its timeout, the router cancels it and returns `StorageError::Timeout`
- Timeouts are grouped by operation class (document read, document write, blob read, blob write)
- Different defaults reflect different expected durations — blob operations are slower than document operations

The caller (workflow step) receives the timeout error and handles it via `on_error` strategy. The data layer does not retry.

## Metrics

The router emits structured log entries for every operation:

- **operation** — The method called (e.g., `document.get`, `blob.store`)
- **collection** or **key** — The target
- **duration_ms** — How long the operation took
- **status** — `ok` or the `StorageError` variant
- **adapter** — Which adapter handled it

This follows the same pattern as `StepTrace` in the workflow engine — structured JSON logs that can be consumed by monitoring tools.

## Adapter Registration

V1 uses a static registry of built-in adapters:

```rust
pub struct AdapterRegistry {
    document_adapters: HashMap<String, Box<dyn DocumentStorageAdapter>>,
    blob_adapters: HashMap<String, Box<dyn BlobStorageAdapter>>,
}
```

Built-in adapters are registered at compile time:

- `"sqlite"` → SQLite/SQLCipher document adapter
- `"filesystem"` → Local filesystem blob adapter

The registry exists as an abstraction point. Future versions can extend it with external adapters loaded from shared libraries without changing the router or any code above it.

## Startup Sequence

The router initialises as part of Core startup:

1. Read `storage.toml` configuration
2. Look up the selected document adapter and blob adapter in the registry
3. Initialise both adapters with their configuration
4. Validate that each adapter's reported capabilities meet the `require.capabilities` from config
5. Run `migrate` on the document adapter for all known collections (shared + plugin-scoped)
6. Run `health` on both adapters
7. If either adapter returns `Unhealthy`, or a required capability is missing, Core refuses to start
8. Register the router as available for `StorageContext`

Steps 5 and 6 run sequentially — migration must complete before the health check.

## Runtime Behaviour

After startup, the router is immutable:

- The active adapters cannot be changed without restarting Core
- The configuration cannot be reloaded at runtime
- The timeout values are fixed for the lifetime of the process

This matches the immutable-at-runtime pattern used by the workflow engine (workflow definitions), trigger system (registered triggers), and router (route table).

## Health Aggregation

When `system.health` is called, the router queries both adapters and returns a combined view:

- If both adapters are `Healthy`, the combined status is `Healthy`
- If either adapter is `Degraded`, the combined status is `Degraded`
- If either adapter is `Unhealthy`, the combined status is `Unhealthy`

Individual adapter health reports are included in the response so the caller can see which adapter is degraded and why.
