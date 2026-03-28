<!--
domain: workflow-engine-contract
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Design Document — Workflow Engine Contract

## Introduction

This document describes the technical design for the workflow engine contract — the data structures, enums, and translation rules that form the boundary between transport handlers and the workflow engine. All types live in a shared `packages/workflow-contract` crate. Handlers depend on this crate to construct requests and interpret responses; the workflow engine depends on it to accept requests and produce responses.

## Architecture

The contract enforces a clean separation between protocol-specific handlers and the protocol-agnostic workflow engine.

```
Transport Layer          Contract Boundary          Workflow Engine
─────────────────       ────────────────────       ──────────────────
REST Handler     ──→    WorkflowRequest     ──→    Engine dispatches
GraphQL Handler  ──→                               workflow steps
                        WorkflowResponse    ←──    Engine returns
REST Handler     ←──                               result
GraphQL Handler  ←──
```

Handlers are responsible for:

- Extracting identity from auth middleware
- Mapping route/operation to a workflow name
- Populating params, query, and body from the wire format
- Translating `WorkflowStatus` back to the wire-format response

The workflow engine is responsible for:

- Resolving the workflow name to a workflow definition
- Executing workflow steps (pass-through, plugin steps, etc.)
- Returning a `WorkflowResponse` with the correct status, data, and errors

## Data Structures

### WorkflowRequest

```rust
pub struct WorkflowRequest {
    pub workflow: String,
    pub identity: Identity,
    pub params: HashMap<String, String>,
    pub query: HashMap<String, String>,
    pub body: Option<Value>,
    pub meta: RequestMeta,
}
```

- **workflow** — Dot-separated workflow name resolved by the router from route config. Examples: `"collection.list"`, `"collection.create"`, `"graphql.query"`, `"system.health"`.
- **identity** — Always present. For authenticated routes, contains the verified identity from auth middleware. For public routes, contains a guest or anonymous identity marker.
- **params** — Path parameters extracted by the router. REST handlers populate this with path segments (`:collection`, `:id`). GraphQL handlers leave it empty.
- **query** — Query parameters. REST handlers populate from URL query strings. GraphQL handlers flatten arguments (limit, offset, filters) into this map.
- **body** — Optional. REST handlers set this to the parsed JSON body. GraphQL handlers set it to the query/mutation string. `None` when no body is present.
- **meta** — Request ID (UUID v7), timestamp, and source binding identifier (e.g. `"rest"`, `"graphql"`).

### RequestMeta

```rust
pub struct RequestMeta {
    pub request_id: String,
    pub timestamp: DateTime<Utc>,
    pub source: String,
}
```

- **request_id** — UUID v7 generated at the handler layer, unique per request.
- **timestamp** — UTC timestamp of when the handler received the request.
- **source** — Binding identifier indicating which handler created the request (e.g. `"rest"`, `"graphql"`).

### WorkflowResponse

```rust
pub struct WorkflowResponse {
    pub status: WorkflowStatus,
    pub data: Option<Value>,
    pub errors: Vec<WorkflowError>,
    pub meta: ResponseMeta,
}
```

- **status** — One of the six `WorkflowStatus` variants.
- **data** — Present on success, `None` on error.
- **errors** — Empty on success, one or more `WorkflowError` values on failure.
- **meta** — Echoes the request ID and includes engine-side timing.

### ResponseMeta

```rust
pub struct ResponseMeta {
    pub request_id: String,
    pub duration_ms: u64,
}
```

- **request_id** — Echoed from the originating `WorkflowRequest.meta.request_id`.
- **duration_ms** — Time in milliseconds from when the engine received the request to when it produced the response.

### WorkflowStatus

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

Rule A governs additions: a new variant shall only be added if it carries distinct semantics in at least two handler types. Candidates identified but not yet confirmed:

- `Conflict` — concurrent write collision
- `Accepted` — async workflow acknowledged

### WorkflowError

```rust
pub struct WorkflowError {
    pub code: String,
    pub message: String,
    pub field: Option<String>,
}
```

