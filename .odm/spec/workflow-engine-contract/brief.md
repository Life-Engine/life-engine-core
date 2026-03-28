<!--
domain: workflow-engine-contract
updated: 2026-03-28
-->

# Workflow Engine Contract Spec

## Overview

This spec defines the contract between transport handlers and the workflow engine. The workflow engine is a black box — handlers translate incoming requests (HTTP, GraphQL, or future protocols) into a `WorkflowRequest`, dispatch it, and translate the returned `WorkflowResponse` back to the wire format. The contract ensures protocol-agnostic processing: the workflow engine never thinks about HTTP status codes or GraphQL error extensions.

Core ships seven system workflows covering CRUD operations, GraphQL query resolution, and health checks. These are real workflow definitions, not shortcuts — they are editable and treated identically to plugin-defined workflows. Authentication happens at the transport boundary before requests reach the workflow engine. In v1, authorisation is all-or-nothing: authenticated equals authorised.

## Goals

- Define `WorkflowRequest` and `WorkflowResponse` as the sole communication structures between handlers and the workflow engine
- Ensure the workflow engine is fully protocol-agnostic — no HTTP or GraphQL concepts leak inward
- Provide a minimal, semantically distinct `WorkflowStatus` enum with variants that carry meaning across multiple protocols
- Ship seven system workflows as editable, first-class workflow definitions
- Establish deterministic handler translation rules mapping `WorkflowStatus` to REST and GraphQL wire formats
- Enforce that `Identity` is always present in requests, with guest markers for public routes
- Support extensibility by defining rules for adding new status variants

## User Stories

- As a handler developer, I want a single `WorkflowRequest` structure so that I can translate any protocol into one consistent format for the workflow engine.
- As a handler developer, I want deterministic status-to-wire-format mappings so that every `WorkflowStatus` variant produces the correct HTTP status code or GraphQL error shape.
- As a workflow engine developer, I want protocol-agnostic request and response types so that the engine logic has no dependency on transport concerns.
- As a plugin author, I want system workflows to be editable so that I can insert validation, transformation, or logging steps into standard CRUD operations.
- As a Core developer, I want a minimal status enum governed by a clear addition rule so that the contract does not bloat with protocol-specific distinctions.
- As a Core developer, I want `Identity` always present in `WorkflowRequest` so that the workflow engine can rely on it without null checks.

## Functional Requirements

- The system must accept a `WorkflowRequest` containing workflow name, identity, params, query, optional body, and request metadata for every dispatch to the workflow engine.
- The system must return a `WorkflowResponse` containing status, optional data, error list, and response metadata for every workflow execution.
- The `WorkflowStatus` enum must include exactly six variants: `Ok`, `Created`, `NotFound`, `Denied`, `Invalid`, and `Error`.
- New `WorkflowStatus` variants must only be added if they carry distinct semantics in at least two handler types (Rule A).
- The REST handler must translate each `WorkflowStatus` to its defined HTTP status code (200, 201, 404, 403, 400, 500).
- The GraphQL handler must translate each `WorkflowStatus` to its defined response shape, including extension codes for error variants.
- Core must ship seven system workflows: `collection.list`, `collection.get`, `collection.create`, `collection.update`, `collection.delete`, `graphql.query`, and `system.health`.
- System workflows must be editable and treated identically to plugin-defined workflows by the engine.
- `Identity` must always be present in `WorkflowRequest` — public routes use a guest or anonymous identity marker.
- Authentication must occur at the transport boundary before the workflow engine receives the request.
- In v1, authorisation is all-or-nothing: authenticated equals authorised, with no per-collection ACLs.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
