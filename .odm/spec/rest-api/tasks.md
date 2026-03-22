<!--
domain: core
updated: 2026-03-22
spec-brief: ./brief.md
-->

# REST API — Tasks

> spec: ./brief.md

## 1.1 — Axum Router Setup
> spec: ./brief.md
> depends: none

- Create `crates/api/src/router.rs` with the top-level `axum::Router`
- Define route groups as nested routers: `/api/data`, `/api/plugins`, `/api/workflows`, `/api/scheduler`, `/api/credentials`, `/api/system`, `/api/auth`, `/api/events`
- Wire the router into the server startup in `crates/core/src/main.rs`

**Files:** `crates/api/src/router.rs`, `crates/api/src/lib.rs`
**Est:** 20 min

## 1.2 — Middleware Stack
> spec: ./brief.md
> depends: 1.1

- Create `crates/api/src/middleware/mod.rs` with sub-modules for each layer
- Implement TLS layer using `rustls` (auto-enabled for non-localhost bindings)
- Implement auth middleware that validates tokens and API keys
- Implement rate limiting middleware (60 req/min default, 5 failed auth/min per IP)
- Implement CORS middleware locked to configured origins
- Implement request logging middleware using the `tracing` crate

**Files:** `crates/api/src/middleware/mod.rs`, `crates/api/src/middleware/auth.rs`, `crates/api/src/middleware/rate_limit.rs`
**Est:** 30 min

## 1.3 — Data CRUD Endpoints
> spec: ./brief.md
> depends: 1.1

- Create `crates/api/src/routes/data.rs`
- Implement handlers: `list_records`, `get_record`, `create_record`, `update_record`, `delete_record`
- Wire to `GET/POST /api/data/{collection}` and `GET/PUT/DELETE /api/data/{collection}/{id}`
- Add pagination support via `?offset=N&limit=N` query parameters
- Return consistent response shapes: `{ "data": ... }` and `{ "data": [...], "total": N }`

**Files:** `crates/api/src/routes/data.rs`, `crates/api/src/routes/mod.rs`
**Est:** 30 min

## 1.4 — Plugin Management Endpoints
> spec: ./brief.md
> depends: 1.1

- Create `crates/api/src/routes/plugins.rs`
- Implement handlers: `list_plugins`, `install_plugin`, `enable_plugin`, `disable_plugin`
- Wire to `GET /api/plugins`, `POST /api/plugins/install`, `POST /api/plugins/{id}/enable`, `POST /api/plugins/{id}/disable`
- Integrate with the plugin lifecycle manager for enable/disable operations

**Files:** `crates/api/src/routes/plugins.rs`, `crates/api/src/routes/mod.rs`
**Est:** 25 min

## 1.5 — SSE Event Handler
> spec: ./brief.md
> depends: 1.1

- Create `crates/api/src/routes/events.rs`
- Implement the SSE endpoint at `GET /api/events/stream` using `axum::response::Sse`
- Subscribe to the Core event bus and forward events to connected clients
- Handle client disconnection gracefully (clean up subscriptions)
- Include `type`, `timestamp`, and `payload` fields in each SSE event

**Files:** `crates/api/src/routes/events.rs`, `crates/api/src/routes/mod.rs`
**Est:** 25 min

## 1.6 — Error Types and Response Shape
> spec: ./brief.md
> depends: none

- Create `crates/api/src/error.rs`
- Define `ApiError` enum with variants for each subsystem: `Auth`, `Plugin`, `Workflow`, `Data`, `System`
- Implement `IntoResponse` for `ApiError` to produce `{ "error": { "code": "...", "message": "..." } }`
- Ensure no stack traces or file paths are included in error responses
- Map each variant to the correct HTTP status code (400, 401, 404, 429, 500, 503)

**Files:** `crates/api/src/error.rs`, `crates/api/src/lib.rs`
**Est:** 20 min

## 1.7 — Health Check Endpoint
> spec: ./brief.md
> depends: 1.1

- Create `crates/api/src/routes/system.rs`
- Implement `GET /api/system/health` that checks database, Pocket ID, plugins, workflow engine, and scheduler
- Return HTTP 200 with per-subsystem status when all healthy
- Return HTTP 503 when any critical subsystem is degraded
- Implement `GET /api/system/version` returning Core version and build info

**Files:** `crates/api/src/routes/system.rs`, `crates/api/src/routes/mod.rs`
**Est:** 20 min
