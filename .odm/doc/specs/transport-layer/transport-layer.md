---
title: Transport Layer Specification
type: reference
created: 2026-03-28
status: active
tags:
  - life-engine
  - transport
  - rest
  - graphql
  - auth
---

# Transport Layer Specification

Part of [[architecture/core/overview|Core Overview]] · [[architecture/core/README|Core Documentation]]

## Introduction

This specification defines the transport layer of Core. The transport layer accepts inbound requests over supported wire protocols, applies middleware, dispatches to the workflow engine via the [[workflow-engine-contract|Workflow Engine Contract]], and translates responses back to the wire format.

Related specifications: [[workflow-engine-contract]], [[cdm-specification]]

## Scope

The transport layer handles two protocols in v1:

- **REST** — Primary transport. JSON over HTTP via Axum.
- **GraphQL** — Alternative query interface. Shares the HTTP binding with REST on a different path prefix.

The following protocols are deferred to future iterations:

- WebSocket (real-time streaming)
- CalDAV / CardDAV (native calendar/contacts sync)
- Webhooks (inbound event receiver)

## Listener Model

Each listener binds one socket. Multiple handler types mount on a single listener, or the user may split them across separate listeners. The user controls this entirely through config.

### Config Structure

The listeners config uses a `handlers` array. New transport types (WebSocket, CalDAV, webhooks) slot into the same structure without migration.

```yaml
listeners:
  - binding: http
    port: 8080
    address: 127.0.0.1
    tls:
      cert: /etc/life/cert.pem
      key: /etc/life/key.pem
    auth:
      verify: token
    handlers:
      - type: rest
        routes: [...]
      - type: graphql
        routes: [...]
```

### Default Config

Core generates a default listener config file on first run. The default config includes:

- Generic CRUD routes for REST (`collection.list`, `collection.get`, `collection.create`, `collection.update`, `collection.delete`)
- A GraphQL endpoint (`graphql.query`)
- A public health check (`system.health`)

The user may modify, remove, or extend any of these. Nothing is hardcoded.

### Plugin Manifest Route Merging

Plugins declare additional routes in their manifest. At startup, Core merges config routes with plugin manifest routes. This is a static merge — adding or removing a plugin requires restarting Core to rebuild the router.

### Route Namespace Validation

At startup, Core must validate that:

- REST routes fall under `/api/`
- GraphQL routes fall under `/graphql`

If a conflict is detected, Core must reject startup with a clear error message (e.g., `"Route conflict: REST route '/graphql' collides with GraphQL handler"`). This validation runs before Axum builds its router, so the user receives a readable error instead of a panic.

### Public Routes

Any route may set `public: true` to skip auth. Core does not enforce which routes must be public — the user decides. The default config ships with one public route:

- `GET /api/v1/health` — Health check

### TLS

TLS is optional per listener. Two fields control it:

- `cert` — Path to certificate file
- `key` — Path to private key file

Core provides no cert renewal, OCSP, or cipher suite configuration. For anything beyond basic TLS, the user must use a reverse proxy.

### Address Binding

- `127.0.0.1` (default) — Localhost only, no external access
- `0.0.0.0` — LAN/internet access. Core must log a startup warning when bound to a non-localhost address.

## Middleware Stack

The transport layer applies middleware in the following order:

1. **TLS** (optional) — For simple setups (LAN, self-signed cert). Points at a cert file and key file. No cert management in Core. Reverse proxy recommended for internet-facing deployments.
2. **CORS** — Permissive on `127.0.0.1`, strict on `0.0.0.0`. The user may configure allowed origins explicitly.
3. **Auth** — Token validation via Pocket ID (OIDC). Routes marked `public: true` skip auth entirely. See the Authentication section below.
4. **Logging** — Structured JSON. All requests must be logged.
5. **Error handling** — Consistent error shape per protocol, translated from [[workflow-engine-contract|WorkflowResponse]].

Rate limiting is deferred to the reverse proxy. It is not needed for single-user localhost deployments.

## Router

### Build Behaviour

The router is built once at startup and is immutable at runtime. It must not be rebuilt or modified while Core is running.

### Route Sources

Routes come from two sources, merged at startup:

- **Listener config** — Explicitly declared routes in the config file, including the default generic CRUD routes shipped with Core.
- **Plugin manifests** — Plugins declare additional routes in their manifest. These are merged into the router alongside config routes.

### Route Matching

