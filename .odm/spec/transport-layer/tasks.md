<!--
domain: transport-layer
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Implementation Tasks — Transport Layer

**Progress:** 0 / 24 tasks complete

## 1.1 — Config Data Structures

- [ ] Define `ListenerConfig`, `TlsConfig`, `AuthConfig`, `HandlerConfig`, `HandlerType`, and `RouteConfig` structs
  <!-- files: packages/transport/src/config/mod.rs, packages/transport/src/config/types.rs -->
  <!-- purpose: Establish YAML-deserializable config types for listener configuration -->
  <!-- requirements: 1.1, 1.2, 1.3, 1.4 -->

- [ ] Implement config validation (port range, TLS file existence, duplicate route detection)
  <!-- files: packages/transport/src/config/validation.rs -->
  <!-- purpose: Validate listener config at startup with human-readable errors -->
  <!-- requirements: 1.1, 15.1 -->

- [ ] Implement default config generation on first run
  <!-- files: packages/transport/src/config/defaults.rs -->
  <!-- purpose: Generate default listener YAML with CRUD routes, GraphQL endpoint, and health check -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.4, 2.5 -->

- [ ] Write unit tests for config parsing, validation, and default generation
  <!-- files: packages/transport/src/config/tests.rs -->
  <!-- purpose: Verify config structs deserialize correctly and validation catches invalid configs -->
  <!-- requirements: 1.1, 1.2, 2.1, 2.2, 2.3, 2.4 -->

## 1.2 — Shared Types

- [ ] Define `WorkflowRequest`, `WorkflowResponse`, `WorkflowStatus`, `WorkflowError`, `RequestMeta`, and `ResponseMeta` structs
  <!-- files: packages/types/src/workflow.rs -->
  <!-- purpose: Establish the contract types shared between transport and workflow engine -->
  <!-- requirements: 7.1, 8.1 -->

- [ ] Define `Identity` struct
  <!-- files: packages/types/src/identity.rs -->
  <!-- purpose: Establish the auth identity type passed via Axum extension -->
  <!-- requirements: 13.2 -->

## 1.3 — Router

- [ ] Implement route merging from config and plugin manifests
  <!-- files: packages/transport/src/router/merge.rs -->
  <!-- purpose: Combine config routes with plugin manifest routes into a single route list -->
  <!-- requirements: 3.1, 3.2, 3.4, 5.1 -->

- [ ] Implement route namespace validation (REST under `/api/`, GraphQL under `/graphql`)
  <!-- files: packages/transport/src/router/validation.rs -->
  <!-- purpose: Validate route prefixes and detect cross-handler collisions before Axum router build -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4 -->

- [ ] Build immutable Axum router from validated routes
  <!-- files: packages/transport/src/router/build.rs -->
  <!-- purpose: Construct the Axum Router with path extractors and workflow dispatch closures -->
  <!-- requirements: 5.1, 5.2, 5.3, 5.4 -->

- [ ] Write unit tests for route merging, namespace validation, and router construction
  <!-- files: packages/transport/src/router/tests.rs -->
  <!-- purpose: Verify merging, collision detection, and path parameter extraction -->
  <!-- requirements: 3.1, 3.4, 4.1, 4.2, 4.3, 5.3 -->

## 1.4 — REST Handler

- [ ] Implement REST request-to-WorkflowRequest translation
  <!-- files: packages/transport/src/handlers/rest.rs -->
  <!-- purpose: Extract workflow, identity, params, query, body, and meta from HTTP requests -->
  <!-- requirements: 7.1 -->

- [ ] Implement WorkflowResponse-to-HTTP translation with status code mapping
  <!-- files: packages/transport/src/handlers/rest.rs -->
  <!-- purpose: Map WorkflowStatus to HTTP status codes and produce JSON response shapes -->
  <!-- requirements: 7.2, 7.3, 7.4 -->

