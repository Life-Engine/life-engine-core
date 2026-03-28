<!--
domain: storage-context
updated: 2026-03-28
-->

# Storage Context Spec

## Overview

`StorageContext` is the API surface between callers and `StorageRouter`. It enforces permissions, validates schemas, scopes collections, emits audit events, and bridges storage changes to the event bus. Plugins never interact with adapters directly — all storage access flows through `StorageContext`.

There are two access paths. Plugin access (via host functions) is scoped to the calling plugin's identity and enforces capability checks. Workflow engine access uses a system-level identity, bypasses plugin-scoped permission checks, but still applies schema validation.

## Goals

- Enforce deny-by-default capability checks before any storage operation reaches an adapter
- Scope collection access so plugins can only touch declared collections
- Validate write payloads against JSON Schema (draft 2020-12) when a schema is declared
- Manage system base fields (`id`, `created_at`, `updated_at`) transparently on every document
- Isolate plugin extension fields under `ext.{plugin_id}` namespaces
- Expose a fluent, backend-agnostic query builder that produces `QueryDescriptor` values
- Provide host functions for WASM plugin modules covering document and blob operations
- Emit audit events for all write operations via the event bus
- Bridge adapter-level change notifications to the event bus without duplicate events
- Encrypt sensitive fields in the `credentials` collection before they reach the adapter

## User Stories

- As a plugin author, I want to read and write documents through host functions so that my WASM module can persist data without direct adapter access.
- As a plugin author, I want permission errors returned immediately when my manifest lacks the required capability so that I can fix my configuration without guessing.
- As a plugin author, I want to attach custom fields under my extension namespace so that my data coexists with other plugins without collisions.
- As a workflow engine developer, I want system-level access that bypasses plugin capability checks so that internal orchestration is not blocked by plugin permission rules.
- As a Core developer, I want audit events emitted for every write so that security-sensitive changes are traceable.
- As a Core developer, I want the watch-to-event-bus bridge to avoid duplicate events so that downstream consumers process each change exactly once.
- As a user, I want credentials encrypted at the field level before storage so that sensitive data is protected independently of adapter-level encryption.

## Functional Requirements

- The system must check declared capabilities before forwarding any operation to the router, returning `StorageError::CapabilityDenied` on failure.
- The system must scope plugin access to declared shared collections and plugin-namespaced private collections only.
- The system must validate write payloads against JSON Schema when a schema is declared for the collection.
- The system must inject and manage `id`, `created_at`, and `updated_at` base fields on every document.
- The system must enforce extension field isolation so plugins can only write to their own `ext.{plugin_id}` namespace.
- The system must expose document and blob host functions to WASM plugin modules.
- The system must emit `system.storage.*` and `system.blob.*` audit events for write operations, including the originating caller identity.
- The system must bridge adapter watch streams to the event bus, falling back to write-path emission when native watch is unsupported.
- The system must encrypt sensitive fields in the `credentials` collection with a derived key before passing data to the adapter.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
