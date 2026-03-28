<!--
domain: workflow-engine-contract
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Tasks — Workflow Engine Contract

**Progress:** 0 / 14 tasks complete

## 1.1 — Crate Scaffold and Core Types

- [ ] Create `packages/workflow-contract` crate with `Cargo.toml` and `src/lib.rs`
  <!-- files: packages/workflow-contract/Cargo.toml, packages/workflow-contract/src/lib.rs -->
  <!-- purpose: Scaffold the crate with serde, serde_json, chrono, and uuid dependencies; lib.rs re-exports all public modules -->
  <!-- requirements: R1, R2, R3 -->

- [ ] Implement `WorkflowStatus` enum in `src/status.rs`
  <!-- files: packages/workflow-contract/src/status.rs -->
  <!-- purpose: Define the six-variant enum (Ok, Created, NotFound, Denied, Invalid, Error) with Serialize/Deserialize derives -->
  <!-- requirements: R3 -->

- [ ] Implement `Identity` and `IdentityKind` in `src/identity.rs`
  <!-- files: packages/workflow-contract/src/identity.rs -->
  <!-- purpose: Define Identity struct with subject and kind fields, IdentityKind enum (Authenticated, Guest, System), convenience constructors -->
  <!-- requirements: R1, R7 -->

- [ ] Implement `WorkflowRequest` and `RequestMeta` in `src/request.rs`
  <!-- files: packages/workflow-contract/src/request.rs -->
  <!-- purpose: Define WorkflowRequest with workflow, identity, params, query, body, meta fields; RequestMeta with request_id, timestamp, source -->
  <!-- requirements: R1 -->

- [ ] Implement `WorkflowResponse`, `ResponseMeta`, and `WorkflowError` in `src/response.rs`
  <!-- files: packages/workflow-contract/src/response.rs -->
  <!-- purpose: Define WorkflowResponse with status, data, errors, meta fields; ResponseMeta with request_id and duration_ms; WorkflowError with code, message, field -->
  <!-- requirements: R2 -->

## 1.2 — Unit Tests for Contract Types

- [ ] Add unit tests for `WorkflowStatus` serialization round-trip
  <!-- files: packages/workflow-contract/src/status.rs -->
  <!-- purpose: Verify all six variants serialize/deserialize correctly; confirm enum is exhaustive -->
  <!-- requirements: R3 -->

- [ ] Add unit tests for `WorkflowRequest` construction and field validation
  <!-- files: packages/workflow-contract/src/request.rs -->
  <!-- purpose: Verify request construction with all field combinations; test empty params/query for GraphQL, populated params for REST -->
  <!-- requirements: R1 -->

- [ ] Add unit tests for `WorkflowResponse` success and error shapes
  <!-- files: packages/workflow-contract/src/response.rs -->
  <!-- purpose: Verify success responses have data and empty errors; error responses have errors and None data; meta echoes request_id -->
  <!-- requirements: R2 -->

- [ ] Add unit tests for `Identity` variants and constructors
  <!-- files: packages/workflow-contract/src/identity.rs -->
  <!-- purpose: Verify Authenticated, Guest, and System identity construction; confirm subject values are correct -->
  <!-- requirements: R1, R7 -->

## 1.3 — Handler Translation Logic

- [ ] Implement REST status-to-HTTP mapping function
  <!-- files: packages/workflow-contract/src/status.rs -->
  <!-- purpose: Add a method or free function mapping each WorkflowStatus variant to its HTTP status code (200, 201, 404, 403, 400, 500) -->
  <!-- requirements: R5 -->

- [ ] Implement GraphQL status-to-response-shape mapping function
  <!-- files: packages/workflow-contract/src/status.rs -->
  <!-- purpose: Add a method or free function returning the GraphQL extension code (None for Ok/Created/NotFound, FORBIDDEN, BAD_USER_INPUT, INTERNAL_SERVER_ERROR) -->
  <!-- requirements: R6 -->

- [ ] Add unit tests for REST and GraphQL translation functions
  <!-- files: packages/workflow-contract/src/status.rs -->
  <!-- purpose: Verify every WorkflowStatus variant maps to the correct HTTP code and GraphQL extension code -->
  <!-- requirements: R5, R6 -->

## 1.4 — System Workflow Registration

- [ ] Define system workflow name constants and registration list
  <!-- files: packages/workflow-contract/src/lib.rs -->
  <!-- purpose: Export the seven system workflow names as constants (COLLECTION_LIST, COLLECTION_GET, etc.) and a SYSTEM_WORKFLOWS slice for startup registration -->
  <!-- requirements: R4 -->

- [ ] Add unit tests for system workflow constants
  <!-- files: packages/workflow-contract/src/lib.rs -->
  <!-- purpose: Verify the seven system workflow names match their expected dot-separated values; confirm SYSTEM_WORKFLOWS contains all seven -->
  <!-- requirements: R4 -->
