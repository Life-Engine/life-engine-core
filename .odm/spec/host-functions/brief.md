<!--
domain: host-functions
updated: 2026-03-28
-->

# Host Functions Spec

## Overview

This spec defines the host functions that Core exports to WASM plugins via Extism. Host functions are the sole interface through which plugins interact with storage, events, configuration, and the external network. Every host function requires a corresponding capability declaration in the plugin's manifest. Calling a host function without the required capability returns a `CapabilityDenied` error.

Host functions are organised into six domains: document storage (read, write, delete), blob storage (read, write, delete), event emission, configuration reading, and HTTP outbound. All functions return `Result<T, PluginError>` with a typed error set that the plugin SDK surfaces for action-level error handling.

Document storage functions are scoped to collections declared in the plugin's manifest. Blob storage keys are automatically prefixed with the calling plugin's ID, ensuring namespace isolation. A set of Core internals (transactions, watch, migrate, health, copy) are explicitly excluded from the host function surface.

## Goals

- Define every host function signature, required capability, and error behaviour for the six host function domains
- Enforce capability-gated access so plugins can only call functions matching their approved capabilities
- Scope document storage to manifest-declared collections and blob storage to the calling plugin's namespace
- Provide batch operations for document create, update, and delete to support efficient bulk workflows
- Define a typed error set (`PluginError`) with distinct variants for capability denial, not-found, validation, storage, network, and internal errors
- Ensure Core internals (transaction, watch, migrate, health, copy) are never exposed to plugins

## User Stories

- As a plugin author, I want to read, create, update, and delete documents in my declared collections so that I can persist structured data through Core.
- As a plugin author, I want batch document operations so that I can efficiently process multiple records in a single call.
- As a plugin author, I want to store and retrieve binary blobs so that I can manage files and attachments without a separate storage system.
- As a plugin author, I want to emit events so that other plugins and workflows can react to my actions.
- As a plugin author, I want to read my plugin's configuration so that I can adapt behaviour based on user settings.
- As a plugin author, I want to make outbound HTTP requests so that I can integrate with external APIs and services.
- As a plugin author, I want typed errors from host functions so that I can handle failures appropriately in my action logic.
- As a Core developer, I want host functions gated by capabilities so that plugins cannot access resources beyond their approved scope.

## Functional Requirements

- The system must export document storage host functions (`get`, `list`, `count`, `create`, `update`, `partial_update`, `delete`, `batch_create`, `batch_update`, `batch_delete`) gated by the appropriate `storage:doc:read`, `storage:doc:write`, or `storage:doc:delete` capability.
- The system must scope all document storage calls to collections declared in the calling plugin's manifest, returning `CapabilityDenied` for undeclared collections.
- The system must export blob storage host functions (`retrieve`, `exists`, `list`, `metadata`, `store`, `delete`) gated by `storage:blob:read`, `storage:blob:write`, or `storage:blob:delete`.
- The system must automatically prefix blob keys with the calling plugin's ID so that plugins can only access their own blobs.
- The system must export `emit_event` gated by `events:emit`, validating that the event name is declared in the plugin's manifest `[events.emit]` section.
- The system must export `config_read` gated by `config:read`, returning only the calling plugin's configuration.
- The system must export `http_request` gated by `http:outbound`, accepting a JSON request object and returning a JSON response object.
- The system must return `Result<T, PluginError>` from all host functions with typed error variants: `CapabilityDenied`, `NotFound`, `ValidationError`, `StorageError`, `NetworkError`, `InternalError`.
- The system must never expose `transaction`, `watch`, `migrate`, `health`, or `copy` as host functions.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
