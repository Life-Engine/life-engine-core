---
title: Data Layer Outline
type: adr
created: 2026-03-28
status: draft
tags:
  - architecture
  - data-layer
  - core
---

# Data Layer Outline

## Scope (v1)

The data layer provides persistent storage behind pluggable adapters. It owns two storage categories and the mediation layer between them and the rest of Core:

- **Document storage** — Structured data (events, tasks, contacts, notes, emails, credentials, plugin private collections). Default adapter: SQLite/SQLCipher.
- **Blob storage** — Binary content (files, images, attachments). Self-contained with its own internal metadata. Default adapter: local filesystem.
- **StorageContext** — The API surface that plugins and workflows use to interact with storage. Handles permissions, validation, scoping, and audit events.
- **StorageRouter** — Routes operations to the active document or blob adapter. Enforces timeouts and emits metrics.

The following are deferred to future considerations:

- External adapters (shared library loading)
- Data export and import
- Cross-adapter transactions
- Per-collection adapter routing

## Request Flow

```
Plugin / Workflow Step
  ↓
Host function call (storage_doc_* or storage_blob_*)
  ↓
StorageContext (permission check, collection scoping, schema validation)
  ↓
StorageRouter (route to correct adapter, enforce timeout, emit metrics)
  ↓
Adapter (execute operation against storage engine)
  ↓
Result returned up the chain
```

## Design Principles

- **Backend-agnostic** — Everything above the adapter traits works identically regardless of which adapter is active. No leaky abstractions.
- **Validate at the boundary** — `StorageContext` is the single enforcement point. Adapters trust what they receive. Plugins trust what they get back.
- **Fail fast** — Storage operations return errors immediately. No retries at the data layer — the workflow layer decides retry strategy.
- **Minimal adapter contract** — The traits ask for the least possible from implementations. Optional capabilities (indexing, encryption, native watch, cursors) are declared, not required.
- **Data ownership is explicit** — Every collection has a clear owner (the plugin that created it). Every blob has a clear reference. No orphans by design.
- **Data portability** — Switching adapters requires only a config change and restart. No application-level changes.
- **Future-proof surface, minimal v1 implementation** — Trait signatures accommodate future needs. V1 built-in adapters implement the pragmatic subset.

## Components

The data layer comprises six components, each documented separately:

- [[storage-context]] — API surface, permission enforcement, schema validation, audit events, watch bridge
- [[document-storage-trait]] — Trait definition for structured data adapters
- [[blob-storage-trait]] — Trait definition for binary content adapters
- [[storage-router]] — Routing, configuration, timeouts, metrics, startup sequence
- [[schema-and-validation]] — Schema format, manifest declarations, validation rules, index hints

## CDM Recommended Schemas

The SDK ships recommended schemas (the Canonical Data Model) for common personal data:

- `events` — Calendar events
- `tasks` — To-dos, reminders
- `contacts` — People
- `notes` — Freeform text
- `emails` — Email messages
- `credentials` — Identity documents, OAuth tokens, API keys

These are suggestions, not enforced types. There is no hard distinction between "canonical" and "private" collections — there are just collections, some of which follow a recommended schema. Plugins choose whether to adopt CDM schemas in their manifest. The incentive is interoperability.

Files are not a CDM collection. Binary content is owned entirely by the blob storage adapter, which manages its own metadata internally.

## Adapter Model

Storage adapters are native Rust trait implementations compiled into Core. They are not WASM plugins — they need raw filesystem/network access and operate at a different trust level.

V1 ships two built-in adapters:

- **SQLite/SQLCipher** — Document storage adapter. Transparent encryption via SQLCipher.
- **Local filesystem** — Blob storage adapter. Stores files in a configured directory.

The trait interfaces are fully defined so that additional adapters (Postgres, S3, etc.) can be implemented in future versions without changing the architecture.

## What the Data Layer Owns

- Persistent storage of structured documents and binary blobs
- Schema validation and enforcement
- Collection scoping and permission enforcement
- Query building and execution
- Index management
- Adapter lifecycle (init, migrate, health)
- Storage change event emission
- Timeout enforcement on adapter operations
- Credential field-level encryption (per [[encryption-and-audit]])
- Collection creation via `migrate`, triggered by plugin lifecycle

## What the Data Layer Does Not Own

- **Workflow orchestration** — That belongs to the [[architecture/core/design/workflow-engine-layer/outline|workflow engine layer]].
- **Cross-backend file orchestration** — File upload/download workflows live in the workflow layer.
- **Retry logic** — Workflows handle retries via `on_error`.
- **Data export/import** — Deferred.
- **Plugin execution** — Plugins are invoked by the workflow engine, not by the data layer.
- **Transport concerns** — No awareness of HTTP, GraphQL, or any protocol.
- **Admin panel** — Admin queries go through workflows like everything else.
- **Backup scheduling** — A future workflow concern, not a storage concern.