- [ ] Write unit tests for REST handler translation (request and response)
  <!-- files: packages/transport/src/handlers/rest_tests.rs -->
  <!-- purpose: Verify correct mapping of HTTP to WorkflowRequest and WorkflowResponse to HTTP -->
  <!-- requirements: 7.1, 7.2, 7.3, 7.4 -->

## 1.5 — GraphQL Handler

- [ ] Implement GraphQL request-to-WorkflowRequest translation
  <!-- files: packages/transport/src/handlers/graphql.rs -->
  <!-- purpose: Parse GraphQL query, flatten arguments into query field, build WorkflowRequest -->
  <!-- requirements: 8.1 -->

- [ ] Implement WorkflowResponse-to-GraphQL translation
  <!-- files: packages/transport/src/handlers/graphql.rs -->
  <!-- purpose: Map WorkflowResponse data and errors to GraphQL response shape -->
  <!-- requirements: 8.2, 8.3 -->

- [ ] Implement GraphQL schema generation from plugin manifest schemas
  <!-- files: packages/transport/src/handlers/graphql_schema.rs -->
  <!-- purpose: Generate GraphQL types at startup from plugin-declared schemas -->
  <!-- requirements: 9.1, 9.2, 9.3, 9.4 -->

- [ ] Write unit tests for GraphQL handler and schema generation
  <!-- files: packages/transport/src/handlers/graphql_tests.rs -->
  <!-- purpose: Verify GraphQL translation and schema generation from manifest declarations -->
  <!-- requirements: 8.1, 8.2, 8.3, 9.1, 9.2, 9.3 -->

## 1.6 — Middleware

- [ ] Implement CORS middleware with auto-config based on bind address
  <!-- files: packages/transport/src/middleware/cors.rs -->
  <!-- purpose: Permissive CORS on localhost, strict on 0.0.0.0, user-overridable origins -->
  <!-- requirements: 12.1, 12.2, 12.3 -->

- [ ] Implement auth middleware with OIDC token validation and public route bypass
  <!-- files: packages/transport/src/middleware/auth.rs -->
  <!-- purpose: Validate tokens via Pocket ID, insert Extension<Identity>, skip public routes -->
  <!-- requirements: 13.1, 13.2, 13.3, 13.4, 14.1, 14.2 -->

- [ ] Implement structured JSON request logging middleware
  <!-- files: packages/transport/src/middleware/logging.rs -->
  <!-- purpose: Log every request with method, path, status, and duration as structured JSON -->
  <!-- requirements: 11.1, 11.2 -->

- [ ] Implement error handling middleware for consistent error shapes
  <!-- files: packages/transport/src/middleware/error.rs -->
  <!-- purpose: Catch panics and unhandled errors, translate to protocol-appropriate error shape -->
  <!-- requirements: 11.1, 11.3 -->

- [ ] Write unit tests for CORS, auth, logging, and error handling middleware
  <!-- files: packages/transport/src/middleware/tests.rs -->
  <!-- purpose: Verify middleware behaviour for each layer -->
  <!-- requirements: 11.1, 12.1, 12.2, 13.1, 13.4 -->

## 1.7 — Listener and TLS

- [ ] Implement listener socket binding with optional TLS via rustls
  <!-- files: packages/transport/src/listener.rs -->
  <!-- purpose: Bind sockets, configure TLS when present, log warnings for non-localhost binding -->
  <!-- requirements: 15.1, 15.2, 15.3, 16.1, 16.2 -->

- [ ] Write integration test for listener startup with and without TLS
  <!-- files: packages/transport/src/listener_tests.rs -->
  <!-- purpose: Verify listener binds correctly and TLS terminates connections -->
  <!-- requirements: 15.1, 15.2, 16.1 -->

## 1.8 — Transport Equivalence

- [ ] Write integration test verifying REST and GraphQL produce identical results for the same workflow
  <!-- files: packages/transport/tests/transport_equivalence.rs -->
  <!-- purpose: Confirm both transports dispatch to the same workflow and return equivalent data -->
  <!-- requirements: 10.1, 10.2 -->
