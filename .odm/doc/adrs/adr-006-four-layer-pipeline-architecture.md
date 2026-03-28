---
title: "ADR-006: Four-Layer Pipeline Architecture"
type: adr
created: 2026-03-28
status: active
---

# ADR-006: Four-Layer Pipeline Architecture

## Status

Accepted

## Context

Life Engine Core is a self-hosted personal data sovereignty platform that loads third-party plugins, manages encrypted storage, and serves HTTP requests. The system must support a clean separation between protocol handling, business-logic orchestration, plugin execution, and data persistence so that each concern can evolve independently.

Previous iterations considered a monolithic design where HTTP handlers called plugins directly and plugins managed their own storage connections. This approach tangled protocol translation with business logic, made it difficult to add new transports (GraphQL, WebSocket) without duplicating orchestration code, and gave plugins direct access to storage backends — undermining the security isolation model.

The architecture must also accommodate pluggable storage backends (SQLite today, Postgres or S3 in the future) without any layer above the adapter needing to change.

## Decision

Core is structured as a four-layer pipeline. A request enters through a transport, flows through a workflow of plugin steps, and reads or writes data through the storage layer. Each layer has a single responsibility and communicates with its neighbours through well-defined contracts.

The four layers are:

- **Transport layer** — Receives external requests over REST and GraphQL from a single Axum HTTP listener. Handles TLS, CORS, authentication, and route matching. Translates each request into a `WorkflowRequest` and hands it to the workflow engine. Converts the `WorkflowResponse` back into a protocol-specific reply.
- **Workflow engine layer** — The central orchestrator. Receives trigger contexts (from transport, events, or schedules), resolves the workflow definition, and executes plugin steps in sequence. Passes a `PipelineMessage` between each step. Owns the trigger system, pipeline executor, event bus, and scheduler.
- **Plugin system** — All business logic lives in plugins. Plugins are WASM modules loaded at runtime via Extism — memory-isolated, language-agnostic, and crash-safe. Plugins communicate only through workflows (chained steps) and shared collections, never directly with each other.
- **Data layer** — Provides persistent storage behind pluggable adapters. Document storage (SQLite/SQLCipher in v1) handles structured data; blob storage (local filesystem in v1) handles binary content. All access flows through a `StorageContext` API that enforces permissions, validates schemas, and emits audit events.

The layers connect through three cross-layer contracts:

- `WorkflowRequest` / `WorkflowResponse` between transport and workflow engine
- `PipelineMessage` between plugin steps within a workflow
- `StorageContext` and host functions between plugins and the data layer

## Consequences

Positive consequences:

- Each layer can be tested, reasoned about, and modified independently. Adding a new transport (e.g., WebSocket) requires only a new handler that produces `WorkflowRequest` values — no workflow or plugin changes.
- Security boundaries are enforced at layer transitions. Plugins never see raw HTTP. The transport layer never touches storage. The data layer never executes plugin logic.
- Storage backends can be swapped by changing a config file. Everything above the adapter trait works identically regardless of which adapter is active.
- The pipeline model makes request flow predictable and debuggable — every request follows the same path through the same layers.

Negative consequences:

- The layered structure adds indirection. A simple "read one record" request traverses four layers (transport → workflow → plugin → data) when a direct handler-to-database call would be shorter.
- Contributors must understand the layer boundaries and respect the contracts. Writing a feature that crosses layers (e.g., a transport-aware plugin) is not possible by design — this constraint must be learned.
- The strict separation means some optimisations (e.g., short-circuiting a health check without entering the workflow engine) are deliberately avoided in favour of architectural consistency.
