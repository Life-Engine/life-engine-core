# Architecture

Life Engine Core is a personal data sovereignty platform. It aggregates, normalizes, encrypts, and stores personal data from external services, exposing it through a unified API. Users retain full ownership of their data — everything stays on their hardware.

This document is the architectural source of truth.

## Design Principles

- **Separation of Concerns** — Each module has one responsibility
- **Fail-Fast with Defined States** — Invalid states are unrepresentable
- **Defence in Depth** — Every layer is independently secure
- **Principle of Least Privilege** — Plugins access only declared capabilities
- **Parse, Don't Validate** — The type system prevents invalid data
- **Open/Closed Principle** — Open for plugins, closed to modification
- **Single Source of Truth** — One canonical definition per concept
- **Explicit Over Implicit** — Behavior is declared in manifests
- **The Pit of Success** — The easy path for plugin authors is the correct path

## Layers

The system has four layers, each independently deployable as crates:

1. **Transports** — Protocol-specific entry points (REST, GraphQL, CalDAV, CardDAV, Webhook). Configurable — the admin chooses which are active.
2. **Workflow Engine** — Declarative pipelines connecting transports to plugin steps. Owns the event bus and scheduler.
3. **Plugins** — WASM modules loaded at runtime. Accept a standard `PipelineMessage` input, produce a standard `PipelineMessage` output. No knowledge of transports or storage internals.
4. **Data Layer** — Abstract storage behind a `StorageBackend` trait. SQLite/SQLCipher is the current implementation. Swappable without changing any other layer.

## Request Flow

```
Transport receives request
  → Routes to a Workflow
    → Workflow runs a pipeline of Plugin steps
      → Each step receives PipelineMessage, returns PipelineMessage
    → Final output returns through the transport
```

## Core Binary

Core is a thin orchestrator. It does five things:

1. Load config (TOML)
2. Initialize modules (storage, auth, workflow engine)
3. Discover and load plugins (WASM from a plugins directory)
4. Start active transports
5. Coordinate graceful shutdown

After extraction, `apps/core/src/` contains three files:

- `main.rs` — Startup wiring
- `config.rs` — Config loading and validation
- `shutdown.rs` — Graceful shutdown coordination

Everything else lives in independent crates.

## Repository Structure

```
apps/core/                      → Thin binary (config, startup, shutdown)
packages/
  types/                        → CDM types, PipelineMessage, envelopes, shared enums
  traits/                       → Infrastructure contracts (StorageBackend, Transport, Plugin, EngineError)
  crypto/                       → Shared encryption primitives (AES-256-GCM, key derivation, HMAC)
  plugin-sdk/                   → Plugin author DX (re-exports, StorageContext, test helpers)
  storage-sqlite/               → StorageBackend impl for SQLite/SQLCipher
  auth/                         → Auth module (Pocket ID/OIDC, WebAuthn)
  workflow-engine/              → Pipeline executor, event bus, cron scheduler, YAML config parsing
  transport-rest/               → REST transport
  transport-graphql/            → GraphQL transport
  transport-caldav/             → CalDAV transport
  transport-carddav/            → CardDAV transport
  transport-webhook/            → Inbound webhook transport
  test-utils/                   → Shared test utilities
plugins/
  connector-email/              → Email fetch/send (WASM)
  connector-calendar/           → Calendar sync (WASM)
  connector-contacts/           → Contact sync (WASM)
  connector-filesystem/         → File operations (WASM)
  webhook-sender/               → Outbound webhook step (WASM)
  search-indexer/               → Full-text search indexing (WASM)
  backup/                       → Backup pipeline steps (WASM)
```

## Crate Internal Layout

Every crate follows the same convention:

```
src/
  lib.rs          → Public API (init, Config re-export, trait impls)
  config.rs       → Config struct + deserialization
  error.rs        → Module-specific error types implementing EngineError
  handlers/       → Request/response handling (transports) or steps/ (plugins)
    mod.rs
    ...
  types.rs        → Module-internal types (not shared)
  tests/
    mod.rs
    ...
```

For plugins, `handlers/` is replaced with `steps/` (one file per pipeline action) and `transform/` (input/output mapping to `PipelineMessage`).

## Dependency Graph

```
types (no dependencies)
  ↑
traits (depends on types)
  ↑
crypto (depends on types)
  ↑
plugin-sdk (depends on types + traits, re-exports both)
  ↑
storage-sqlite (depends on types + traits + crypto)
auth (depends on types + traits + crypto)
workflow-engine (depends on types + traits)
transport-* (depends on types + traits)
  ↑
apps/core (wires everything together)

Plugins depend only on plugin-sdk (which re-exports types + traits)
```

## PipelineMessage

The standard envelope for all data flowing through workflows:

```rust
struct PipelineMessage {
    metadata: MessageMetadata,    // correlation ID, source, timestamp, auth context
    payload: TypedPayload,        // Cdm(CdmType) | Custom(SchemaValidated<Value>)
}
```

- **CDM types** — The 7 canonical collection types: Events, Tasks, Contacts, Emails, Notes, Files, Credentials
- **Custom types** — Plugin-defined types validated against a JSON Schema declared in the plugin manifest

## Plugins

Plugins are WASM modules loaded at runtime via Extism. Core does not compile against any plugin.

### Discovery

Core scans a configured directory. Each plugin is a directory containing:

