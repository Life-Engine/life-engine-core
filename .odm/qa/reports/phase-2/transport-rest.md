# REST Transport Layer Review

Package: `packages/transport-rest`

## Summary

The transport-rest crate provides the HTTP/REST boundary for Life Engine. It has been restructured from a monolithic `config.rs` into a well-organized module hierarchy: `config/`, `middleware/`, `router/`, `listener.rs`, `handlers/`, `error.rs`, and `types.rs`. The architecture cleanly separates concerns and the code is generally well-written with good test coverage for config validation, CORS, auth, and routing.

However, there are several issues ranging from a critical security flaw in the auth middleware's public-route bypass logic, through identity type fragmentation and missing request size limits, to minor gaps in PATCH method support and Retry-After headers.

## File-by-File Analysis

### Cargo.toml

Clean workspace-inherited fields. Dependencies are appropriate. No unnecessary crates.

One note: `tower-http` is listed but `CatchPanicLayer` (referenced in `error_handler.rs` doc comment) is not actually wired anywhere in this crate. Either the panic layer needs to be assembled somewhere, or the doc comment is aspirational.

### src/lib.rs

Defines `RestTransportConfig` (host/port) and the `RestTransport` struct implementing the `Transport` trait. The `start()` method only logs and returns `Ok(())` -- it does not actually bind or serve. This is noted in comments as intentional (the Core binary builds the router), but it means the `Transport` trait implementation is a no-op adapter. The `_config` parameter in `start()` is unused, which is consistent with the adapter pattern but worth noting.

The `RestTransportConfig` struct duplicates concepts with `ListenerConfig` in `config/mod.rs` (both have host/address and port). This creates two competing config shapes for the same transport.

### src/config/mod.rs

Well-structured config types covering listeners, handlers, routes, TLS, and auth. Good defaults, good validation.

Strengths:

- Duplicate route detection across all handlers
- Route namespace enforcement (REST must start with `/api/`, GraphQL with `/graphql`)
- Plugin route merging with conflict detection
- Default config ships a complete CRUD route set plus health check

Issues:

- `validate_listener` returns on the first error found rather than accumulating all validation errors. This means users must fix errors one-at-a-time
- The `auth` field on `ListenerConfig` is `Option<AuthConfig>` but nothing in this crate reads or uses it -- the auth middleware's `AuthState` takes its own `AuthProvider` from elsewhere
- Unknown `handler_type` values silently pass namespace validation (the `_ => {}` match arm). Should at least log a warning
- No validation that `method` values are valid HTTP methods -- a route with `method: "FROBNICATE"` would silently pass validation and then be ignored by the router

### src/handlers/mod.rs

Clean translation layer between HTTP and `WorkflowRequest`/`WorkflowResponse`. Good status code mapping and error envelope handling.

Issues:

- Uses `life_engine_types::identity::Identity` (fields: `subject`, `issuer`, `claims`) while the auth middleware defines its own `middleware::auth::Identity` (fields: `user_id`, `provider`, `scopes`). These are two incompatible identity types in the same crate. The handler expects one shape but the auth middleware inserts another. This will cause a runtime `Extension` extraction failure for any authenticated request that goes through both the auth middleware and these handlers
- The `handle_with_body` and `handle_without_body` functions are placeholder stubs returning hardcoded responses. This is expected during development but should be tracked
- The `handle_with_body` doc comment mentions PATCH but the router (`router/mod.rs`) does not handle PATCH method routing
- No request body size validation before deserializing JSON

### src/router/mod.rs

Clean router construction. The `:param` to `{param}` path conversion is correct and well-tested.

Issues:

- Only handles GET, POST, PUT, DELETE. PATCH is silently ignored (falls through the `_ => router` arm). The handler module documents PATCH support but the router cannot route PATCH requests
- The router builds handlers that return a JSON debug response (`{"workflow": ..., "params": ..., "public": ...}`) rather than dispatching to the workflow engine. This is intentional for the current stage but creates a divergence from the handlers in `handlers/mod.rs`, which build `WorkflowRequest` objects. Two parallel handler implementations exist
- No catch-all or fallback handler -- unmatched routes return Axum's default 404, which is a plain text response, not the JSON error envelope defined in `handlers/mod.rs`

### src/middleware/auth.rs

Generally solid auth middleware. Good error mapping, good public route bypass, good separation of auth errors.

Critical issue:

