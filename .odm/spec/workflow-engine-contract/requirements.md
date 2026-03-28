<!--
domain: workflow-engine-contract
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Requirements Document — Workflow Engine Contract

## Introduction

This document specifies the requirements for the contract between transport handlers and the workflow engine in Life Engine Core. The contract consists of two data structures (`WorkflowRequest` and `WorkflowResponse`), a status enum (`WorkflowStatus`), seven system workflows, handler translation rules, and a v1 auth model. The workflow engine is protocol-agnostic — handlers are responsible for all wire-format translation.

## Alignment with Product Vision

- **Protocol Agnosticism** — The workflow engine processes requests without knowledge of the underlying transport protocol, enabling REST, GraphQL, and future bindings to share one engine
- **Pit of Success** — The fixed `WorkflowRequest`/`WorkflowResponse` pair eliminates guesswork for handler developers; every field has clear rules
- **Minimal Surface** — The `WorkflowStatus` enum only grows when a variant carries distinct semantics across multiple protocols (Rule A), preventing status bloat
- **No Special Cases** — System workflows are real definitions, editable like any other workflow, ensuring uniform engine behaviour
- **Defence in Depth** — Authentication at the transport boundary ensures identity is established before the workflow engine processes any request

## Requirements

### Requirement 1 — WorkflowRequest Structure

**User Story:** As a handler developer, I want a single, well-defined request structure so that I can translate any incoming protocol request into one consistent format for the workflow engine.

#### Acceptance Criteria

- 1.1. WHEN a handler dispatches a request to the workflow engine THEN it SHALL construct a `WorkflowRequest` containing all six required fields: `workflow`, `identity`, `params`, `query`, `body`, and `meta`.
- 1.2. WHEN the `workflow` field is populated THEN it SHALL contain a dot-separated workflow name resolved from the route config (e.g. `"collection.list"`, `"graphql.query"`).
- 1.3. WHEN the request targets an authenticated route THEN the `identity` field SHALL contain the verified identity from the auth middleware.
- 1.4. WHEN the request targets a public route THEN the `identity` field SHALL still be present but carry a guest or anonymous identity marker.
- 1.5. WHEN a REST handler extracts path parameters THEN it SHALL populate the `params` map with segments like `:collection` and `:id`.
- 1.6. WHEN a GraphQL handler processes a request THEN it SHALL leave the `params` map empty.
- 1.7. WHEN a REST handler receives URL query string parameters THEN it SHALL populate the `query` map from those parameters.
- 1.8. WHEN a GraphQL handler processes arguments THEN it SHALL populate the `query` map by flattening GraphQL arguments (limit, offset, filters).
- 1.9. WHEN a REST handler receives a JSON request body THEN it SHALL populate the `body` field with the parsed JSON value.
- 1.10. WHEN a GraphQL handler receives a query or mutation THEN it SHALL populate the `body` field with the GraphQL query string.
- 1.11. WHEN a request has no body THEN the `body` field SHALL be `None`.
- 1.12. WHEN the `meta` field is populated THEN it SHALL contain the request ID, timestamp, and source binding identifier.

### Requirement 2 — WorkflowResponse Structure

**User Story:** As a handler developer, I want a single, well-defined response structure so that I can translate every workflow result back to the correct wire format.

#### Acceptance Criteria

- 2.1. WHEN the workflow engine completes processing THEN it SHALL return a `WorkflowResponse` containing all four fields: `status`, `data`, `errors`, and `meta`.
- 2.2. WHEN the workflow succeeds THEN `data` SHALL contain the result payload and `errors` SHALL be empty.
- 2.3. WHEN the workflow fails THEN `errors` SHALL contain one or more `WorkflowError` values and `data` SHALL be `None`.
- 2.4. WHEN the response `meta` is populated THEN it SHALL echo the request ID from the originating `WorkflowRequest` and include timing information.

### Requirement 3 — WorkflowStatus Enum

**User Story:** As a Core developer, I want a minimal status enum so that every variant carries distinct semantics across multiple protocols without protocol-specific bloat.

#### Acceptance Criteria

