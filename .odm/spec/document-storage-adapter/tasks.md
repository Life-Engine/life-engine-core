<!--
domain: document-storage-adapter
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Implementation Plan — Document Storage Adapter

## Task Overview

This plan implements the `DocumentStorageAdapter` trait and all supporting types in the `le-storage` crate. Work begins with error types and core data structures, then the trait definition, query types, transaction and change event types, migration and health types, capability negotiation, and finally workflow error mapping. Each task produces a narrow, testable slice of functionality.

**Progress:** 0 / 14 tasks complete

## Steering Document Compliance

- Single `DocumentStorageAdapter` trait follows Open/Closed Principle — new backends implement without upstream changes
- Typed filter trees and field descriptors follow Parse, Don't Validate — invalid queries are unrepresentable
- Capability negotiation follows Defence in Depth — missing encryption blocks startup
- Atomic batch semantics follow Single Source of Truth — no partial writes
- `StorageError` variants carry contextual fields for diagnosability

## Atomic Task Requirements

- **File Scope:** 1-3 related files maximum
- **Time Boxing:** 15-30 minutes per task
- **Single Purpose:** one testable outcome per task
- **Specific Files:** exact file paths specified
- **Agent-Friendly:** clear input/output, minimal context switching

---

## 1.1 — Error Types

> spec: ./brief.md

- [ ] Define StorageError enum with all variants
  <!-- file: crates/le-storage/src/error.rs -->
  <!-- purpose: Define StorageError enum with NotFound, AlreadyExists, ValidationFailed, CapabilityDenied, SchemaConflict, Timeout, ConnectionFailed, UnsupportedOperation, Internal variants and Display/Error impls -->
  <!-- requirements: 10.3 -->

- [ ] Implement workflow retryable flag mapping for StorageError
  <!-- file: crates/le-storage/src/error.rs -->
  <!-- purpose: Add is_retryable() method returning true for Timeout, ConnectionFailed, Internal and false for all other variants -->
  <!-- requirements: 10.1, 10.2 -->

## 1.2 — Core Data Structures

> spec: ./brief.md

- [ ] Define Document type
  <!-- file: crates/le-storage/src/document.rs -->
  <!-- purpose: Define Document struct wrapping serde_json::Value with an id field, plus constructors and accessors -->
  <!-- requirements: 1.1, 1.2 -->

- [ ] Define DocumentList and Pagination types
  <!-- file: crates/le-storage/src/query.rs -->
  <!-- purpose: Define DocumentList (documents, next_cursor, total_estimate) and Pagination (limit, cursor) structs -->
  <!-- requirements: 2.1, 2.2 -->

## 1.3 — Query and Filter Types

> spec: ./brief.md

- [ ] Define FilterNode, FilterOperator, and QueryDescriptor
  <!-- file: crates/le-storage/src/query.rs -->
  <!-- purpose: Define FilterNode enum (Condition, And, Or, Not), FilterOperator enum (Eq, Ne, Gt, Gte, Lt, Lte, In, NotIn, Contains, StartsWith, Exists), SortField, SortDirection, and QueryDescriptor struct -->
  <!-- requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 3.8 -->

## 1.4 — Change Event Types

> spec: ./brief.md

- [ ] Define ChangeEvent and ChangeType
  <!-- file: crates/le-storage/src/events.rs -->
  <!-- purpose: Define ChangeEvent struct (collection, document_id, change_type, timestamp) and ChangeType enum (Created, Updated, Deleted) -->
  <!-- requirements: 6.1, 6.3 -->

## 1.5 — Transaction Types

> spec: ./brief.md

- [ ] Define TransactionHandle trait
  <!-- file: crates/le-storage/src/transaction.rs -->
  <!-- purpose: Define TransactionHandle trait with synchronous get, create, update, delete methods that participate in an enclosing transaction -->
  <!-- requirements: 5.5 -->

## 2.1 — Schema Migration Types

> spec: ./brief.md

- [ ] Define CollectionDescriptor, FieldDescriptor, and FieldType
  <!-- file: crates/le-storage/src/migration.rs -->
  <!-- purpose: Define CollectionDescriptor (name, fields, indexes), FieldDescriptor (name, field_type, required), and FieldType enum (String, Integer, Float, Boolean, DateTime, Json, Array) -->
  <!-- requirements: 7.1, 7.3, 7.4 -->

## 2.2 — Health Reporting Types

> spec: ./brief.md

- [ ] Define HealthReport, HealthStatus, and HealthCheck
  <!-- file: crates/le-storage/src/health.rs -->
  <!-- purpose: Define HealthReport (status, checks), HealthStatus enum (Healthy, Degraded, Unhealthy), and HealthCheck (name, status, message) -->
  <!-- requirements: 8.1, 8.2 -->

## 2.3 — Capability Types

> spec: ./brief.md

- [ ] Define AdapterCapabilities struct
  <!-- file: crates/le-storage/src/capabilities.rs -->
  <!-- purpose: Define AdapterCapabilities struct with encryption, indexing, full_text_search, watch, transactions boolean fields -->
  <!-- requirements: 9.1 -->

## 3.1 — Trait Definition

> spec: ./brief.md

- [ ] Define DocumentStorageAdapter trait
  <!-- file: crates/le-storage/src/adapter.rs -->
  <!-- purpose: Define the async DocumentStorageAdapter trait with all 16 methods (get, create, update, partial_update, delete, list, count, batch_create, batch_update, batch_delete, transaction, watch, migrate, health, capabilities) using the types from previous tasks -->
  <!-- requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 2.1, 2.4, 4.1, 4.2, 4.3, 5.1, 6.1, 7.1, 8.1, 9.1 -->

## 3.2 — Crate Module Structure

> spec: ./brief.md

- [ ] Wire up le-storage crate lib.rs with public module exports
  <!-- file: crates/le-storage/src/lib.rs -->
  <!-- purpose: Declare all modules (error, document, query, events, transaction, migration, health, capabilities, adapter) and re-export primary types -->
  <!-- requirements: all -->

## 4.1 — Capability Enforcement

> spec: ./brief.md

- [ ] Implement startup capability validation logic
  <!-- file: crates/le-storage/src/capabilities.rs -->
  <!-- purpose: Add validate_against_config method that checks adapter capabilities against storage.toml requirements and returns errors for missing required capabilities (e.g., encryption) -->
  <!-- requirements: 9.2, 9.3, 9.4, 9.5, 9.6 -->

- [ ] Add unit tests for capability validation
  <!-- file: crates/le-storage/src/capabilities.rs -->
  <!-- purpose: Test that missing encryption with required config returns error, missing indexing is silently accepted, missing full_text_search/watch/transactions degrade correctly -->
  <!-- requirements: 9.2, 9.3, 9.4, 9.5, 9.6 -->
