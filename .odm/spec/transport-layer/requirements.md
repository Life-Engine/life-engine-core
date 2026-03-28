<!--
domain: transport-layer
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Requirements Document — Transport Layer

## Introduction

The transport layer is the inbound boundary of Life Engine Core. It accepts HTTP requests, applies middleware, translates wire-protocol messages into `WorkflowRequest` structs, dispatches them to the workflow engine, and translates `WorkflowResponse` back to the wire format. In v1, two protocols are supported: REST (JSON over HTTP via Axum) and GraphQL (sharing the HTTP binding on a separate path prefix). WebSocket, CalDAV/CardDAV, and webhooks are deferred to future iterations.

The transport layer never executes plugin logic, accesses storage directly, or manages users. It is a thin translation and dispatch boundary.

## Alignment with Product Vision

- **User sovereignty** — The user controls every aspect of the transport configuration: listeners, ports, TLS, routes, and public endpoints. Nothing is hardcoded.
- **Open/Closed Principle** — The `handlers` array in the listener config is extensible. New transport types (WebSocket, CalDAV, webhooks) slot into the same structure without migration.
- **Protocol agnosticism** — The workflow engine never thinks about HTTP or GraphQL. Handlers isolate all protocol-specific translation.
- **Defence in depth** — Auth middleware validates tokens at the transport boundary before any handler or workflow executes.
- **The Pit of Success** — Sensible defaults are generated on first run, namespace validation prevents route collisions, and CORS auto-configures based on bind address.

## Requirements

### Requirement 1 — Listener Configuration

**User Story:** As a user, I want to configure listeners via YAML so that I can control which ports, addresses, and protocols Core exposes.

#### Acceptance Criteria

- 1.1. WHEN Core reads the listener config THEN each listener entry SHALL support the fields `binding`, `port`, `address`, `tls` (optional), `auth` (optional), and `handlers` (array).
- 1.2. WHEN `address` is omitted THEN Core SHALL default to `127.0.0.1`.
- 1.3. WHEN multiple listeners are defined THEN each SHALL bind its own socket independently.
- 1.4. WHEN a listener's `handlers` array contains multiple entries THEN all handler types SHALL mount on the same socket.

---

### Requirement 2 — Default Config Generation

**User Story:** As a user, I want Core to generate a working default listener config on first run so that the API works immediately without manual setup.

#### Acceptance Criteria

- 2.1. WHEN Core starts and no listener config file exists THEN Core SHALL generate a default config file.
- 2.2. WHEN the default config is generated THEN it SHALL include generic CRUD REST routes (`collection.list`, `collection.get`, `collection.create`, `collection.update`, `collection.delete`).
- 2.3. WHEN the default config is generated THEN it SHALL include a GraphQL endpoint (`graphql.query`) at `POST /graphql`.
- 2.4. WHEN the default config is generated THEN it SHALL include a public health check route at `GET /api/v1/health` with `public: true`.
- 2.5. WHEN the default config is generated THEN the user SHALL be able to modify, remove, or extend any route without affecting Core internals.

---

### Requirement 3 — Plugin Manifest Route Merging

**User Story:** As a plugin author, I want to declare custom routes in my plugin manifest so that my plugin exposes API endpoints without editing the Core config.

#### Acceptance Criteria

- 3.1. WHEN Core starts THEN it SHALL merge routes from all loaded plugin manifests with routes from the listener config.
- 3.2. WHEN a plugin manifest declares a route THEN the route SHALL be added to the router alongside config routes.
- 3.3. WHEN a plugin is added or removed THEN Core SHALL require a restart to rebuild the router.
- 3.4. WHEN a plugin route conflicts with a config route THEN Core SHALL reject startup with a descriptive error message.

---

### Requirement 4 — Route Namespace Validation

**User Story:** As a Core developer, I want route prefix validation at startup so that REST and GraphQL routes do not collide.

#### Acceptance Criteria

- 4.1. WHEN the router is built THEN REST handler routes SHALL be validated to start with `/api/`.
- 4.2. WHEN the router is built THEN GraphQL handler routes SHALL be validated to start with `/graphql`.
- 4.3. WHEN a route fails namespace validation THEN Core SHALL reject startup with a clear error message (e.g., `"Route conflict: REST route '/graphql' collides with GraphQL handler"`).
- 4.4. WHEN namespace validation runs THEN it SHALL complete before Axum builds its router so that the user receives a readable error instead of a panic.

---

### Requirement 5 — Router Build and Immutability