```
connector-email/
  plugin.wasm       → Compiled WASM module
  manifest.toml     → Plugin metadata, actions, config schema, capabilities
```

### Capabilities

Plugins declare required capabilities in their manifest. Capabilities are granted as host functions injected into the WASM runtime:

- `storage:read` — Read from collections via StorageContext
- `storage:write` — Write to collections via StorageContext
- `http:outbound` — Make outbound HTTP requests
- `events:emit` — Emit events into the workflow engine
- `events:subscribe` — Listen for events
- `config:read` — Read own config section

**Approval policy:**

- First-party plugins (in the monorepo) — auto-granted
- Third-party plugins — explicitly approved in config:

```toml
[plugins.some-third-party]
approved_capabilities = ["storage:read", "http:outbound"]
```

If a manifest declares a capability not in the approved list, Core refuses to load the plugin.

### StorageContext

Plugins interact with storage through a query builder abstraction:

```rust
trait StorageBackend {
    async fn execute(&self, query: StorageQuery) -> Result<Vec<PipelineMessage>>;
    async fn mutate(&self, op: StorageMutation) -> Result<()>;
}
```

Plugins use a fluent query builder that produces `StorageQuery` / `StorageMutation` values. The active `StorageBackend` translates these to native queries. Plugins never import database crates directly.

## Workflows

Workflows are declarative pipelines defined in YAML files within a configured directory.

### Triggers

A workflow can be triggered by one or more mechanisms:

- **endpoint** — An HTTP path handled by a transport
- **event** — An event emitted by a plugin or the system
- **schedule** — A cron expression

```yaml
workflows:
  sync-email:
    mode: async
    trigger:
      schedule: "*/5 * * * *"
      endpoint: "POST /email/sync"
      event: "webhook.email.received"
    steps:
      - plugin: connector-email
        action: fetch
      - plugin: search-indexer
        action: index
```

### Execution Modes

- `sync` — All steps complete before the transport responds. Used for queries.
- `async` — Returns a job ID immediately. Steps run in background. Used for long-running operations.

### Control Flow (v1)

- **Sequential** — Steps run in order, output of step N is input to step N+1
- **Conditional branching** — Route to different steps based on output content
- **Error handling** — Retry count + fallback step on failure

### Validation

Configurable per workflow:

- `strict` — Validate output schema at every step boundary
- `edges` (default) — Validate at pipeline entry and exit only
- `none` — No schema validation

## Error Handling

Each module defines its own error types internally. At module boundaries, errors implement the `EngineError` trait:

```rust
trait EngineError: std::error::Error {
    fn code(&self) -> &str;         // e.g., "STORAGE_001"
    fn severity(&self) -> Severity; // Fatal, Retryable, Warning
    fn source_module(&self) -> &str;
}
```

The workflow engine uses severity to decide behavior:

- `Fatal` — Abort the pipeline, run error handler if configured
- `Retryable` — Retry the step up to the configured limit, then fail
- `Warning` — Log and continue

## Configuration

Two formats, each for its strength:

- **`config.toml`** — Application settings (flat key-value). Module config, active transports, plugin activation.
- **`workflows/*.yaml`** — Workflow definitions (ordered nested structures). Pipeline steps, triggers, control flow.

Each module declares its own config type. Core reads the top-level section key to determine which modules are active, then hands each section to the relevant module for parsing:

```toml
[storage]
backend = "sqlite"
path = "./data/core.db"

[transports.rest]
port = 3000

[transports.graphql]
port = 3001

[auth]
provider = "pocket-id"
issuer = "https://auth.local"

[workflows]
path = "./workflows/"

[plugins.connector-email]
poll_interval = "5m"
```

## Traits and Contracts

Infrastructure traits live in `packages/traits`. Plugin-facing traits are re-exported through `packages/plugin-sdk`.

- Plugin authors depend on `plugin-sdk` only (one dependency, everything included)
- Module developers (new storage backend, new transport) depend on `types` + `traits`

## Testing Strategy

- **Unit tests** — Inside each crate (`#[cfg(test)]`). Mock trait dependencies. This is where most tests live.
- **Integration tests** — In each crate's `tests/` directory. Used when the module's value is integration (e.g., `storage-sqlite` tests against a real in-memory SQLite).
- **End-to-end tests** — In `apps/core/tests/`. Boot the full system, verify wiring correctness. Minimal — not for testing module logic.
- **Plugin tests** — Plugin authors use mock `StorageContext` and `PipelineMessage` builders from `plugin-sdk`.

## Migration Path

Incremental extraction from the current codebase, ordered by dependency graph:

1. `packages/types` — Refine existing crate (add `PipelineMessage`, envelopes)
2. `packages/traits` — New crate, define infrastructure contracts
3. `packages/crypto` — Extract encryption primitives from Core
4. `packages/plugin-sdk` — Refactor to re-export from `types` + `traits`, add `StorageContext`
5. `packages/storage-sqlite` — Extract storage logic from Core
6. `packages/auth` — Extract auth module from Core
7. `packages/workflow-engine` — New crate, build pipeline executor
8. `packages/transport-rest` — Extract REST API from Core
9. `packages/transport-graphql` — Extract GraphQL API from Core
10. Slim `apps/core` down to thin orchestrator
11. Convert plugins to WASM modules

Each step is a self-contained change. The project compiles after every step.