The router extracts path parameters (`:collection`, `:id`) and passes them as `params: HashMap<String, String>` in the `WorkflowRequest`.

Each route maps to a workflow by name (e.g., `collection.list`, `collection.get`). The router does not know what the workflow does — it dispatches only.

### Generic CRUD Routes

Core ships five default REST routes for collection-level CRUD:

- `GET /api/v1/data/:collection` → `collection.list`
- `GET /api/v1/data/:collection/:id` → `collection.get`
- `POST /api/v1/data/:collection` → `collection.create`
- `PUT /api/v1/data/:collection/:id` → `collection.update`
- `DELETE /api/v1/data/:collection/:id` → `collection.delete`

These are declared in the default config file, not hardcoded. The `:collection` parameter is dynamic — any collection in storage is accessible. The user may modify or remove any of these routes.

### GraphQL Routing

GraphQL has a single route:

- `POST /graphql` → `graphql.query`

The GraphQL handler internally resolves the query against collections, but from the router's perspective it is a single route to a single workflow.

## Handlers

### REST Handler

The REST handler translates HTTP requests into `WorkflowRequest` fields:

- `workflow` — From route config (e.g., route `/api/v1/data/:collection` maps to `collection.list`)
- `identity` — From auth middleware (`Extension<Identity>`)
- `params` — From path parameters (`:collection`, `:id`)
- `query` — From URL query string (`?offset=0&limit=10`)
- `body` — From JSON request body
- `meta` — Request ID, timestamp, source binding

The REST handler translates `WorkflowResponse` to HTTP:

- `status` → HTTP status code
- `data` → JSON body wrapped in `{ "data": ... }`
- `errors` → JSON body wrapped in `{ "error": { "code": "...", "message": "..." } }`

### GraphQL Handler

The GraphQL handler translates GraphQL requests into `WorkflowRequest` fields:

- `workflow` — Always `graphql.query` from the route
- `identity` — From auth middleware
- `params` — Empty (no path parameters)
- `query` — Flattened from GraphQL arguments (limit, offset, filters)
- `body` — The GraphQL query/mutation itself
- `meta` — Request ID, timestamp, source binding

The GraphQL handler translates `WorkflowResponse` to GraphQL:

- `data` → `{ "data": ... }`
- `errors` → `{ "errors": [...] }`

### GraphQL Schema Generation

The GraphQL handler generates its schema at startup from plugin manifest-declared schemas. Any collection whose plugin declares a schema in its manifest becomes a queryable GraphQL type.

- Collections without a declared schema are not queryable via GraphQL. They remain accessible via REST generic CRUD.
- This gives plugin authors a natural incentive to declare schemas without forcing them.
- The generated schema reflects whatever collections actually exist — no enforced canonical types.

### Same Workflows, Both Transports

REST and GraphQL resolve through the same system workflows. A `collection.list` request must produce the same result regardless of which transport initiated it. Any plugins or hooks attached to a system workflow apply equally to both transports.

## Authentication

### Token Validation

Auth middleware validates tokens via Pocket ID (OIDC) at the transport boundary, as defined in [[adr-004-pocket-id-oidc-auth]]. The transport layer validates tokens — it does not manage users, sessions, or token issuance.

The resulting `Identity` is passed as an Axum `Extension<Identity>` and included in every `WorkflowRequest`.

### Public Routes

Routes marked `public: true` skip auth entirely. The default config ships with one public route:

- `GET /api/v1/health`

### Authorisation (v1)

Authorisation is all-or-nothing for v1:

- Authenticated = authorised to access all collections and workflows
- No per-collection ACLs
- No role-based access control

This is appropriate for a single-user self-hosted system. Per-collection permissions will be considered when multi-user support is added.

### Admin Panel Auth

The admin panel has its own auth mechanism, separate from OIDC:

- Local passphrase, independent of OIDC. This solves the bootstrap problem — the user needs admin panel access to configure OIDC.
- Configured in the admin panel's top-level config section, not in the listeners config.
- On first run, the admin panel is unauthenticated on localhost. The first action is setting a passphrase. After setup, the panel is locked.

## What the Transport Layer Does Not Own

- **Admin panel** — Top-level Core feature with its own config, auth (local passphrase), and first-run setup wizard. Not a transport handler.
- **Workflow execution** — The transport layer dispatches to the workflow engine and translates the response. It never executes plugin logic.
- **Storage** — The transport layer never reads or writes storage directly.