**User Story:** As a Core developer, I want the router built once at startup and immutable at runtime so that route resolution is fast and predictable.

#### Acceptance Criteria

- 5.1. WHEN Core starts THEN the router SHALL be built once from merged config and plugin manifest routes.
- 5.2. WHEN the router is built THEN it SHALL be immutable at runtime and SHALL NOT be rebuilt or modified while Core is running.
- 5.3. WHEN a route is matched THEN the router SHALL extract path parameters (`:collection`, `:id`) and pass them as `params: HashMap<String, String>` in the `WorkflowRequest`.
- 5.4. WHEN a route is matched THEN the router SHALL resolve the target workflow by name (e.g., `collection.list`) without knowledge of what the workflow does.

---

### Requirement 6 — Generic CRUD Routes

**User Story:** As a user, I want default REST routes for collection-level CRUD so that any collection in storage is accessible via the API.

#### Acceptance Criteria

- 6.1. WHEN the default config is loaded THEN the following routes SHALL be declared:
  - `GET /api/v1/data/:collection` mapping to `collection.list`
  - `GET /api/v1/data/:collection/:id` mapping to `collection.get`
  - `POST /api/v1/data/:collection` mapping to `collection.create`
  - `PUT /api/v1/data/:collection/:id` mapping to `collection.update`
  - `DELETE /api/v1/data/:collection/:id` mapping to `collection.delete`
- 6.2. WHEN a CRUD route is matched THEN the `:collection` parameter SHALL be dynamic, allowing access to any collection in storage.
- 6.3. WHEN the user modifies or removes a default CRUD route THEN Core SHALL respect the user's config without fallback to hardcoded routes.

---

### Requirement 7 — REST Handler Translation

**User Story:** As a Core developer, I want the REST handler to translate HTTP requests and responses to and from `WorkflowRequest`/`WorkflowResponse` so that the workflow engine is HTTP-agnostic.

#### Acceptance Criteria

- 7.1. WHEN an HTTP request is received by the REST handler THEN it SHALL populate the `WorkflowRequest` with: `workflow` (from route config), `identity` (from auth middleware `Extension<Identity>`), `params` (from path parameters), `query` (from URL query string), `body` (from JSON request body), and `meta` (request ID, timestamp, source binding).
- 7.2. WHEN the workflow engine returns a `WorkflowResponse` with `data` THEN the REST handler SHALL respond with the HTTP status code derived from `status` and a JSON body `{ "data": ... }`.
- 7.3. WHEN the workflow engine returns a `WorkflowResponse` with `errors` THEN the REST handler SHALL respond with the HTTP status code derived from `status` and a JSON body `{ "error": { "code": "...", "message": "..." } }`.
- 7.4. WHEN `WorkflowStatus` is `Ok` THEN the REST handler SHALL map to HTTP 200; `Created` to 201; `NotFound` to 404; `Denied` to 403; `Invalid` to 400; `Error` to 500.

---

### Requirement 8 — GraphQL Handler Translation

**User Story:** As a Core developer, I want the GraphQL handler to translate GraphQL requests and responses to and from `WorkflowRequest`/`WorkflowResponse` so that the workflow engine is GraphQL-agnostic.

#### Acceptance Criteria

- 8.1. WHEN a GraphQL request is received THEN the handler SHALL populate the `WorkflowRequest` with: `workflow` as `graphql.query`, `identity` from auth middleware, `params` as empty, `query` flattened from GraphQL arguments (limit, offset, filters), `body` as the GraphQL query/mutation string, and `meta` (request ID, timestamp, source binding).
- 8.2. WHEN the workflow engine returns data THEN the GraphQL handler SHALL respond with `{ "data": ... }`.
- 8.3. WHEN the workflow engine returns errors THEN the GraphQL handler SHALL respond with `{ "errors": [...] }`.

---

### Requirement 9 — GraphQL Schema Generation

**User Story:** As a plugin author, I want my plugin's schema declarations to automatically generate GraphQL types so that collections with declared schemas are queryable via GraphQL.

#### Acceptance Criteria

- 9.1. WHEN Core starts THEN the GraphQL handler SHALL generate its schema from plugin manifest-declared schemas.
- 9.2. WHEN a plugin declares a schema in its manifest THEN the corresponding collection SHALL become a queryable GraphQL type.
- 9.3. WHEN a collection has no declared schema THEN it SHALL NOT be queryable via GraphQL but SHALL remain accessible via REST generic CRUD.
- 9.4. WHEN the schema is generated THEN it SHALL reflect the collections that actually exist with no enforced canonical types.

