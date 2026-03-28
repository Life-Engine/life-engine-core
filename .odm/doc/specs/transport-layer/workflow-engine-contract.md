---
title: Workflow Engine Contract Specification
type: reference
created: 2026-03-28
status: active
tags:
  - life-engine
  - workflow
  - contract
  - transport
---

# Workflow Engine Contract Specification

Part of [[architecture/core/overview|Core Overview]] · [[architecture/core/README|Core Documentation]]

## Introduction

The workflow engine is a black box with a well-defined contract. Handlers translate to and from the wire format — the workflow engine never thinks about HTTP or GraphQL. This specification defines the exact data structures and rules governing that contract.

Related specifications: [[transport-layer]], [[cdm-specification]]

```
Handler → WorkflowRequest → Workflow Engine → WorkflowResponse → Handler
```

## WorkflowRequest

Every request dispatched to the workflow engine must use this structure:

```rust
pub struct WorkflowRequest {
    pub workflow: String,                    // e.g. "collection.list"
    pub identity: Identity,                  // verified, always present
    pub params: HashMap<String, String>,     // path params — :collection, :id
    pub query: HashMap<String, String>,      // query string / GraphQL args
    pub body: Option<Value>,                 // parsed request body
    pub meta: RequestMeta,                   // request id, timestamp, source binding
}
```

Field rules:

- `workflow` — Required. A dot-separated workflow name (e.g. `"collection.list"`, `"graphql.query"`, `"system.health"`). The router resolves this from the route config.
- `identity` — Required for authenticated routes. For public routes (where auth is skipped), this field must still be present but may carry a guest or anonymous identity marker.
- `params` — Path parameters extracted by the router. For REST, this includes segments like `:collection` and `:id`. For GraphQL, this is empty.
- `query` — For REST, populated from URL query string parameters. For GraphQL, populated by flattening GraphQL arguments (limit, offset, filters).
- `body` — Optional. For REST, the parsed JSON request body. For GraphQL, the GraphQL query/mutation string.
- `meta` — Required. Contains the request ID, timestamp, and source binding identifier.

## WorkflowResponse

The workflow engine must return this structure for every request:

```rust
pub struct WorkflowResponse {
    pub status: WorkflowStatus,
    pub data: Option<Value>,
    pub errors: Vec<WorkflowError>,
    pub meta: ResponseMeta,                  // request id echo, timing
}
```

Field rules:

- `status` — Required. One of the `WorkflowStatus` variants defined below.
- `data` — Optional. The response payload. Present on success, absent on error.
- `errors` — A list of `WorkflowError` values. Empty on success, one or more on failure.
- `meta` — Required. Echoes the request ID from the originating `WorkflowRequest` and includes timing information.

## WorkflowStatus

A `WorkflowStatus` variant only exists if it carries distinct semantics across multiple protocols (Rule A). If a distinction only matters in one protocol, the handler infers it from context.

Current variants:

- **Ok** — Success with data. The request completed and `data` contains the result.
- **Created** — Resource created. A new item was persisted. `data` contains the created resource.
- **NotFound** — Requested resource does not exist. `errors` must include a descriptive message.
- **Denied** — Identity lacks permission. The request was authenticated but not authorised.
- **Invalid** — Request is malformed or fails validation. `errors` must include details about what failed.
- **Error** — Internal failure. Something unexpected went wrong inside the workflow engine.

```rust
pub enum WorkflowStatus {
    Ok,
    Created,
    NotFound,
    Denied,
    Invalid,
    Error,
}
```

Candidates identified but not yet confirmed (must be evaluated against Rule A when the need arises):

- `Conflict` — Concurrent write collision
- `Accepted` — Async workflow acknowledged

New variants shall only be added if they carry distinct semantics in at least two handler types.

## System Workflows

Core ships seven default system workflows:

- `collection.list` — List items in a collection
- `collection.get` — Get a single item by ID
- `collection.create` — Create a new item
- `collection.update` — Update an existing item
- `collection.delete` — Delete an item
- `graphql.query` — Resolve a GraphQL query
- `system.health` — Health check

These are real workflow definitions — simple pass-throughs to storage by default. The user may edit them like any other workflow to insert validation, transformation, or logging steps. There are no special cases or shortcuts. The workflow engine treats system workflows identically to plugin-defined workflows.

## Auth Model (v1)

- Authentication happens at the transport boundary (auth middleware), before the request reaches the workflow engine.
- `Identity` must always be present in `WorkflowRequest`, except for public routes which skip auth entirely.
- Authorisation is all-or-nothing: authenticated = authorised. The workflow engine does not perform any permission checks in v1.
- Per-collection ACLs are deferred to a future multi-user iteration.

## Handler Translation Rules

Each handler must translate `WorkflowStatus` to the appropriate wire-format response.

REST handler status mapping:

- `Ok` → HTTP 200
- `Created` → HTTP 201
- `NotFound` → HTTP 404
- `Denied` → HTTP 403
- `Invalid` → HTTP 400
- `Error` → HTTP 500

GraphQL handler status mapping:

- `Ok` → `{ "data": ... }` with no errors
- `Created` → `{ "data": ... }` with no errors
- `NotFound` → `{ "data": null, "errors": [...] }`
- `Denied` → `{ "data": null, "errors": [...] }` with `FORBIDDEN` extension code
- `Invalid` → `{ "data": null, "errors": [...] }` with `BAD_USER_INPUT` extension code
- `Error` → `{ "data": null, "errors": [...] }` with `INTERNAL_SERVER_ERROR` extension code
