<!--
domain: transport-layer
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Design Document — Transport Layer

## Introduction

This document describes the technical design of the Life Engine transport layer. The transport layer is implemented using Axum and provides REST and GraphQL handlers that translate wire-protocol messages into `WorkflowRequest` structs, dispatch them to the workflow engine, and translate `WorkflowResponse` back to the wire format. All configuration is driven by YAML. The design prioritises immutability (router built once at startup), protocol agnosticism (handlers own all translation), and user control (nothing hardcoded).

## Crate and Module Layout

The transport layer lives in the `packages/transport` crate. Internal modules:

- `config` — Listener config parsing and validation
- `listener` — Socket binding and Axum server construction
- `router` — Route merging, namespace validation, and Axum router building
- `middleware` — TLS, CORS, auth, logging, error handling layers
- `handlers/rest` — REST request/response translation
- `handlers/graphql` — GraphQL request/response translation and schema generation

Shared types (`WorkflowRequest`, `WorkflowResponse`, `Identity`, `WorkflowStatus`) live in `packages/types`.

## Listener Configuration

### Config Data Structures

```rust
#[derive(Deserialize, Clone, Debug)]
pub struct ListenerConfig {
    pub binding: String,           // "http"
    pub port: u16,
    pub address: Option<String>,   // defaults to "127.0.0.1"
    pub tls: Option<TlsConfig>,
    pub auth: Option<AuthConfig>,
    pub handlers: Vec<HandlerConfig>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct TlsConfig {
    pub cert: PathBuf,
    pub key: PathBuf,
}

#[derive(Deserialize, Clone, Debug)]
pub struct AuthConfig {
    pub verify: String,            // "token"
}

#[derive(Deserialize, Clone, Debug)]
pub struct HandlerConfig {
    #[serde(rename = "type")]
    pub handler_type: HandlerType,
    pub routes: Vec<RouteConfig>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub enum HandlerType {
    Rest,
    Graphql,
}

#[derive(Deserialize, Clone, Debug)]
pub struct RouteConfig {
    pub method: String,            // "GET", "POST", "PUT", "DELETE"
    pub path: String,              // "/api/v1/data/:collection"
    pub workflow: String,          // "collection.list"
    #[serde(default)]
    pub public: bool,
}
```

### Default Config Generation

On first run, Core writes a default listener config file. The default config declares:

- One listener on `127.0.0.1:8080`
- A REST handler with five generic CRUD routes and a public health check
- A GraphQL handler with a single `POST /graphql` route

The generated file is plain YAML. The user edits it directly. Core reads it on every startup.

### Config Validation

At startup, the config module validates:

- All `port` values are valid (1-65535)
- TLS `cert` and `key` paths exist and are readable (when TLS is configured)
- No duplicate `(method, path)` pairs across all handlers in a listener
- Route namespace constraints (see Router section)

Validation errors produce human-readable messages and prevent startup.

## Router

### Route Merging

The router builder collects routes from two sources:

