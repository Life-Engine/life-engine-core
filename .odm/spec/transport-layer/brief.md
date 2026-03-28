<!--
domain: transport-layer
updated: 2026-03-28
-->

# Transport Layer Spec

## Overview

This spec defines the transport layer of Life Engine Core. The transport layer accepts inbound requests over supported wire protocols (REST and GraphQL in v1), applies middleware (TLS, CORS, auth, logging, error handling), dispatches to the workflow engine via `WorkflowRequest`, and translates `WorkflowResponse` back to the wire format. The transport layer never executes plugin logic, reads storage, or manages users. It is a thin translation boundary between wire protocols and the workflow engine.

Listeners are configured via YAML, with each listener binding a single socket and mounting one or more handler types. Routes come from two sources merged at startup: the listener config file and plugin manifests. The router is built once and is immutable at runtime.

## Goals

- Protocol-agnostic dispatch — handlers translate wire formats to `WorkflowRequest`/`WorkflowResponse`, isolating the workflow engine from HTTP and GraphQL concerns
- User-controlled configuration — listeners, routes, TLS, and public routes are fully configurable via YAML with sensible defaults generated on first run
- Plugin route extensibility — plugins declare additional routes in their manifest, merged into the router at startup alongside config routes
- Consistent auth boundary — token validation via Pocket ID (OIDC) runs as middleware before handlers, with public route opt-out
- Namespace safety — route prefix validation at startup prevents collisions between REST and GraphQL handlers
- Same workflow, both transports — REST and GraphQL dispatch to the same system workflows, producing identical results

## User Stories

- As a user, I want Core to generate a working default listener config on first run so that I can start using the API immediately without manual configuration.
- As a user, I want to configure listeners, ports, TLS, and routes via YAML so that I can adapt the transport layer to my deployment environment.
- As a plugin author, I want to declare custom routes in my plugin manifest so that my plugin exposes its own API endpoints without editing the Core config.
- As a Core developer, I want the router built once at startup and immutable at runtime so that route resolution is fast and predictable.
- As a Core developer, I want REST and GraphQL handlers to translate to the same `WorkflowRequest` so that the workflow engine is protocol-agnostic.
- As a user, I want routes marked `public: true` to skip auth so that I can expose health checks and other unauthenticated endpoints.
- As a Core developer, I want route namespace validation at startup so that REST and GraphQL routes do not collide.
- As a user, I want structured error responses in a consistent shape per protocol so that clients can parse errors reliably.
- As a plugin author, I want my plugin's schema declarations to automatically generate GraphQL types so that collections with declared schemas are queryable via GraphQL.

## Functional Requirements Summary

- The system must support listener configuration via YAML with `binding`, `port`, `address`, `tls`, `auth`, and `handlers` fields.
- The system must generate a default listener config on first run with generic CRUD routes, a GraphQL endpoint, and a public health check.
- The system must merge plugin manifest routes with config routes at startup to build an immutable router.
- The system must validate route namespace prefixes at startup (`/api/` for REST, `/graphql` for GraphQL) and reject startup on conflict.
- The system must apply middleware in order: TLS, CORS, auth, logging, error handling.
- The system must translate HTTP requests to `WorkflowRequest` (REST handler) and GraphQL requests to `WorkflowRequest` (GraphQL handler).
- The system must translate `WorkflowResponse` to HTTP JSON responses (REST) and GraphQL `{ data, errors }` responses (GraphQL).
- The system must validate OIDC tokens via Pocket ID middleware and pass `Identity` as an Axum extension.
- The system must support `public: true` on any route to skip auth.
- The system must generate GraphQL schema at startup from plugin manifest-declared schemas.
- The system must log a startup warning when bound to a non-localhost address.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
