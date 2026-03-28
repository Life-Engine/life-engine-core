<!--
domain: host-functions
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Host Functions — Tasks

> spec: ./brief.md

## Task Overview

This plan implements the complete set of host functions that Core exports to WASM plugins via Extism. It covers document storage (read, write, delete with batch support), blob storage (read, write, delete with key prefixing), event emission, configuration reading, HTTP outbound, error types, and host function registration. Each task targets 1-3 files and takes 15-30 minutes.

**Progress:** 0 / 20 tasks complete

## Steering Document Compliance

- All host functions gated by capability declarations from the plugin manifest
- Document storage scoped to manifest-declared collections
- Blob storage keys automatically prefixed with plugin ID for namespace isolation
- Event names validated against manifest `[events.emit]` section
- Core internals (transaction, watch, migrate, health, copy) never exposed
- Typed `PluginError` returned across the WASM boundary as serialised JSON

## Atomic Task Requirements

- **File Scope:** 1-3 related files maximum
- **Time Boxing:** 15-30 minutes per task
- **Single Purpose:** one testable outcome per task
- **Specific Files:** exact file paths specified
- **Agent-Friendly:** clear input/output, minimal context switching

---

## 1.1 — PluginError Type

> depends: none

- [ ] Define PluginError enum with all six variants
  <!-- file: packages/plugin-system/src/error.rs -->
  <!-- purpose: Define PluginError enum with CapabilityDenied, NotFound, ValidationError, StorageError, NetworkError, InternalError variants; derive Serialize/Deserialize for WASM boundary; implement Display and Error traits -->
  <!-- requirements: 10.1, 10.2, 10.3 -->

- [ ] Add PluginError unit tests
  <!-- file: packages/plugin-system/src/error.rs -->
  <!-- purpose: Test serialisation round-trip for each variant, test Display output, verify JSON encoding matches plugin SDK expectations -->
  <!-- requirements: 10.1, 10.3 -->

---

## 1.2 — Collection Validation Helper

> depends: 1.1

- [ ] Implement collection validation against manifest declarations
  <!-- file: packages/plugin-system/src/host_functions/storage.rs -->
  <!-- purpose: Add validate_collection function that checks a collection name against the plugin manifest's declared collections, returning CapabilityDenied if undeclared -->
  <!-- requirements: 1.5, 2.7, 3.4 -->

- [ ] Add collection validation tests
  <!-- file: packages/plugin-system/src/host_functions/storage.rs -->
  <!-- purpose: Test declared collection passes validation, undeclared collection returns CapabilityDenied with descriptive detail message -->
  <!-- requirements: 1.5, 2.7, 3.4 -->

---

## 1.3 — Document Read Host Functions

> depends: 1.2

- [ ] Implement storage_doc_get host function
  <!-- file: packages/plugin-system/src/host_functions/storage.rs -->
  <!-- purpose: Implement storage_doc_get that validates collection, retrieves document by ID via StorageBackend, returns Document or NotFound error -->
  <!-- requirements: 1.1, 1.2, 1.5, 1.6 -->

- [ ] Implement storage_doc_list and storage_doc_count host functions
  <!-- file: packages/plugin-system/src/host_functions/storage.rs -->
  <!-- purpose: Implement storage_doc_list (query with filters, sorting, pagination) and storage_doc_count (count matching query); both validate collection first -->
  <!-- requirements: 1.3, 1.4, 1.5, 1.6 -->

- [ ] Add document read host function tests
  <!-- file: packages/plugin-system/src/host_functions/storage.rs -->
  <!-- purpose: Test get returns document, get returns NotFound for missing ID, list returns filtered results, count returns correct count, undeclared collection denied -->
  <!-- requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6 -->

---

## 1.4 — Document Write Host Functions

> depends: 1.2

- [ ] Implement single document write host functions
  <!-- file: packages/plugin-system/src/host_functions/storage.rs -->
  <!-- purpose: Implement storage_doc_create (returns assigned ID), storage_doc_update (full replace, NotFound if missing), storage_doc_partial_update (merge patch into existing) -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.4, 2.7, 2.8, 2.9 -->

- [ ] Implement batch document write host functions
  <!-- file: packages/plugin-system/src/host_functions/storage.rs -->
  <!-- purpose: Implement storage_doc_batch_create (create multiple, return IDs in order) and storage_doc_batch_update (update multiple from [{id, doc}] array) -->
  <!-- requirements: 2.5, 2.6, 2.7, 2.8 -->

- [ ] Add document write host function tests
  <!-- file: packages/plugin-system/src/host_functions/storage.rs -->
  <!-- purpose: Test create returns ID, update replaces doc, update returns NotFound for missing, partial_update merges fields, batch_create returns ordered IDs, batch_update updates all, undeclared collection denied, validation error on bad data -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8, 2.9 -->

---

## 1.5 — Document Delete Host Functions

> depends: 1.2

- [ ] Implement document delete host functions
  <!-- file: packages/plugin-system/src/host_functions/storage.rs -->
  <!-- purpose: Implement storage_doc_delete (single delete, NotFound if missing) and storage_doc_batch_delete (delete multiple from ID array); both validate collection first -->
  <!-- requirements: 3.1, 3.2, 3.3, 3.4, 3.5 -->

