---
title: "Core — Transport Layer"
tags: [life-engine, core, transports, rest, graphql, caldav, carddav, webhook]
created: 2026-03-23
---

# Transport Layer

Transports are protocol-specific entry points into Core. Each transport is an independent crate that implements the `Transport` trait. The admin configures which transports are active — Core starts only those.

Transports receive requests, authenticate them, and route them to the workflow engine. They do not contain business logic.

## Available Transports

- **REST** (`packages/transport-rest`) — JSON over HTTP via axum. The primary transport for client applications.
- **GraphQL** (`packages/transport-graphql`) — GraphQL via async-graphql. Alternative query interface.
- **CalDAV** (`packages/transport-caldav`) — CalDAV protocol for native calendar app compatibility.
- **CardDAV** (`packages/transport-carddav`) — CardDAV protocol for native contacts app compatibility.
- **Webhook** (`packages/transport-webhook`) — Inbound webhook receiver. Triggers workflows from external services.

## Configuration

Transports are enabled in `config.toml`. Only listed transports are started:

```toml
[transports.rest]
port = 3000

[transports.graphql]
port = 3001

# CalDAV and CardDAV share a port for protocol compatibility
[transports.caldav]
port = 5232

[transports.carddav]
port = 5232

[transports.webhook]
port = 3002
path_prefix = "/hooks"
```

A Core instance can run any combination — all five, just REST, or just CalDAV/CardDAV for a pure standards-compliant personal server.

## Transport Trait

Defined in `packages/traits`:

```rust
#[async_trait]
pub trait Transport: Send + Sync {
    fn name(&self) -> &str;
    async fn start(&self, engine: Arc<WorkflowEngine>) -> Result<()>;
    async fn stop(&self) -> Result<()>;
}
```

Each transport receives a reference to the workflow engine. When a request arrives, the transport:

1. Authenticates the request (via the shared auth module)
2. Validates the input
3. Constructs a `PipelineMessage`
4. Calls the workflow engine with the message
5. Formats the workflow output for its protocol (JSON for REST, GraphQL response for GraphQL, iCal for CalDAV, vCard for CardDAV)

## Auth

Auth is handled by the shared `packages/auth` module, not by individual transports. Every transport calls into the same auth layer.

Two authentication mechanisms:

- **Pocket ID (OIDC)** — Primary auth for user sessions. JWT with 15-min access tokens, 7-day refresh tokens.
- **API Keys** — Secondary auth for local development and scripting. Same validation pipeline as OIDC.

## Middleware Stack

Applied by each transport before routing to the workflow engine:

1. **TLS** — rustls for any non-localhost connections
2. **Auth** — Token validation via shared auth module
3. **Rate limiting** — Per-client, configurable
4. **CORS** — Locked to configured origins (REST/GraphQL only)
5. **Logging** — Structured JSON, all requests logged
6. **Error handling** — Consistent error shape per protocol

## How Transports Connect to Workflows

Workflows declare endpoint triggers. Transports match incoming requests to these triggers:

```yaml
# In a workflow definition
trigger:
  endpoint: "POST /email/sync"
```

The REST transport matches `POST /email/sync` to this workflow. The GraphQL transport could expose it as a mutation. The webhook transport could match it to an inbound webhook path.

Each transport is responsible for mapping its protocol's request format into a `PipelineMessage` and the workflow's output back into its protocol's response format.

## Request/Response Conventions (REST)

- All payloads are JSON
- Success responses return `{ "data": ... }`
- Error responses return `{ "error": { "code": "...", "message": "..." } }`
- Pagination via `?offset=N&limit=N` with `{ "data": [...], "total": N }`
- Dates in ISO 8601 / RFC 3339

## Error Codes

Error codes are namespaced: `AUTH_*`, `PLUGIN_*`, `WORKFLOW_*`, `STORAGE_*`, `SYSTEM_*`.

```json
{
  "error": {
    "code": "PLUGIN_AUTH_EXPIRED",
    "message": "Google Calendar connector plugin requires re-authentication.",
    "details": {}
  }
}
```

## Health Check

Each transport exposes a health endpoint appropriate for its protocol. The REST transport exposes `GET /api/system/health` returning:

- Database status
- Auth provider status
- Plugin load status
- Workflow engine status
- Active transport status

## Real-Time Events (SSE)

The REST transport provides `GET /api/events/stream` for server-sent events:

- Plugin status changes
- Workflow completion events
- Plugin-emitted events

SSE is transport-specific (REST only). Other transports may provide equivalent mechanisms appropriate for their protocol.

## Network Exposure

- Default: `127.0.0.1` — no external access
- LAN/internet exposure requires explicit config change + startup warning
- When exposed: TLS mandatory, auth mandatory, rate limiting mandatory
- Recommended: reverse proxy (Caddy/nginx) for internet-facing deployments
