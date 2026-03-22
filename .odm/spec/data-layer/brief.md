<!--
domain: data-layer
status: draft
tier: 1
updated: 2026-03-23
-->

# Data Layer Spec

## Overview

This spec defines the storage model, schema, encryption, data access patterns, and query conventions for Core's data layer. The data layer lives behind an abstract `StorageBackend` trait defined in `packages/traits`. SQLite/SQLCipher is the current implementation, housed in `packages/storage-sqlite` as its own crate. The backend is swappable — implementing `StorageBackend` for Postgres or another engine requires no plugin changes.

Plugins never import database crates directly. All storage access goes through a `StorageContext` fluent query builder provided by `packages/plugin-sdk`. The data model uses a universal document envelope in a `plugin_data` table — plugins never run DDL statements.

## Goals

- Universal document model — a single `plugin_data` table with a JSON data column serves all plugins without dynamic DDL
- Abstract storage trait — `StorageBackend` in `packages/traits` decouples plugins and modules from any specific database engine
- Fluent query builder — `StorageContext` in `packages/plugin-sdk` provides a type-safe, ergonomic API for all storage operations
- Defence in depth encryption — SQLCipher provides full-database encryption with Argon2id key derivation; credentials receive additional per-record encryption via `packages/crypto`
- Comprehensive audit trail — all security-relevant events are logged to an `audit_log` table with 90-day retention
- Queryable JSON — SQLite's `json_extract` enables filtering, sorting, and pagination over plugin data at personal scale

## User Stories

- As a plugin author, I want to store and retrieve structured data via a fluent query builder so that my plugin can persist state without importing any database crate.
- As a module developer, I want to implement a new storage backend by satisfying the `StorageBackend` trait so that I can add database support without modifying plugins.
- As a user, I want my data encrypted at rest so that it is protected even if the device is compromised.
- As a user, I want to export all my data in standard formats so that I am never locked in to Life Engine.
- As a maintainer, I want an audit log of security events so that I can investigate incidents and verify correct system behaviour.

## Functional Requirements

- The system must define the `StorageBackend` trait in `packages/traits` with `execute(StorageQuery) -> Vec<PipelineMessage>` and `mutate(StorageMutation) -> ()` methods.
- The system must provide a `StorageContext` fluent query builder in `packages/plugin-sdk` that produces `StorageQuery` and `StorageMutation` values without exposing database internals.
- The system must implement the `StorageBackend` trait for SQLite/SQLCipher in `packages/storage-sqlite`.
- The system must store all plugin data in a single `plugin_data` table with `id`, `plugin_id`, `collection`, `data` (JSON), `version`, `created_at`, and `updated_at` columns.
- The system must enforce optimistic concurrency via the `version` column, rejecting updates with stale versions.
- The system must validate data against canonical schemas (SDK-defined) and private schemas (manifest-defined) before writes.
- The system must encrypt the database with SQLCipher using a key derived from the user's master passphrase via Argon2id, with shared crypto primitives from `packages/crypto`.
- The system must log security events (auth attempts, credential access, plugin installs, permission changes) to the `audit_log` table.
- The system must support query filters (equality, comparison, text search, logical operators), sorting, and pagination via the `StorageContext` API.
- The system must support full and per-service data exports in standard formats.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