- [ ] Add document delete host function tests
  <!-- file: packages/plugin-system/src/host_functions/storage.rs -->
  <!-- purpose: Test single delete succeeds, single delete returns NotFound for missing, batch delete removes all specified, undeclared collection denied -->
  <!-- requirements: 3.1, 3.2, 3.3, 3.4, 3.5 -->

---

## 1.6 — Blob Key Prefixing Helper

> depends: 1.1

- [ ] Implement blob key scoping function
  <!-- file: packages/plugin-system/src/host_functions/storage.rs -->
  <!-- purpose: Add scoped_key function that prefixes blob keys with plugin_id; add unscoped_key that strips the prefix for returning keys to the plugin in BlobMeta -->
  <!-- requirements: 4.6, 5.2, 6.3 -->

---

## 1.7 — Blob Read Host Functions

> depends: 1.6

- [ ] Implement blob read host functions
  <!-- file: packages/plugin-system/src/host_functions/storage.rs -->
  <!-- purpose: Implement storage_blob_retrieve (return bytes, NotFound if missing), storage_blob_exists (return bool), storage_blob_list (metadata for prefix matches), storage_blob_metadata (size, content_type, created_at); all auto-prefix keys -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7 -->

- [ ] Add blob read host function tests
  <!-- file: packages/plugin-system/src/host_functions/storage.rs -->
  <!-- purpose: Test retrieve returns bytes, retrieve returns NotFound for missing, exists returns true/false, list returns metadata with unprefixed keys, metadata returns correct fields, keys are auto-prefixed -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7 -->

---

## 1.8 — Blob Write and Delete Host Functions

> depends: 1.6

- [ ] Implement blob write and delete host functions
  <!-- file: packages/plugin-system/src/host_functions/storage.rs -->
  <!-- purpose: Implement storage_blob_store (store bytes, overwrite if exists, auto-prefix key) and storage_blob_delete (delete by key, NotFound if missing, auto-prefix key) -->
  <!-- requirements: 5.1, 5.2, 5.3, 6.1, 6.2, 6.3, 6.4 -->

- [ ] Add blob write and delete host function tests
  <!-- file: packages/plugin-system/src/host_functions/storage.rs -->
  <!-- purpose: Test store creates blob, store overwrites existing, delete removes blob, delete returns NotFound for missing, keys are auto-prefixed, missing capability denied -->
  <!-- requirements: 5.1, 5.2, 5.3, 6.1, 6.2, 6.3, 6.4 -->

---

## 1.9 — Event Emission Host Function

> depends: 1.1

- [ ] Implement emit_event host function
  <!-- file: packages/plugin-system/src/host_functions/events.rs -->
  <!-- purpose: Implement emit_event that validates event name against manifest [events.emit] section, sets source to plugin_id and depth from execution context, publishes to event bus -->
  <!-- requirements: 7.1, 7.2, 7.3, 7.4 -->

- [ ] Add event emission tests
  <!-- file: packages/plugin-system/src/host_functions/events.rs -->
  <!-- purpose: Test declared event emits successfully, undeclared event returns CapabilityDenied, source is set to plugin_id, depth is set from context, payload is passed through -->
  <!-- requirements: 7.1, 7.2, 7.3, 7.4 -->

---

## 1.10 — Configuration Host Function

> depends: 1.1

- [ ] Implement config_read host function
  <!-- file: packages/plugin-system/src/host_functions/config.rs -->
  <!-- purpose: Implement config_read that returns the calling plugin's runtime configuration as a JSON value from the ConfigStore -->
  <!-- requirements: 8.1, 8.2, 8.3 -->

- [ ] Add configuration host function tests
  <!-- file: packages/plugin-system/src/host_functions/config.rs -->
  <!-- purpose: Test config_read returns correct plugin config, config conforms to declared schema, missing capability returns CapabilityDenied -->
  <!-- requirements: 8.1, 8.2, 8.3 -->

---

## 1.11 — HTTP Outbound Host Function

> depends: 1.1

- [ ] Implement http_request host function
  <!-- file: packages/plugin-system/src/host_functions/http.rs -->
  <!-- purpose: Implement http_request that deserialises HttpRequestPayload JSON, executes request via HttpClient, returns HttpResponsePayload JSON; map failures to NetworkError -->
  <!-- requirements: 9.1, 9.2, 9.3 -->

- [ ] Add HTTP outbound host function tests
  <!-- file: packages/plugin-system/src/host_functions/http.rs -->
  <!-- purpose: Test successful request returns status/headers/body, timeout returns NetworkError, DNS failure returns NetworkError, missing capability returns CapabilityDenied -->
  <!-- requirements: 9.1, 9.2, 9.3 -->

---

## 1.12 — Host Function Registration

> depends: 1.3, 1.4, 1.5, 1.7, 1.8, 1.9, 1.10, 1.11

- [ ] Implement capability-gated host function registration
  <!-- file: packages/plugin-system/src/injection.rs, packages/plugin-system/src/host_functions/mod.rs -->
  <!-- purpose: Implement register_host_functions that reads approved capabilities and registers only matching host functions on the Extism PluginBuilder; re-export all host functions from mod.rs -->
  <!-- requirements: 11.2 -->

- [ ] Add registration tests verifying excluded functions
  <!-- file: packages/plugin-system/src/injection.rs -->
  <!-- purpose: Test that only approved host functions are registered, unapproved functions are absent, transaction/watch/migrate/health/copy are never registered regardless of capabilities -->
  <!-- requirements: 11.1, 11.2 -->