- Public route matching uses the concrete request path (`request.uri().path()`) rather than the route pattern. A route registered as `GET /api/v1/data/:collection` would be in the public routes set as `"GET /api/v1/data/:collection"`, but the actual request path would be `"GET /api/v1/data/tasks"`. These will never match. This means *public route bypass does not work for any parameterized route*. It only works for static paths like `/api/v1/health`
- Defines its own `Identity` struct (`user_id`, `provider`, `scopes`) that conflicts with `life_engine_types::identity::Identity` (`subject`, `issuer`, `claims`). See handlers issue above
- Client IP extraction trusts `X-Forwarded-For` without validation. An attacker can spoof this header to bypass rate limiting or poison logs. Should use `axum::extract::ConnectInfo` for the real peer address, or at minimum take the rightmost entry from `X-Forwarded-For` (the one set by the reverse proxy)
- Rate limited responses set `AUTH_006` code and include retry timing in the message body, but do not set the standard `Retry-After` HTTP header (RFC 6585 Section 4)

### src/middleware/cors.rs

Good auto-configuration strategy (permissive on localhost, strict otherwise). IPv6 handling is careful.

Issues:

- The "strict" fallback allows only `https://localhost` as the origin, which is unusual. When bound to `0.0.0.0` without explicit origins, the API is effectively unusable from any browser that isn't on `https://localhost`. This may be intentional as a safe default but the behavior should be documented
- When explicit origins are provided, `allow_headers` uses `tower_http::cors::Any`, which reflects all requested headers. The `Access-Control-Allow-Headers: *` wildcard combined with credentials is rejected by browsers. If auth cookies or `Authorization` headers are used, `Any` headers plus credentials will fail
- Invalid origin strings in `explicit_origins` are silently filtered out via `.filter_map(|o| o.parse().ok())`. If all origins fail to parse, the list is empty but the code already returned early, so it would produce a CORS layer with an empty allow-list -- effectively blocking all cross-origin requests with no error message

### src/middleware/logging.rs

Simple and correct. Logs method, path, status, and duration with structured tracing.

One minor gap: does not log the request ID (if one is generated or passed via header), making it harder to correlate log entries with specific requests in a multi-request trace.

### src/middleware/error_handler.rs

Correct panic handler that produces a safe 500 response without leaking internal details.

Issue:

- The doc comment says to use it with `CatchPanicLayer::custom(panic_handler)`, but `CatchPanicLayer` is not imported in this file and there is no middleware stack assembly anywhere in the crate that actually wires this in. The panic handler is tested in isolation but may not be connected to the real middleware chain

### src/listener.rs

Well-structured listener with TLS support. Good use of `tokio-rustls` and `rustls-pemfile`.

Issues:

- The TLS serving loop (`serve_tls`) spawns tasks without any concurrency limit. Under heavy load or a connection flood, this could exhaust memory with unbounded task spawning. Consider using a semaphore or connection pool
- TLS handshake failures and connection errors are logged at `debug` level, which may be too quiet for production monitoring. TLS handshake failures can indicate misconfigured clients or active attacks
- No graceful shutdown mechanism. The `serve_plain` and `serve_tls` functions run indefinitely. The `Transport::stop()` method just logs but cannot actually stop the listener
- `build_tls_acceptor` reads cert/key files synchronously (`std::fs::read`) in what is otherwise an async context. This blocks the async runtime during file I/O. Should use `tokio::fs::read`

### src/error.rs

Clean error types with proper `EngineError` implementation. Good severity classification.

The `RequestFailed` variant exists but is never used anywhere in the crate.

### src/types.rs

Empty module -- just a doc comment. Can be removed or left as a placeholder.

### src/tests/mod.rs

Comprehensive test coverage for config validation, route merging, and router construction. Tests verify:
- Default config validity
- Port zero rejection
- TLS empty path rejection
- Duplicate route rejection
- Namespace enforcement for REST and GraphQL
- Default config content (health, CRUD, graphql)
- Route merging and conflict detection
- Router path parameter extraction
- 404 for unknown routes

### src/tests/middleware_test.rs

Good test coverage for CORS, auth, logging, and error handling middleware. The mock auth provider is simple and effective.

Note: the auth bypass test only tests with a static path (`/api/v1/health`). There is no test for public route bypass on a parameterized path, which would reveal the critical bug described above.

## Problems Found

### Critical