---

### Requirement 10 — Transport Equivalence

**User Story:** As a Core developer, I want REST and GraphQL to resolve through the same system workflows so that behaviour is consistent regardless of transport.

#### Acceptance Criteria

- 10.1. WHEN a `collection.list` request is dispatched THEN it SHALL produce the same result regardless of whether the REST or GraphQL handler initiated it.
- 10.2. WHEN a plugin or hook is attached to a system workflow THEN it SHALL apply equally to requests from both transports.

---

### Requirement 11 — Middleware Stack

**User Story:** As a Core developer, I want middleware applied in a defined order so that cross-cutting concerns are handled consistently for every request.

#### Acceptance Criteria

- 11.1. WHEN a request is received THEN middleware SHALL be applied in the following order: TLS (optional), CORS, auth, logging, error handling.
- 11.2. WHEN all requests are processed THEN the logging middleware SHALL emit structured JSON log entries.
- 11.3. WHEN error handling middleware processes a `WorkflowResponse` THEN it SHALL produce a consistent error shape per protocol.

---

### Requirement 12 — CORS Configuration

**User Story:** As a user, I want CORS to auto-configure based on bind address so that localhost development works out of the box while non-localhost deployments are locked down.

#### Acceptance Criteria

- 12.1. WHEN the listener is bound to `127.0.0.1` THEN CORS SHALL be permissive by default.
- 12.2. WHEN the listener is bound to `0.0.0.0` THEN CORS SHALL be strict by default.
- 12.3. WHEN the user configures explicit allowed origins THEN Core SHALL use those instead of the default behaviour.

---

### Requirement 13 — Authentication Middleware

**User Story:** As a user, I want token validation at the transport boundary so that unauthenticated requests never reach the workflow engine.

#### Acceptance Criteria

- 13.1. WHEN a request arrives on a non-public route THEN the auth middleware SHALL validate the token via Pocket ID (OIDC).
- 13.2. WHEN token validation succeeds THEN the middleware SHALL produce an `Identity` and pass it as an Axum `Extension<Identity>`.
- 13.3. WHEN token validation fails THEN the middleware SHALL reject the request before it reaches a handler.
- 13.4. WHEN a route is marked `public: true` THEN the auth middleware SHALL skip token validation entirely.

---

### Requirement 14 — Authorisation (v1)

**User Story:** As a user of a single-user self-hosted system, I want all-or-nothing authorisation so that any authenticated request is authorised to access all collections and workflows.

#### Acceptance Criteria

- 14.1. WHEN a request is authenticated THEN the user SHALL be authorised to access all collections and workflows.
- 14.2. WHEN v1 is running THEN the system SHALL NOT enforce per-collection ACLs or role-based access control.

---

### Requirement 15 — TLS Configuration

**User Story:** As a user, I want optional TLS per listener so that I can secure my transport without a reverse proxy for simple deployments.

#### Acceptance Criteria

- 15.1. WHEN the `tls` section is present in a listener config THEN Core SHALL enable TLS using the specified `cert` and `key` file paths.
- 15.2. WHEN the `tls` section is absent THEN the listener SHALL accept plaintext HTTP.
- 15.3. WHEN TLS is enabled THEN Core SHALL NOT provide cert renewal, OCSP, or cipher suite configuration.

---

### Requirement 16 — Address Binding Warnings

**User Story:** As a user, I want a startup warning when Core is bound to a non-localhost address so that I am aware of potential exposure.

#### Acceptance Criteria

- 16.1. WHEN a listener is bound to `0.0.0.0` THEN Core SHALL log a startup warning indicating the listener is accessible from the network.
- 16.2. WHEN a listener is bound to `127.0.0.1` THEN no warning SHALL be logged.

---

### Requirement 17 — Admin Panel Separation

**User Story:** As a user, I want the admin panel to have its own auth mechanism so that I can access it before OIDC is configured.

#### Acceptance Criteria

- 17.1. WHEN the admin panel is accessed THEN it SHALL use a local passphrase for auth, independent of OIDC.
- 17.2. WHEN the admin panel auth is configured THEN it SHALL be in the admin panel's top-level config section, not in the listeners config.
- 17.3. WHEN Core runs for the first time THEN the admin panel SHALL be unauthenticated on localhost; the first action SHALL be setting a passphrase.
- 17.4. WHEN the passphrase is set THEN the admin panel SHALL be locked and require the passphrase for subsequent access.
