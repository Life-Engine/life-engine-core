---
title: Core Design Overview
type: reference
created: 2026-03-28
status: active
tags:
  - life-engine
  - core
  - design
---

# Core Design Overview

Part of [[architecture/core/overview|Core Overview]] · [[architecture/core/README|Core Documentation]]

Core is a four-layer pipeline. A request enters through a transport, flows through a workflow of plugin steps, and reads or writes data through the storage layer. Each layer has a single responsibility and communicates with its neighbours through well-defined contracts.

## Transport Layer

The transport layer receives external requests over REST and GraphQL (both served from a single Axum HTTP listener). It handles TLS, CORS, authentication, and route matching, then translates each request into a `WorkflowRequest` and hands it to the workflow engine. The handler converts the `WorkflowResponse` back into a protocol-specific reply.

- [[architecture/core/design/transport-layer/outline|Outline]] — Scope, request flow, and component overview

## Workflow Engine Layer

The workflow engine is the central orchestrator. Workflows are declarative YAML pipelines of plugin steps, activated by three trigger types: endpoint bindings, internal events, and cron schedules. The pipeline executor runs steps in sequence, passing a `PipelineMessage` between each one. It supports conditional branching, per-step error handling (halt, retry, skip), and sync/async execution modes.

- [[architecture/core/design/workflow-engine-layer/outline|Outline]] — Scope, design principles, and component overview

## Plugin System

All business logic lives in plugins. Plugins are WASM modules loaded at runtime via Extism — memory-isolated, language-agnostic, and crash-safe. Each plugin declares the capabilities it needs in a manifest; Core grants or denies them on a deny-by-default basis. Plugins communicate only through workflows (chained steps) and shared collections (common data schemas), never directly with each other.

- [[architecture/core/design/plugins|Plugin System]] — Isolation model, lifecycle, capabilities, and standard contract

## Data Layer

The data layer provides persistent storage behind pluggable adapters. Document storage (SQLite/SQLCipher in v1) handles structured data; blob storage (local filesystem in v1) handles binary content. Plugins never touch adapters directly — all access flows through a `StorageContext` API that enforces permissions, validates schemas, and emits audit events.

- [[architecture/core/design/data-layer/outline|Outline]] — Scope, design principles, and component overview

## Plugin SDK

The Plugin SDK is the developer-facing toolkit for building workflow plugins — WASM modules that run as steps in pipelines. It provides language bindings, the manifest schema, CDM recommended schemas, host function stubs, and the `PipelineMessage` types.

- [[architecture/core/design/plugin-sdk/outline|Outline]] — Scope, SDK components, language support
- [[architecture/core/design/plugin-sdk/manifest|Manifest]] — Full `manifest.toml` specification
- [[architecture/core/design/plugin-sdk/pipeline-message|PipelineMessage]] — Message shape, metadata, status hints
- [[architecture/core/design/plugin-sdk/host-functions|Host Functions]] — Complete host function reference
- [[architecture/core/design/plugin-sdk/plugin-actions|Plugin Actions]] — Action signatures, lifecycle hooks, connector pattern

## Adapter SDK

The Adapter SDK defines the contract for implementing storage adapters — native Rust trait implementations compiled into Core. It provides trait definitions, shared types, the error model, and a conformance test harness.

- [[architecture/core/design/adapter-sdk/outline|Outline]] — Scope, adapter model, registration, design principles
- [[architecture/core/design/adapter-sdk/document-adapter|Document Adapter]] — Implementation guide for `DocumentStorageAdapter`
- [[architecture/core/design/adapter-sdk/blob-adapter|Blob Adapter]] — Implementation guide for `BlobStorageAdapter`

## Cross-Layer Contracts

The layers connect through a small set of shared types:

- **WorkflowRequest / WorkflowResponse** — The contract between transport and workflow engine
- **PipelineMessage** — The envelope passed between plugin steps within a workflow
- **StorageContext** — The scoped API plugins use to read and write data
- **Host functions** — The capabilities Core exports into each plugin's WASM sandbox
