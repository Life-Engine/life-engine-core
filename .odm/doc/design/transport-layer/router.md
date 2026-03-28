---
title: Router
type: reference
created: 2026-03-28
status: draft
---

# Router

## Overview

The router matches incoming HTTP requests to handlers by path and method. It is built once at startup and is immutable at runtime.

## Route Sources

Routes come from two places, merged at startup:

- **Listener config** — Explicitly declared routes in the config file. This includes the default generic CRUD routes shipped with Core.
- **Plugin manifests** — Plugins declare additional routes in their manifest. These are merged into the router alongside config routes.

Adding or removing a plugin requires restarting Core to rebuild the router.

## Route Matching

The router extracts path parameters (`:collection`, `:id`) and passes them as `params: HashMap<String, String>` in the [[workflow-engine-contract|WorkflowRequest]].

Each route maps to a workflow by name (e.g., `collection.list`, `collection.get`). The router does not know what the workflow does — it just dispatches.

## Generic CRUD Routes

Core ships default routes for collection-level CRUD:

- `GET /api/v1/data/:collection` → `collection.list`
- `GET /api/v1/data/:collection/:id` → `collection.get`
- `POST /api/v1/data/:collection` → `collection.create`
- `PUT /api/v1/data/:collection/:id` → `collection.update`
- `DELETE /api/v1/data/:collection/:id` → `collection.delete`

These are declared in the default config file, not hardcoded. The user can modify or remove them. The `:collection` parameter is dynamic — any collection in storage is accessible.

## Namespace Enforcement

At startup, the router validates route prefixes:

- REST handler routes must start with `/api/`
- GraphQL handler routes must start with `/graphql`

Conflicts produce a clear error message and prevent startup. This validation runs before Axum builds its router.

## GraphQL Routing

GraphQL has a single route (`POST /graphql`) that dispatches to `graphql.query`. The GraphQL handler internally resolves the query against collections, but from the router's perspective it is a single route to a single workflow.

GraphQL query arguments (limit, offset, filters) are flattened into the `query` field of `WorkflowRequest`, same as REST query parameters.
