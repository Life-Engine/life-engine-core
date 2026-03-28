---
title: Transport Layer Outline
type: reference
created: 2026-03-28
status: draft
---

# Transport Layer Outline

## Scope (v1)

The transport layer handles two protocols:

- **REST** — Primary transport. JSON over HTTP via Axum.
- **GraphQL** — Alternative query interface. Shares the HTTP binding with REST on a different path.

The following are deferred to future-considerations:

- WebSocket (real-time streaming)
- CalDAV / CardDAV (native calendar/contacts)
- Webhooks (inbound event receiver)

## Request Flow

```
HTTP Request (bytes)
  ↓
Listener (one socket, one or more handlers)
  ↓
TLS termination (optional)
  ↓
CORS middleware
  ↓
Auth middleware (token validation — skip if route is public)
  ↓
Route namespace validation (REST under /api/, GraphQL under /graphql)
  ↓
Router (match path → handler → workflow)
  ↓
Handler (protocol-specific → WorkflowRequest)
  ↓
Workflow Engine (execute system or plugin workflow)
  ↓
Handler (WorkflowResponse → protocol-specific response)
  ↓
HTTP Response (bytes on wire)
```

## Design Principles

- **One path** — Every request goes through the workflow engine. No shortcuts for generic CRUD. REST and GraphQL resolve through the same system workflows.
- **Complete configurability** — Everything is declared in config and editable by the user. No hardcoded routes, no enforced public routes, no hidden behaviour.
- **Protocol-agnostic workflows** — Handlers translate wire format to [[workflow-engine-contract|WorkflowRequest/WorkflowResponse]]. The workflow engine never thinks about HTTP or GraphQL.
- **Schema-flexible** — Collections are not enforced to follow canonical data types. The CDM provides recommended schemas for interoperability. Plugins choose whether to adopt them.
- **Sensible defaults** — Core ships a default config with generic CRUD routes and system workflows. Users can modify everything.

## Middleware Stack (v1)

The transport layer applies the following middleware in order:

- **TLS** — Optional. For simple setups (LAN, self-signed cert). Point at a cert file and key file. No cert management in Core. Reverse proxy recommended for internet-facing deployments.
- **CORS** — Smart defaults. Permissive on `127.0.0.1`, strict on `0.0.0.0`. Configurable allowed origins.
- **Auth** — Token validation via Pocket ID (OIDC). Authenticated = authorised for v1 (all-or-nothing, no per-collection ACLs). Routes marked `public: true` skip auth.
- **Logging** — Structured JSON, all requests logged.
- **Error handling** — Consistent error shape per protocol, translated from [[workflow-engine-contract|WorkflowResponse]].

The following are deferred:

- **Rate limiting** — Defer to reverse proxy. Not needed for single-user localhost.

## What the Transport Layer Does Not Own

- **Admin panel** — Top-level Core feature with its own config, auth (local passphrase), and first-run setup wizard. Not a transport handler.
- **Workflow execution** — The transport layer dispatches to the workflow engine and translates the response. It never executes plugin logic.
- **Storage** — The transport layer never reads or writes storage directly.