- **Auth middleware public-route bypass broken for parameterized routes** (`src/middleware/auth.rs:56-60`). Route keys use pattern syntax (`:collection`) but matching uses concrete paths (`tasks`). Any parameterized route marked as public will still require authentication. This affects any future route that is both parameterized and public
- **Dual Identity types cause runtime extraction failure** (`src/middleware/auth.rs:19-24` vs `src/handlers/mod.rs:17`). The auth middleware inserts `middleware::auth::Identity` as an extension, but handlers extract `life_engine_types::identity::Identity`. These are different types -- Axum's `Extension<T>` is type-keyed, so the handler will get `None` and return a 500 error for every authenticated request once these modules are wired together

### Major

- **X-Forwarded-For spoofable for rate-limit bypass** (`src/middleware/auth.rs:69-74`). Client IP comes from a user-controlled header. Attackers can rotate the header value to circumvent per-IP rate limiting entirely
- **No request body size limits** (crate-wide). Neither `DefaultBodyLimit` nor any other size constraint is applied. A client can send an arbitrarily large JSON body, potentially causing out-of-memory conditions
- **PATCH method not routed** (`src/router/mod.rs:56-62`). The handler module documents PATCH support but the router silently ignores PATCH routes. Any config with `method: "PATCH"` will be silently dropped
- **TLS serve loop has unbounded connection spawning** (`src/listener.rs:54-97`). No concurrency limit on `tokio::spawn` for incoming TLS connections
- **No graceful shutdown** (`src/listener.rs`). `Transport::stop()` is a no-op. The listener cannot be stopped without killing the process
- **Synchronous file I/O in async context** (`src/listener.rs:102-107`). `std::fs::read` blocks the Tokio runtime when reading TLS cert/key files
- **Rate-limited response missing Retry-After header** (`src/middleware/auth.rs:118-126`). The retry timing is in the body but not in the standard HTTP header

### Minor

- **Duplicate config structures** (`src/lib.rs:22-29` vs `src/config/mod.rs:16-31`). `RestTransportConfig` and `ListenerConfig` both model address/port for the same transport
- **Unknown handler types silently pass validation** (`src/config/mod.rs:213`). The `_ => {}` arm means invalid handler types are not flagged
- **No HTTP method validation in config** (`src/config/mod.rs`). Invalid method strings pass validation and are silently ignored by the router
- **Panic handler not wired** (`src/middleware/error_handler.rs`). `CatchPanicLayer` is referenced in docs but never assembled into a middleware stack
- **Empty `types.rs` module** -- dead code
- **Logging middleware omits request ID** (`src/middleware/logging.rs`). No correlation ID in log entries
- **Strict CORS fallback only allows `https://localhost`** (`src/middleware/cors.rs:35-41`). Effectively blocks all browser access when bound to a non-localhost address without explicit origins
- **`RestError::RequestFailed` is unused** (`src/error.rs:11`)
- **`AuthConfig` in `ListenerConfig` is unused** (`src/config/mod.rs:28`). Defined but never read by the auth middleware
- **Router and handlers diverge** (`src/router/mod.rs` vs `src/handlers/mod.rs`). Two separate handler implementations exist -- the router's debug handlers and the handlers module's `WorkflowRequest` builders. These need to be unified

## Recommendations

1. **Fix the public route bypass** by matching against the Axum matched-path (`MatchedPath` extractor) or the route pattern rather than the concrete URI path. Alternatively, use Axum's route-level middleware (`.route_layer()`) to skip auth on specific routes rather than pattern matching in a global middleware
2. **Unify Identity types**. Remove `middleware::auth::Identity` and convert `AuthIdentity` directly to `life_engine_types::identity::Identity` in the middleware. Map `user_id` -> `subject`, `provider` -> `issuer`, and `scopes` -> `claims`
3. **Use `ConnectInfo<SocketAddr>`** for client IP instead of trusting `X-Forwarded-For`, or require a trusted proxy configuration and take the rightmost XFF entry
4. **Add `DefaultBodyLimit`** to the middleware stack. Axum's default is 2MB; consider making it configurable
5. **Add PATCH to the router** (`routing::patch`) alongside GET/POST/PUT/DELETE
6. **Add a connection semaphore** in `serve_tls` to bound concurrent connections
7. **Implement graceful shutdown** using `tokio::signal` and a shutdown channel, stored in the `RestTransport` struct so `stop()` can trigger it
8. **Use `tokio::fs::read`** in `build_tls_acceptor` instead of `std::fs::read`
9. **Set the `Retry-After` header** on rate-limited responses
10. **Consolidate config types** -- remove `RestTransportConfig` from `lib.rs` and use `ListenerConfig` throughout, or vice versa
11. **Wire the panic handler** into the actual middleware stack using `CatchPanicLayer`
12. **Add HTTP method validation** in `validate_listener` to reject invalid method strings
