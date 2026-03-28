---
title: Handler
type: reference
created: 2026-03-28
status: draft
---

# Handler

## Role

A handler translates between a wire protocol and the [[workflow-engine-contract|WorkflowRequest/WorkflowResponse]] contract. The handler is the boundary between protocol-specific concerns and protocol-agnostic workflow execution.

## Handler Types (v1)

- **REST** — Translates HTTP method, path, query params, and JSON body into a `WorkflowRequest`. Translates `WorkflowResponse` into HTTP status code and JSON body.
- **GraphQL** — Parses a GraphQL query, flattens arguments into the `query` field of `WorkflowRequest`. Translates `WorkflowResponse` into `{ data, errors }` shape.

## Translation Principle

Handlers map `WorkflowStatus` to wire format. A `WorkflowStatus` variant only exists if it means something distinct across multiple protocols (Rule A). If a distinction only matters in one protocol, the handler infers it from context.

Current variants (to be mapped out concretely):

- `Ok`
- `Created`
- `NotFound`
- `Denied`
- `Invalid`
- `Error`

Additional variants may be added following Rule A — they must carry distinct semantics in at least two handler types. Candidates identified but not yet confirmed:

- `Conflict` — concurrent write collision
- `Accepted` — async workflow acknowledged

## REST Handler

Translates `WorkflowRequest` fields from HTTP:

- `workflow` — From route config (e.g., route `/api/v1/data/:collection` maps to `collection.list`)
- `identity` — From auth middleware (`Extension<Identity>`)
- `params` — From path parameters (`:collection`, `:id`)
- `query` — From URL query string (`?offset=0&limit=10`)
- `body` — From JSON request body
- `meta` — Request ID, timestamp, source binding

Translates `WorkflowResponse` to HTTP:

- `status` → HTTP status code
- `data` → JSON body wrapped in `{ "data": ... }`
- `errors` → JSON body wrapped in `{ "error": { "code": "...", "message": "..." } }`

## GraphQL Handler

Translates `WorkflowRequest` fields from GraphQL:

- `workflow` — Always `graphql.query` from the route
- `identity` — From auth middleware
- `params` — Empty (no path parameters)
- `query` — Flattened from GraphQL arguments (limit, offset, filters)
- `body` — The GraphQL query/mutation itself
- `meta` — Request ID, timestamp, source binding

Translates `WorkflowResponse` to GraphQL:

- `data` → `{ "data": ... }`
- `errors` → `{ "errors": [...] }`

## GraphQL Schema Generation

The GraphQL handler generates its schema at startup from plugin manifest-declared schemas. Any collection whose plugin declares a schema in its manifest becomes a queryable GraphQL type.

- Collections without a declared schema are not queryable via GraphQL (still accessible via REST generic CRUD)
- This gives plugin authors a natural incentive to declare schemas without forcing them
- The schema reflects whatever collections actually exist — no enforced canonical types

## Same Workflows, Both Transports

REST and GraphQL resolve through the same system workflows. A `collection.list` request produces the same result regardless of which transport initiated it. Any plugins or hooks attached to a system workflow apply equally to both transports.
