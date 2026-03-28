---
title: Authentication
type: reference
created: 2026-03-28
status: draft
---

# Authentication

## Overview

Auth runs at the transport boundary as middleware. It validates tokens before the request reaches a handler. The resulting `Identity` is passed forward as an Axum `Extension<Identity>` and included in every `WorkflowRequest`.

## Auth Provider

Pocket ID (OIDC) is the primary auth provider, as defined in [[adr-004-pocket-id-oidc-auth]]. The transport layer validates tokens against the OIDC provider — it does not manage users, sessions, or token issuance.

## Public Routes

Any route can be marked `public: true` in the listener config to skip auth. Core does not enforce which routes must be public — the user decides.

The default config ships with sensible public routes:

- `GET /api/v1/health` — Health check

The user can add or remove public routes freely.

## Authorisation (v1)

Authorisation is all-or-nothing for v1:

- Authenticated = authorised to access all collections and workflows
- No per-collection ACLs
- No role-based access control

This is appropriate for a single-user self-hosted system. Per-collection permissions will be considered when multi-user support is added.

## Admin Panel Auth

The admin panel has its own auth mechanism, separate from OIDC:

- Local passphrase, independent of OIDC (solves the bootstrap problem — user needs admin panel access to configure OIDC)
- Configured in the admin panel's top-level config section, not in the listeners config
- On first run, the admin panel is unauthenticated on localhost. The first action is setting a passphrase. After setup, it is locked.