- **code** — Machine-readable error code (e.g. `"NOT_FOUND"`, `"VALIDATION_FAILED"`).
- **message** — Human-readable description.
- **field** — Optional. Identifies the field that caused the error, used for validation errors.

### Identity

```rust
pub struct Identity {
    pub subject: String,
    pub kind: IdentityKind,
}

pub enum IdentityKind {
    Authenticated,
    Guest,
    System,
}
```

- **subject** — Unique identifier for the identity (user ID for authenticated, `"guest"` for public routes, `"system"` for internal engine operations).
- **kind** — Discriminator for the identity type. `System` is used for internal engine-initiated operations.

## Handler Translation Rules

### REST Handler

The REST handler maps `WorkflowStatus` to HTTP status codes.

- **Ok** — HTTP 200 with JSON body from `data`
- **Created** — HTTP 201 with JSON body from `data`
- **NotFound** — HTTP 404 with JSON error body from `errors`
- **Denied** — HTTP 403 with JSON error body from `errors`
- **Invalid** — HTTP 400 with JSON error body from `errors`
- **Error** — HTTP 500 with JSON error body from `errors`

REST error response shape:

```json
{
  "errors": [
    {
      "code": "NOT_FOUND",
      "message": "Collection 'tasks' has no item with id '123'",
      "field": null
    }
  ],
  "meta": {
    "request_id": "01956...",
    "duration_ms": 4
  }
}
```

### GraphQL Handler

The GraphQL handler maps `WorkflowStatus` to GraphQL response shapes.

- **Ok** — `{ "data": ... }` with no errors
- **Created** — `{ "data": ... }` with no errors
- **NotFound** — `{ "data": null, "errors": [...] }` with no extension code
- **Denied** — `{ "data": null, "errors": [...] }` with `extensions.code = "FORBIDDEN"`
- **Invalid** — `{ "data": null, "errors": [...] }` with `extensions.code = "BAD_USER_INPUT"`
- **Error** — `{ "data": null, "errors": [...] }` with `extensions.code = "INTERNAL_SERVER_ERROR"`

GraphQL error response shape:

```json
{
  "data": null,
  "errors": [
    {
      "message": "Collection 'tasks' has no item with id '123'",
      "extensions": {
        "code": "FORBIDDEN"
      }
    }
  ]
}
```

## System Workflows

Core registers seven system workflows at startup.

- **collection.list** — Lists items in a collection. Default behaviour: pass-through to storage list operation.
- **collection.get** — Gets a single item by ID. Default behaviour: pass-through to storage get operation.
- **collection.create** — Creates a new item. Default behaviour: pass-through to storage create operation.
- **collection.update** — Updates an existing item. Default behaviour: pass-through to storage update operation.
- **collection.delete** — Deletes an item. Default behaviour: pass-through to storage delete operation.
- **graphql.query** — Resolves a GraphQL query. Default behaviour: pass-through to GraphQL resolution logic.
- **system.health** — Health check. Default behaviour: returns `Ok` with engine status.

System workflows are stored as regular workflow definitions. Users can edit them to insert additional steps (validation, transformation, logging). The workflow engine resolves them by name, with no special-case code paths.

## Crate Layout

All contract types live in `packages/workflow-contract`:

- `packages/workflow-contract/src/lib.rs` — Re-exports all public types
- `packages/workflow-contract/src/request.rs` — `WorkflowRequest`, `RequestMeta`
- `packages/workflow-contract/src/response.rs` — `WorkflowResponse`, `ResponseMeta`, `WorkflowError`
- `packages/workflow-contract/src/status.rs` — `WorkflowStatus` enum
- `packages/workflow-contract/src/identity.rs` — `Identity`, `IdentityKind`

Both handler crates and the workflow engine crate depend on `packages/workflow-contract`. This ensures the contract types are the sole coupling point.

## Conventions

- All struct fields use `snake_case`.
- `Value` refers to `serde_json::Value`.
- `DateTime<Utc>` refers to `chrono::DateTime<Utc>`.
- Request IDs use UUID v7 for time-sortable uniqueness.
- Workflow names use dot-separated segments: `{domain}.{operation}`.
- Error codes use `UPPER_SNAKE_CASE`.
