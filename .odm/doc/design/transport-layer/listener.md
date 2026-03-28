---
title: Listener Configuration
type: reference
created: 2026-03-28
status: draft
---

# Listener Configuration

## Model

Each listener binds one socket. Multiple handler types can be mounted on a single listener, or split across separate listeners. The user controls this entirely through config.

## Config Structure

The listeners config uses a handlers array. This is future-proof — when new transport types are added (WS, CalDAV, webhooks), they slot into the same structure without migration.

```yaml
listeners:
  - binding: http
    port: 8080
    address: 127.0.0.1
    tls:
      cert: /etc/life/cert.pem
      key: /etc/life/key.pem
    auth:
      verify: token
    handlers:
      - type: rest
        routes:
          - method: GET
            path: /api/v1/health
            workflow: system.health
            public: true

          - method: GET
            path: /api/v1/data/:collection
            workflow: collection.list

          - method: GET
            path: /api/v1/data/:collection/:id
            workflow: collection.get

          - method: POST
            path: /api/v1/data/:collection
            workflow: collection.create

          - method: PUT
            path: /api/v1/data/:collection/:id
            workflow: collection.update

          - method: DELETE
            path: /api/v1/data/:collection/:id
            workflow: collection.delete

      - type: graphql
        routes:
          - method: POST
            path: /graphql
            workflow: graphql.query
```

## Default Config

Core ships a default listener config file generated on first run. It includes:

- Generic CRUD routes for REST (`collection.list`, `collection.get`, `collection.create`, `collection.update`, `collection.delete`)
- GraphQL endpoint
- Health check (public)

The user can modify, remove, or extend any of these. Nothing is hardcoded.

## Plugin Manifest Routes

Plugins declare additional routes in their manifest. At startup, Core merges config routes with plugin manifest routes. This is a static merge — adding or removing a plugin requires restarting Core to rebuild the router.

## Route Namespace Validation

At startup, Core validates that:

- REST routes fall under `/api/`
- GraphQL routes fall under `/graphql`

If a conflict is detected, Core rejects startup with a clear error message (e.g., `"Route conflict: REST route '/graphql' collides with GraphQL handler"`). This runs before Axum's router is built, so the user gets a readable error instead of a panic.

## Public Routes

Any route can be marked `public: true` to skip auth. Core does not enforce which routes must be public — the user decides. The default config ships with sensible public routes (health check), but the user can change this.

## TLS

Optional per listener. Two fields:

- `cert` — Path to certificate file
- `key` — Path to private key file

No cert renewal, no OCSP, no cipher suite config. For anything beyond basic TLS, use a reverse proxy.

## Address Binding

- `127.0.0.1` (default) — Localhost only, no external access
- `0.0.0.0` — LAN/internet access. Core logs a startup warning when bound to non-localhost.
