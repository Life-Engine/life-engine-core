---
title: "Engine — API Layer"
tags: [life-engine, engine, api, axum, rest]
created: 2026-03-14
---

# API Layer

REST/JSON via `axum`. This is the only way clients interact with Core. The API is defined in a shared `packages/api` crate so any client (App, web, mobile, CLI) can consume it without Core changes.

The API layer implements several [[03 - Projects/Life Engine/Design/Principles|Design Principles]]: *Defence in Depth* (every request passes through TLS, auth, rate limiting, CORS, logging, and error handling — no endpoint can opt out), *Parse, Don't Validate* (input validated against JSON Schema at the boundary, config changes pre-validated before applying), and *Separation of Concerns* (the API layer handles transport and auth, the workflow engine handles processing, plugins handle logic).

## Auth

Core supports two authentication mechanisms. Both go through the same middleware stack.

**Pocket ID (OIDC)** — Primary auth for user sessions.

- Pocket ID runs as a bundled Go binary, spawned and managed by Core
- Core handles the OIDC flow and token exchange
- Every request is validated against Pocket ID tokens
- For remote access: JWT with 15-min access tokens, 7-day refresh tokens, Ed25519 signatures
- Plugins inherit auth automatically — no direct credential access

**API Keys** — Secondary auth for local development and scripting.

- Simpler alternative for local access, CLI tools, and automation
- Generated and managed through the API
- Same middleware validation as OIDC tokens — API keys are not a bypass

## Middleware Stack

Applied to all routes in order:

1. **TLS** — `rustls` for any non-localhost connections
2. **Auth** — Token validation (Pocket ID OIDC or API key)
3. **Rate limiting** — Per-client, configurable
4. **CORS** — Locked to configured origins
5. **Logging** — Structured JSON, all requests logged
6. **Error handling** — Consistent error shape across all endpoints

## Route Groups

```
/api/
  /plugins         Plugin management (list, install, enable, disable — includes connector plugins)
  /workflows       Workflow management (list, create, update, delete)
  /scheduler       Scheduled task management (list, create, update, delete)
  /data            Data access (query, create, update, delete per collection)
  /credentials     Unified credential store (identity docs, OAuth tokens — scoped, never exposes raw secrets)
  /system          Health, config, version, audit log
  /auth            Login, token refresh, revocation
  /events/stream   SSE real-time event stream
```

Plugin-registered routes mount under `/api/plugins/{plugin-id}/`.

## Request/Response Conventions

- All payloads are JSON
- Success responses return `{ "data": ... }`
- Error responses return `{ "error": { "code": "...", "message": "..." } }`
- Pagination via `?offset=N&limit=N` with `{ "data": [...], "total": N }`
- Dates in ISO 8601 / RFC 3339

## Error Shape

```json
{
  "error": {
    "code": "PLUGIN_AUTH_EXPIRED",
    "message": "Google Calendar connector plugin requires re-authentication.",
    "details": {}
  }
}
```

Error codes are namespaced: `AUTH_*`, `PLUGIN_*`, `WORKFLOW_*`, `DATA_*`, `SYSTEM_*`.

## System Configuration Endpoints

The `/api/system` route group includes endpoints for managing Core's runtime configuration. These are consumed by the App's Core Configuration plugin (`com.life-engine.core-config`).

### `GET /api/system/config`

Fetch the current configuration across all sections. Sensitive values (passwords, keys, tokens) are redacted in the response — replaced with `"********"`.

Response: `{ "data": { "core": { ... }, "auth": { ... }, "storage": { ... }, "plugins": { ... }, "network": { ... }, "scheduler": { ... } } }`

### `PATCH /api/system/config/{section}`

Update a specific configuration section. Accepts a partial object — only provided keys are updated. Keys not present in the request body are left unchanged.

- Validates the payload against the section's JSON Schema before applying
- Returns `200` with the updated section (sensitive values redacted)
- Returns `400` if validation fails, with details on which keys failed
- Some changes require a Core restart to take effect — the response includes `"restartRequired": true` when applicable

### `GET /api/system/config/schema`

Returns the full JSON Schema for all configuration sections. Used by the Core Configuration plugin to dynamically generate forms. Each section is a top-level key with its own schema object, including `type`, `description`, `default`, and validation constraints.

### `POST /api/system/config/{section}/validate`

Validate a configuration change without applying it. Accepts the same body as `PATCH /api/system/config/{section}`. Returns `200` with `{ "valid": true }` or `{ "valid": false, "errors": [...] }`. Useful for real-time form validation before the user saves.

### `POST /api/system/restart`

Trigger a graceful Core restart. Returns `202 Accepted` immediately. Core finishes in-flight requests, flushes pending writes, and restarts. The client should poll `GET /api/system/health` to detect when Core is back up.

Requires authentication. Logged in the audit trail.

## Health Check

`GET /api/system/health` returns:

- Database status (readable, writable)
- Pocket ID sidecar status (running, responsive)
- Plugin load status (including connector plugins)
- Workflow engine status
- Scheduler status

## Real-Time Events (SSE)

`GET /api/events/stream` provides server-sent events for real-time notifications:

- Plugin status changes (sync complete, auth expired, errors)
- Workflow completion events
- Scheduler task results
- Plugin-emitted events

SSE is unidirectional (server to client), works through proxies, and is natively supported by `axum::response::Sse`.

## Network Exposure

- Default: `127.0.0.1:3750` — no external access
- LAN/internet exposure requires explicit config change + startup warning
- When exposed: TLS mandatory, auth mandatory, rate limiting mandatory
- Recommended: reverse proxy (Caddy/nginx) for internet-facing deployments
