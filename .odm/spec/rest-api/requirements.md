<!--
domain: core
updated: 2026-03-22
spec-brief: ./brief.md
-->

# REST API — Requirements

## 1. Middleware Stack

- **1.1** — WHEN a request arrives on a non-localhost binding, THEN the TLS layer SHALL terminate the connection using `rustls`.
- **1.2** — WHEN a request lacks a valid auth token or API key, THEN the auth middleware SHALL return HTTP 401 with error code `AUTH_INVALID_CREDENTIALS`.
- **1.3** — WHEN a client exceeds 60 requests per minute, THEN the rate limiter SHALL return HTTP 429 with a `Retry-After` header.
- **1.4** — WHEN a client sends 5 failed auth attempts per minute from the same IP, THEN the rate limiter SHALL block further auth attempts from that IP for the remainder of the window.
- **1.5** — WHEN a request's `Origin` header does not match a configured allowed origin, THEN the CORS middleware SHALL reject the request.
- **1.6** — WHEN any request completes, THEN the logging middleware SHALL emit a structured JSON log entry with method, path, status code, duration, and client identifier.
- **1.7** — WHEN any handler returns an error, THEN the error middleware SHALL transform it into `{ "error": { "code": "...", "message": "..." } }` with no internal details exposed.

## 2. Data CRUD Routes

- **2.1** — WHEN `GET /api/data/{collection}` is called, THEN it SHALL return a paginated list of records with `{ "data": [...], "total": N }`.
- **2.2** — WHEN `GET /api/data/{collection}` includes `?offset=N&limit=N` parameters, THEN the response SHALL return the specified page of results.
- **2.3** — WHEN `GET /api/data/{collection}/{id}` is called with a valid ID, THEN it SHALL return the record as `{ "data": {...} }`.
- **2.4** — WHEN `GET /api/data/{collection}/{id}` is called with a non-existent ID, THEN it SHALL return HTTP 404 with error code `DATA_NOT_FOUND`.
- **2.5** — WHEN `POST /api/data/{collection}` is called with a valid body, THEN it SHALL create the record and return HTTP 201 with the created record.
- **2.6** — WHEN `POST /api/data/{collection}` is called with an invalid body, THEN it SHALL return HTTP 400 with error code `DATA_VALIDATION_FAILED`.
- **2.7** — WHEN `PUT /api/data/{collection}/{id}` is called with a valid body, THEN it SHALL update the record and return the updated record.
- **2.8** — WHEN `DELETE /api/data/{collection}/{id}` is called, THEN it SHALL delete the record and return HTTP 204.

## 3. Plugin Management Routes

- **3.1** — WHEN `GET /api/plugins` is called, THEN it SHALL return a list of all plugins with their ID, display name, version, and status (enabled/disabled/error).
- **3.2** — WHEN `POST /api/plugins/install` is called with a valid WASM path, THEN it SHALL install the plugin and return the plugin metadata.
- **3.3** — WHEN `POST /api/plugins/{id}/enable` is called, THEN it SHALL start the plugin lifecycle and mount its routes.
- **3.4** — WHEN `POST /api/plugins/{id}/disable` is called, THEN it SHALL stop the plugin, unmount its routes, and preserve its data.

## 4. Workflow Routes

- **4.1** — WHEN `GET /api/workflows` is called, THEN it SHALL return all workflow definitions.
- **4.2** — WHEN `POST /api/workflows` is called with a valid definition, THEN it SHALL create and return the workflow.
- **4.3** — WHEN `PUT /api/workflows/{id}` is called, THEN it SHALL update the workflow definition and return the updated version.
- **4.4** — WHEN `DELETE /api/workflows/{id}` is called, THEN it SHALL remove the workflow and return HTTP 204.

## 5. Scheduler Routes

- **5.1** — WHEN `GET /api/scheduler` is called, THEN it SHALL return all scheduled tasks with their next run time.
- **5.2** — WHEN `POST /api/scheduler` is called with a valid cron expression and target, THEN it SHALL create the scheduled task.
- **5.3** — WHEN `POST /api/scheduler/{id}/run` is called, THEN it SHALL trigger the task immediately regardless of its schedule.
- **5.4** — WHEN `DELETE /api/scheduler/{id}` is called, THEN it SHALL remove the scheduled task and return HTTP 204.

## 6. Credential Routes

- **6.1** — WHEN `GET /api/credentials` is called, THEN it SHALL return credential metadata only; raw secrets SHALL never appear in responses.
- **6.2** — WHEN `POST /api/credentials` is called, THEN it SHALL store the credential securely and return its metadata.
- **6.3** — WHEN `DELETE /api/credentials/{id}` is called, THEN it SHALL delete the credential and return HTTP 204.

## 7. System Routes

- **7.1** — WHEN `GET /api/system/health` is called and all subsystems are healthy, THEN it SHALL return HTTP 200 with status for each subsystem.
- **7.2** — WHEN `GET /api/system/health` is called and any critical subsystem is degraded, THEN it SHALL return HTTP 503.
- **7.3** — WHEN `GET /api/system/config` is called, THEN it SHALL return the current configuration with sensitive values (secrets, tokens) redacted.
- **7.4** — WHEN `GET /api/system/version` is called, THEN it SHALL return the Core version string and build metadata.

## 8. Auth Routes

- **8.1** — WHEN `POST /api/auth/token` is called with valid credentials, THEN it SHALL return a signed auth token.
- **8.2** — WHEN `POST /api/auth/refresh` is called with a valid refresh token, THEN it SHALL return a new auth token.
- **8.3** — WHEN `DELETE /api/auth/token/{id}` is called, THEN it SHALL revoke the token so it can no longer be used.

## 9. SSE Events

- **9.1** — WHEN a client connects to `GET /api/events/stream`, THEN the server SHALL hold the connection open and send events as they occur.
- **9.2** — WHEN a plugin status changes, THEN an SSE event with type `plugin.status` SHALL be emitted.
- **9.3** — WHEN a workflow completes or fails, THEN an SSE event with type `workflow.complete` or `workflow.error` SHALL be emitted.
- **9.4** — WHEN a plugin emits a custom event via `events:emit`, THEN it SHALL appear on the SSE stream for subscribed clients.
- **9.5** — WHEN a client disconnects from the SSE stream, THEN the server SHALL clean up the subscription without error.

## 10. Error Handling

- **10.1** — WHEN any route returns an error, THEN the response body SHALL conform to `{ "error": { "code": string, "message": string } }`.
- **10.2** — WHEN an error occurs, THEN the error code SHALL be namespaced by subsystem (`AUTH_*`, `PLUGIN_*`, `WORKFLOW_*`, `DATA_*`, `SYSTEM_*`).
- **10.3** — WHEN an internal error occurs, THEN the response SHALL not include stack traces, file paths, or internal implementation details.
