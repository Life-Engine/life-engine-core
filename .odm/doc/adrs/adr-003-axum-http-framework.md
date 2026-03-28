---
title: "ADR-003: axum as the HTTP Framework for Core"
type: adr
created: 2026-03-27
status: active
---

# ADR-003: axum as the HTTP Framework for Core

## Status

Accepted

## Context

Core exposes a REST API over HTTP for the App client and for external integrations. The HTTP framework must:

- Support async/await with `tokio` as the runtime, since Core's plugin execution, database access, and sync operations are all async.
- Compose middleware cleanly so that cross-cutting concerns (authentication, rate limiting, CORS, audit logging, error handling) can be applied uniformly without duplicating logic across routes.
- Integrate naturally with the tower middleware ecosystem, which provides production-ready implementations of many cross-cutting concerns.
- Have a type-safe routing and handler model that makes route errors detectable at compile time rather than runtime.
- Be actively maintained with a stable API.

The framework also needed to support a structured extractor pattern (reading request bodies, query parameters, and headers into typed Rust structs) that aligns with the "Parse, Don't Validate" principle: request inputs are parsed into domain types at the boundary and downstream handlers work with typed data.

## Decision

`axum` is used as the HTTP framework for Core's REST API. axum is built on top of `tower` and `hyper` and integrates directly with the `tokio` async runtime. Routes are defined using a type-safe `Router` with typed extractors (`Json<T>`, `Query<T>`, `Path<T>`, `Extension<T>`). Middleware is layered using tower's `ServiceBuilder` and `Layer` abstractions, enabling uniform application of auth, logging, rate limiting, and error handling across all routes.

The `tower-http` crate provides production-ready middleware for CORS, request tracing (via `tracing`), compression, and request ID injection. These are applied globally in the middleware stack rather than per-route.

## Consequences

Positive consequences:

- axum's extractor model enforces "Parse, Don't Validate": handlers receive typed values, not raw `Request` objects. Invalid requests are rejected at the extractor boundary before handler code runs.
- First-class tower integration means any tower-compatible middleware (rate limiting, circuit breaking, tracing) works without adaptation layers.
- axum's `Router` is composable and nestable. Feature areas (user routes, plugin routes, system routes) are defined in separate modules and merged at the top level, maintaining Separation of Concerns.
- axum uses `hyper` under the hood, which is one of the fastest HTTP implementations in the Rust ecosystem.
- Error handling via axum's `IntoResponse` trait allows domain errors to be converted to HTTP responses at the boundary without leaking internal error types to clients.
- Strong compatibility with `tokio::spawn` for spawning background tasks from request handlers (e.g., triggering a sync after a plugin writes data).

Negative consequences:

- axum's `State` extractor and the requirement to make handler state `Clone + Send + Sync` can require wrapping shared state in `Arc`. This is idiomatic Rust but adds boilerplate for contributors unfamiliar with the pattern.
- axum's macro-free design means routes are verbose compared to attribute-macro frameworks. A route that would be three lines in Rocket can be ten lines in axum.
- The tower middleware model has a learning curve. Composing `Layer` types with correct generic bounds can be opaque for contributors new to tower.
- axum is younger than actix-web and has had some breaking changes between minor versions. The trade-off is better long-term API stability as it matures.

## Alternatives Considered

**actix-web** is the most-used Rust HTTP framework by download count and has the highest benchmark performance numbers. It was rejected because it is built on the actor model, which introduces a distinct concurrency paradigm orthogonal to the rest of Core's async/await code. Managing `Actor` traits and message passing adds cognitive overhead that is not justified for Core's workload. actix-web also does not integrate with tower middleware natively, which would require custom adapter code.

**warp** uses a filter-based composition model where routes and middleware are combined using combinators. This was evaluated but rejected because the filter API is unfamiliar to most contributors (it looks unlike standard router APIs), type errors in filter chains produce extremely long and opaque compiler error messages, and warp's ecosystem is smaller than axum's.

**Rocket** uses proc-macro attributes on handler functions to define routes. While ergonomic, Rocket's macro-heavy approach makes it difficult to introspect routing logic programmatically (e.g., for generating OpenAPI specs) and its async support was added later and is considered less idiomatic than axum's first-class async design. Rocket also does not integrate with tower.