- 3.1. WHEN the system defines `WorkflowStatus` THEN it SHALL include exactly six variants: `Ok`, `Created`, `NotFound`, `Denied`, `Invalid`, and `Error`.
- 3.2. WHEN `Ok` is returned THEN it SHALL indicate success with data — the request completed and the result is in `data`.
- 3.3. WHEN `Created` is returned THEN it SHALL indicate a new resource was persisted and the created resource is in `data`.
- 3.4. WHEN `NotFound` is returned THEN it SHALL indicate the requested resource does not exist and `errors` SHALL include a descriptive message.
- 3.5. WHEN `Denied` is returned THEN it SHALL indicate the identity lacks permission — the request was authenticated but not authorised.
- 3.6. WHEN `Invalid` is returned THEN it SHALL indicate the request is malformed or fails validation and `errors` SHALL include details about what failed.
- 3.7. WHEN `Error` is returned THEN it SHALL indicate an internal failure — something unexpected went wrong inside the workflow engine.
- 3.8. WHEN a new variant is proposed THEN it SHALL only be added if it carries distinct semantics in at least two handler types (Rule A).

### Requirement 4 — System Workflows

**User Story:** As a plugin author, I want system workflows to be editable first-class definitions so that I can customise standard CRUD operations with validation, transformation, or logging steps.

#### Acceptance Criteria

- 4.1. WHEN Core starts THEN it SHALL register seven system workflows: `collection.list`, `collection.get`, `collection.create`, `collection.update`, `collection.delete`, `graphql.query`, and `system.health`.
- 4.2. WHEN a system workflow is dispatched THEN the workflow engine SHALL treat it identically to any plugin-defined workflow — no special-case handling.
- 4.3. WHEN a user edits a system workflow THEN the engine SHALL execute the modified definition, including any inserted validation, transformation, or logging steps.
- 4.4. WHEN a system workflow is unmodified THEN it SHALL act as a simple pass-through to storage.

### Requirement 5 — REST Handler Translation

**User Story:** As a handler developer, I want deterministic REST translation rules so that every `WorkflowStatus` maps to exactly one HTTP status code.

#### Acceptance Criteria

- 5.1. WHEN the REST handler receives `WorkflowStatus::Ok` THEN it SHALL respond with HTTP 200.
- 5.2. WHEN the REST handler receives `WorkflowStatus::Created` THEN it SHALL respond with HTTP 201.
- 5.3. WHEN the REST handler receives `WorkflowStatus::NotFound` THEN it SHALL respond with HTTP 404.
- 5.4. WHEN the REST handler receives `WorkflowStatus::Denied` THEN it SHALL respond with HTTP 403.
- 5.5. WHEN the REST handler receives `WorkflowStatus::Invalid` THEN it SHALL respond with HTTP 400.
- 5.6. WHEN the REST handler receives `WorkflowStatus::Error` THEN it SHALL respond with HTTP 500.

### Requirement 6 — GraphQL Handler Translation

**User Story:** As a handler developer, I want deterministic GraphQL translation rules so that every `WorkflowStatus` maps to the correct GraphQL response shape.

#### Acceptance Criteria

- 6.1. WHEN the GraphQL handler receives `WorkflowStatus::Ok` THEN it SHALL respond with `{ "data": ... }` and no errors.
- 6.2. WHEN the GraphQL handler receives `WorkflowStatus::Created` THEN it SHALL respond with `{ "data": ... }` and no errors.
- 6.3. WHEN the GraphQL handler receives `WorkflowStatus::NotFound` THEN it SHALL respond with `{ "data": null, "errors": [...] }`.
- 6.4. WHEN the GraphQL handler receives `WorkflowStatus::Denied` THEN it SHALL respond with `{ "data": null, "errors": [...] }` with a `FORBIDDEN` extension code.
- 6.5. WHEN the GraphQL handler receives `WorkflowStatus::Invalid` THEN it SHALL respond with `{ "data": null, "errors": [...] }` with a `BAD_USER_INPUT` extension code.
- 6.6. WHEN the GraphQL handler receives `WorkflowStatus::Error` THEN it SHALL respond with `{ "data": null, "errors": [...] }` with an `INTERNAL_SERVER_ERROR` extension code.

### Requirement 7 — Auth Model (v1)

**User Story:** As a Core developer, I want a simple v1 auth model so that identity is always available to the workflow engine without complex permission checks.

#### Acceptance Criteria

- 7.1. WHEN a request arrives at an authenticated route THEN the transport auth middleware SHALL authenticate the identity before constructing the `WorkflowRequest`.
- 7.2. WHEN a request arrives at a public route THEN the handler SHALL skip auth entirely and populate `identity` with a guest marker.
- 7.3. WHEN the workflow engine processes a request with a verified identity THEN it SHALL treat the request as authorised — no per-collection or per-operation permission checks in v1.
- 7.4. WHEN per-collection ACLs are needed THEN they SHALL be deferred to a future multi-user iteration.
