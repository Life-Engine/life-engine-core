---
title: Data Layer
type: reference
created: 2026-03-14
updated: 2026-03-28
status: active
tags:
  - life-engine
  - core
  - data
  - storage
---

# Data Layer

Part of [[architecture/core/overview|Core Overview]] · [[architecture/core/README|Core Documentation]]

The data layer provides persistent storage behind pluggable adapters. Two storage categories handle different kinds of data:

- **Document storage** — Structured data behind a `DocumentStorageAdapter` trait. SQLite/SQLCipher is the v1 adapter.
- **Blob storage** — Binary content behind a `BlobStorageAdapter` trait. Local filesystem is the v1 adapter.

Plugins never interact with adapters directly. All storage access goes through a `StorageContext` API provided via host functions.

## Architecture

```
Plugin / Workflow Step
  ↓
StorageContext (permissions, validation, scoping, audit)
  ↓
StorageRouter (routing, timeouts, metrics)
  ↓
Document Adapter  or  Blob Adapter
```

## Key Concepts

There is no hard distinction between "canonical" and "private" collections. There are just collections, some of which follow a CDM recommended schema. Validation is opt-in per collection — if a plugin declares a schema in its manifest, data is validated on write. If not, data is stored as-is.



## Detailed Design

- [[architecture/core/design/data-layer/outline|Outline]] — Scope, design principles, component overview
- [[architecture/core/design/data-layer/storage-context|StorageContext]] — API surface, permissions, validation, audit events, watch bridge
- [[architecture/core/design/data-layer/document-storage-trait|Document Storage Trait]] — Adapter contract for structured data
- [[architecture/core/design/data-layer/blob-storage-trait|Blob Storage Trait]] — Adapter contract for binary content
- [[architecture/core/design/data-layer/storage-router|StorageRouter]] — Routing, configuration, startup sequence
- [[architecture/core/design/data-layer/schema-and-validation|Schema and Validation]] — JSON Schema format, validation rules, index hints