- Config file routes (from `ListenerConfig.handlers[*].routes`)
- Plugin manifest routes (from each loaded plugin's manifest)

Both sources produce the same `RouteConfig` shape. They are concatenated into a single list before validation.

### Namespace Validation

Before building the Axum router, the router module validates:

- Every route in a `rest` handler starts with `/api/`
- Every route in a `graphql` handler starts with `/graphql`
- No route path in one handler type collides with the prefix of another handler type

If validation fails, Core rejects startup with an error like:

```
Route conflict: REST route '/graphql' collides with GraphQL handler
```

### Axum Router Construction

After validation, the router module builds an `axum::Router` with:

- Each `RouteConfig` registered as an Axum route using `method_router` matching
- Path parameters (`:collection`, `:id`) extracted by Axum's built-in path extractor
- Each route closure captures the `workflow` name and handler type, then delegates to the appropriate handler function

The built router is wrapped in `Arc` and passed to the listener. It is never mutated.

### Route-to-Workflow Dispatch

Each route closure:

1. Extracts path params, query params, body, and `Extension<Identity>` from the Axum request
2. Delegates to the handler (REST or GraphQL) to build a `WorkflowRequest`
3. Calls the workflow engine's dispatch function
4. Delegates to the handler to translate the `WorkflowResponse` to the wire format

## Workflow Engine Contract

### WorkflowRequest

```rust
pub struct WorkflowRequest {
    pub workflow: String,
    pub identity: Identity,
    pub params: HashMap<String, String>,
    pub query: HashMap<String, String>,
    pub body: Option<serde_json::Value>,
    pub meta: RequestMeta,
}
```

### WorkflowResponse

```rust
pub struct WorkflowResponse {
    pub status: WorkflowStatus,
    pub data: Option<serde_json::Value>,
    pub errors: Vec<WorkflowError>,
    pub meta: ResponseMeta,
}
```

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

### RequestMeta and ResponseMeta

```rust
pub struct RequestMeta {
    pub request_id: String,
    pub timestamp: DateTime<Utc>,
    pub source_binding: String,
}

pub struct ResponseMeta {
    pub request_id: String,
    pub duration_ms: u64,
}
```

## Handlers

### REST Handler

The REST handler translates inbound HTTP to `WorkflowRequest`:

- `workflow` — looked up from the matched `RouteConfig.workflow`
- `identity` — extracted from `Extension<Identity>` (set by auth middleware)
- `params` — extracted from Axum path parameters (e.g., `/:collection/:id`)
- `query` — parsed from the URL query string
- `body` — deserialized from the JSON request body (if present)
- `meta` — constructed with a generated request ID (UUID v4), current UTC timestamp, and the listener's binding label

The REST handler translates `WorkflowResponse` to HTTP:

- `WorkflowStatus` maps to HTTP status codes:
  - `Ok` to `200`
  - `Created` to `201`
  - `NotFound` to `404`
  - `Denied` to `403`
  - `Invalid` to `400`
  - `Error` to `500`
- Success response shape: `{ "data": <value> }`
- Error response shape: `{ "error": { "code": "<status>", "message": "<first error message>" } }`

### GraphQL Handler

The GraphQL handler translates inbound GraphQL to `WorkflowRequest`:

- `workflow` — always `graphql.query`
- `identity` — extracted from `Extension<Identity>`
- `params` — always empty (no path parameters)
- `query` — flattened from GraphQL arguments (limit, offset, filter fields)
- `body` — the raw GraphQL query or mutation string
- `meta` — same construction as REST

The GraphQL handler translates `WorkflowResponse` to GraphQL:

- Success: `{ "data": <value> }`
- Error: `{ "errors": [{ "message": "...", "extensions": { "code": "..." } }] }`

### GraphQL Schema Generation

At startup, the GraphQL handler generates its schema by:

1. Iterating all loaded plugin manifests
2. For each plugin that declares a schema, creating a corresponding GraphQL type
3. Building a root query type with fields for each queryable collection

Collections without a declared schema are excluded from GraphQL but remain accessible via REST. The generated schema is immutable after startup.

## Middleware Stack

Middleware is applied in the following order (outermost first):

1. **TLS** — Optional. Axum-server TLS acceptor using `rustls`. Only enabled when `tls` config is present.
2. **CORS** — `tower-http` CORS layer. Permissive when bound to `127.0.0.1`, strict when bound to `0.0.0.0`. User-configured origins override the default.
3. **Auth** — Custom Axum middleware. Validates OIDC token via Pocket ID, inserts `Extension<Identity>`. Skips routes marked `public: true`.
4. **Logging** — `tower-http` trace layer. Emits structured JSON logs for every request including method, path, status, and duration.
5. **Error handling** — Catches panics and unhandled errors, translates them to the consistent error shape for the active protocol.

### Auth Middleware Detail

```rust
pub struct Identity {
    pub sub: String,             // OIDC subject identifier
    pub email: Option<String>,
    pub name: Option<String>,
}
```

The auth middleware:

1. Checks if the matched route has `public: true`. If so, skips validation and proceeds with no `Identity` extension.
2. Extracts the `Authorization: Bearer <token>` header.
3. Validates the token against the Pocket ID OIDC provider (JWKS endpoint, issuer, audience).
4. On success, inserts `Extension<Identity>` into the request.
5. On failure, returns HTTP 401 immediately.

### CORS Behaviour

- **Localhost (`127.0.0.1`)** — `Access-Control-Allow-Origin: *`, all methods and headers permitted
- **Network (`0.0.0.0`)** — No origins allowed by default; user must configure explicit `allowed_origins` in the listener config
- User-configured origins always take precedence

## Error Response Conventions

### REST Error Shape

```json
{
  "error": {
    "code": "not_found",
    "message": "Collection 'widgets' does not exist"
  }
}
```

The `code` field is the snake_case form of the `WorkflowStatus` variant. The `message` is the first error message from `WorkflowResponse.errors`.

### GraphQL Error Shape

```json
{
  "errors": [
    {
      "message": "Collection 'widgets' does not exist",
      "extensions": {
        "code": "not_found"
      }
    }
  ]
}
```

Each entry in `WorkflowResponse.errors` maps to one entry in the GraphQL `errors` array.

## TLS

TLS uses `rustls` via `axum-server`. Configuration is minimal:

- `cert` — Path to PEM certificate file
- `key` — Path to PEM private key file

No cert renewal, OCSP stapling, or cipher suite configuration. For production internet-facing deployments, a reverse proxy (nginx, Caddy) is recommended.

## Address Binding

- `127.0.0.1` — Default. Localhost only.
- `0.0.0.0` — Network access. Core logs a warning at startup: `"WARNING: Listener on port {port} is bound to 0.0.0.0 and accessible from the network"`

## What the Transport Layer Does Not Own

- **Admin panel** — Separate top-level Core feature with its own config, auth (local passphrase), and first-run wizard
- **Workflow execution** — The transport layer dispatches and translates; it never runs plugin logic
- **Storage** — The transport layer never reads or writes storage directly
- **Rate limiting** — Deferred to the reverse proxy; not needed for single-user localhost deployments
