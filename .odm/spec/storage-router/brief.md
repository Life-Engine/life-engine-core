<!--
domain: storage-router
updated: 2026-03-28
-->

# Storage Router Spec

## Overview

This spec defines the Storage Router, the central routing component that directs all storage operations to the appropriate adapter based on operation type. The router reads its configuration from `storage.toml`, enforces per-operation timeouts, validates adapter capabilities at startup, aggregates health status from both adapters, and emits structured metrics for every operation.

In v1, routing is by operation type only: all document operations go to a single document adapter (SQLite/SQLCipher), and all blob operations go to a single blob adapter (filesystem). There is no per-collection routing.

The router sits between the `StorageContext` query builder (used by plugins) and the underlying storage adapters. It is the only component that holds references to live adapter instances.

## Goals

- Route document and blob operations to their respective configured adapters
- Parse and validate `storage.toml` configuration at startup, refusing to start on invalid config
- Enforce per-operation-class timeouts on all adapter calls
- Validate that adapters report all required capabilities before accepting traffic
- Aggregate health status from both adapters into a single router health report
- Emit structured log entries with operation, target, duration, status, and adapter name for every call
- Provide a static adapter registry with compile-time adapter registration in v1

## User Stories

- As a Core developer, I want a single router that dispatches storage calls to the correct adapter so that plugins do not need to know which adapter is active.
- As an operator, I want storage configuration in a single TOML file so that I can change adapters or tune timeouts without modifying code.
- As an operator, I want the engine to refuse to start if a configured adapter is missing a required capability so that misconfiguration is caught early.
- As a Core developer, I want every storage operation logged with timing data so that I can diagnose performance issues.
- As a Core developer, I want adapter calls wrapped with timeouts so that a hung adapter does not block the entire engine.
- As an operator, I want a single health endpoint that reflects the worst-case status across all adapters so that monitoring is straightforward.

## Functional Requirements

- The system must parse `storage.toml` and map the `[document]` section to a document adapter and the `[blob]` section to a blob adapter.
- The system must refuse to start if `storage.toml` is missing, unparseable, or references an unknown adapter name.
- The system must validate adapter capabilities against `require.capabilities` before accepting traffic.
- The system must wrap every adapter call with the configured timeout and return `StorageError::Timeout` if exceeded.
- The system must emit a structured log entry for every adapter operation with operation name, target, duration, status, and adapter name.
- The system must aggregate health from both adapters using worst-case semantics (Unhealthy > Degraded > Healthy).
- The system must run the startup sequence in order: parse config, look up adapters, initialise, validate capabilities, migrate, health check, register.
- The system must provide a static `AdapterRegistry` that maps adapter names to compiled-in adapter instances.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
