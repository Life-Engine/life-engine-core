<!--
domain: blob-storage-adapter
updated: 2026-03-28
-->

# Blob Storage Adapter Spec

## Overview

This spec defines the `BlobStorageAdapter` trait and its supporting types for storing, retrieving, and managing binary blobs in Life Engine. The blob adapter sits alongside the document adapter in the data layer but is self-contained — it stores metadata alongside blob data without depending on the document adapter. Adapters implement the `BlobStorageAdapter` trait defined in `packages/traits`, and plugins interact with blobs exclusively through `StorageContext`.

The trait uses `ByteStream` for streaming large files without loading them entirely into memory. Each adapter reports its capabilities (encryption, server-side copy, streaming) so that the engine can enforce configuration requirements and select optimal code paths.

## Goals

- Streaming blob I/O — read and write blobs of arbitrary size without buffering entire files in memory
- Self-contained metadata — blob adapters manage their own metadata storage, decoupled from the document adapter
- Swappable backends — the `BlobStorageAdapter` trait in `packages/traits` allows filesystem, S3, or other implementations without changing callers
- Capability reporting — adapters declare encryption, server-side copy, and streaming support so the engine can enforce policy
- Atomic writes — a failed store operation must not leave partial blob data or orphaned metadata
- Consistent key format — blob keys follow `{plugin_id}/{context}/{filename}` with validation enforced by `StorageContext`

## User Stories

- As a plugin author, I want to store and retrieve binary files through `StorageContext` so that my plugin can manage attachments and exports without importing storage crates directly.
- As a module developer, I want to implement a new blob storage backend by satisfying the `BlobStorageAdapter` trait so that I can add S3 or other object storage without modifying plugins.
- As a user, I want my blobs encrypted at rest so that attachments and exports are protected if my device is compromised.
- As a workflow author, I want to copy blobs between keys without transferring data through the engine so that large file operations are efficient.
- As a maintainer, I want blob change events emitted by `StorageContext` so that downstream systems can react to blob mutations.

## Functional Requirements Summary

- The system must define the `BlobStorageAdapter` trait in `packages/traits` with `store`, `retrieve`, `delete`, `exists`, `copy`, `list`, `metadata`, `health`, and `capabilities` methods.
- The system must define `ByteStream`, `BlobInput`, `BlobMeta`, and `BlobAdapterCapabilities` types in `packages/types`.
- The system must enforce the blob key format `{plugin_id}/{context}/{filename}` with ASCII-safe character validation in `StorageContext`.
- The system must compute SHA-256 checksums on store and persist them in `BlobMeta`.
- The system must support streaming reads and writes via `ByteStream` without buffering entire blobs in memory.
- The system must report adapter capabilities and refuse to start if required capabilities (e.g., encryption) are missing.
- The system must ensure atomic write semantics — partial store operations must not leave orphaned data or metadata.
- The system must emit `system.blob.stored` and `system.blob.deleted` events from `StorageContext` on the write path.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
