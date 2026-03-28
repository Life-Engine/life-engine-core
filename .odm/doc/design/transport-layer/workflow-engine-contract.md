---
title: Workflow Engine Contract
type: reference
created: 2026-03-28
status: draft
---

# Workflow Engine Contract

## Overview

The workflow engine is a black box with a well-defined contract. Handlers translate to and from the wire format — the workflow engine never thinks about HTTP or GraphQL.

```
Handler → WorkflowRequest → Workflow Engine → WorkflowResponse → Handler
```

## Input — WorkflowRequest

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

## Output — WorkflowResponse

```rust
pub struct WorkflowResponse {
    pub status: WorkflowStatus,
    pub data: Option<Value>,
    pub errors: Vec<WorkflowError>,
    pub meta: ResponseMeta,                  // request id echo, timing
}
```

## WorkflowStatus

Variants must earn their place by carrying distinct semantics across multiple protocols (Rule A). If a distinction only matters in one transport, the handler infers it from context.

Current variants:

- **Ok** — Success with data
- **Created** — Resource created
- **NotFound** — Requested resource does not exist
- **Denied** — Identity lacks permission
- **Invalid** — Request is malformed or fails validation
- **Error** — Internal failure

This list is intentionally minimal. Candidates for addition (`Conflict`, `Accepted`) are noted but not confirmed — they will be evaluated against Rule A when the need arises.

## System Workflows

Core ships default system workflows for generic CRUD:

- `collection.list` — List items in a collection
- `collection.get` — Get a single item by ID
- `collection.create` — Create a new item
- `collection.update` — Update an existing item
- `collection.delete` — Delete an item
- `graphql.query` — Resolve a GraphQL query
- `system.health` — Health check

These are real workflow definitions — simple pass-throughs to storage by default. The user can edit them like any other workflow to insert validation, transformation, or logging steps. There are no special cases or shortcuts.

## Auth Model (v1)

- Authentication happens at the transport boundary (auth middleware)
- `Identity` is always present in `WorkflowRequest` (except public routes, which skip auth entirely)
- Authorisation is all-or-nothing: authenticated = authorised
- Per-collection ACLs are deferred to a future multi-user iteration
