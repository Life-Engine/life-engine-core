<!--
domain: blob-storage-adapter
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Implementation Plan — Blob Storage Adapter

## Task Overview

This plan implements the blob storage adapter layer across the crate architecture. Work begins with the core types in `packages/types`, then the `BlobStorageAdapter` trait in `packages/traits`, followed by `StorageContext` blob methods in `packages/plugin-sdk`, and finally the filesystem adapter implementation in `packages/storage-fs`. Each task targets a narrow slice of functionality that can be tested in isolation.

**Progress:** 0 / 14 tasks complete

## Steering Document Compliance

- `BlobStorageAdapter` trait in `packages/traits` follows Open/Closed Principle — new backends without caller changes
- `StorageContext` blob methods follow The Pit of Success — plugins use the correct API by default
- Capability enforcement at startup follows Defence in Depth — missing encryption causes startup failure
- Key validation in `StorageContext` follows Parse, Don't Validate — only valid keys reach the adapter
- Plugin-scoped keys follow Principle of Least Privilege — plugins cannot access other plugins' blobs

## Atomic Task Requirements

- **File Scope:** 1-3 related files maximum
- **Time Boxing:** 15-30 minutes per task
- **Single Purpose:** one testable outcome per task
- **Specific Files:** exact file paths specified
- **Agent-Friendly:** clear input/output, minimal context switching

---

## 1.1 — Blob Types

> spec: ./brief.md

- [ ] Define ByteStream type alias
  <!-- file: packages/types/src/blob.rs -->
  <!-- purpose: Define ByteStream as Pin<Box<dyn Stream<Item = Result<Bytes, StorageError>> + Send>> -->
  <!-- requirements: 1.3 -->

- [ ] Define BlobInput, BlobMeta, and BlobAdapterCapabilities structs
  <!-- file: packages/types/src/blob.rs -->
  <!-- purpose: Define the three data structures used by BlobStorageAdapter methods with all fields from the spec -->
  <!-- requirements: 1.3, 2.3, 2.4, 8.1, 10.1 -->

- [ ] Re-export blob types from packages/types lib.rs
  <!-- file: packages/types/src/lib.rs -->
  <!-- purpose: Add pub mod blob and re-export ByteStream, BlobInput, BlobMeta, BlobAdapterCapabilities -->
  <!-- requirements: 1.3 -->

## 1.2 — BlobStorageAdapter Trait

> spec: ./brief.md

- [ ] Define BlobStorageAdapter async trait
  <!-- file: packages/traits/src/blob_storage.rs -->
  <!-- purpose: Define the trait with all 9 methods (store, retrieve, delete, exists, copy, list, metadata, health, capabilities) using the types from packages/types -->
  <!-- requirements: 1.1, 1.2 -->

- [ ] Re-export blob storage trait from packages/traits lib.rs
  <!-- file: packages/traits/src/lib.rs -->
  <!-- purpose: Add pub mod blob_storage and re-export BlobStorageAdapter -->
  <!-- requirements: 1.1 -->

## 2.1 — StorageContext Blob Key Validation

> spec: ./brief.md

- [ ] Implement blob key validation in StorageContext
  <!-- file: packages/plugin-sdk/src/storage_context.rs -->
  <!-- purpose: Add validate_blob_key function that enforces {plugin_id}/{context}/{filename} format with ASCII-safe characters. Add unit tests for valid and invalid keys. -->
  <!-- requirements: 11.1, 11.2, 11.3 -->

## 2.2 — StorageContext Blob Methods

> spec: ./brief.md

- [ ] Add blob store and retrieve methods to StorageContext
  <!-- file: packages/plugin-sdk/src/storage_context.rs -->
  <!-- purpose: Add store_blob() and retrieve_blob() methods that validate the key, scope to plugin_id, delegate to BlobStorageAdapter, and emit events -->
  <!-- requirements: 2.1, 3.1, 3.2, 11.1, 12.1 -->

- [ ] Add blob delete, exists, list, and metadata methods to StorageContext
  <!-- file: packages/plugin-sdk/src/storage_context.rs -->
  <!-- purpose: Add delete_blob(), blob_exists(), list_blobs(), blob_metadata() methods that validate the key, scope to plugin_id, and delegate to BlobStorageAdapter -->
  <!-- requirements: 4.1, 4.2, 5.1, 5.2, 7.1, 7.2, 8.1, 8.2, 12.2 -->

## 3.1 — Filesystem Adapter Core

> spec: ./brief.md

- [ ] Scaffold filesystem blob adapter with store and retrieve
  <!-- file: packages/storage-fs/src/blob_adapter.rs, packages/storage-fs/src/lib.rs -->
  <!-- purpose: Implement FsBlobAdapter struct with store (atomic temp-file write with SHA-256 checksum) and retrieve (streaming file read) methods -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.5, 2.6, 3.1, 3.2 -->

- [ ] Implement sidecar metadata read/write helpers
  <!-- file: packages/storage-fs/src/blob_meta_io.rs -->
  <!-- purpose: Implement write_sidecar_meta() and read_sidecar_meta() functions that serialize/deserialize .meta.json files with atomic write-then-rename -->
  <!-- requirements: 14.1, 14.2, 14.3 -->

- [ ] Implement delete, exists, copy, list, and metadata methods
  <!-- file: packages/storage-fs/src/blob_adapter.rs -->
  <!-- purpose: Complete the remaining BlobStorageAdapter trait methods for the filesystem adapter. Copy uses retrieve-then-store fallback. List scans directory by prefix. -->
  <!-- requirements: 4.1, 4.2, 5.1, 5.2, 6.2, 6.3, 6.4, 7.1, 7.2, 7.3, 8.1, 8.2 -->

## 3.2 — Filesystem Adapter Health and Capabilities

> spec: ./brief.md

- [ ] Implement health check and capabilities for filesystem adapter
  <!-- file: packages/storage-fs/src/blob_adapter.rs -->
  <!-- purpose: Implement health() to check root directory existence and writability, and capabilities() returning encryption: false, server_side_copy: false, streaming: true -->
  <!-- requirements: 9.1, 9.2, 10.1, 10.3 -->

## 4.1 — Capability Enforcement and MIME Detection

> spec: ./brief.md

- [ ] Implement startup capability enforcement
  <!-- file: packages/core/src/startup.rs -->
  <!-- purpose: At startup, call capabilities() on the blob adapter and refuse to start if require_encryption is true but adapter reports encryption: false -->
  <!-- requirements: 10.2 -->

- [ ] Implement MIME type detection helper
  <!-- file: packages/types/src/mime.rs, packages/types/src/lib.rs -->
  <!-- purpose: Implement detect_content_type(filename: Option<&str>) -> String with common extension mappings, defaulting to application/octet-stream -->
  <!-- requirements: 2.4 -->
