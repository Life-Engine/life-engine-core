<!--
domain: core
updated: 2026-03-22
spec-brief: ./brief.md
-->

# REST API

Reference: [[03 - Projects/Life Engine/Design/Core/API]]

## Purpose

This spec defines all HTTP routes, the middleware stack, request/response conventions, and real-time event streaming for the Core REST API. The API is the only way clients interact with Core. It is implemented in `axum` and defined in a shared `packages/api` crate so any client (App, web, mobile, CLI) can consume it without Core changes.

## Middleware Stack

Middleware is applied to all routes in this order:

1. **TLS** — `rustls` terminates TLS for any non-localhost connections. Automatically enabled when the host is not `127.0.0.1`.
2. **Auth** — Token validation via Pocket ID OIDC tokens or API keys. See [[03 - Projects/Life Engine/Planning/specs/core/Auth and Pocket ID]] for details.
3. **Rate limiting** — Per-client, configurable. Default 60 requests per minute. Failed auth attempts limited to 5 per minute per IP.
4. **CORS** — Locked to configured origins. Default allows only `http://localhost:1420` (the Tauri App).
5. **Logging** — Structured JSON logging of all requests via the `tracing` crate. Includes method, path, status code, duration, and client identifier.
6. **Error handling** — Catches all errors and returns a consistent error shape. No stack traces or internal details leak to the client.

## Route Groups

### Data — `/api/data/{collection}`

Standard CRUD operations on any collection (canonical or plugin-private):

- `GET /api/data/{collection}` — List records with filters, sort, and pagination
- `GET /api/data/{collection}/{id}` — Get a single record by ID
- `POST /api/data/{collection}` — Create a new record
- `PUT /api/data/{collection}/{id}` — Update an existing record
- `DELETE /api/data/{collection}/{id}` — Delete a record

### Plugins — `/api/plugins`

Plugin management:

- `GET /api/plugins` — List all plugins with status
- `POST /api/plugins/install` — Install a plugin from a WASM path or URL
- `POST /api/plugins/{id}/enable` — Enable a plugin
- `POST /api/plugins/{id}/disable` — Disable a plugin

### Workflows — `/api/workflows`

Workflow CRUD:

- `GET /api/workflows` — List all workflow definitions
- `GET /api/workflows/{id}` — Get a workflow definition
- `POST /api/workflows` — Create a new workflow
- `PUT /api/workflows/{id}` — Update a workflow
- `DELETE /api/workflows/{id}` — Delete a workflow

### Scheduler — `/api/scheduler`

Scheduled task management:

- `GET /api/scheduler` — List all scheduled tasks
- `GET /api/scheduler/{id}` — Get a scheduled task
- `POST /api/scheduler` — Create a scheduled task
- `PUT /api/scheduler/{id}` — Update a scheduled task
- `DELETE /api/scheduler/{id}` — Delete a scheduled task
- `POST /api/scheduler/{id}/run` — Trigger a task immediately

### Credentials — `/api/credentials`

Unified credential store. Scoped access — plugins and clients can only access credentials they are authorised for. Raw secrets are never exposed in responses.

- `GET /api/credentials` — List credentials (metadata only)
- `GET /api/credentials/{id}` — Get credential metadata
- `POST /api/credentials` — Store a new credential
- `PUT /api/credentials/{id}` — Update a credential
- `DELETE /api/credentials/{id}` — Delete a credential

### System — `/api/system`

System information and administration:

- `GET /api/system/health` — Health check (see [[#Health Check]])
- `GET /api/system/config` — Current configuration (sensitive values redacted)
- `GET /api/system/version` — Core version and build information
- `GET /api/system/audit` — Audit log entries (paginated)

### Auth — `/api/auth`

Authentication endpoints:

- `POST /api/auth/token` — Generate a local auth token (Phase 1)
- `POST /api/auth/refresh` — Refresh an expired token
- `DELETE /api/auth/token/{id}` — Revoke a token
- `GET /api/auth/oidc/callback` — Pocket ID OIDC callback (Phase 2)

### Events — `/api/events/stream`

Server-Sent Events (SSE) endpoint for real-time notifications. See [[#SSE Events]].

### Plugin Routes — `/api/plugins/{plugin-id}/*`

Plugin-registered routes mount under their plugin ID namespace. Core dynamically mounts these routes when a plugin registers them during `on_load`. Routes are removed when a plugin is unloaded.

## Request/Response Conventions

- All payloads are JSON
- Success responses return `{ "data": ... }`
- Paginated responses return `{ "data": [...], "total": N }`
- Error responses return `{ "error": { "code": "...", "message": "..." } }`
- Pagination via query parameters: `?offset=N&limit=N`
- Dates in ISO 8601 / RFC 3339 format

## Error Codes

Error codes are namespaced by subsystem:

- `AUTH_*` — Authentication and authorisation errors (e.g., `AUTH_TOKEN_EXPIRED`, `AUTH_INVALID_CREDENTIALS`)
- `PLUGIN_*` — Plugin-related errors (e.g., `PLUGIN_NOT_FOUND`, `PLUGIN_AUTH_EXPIRED`)
- `WORKFLOW_*` — Workflow errors (e.g., `WORKFLOW_STEP_FAILED`, `WORKFLOW_INCOMPATIBLE`)
- `DATA_*` — Data layer errors (e.g., `DATA_VALIDATION_FAILED`, `DATA_NOT_FOUND`)
- `SYSTEM_*` — System-level errors (e.g., `SYSTEM_UNAVAILABLE`, `SYSTEM_CONFIG_INVALID`)

Example error response:

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

`GET /api/system/health` returns the status of all Core subsystems:

- **Database** — Readable and writable
- **Pocket ID** — Sidecar running and responsive
- **Plugins** — Load status for each plugin (including connector plugins)
- **Workflow engine** — Running and processing
- **Scheduler** — Running and executing tasks

The health endpoint returns HTTP 200 if all subsystems are healthy, HTTP 503 if any critical subsystem is degraded.

## SSE Events

`GET /api/events/stream` provides server-sent events for real-time notifications. The connection is unidirectional (server to client), works through proxies, and is implemented via `axum::response::Sse`.

Event types emitted on the stream:

- **Plugin status changes** — Sync complete, auth expired, plugin errors
- **Workflow completion** — Workflow finished successfully or failed
- **Scheduler results** — Scheduled task completed or errored
- **Plugin-emitted events** — Custom events emitted by plugins via the `events:emit` capability

Each event includes a `type` field, a `timestamp`, and a `payload` object.

## Network Exposure

- **Default** — `127.0.0.1:3750`, no external access
- **LAN/internet exposure** requires explicit config change and triggers a startup warning
- When exposed externally, the following are mandatory: TLS, authentication, and rate limiting
- Recommended: reverse proxy (Caddy/nginx) for internet-facing deployments

## Acceptance Criteria

- Full CRUD works on `/api/data/{collection}` with correct response shapes
- SSE stream at `/api/events/stream` delivers events in real time
- Plugin routes mount dynamically when plugins are loaded and unmount when unloaded
- All errors follow the consistent `{ "error": { "code", "message" } }` shape
- Rate limiting correctly throttles excessive requests
- Pagination, filtering, and sorting work on list endpoints
- Health check returns accurate status for all subsystems
