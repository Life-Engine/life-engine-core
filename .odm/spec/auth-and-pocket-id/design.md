<!--
domain: auth-and-pocket-id
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Auth and Pocket ID

Reference: [[03 - Projects/Life Engine/Design/Core/API]]

## Purpose

This spec defines the authentication mechanisms for Core and the Pocket ID OIDC sidecar. Core supports two auth mechanisms — Pocket ID (OIDC) for user sessions and API keys for local dev and scripting. Both go through the same middleware stack.

## Auth Mechanisms

### Pocket ID (OIDC) — Primary Auth

Pocket ID is the primary authentication mechanism for user sessions. It runs as a bundled Go binary sidecar, spawned and managed by Core.

- Core handles the OIDC flow and token exchange
- Every request is validated against Pocket ID tokens
- JWT with 15-minute access tokens and 7-day refresh tokens
- Ed25519 signatures for token signing
- Supports passkey/WebAuthn for passwordless login
- Supports user registration (single-user by default, multi-user configurable)
- Plugins inherit auth automatically — no direct credential access, no auth bypass possible

### API Keys — Secondary Auth

API keys provide a simpler alternative for local development, CLI tools, and automation scripts.

- Generated and managed through the `/api/auth` endpoints
- Same middleware validation as OIDC tokens — API keys are not a bypass or lower-security path
- Scoped — keys can be restricted to specific route groups
- Revocable individually or in bulk
- Stored as salted hashes — the raw key is shown once at creation and never retrievable again

## AuthProvider Trait

Core abstracts authentication behind an `AuthProvider` trait so the local-token and Pocket ID implementations are swappable. The active provider is set in config:

```yaml
auth:
  provider: "local-token"  # Phase 1 default
  # provider: "pocket-id"  # Phase 2 default
```

- **local-token** — Simple bearer token auth. Default in Phase 1. Suitable for local development and single-user setups.
- **pocket-id** — Full OIDC auth via the Pocket ID sidecar. Default in Phase 2. Required for multi-user and remote access.

Both providers implement the same trait and plug into the same middleware. Switching requires only a config change and restart.

## Local Token Auth (Phase 1)

Local token auth is the default in Phase 1. It provides a simple bearer token mechanism without the complexity of OIDC.

- `POST /api/auth/token` — Generate a new token. Requires the master passphrase in the request body for verification.
- Tokens are stored as salted hashes in the database — the raw token is returned once at creation.
- Configurable expiry (default 30 days).
- Revocation via `DELETE /api/auth/token/{id}`.
- Expired tokens are rejected automatically.

## Auth Middleware

The auth middleware runs on every `/api/*` route (except `/api/system/health` which is unauthenticated for monitoring). It performs the following checks in order:

1. **Extract token** — Read the `Authorization: Bearer <token>` header.
2. **Validate** — Check the token against stored hashes (local-token) or validate the JWT against Pocket ID's public key (OIDC).
3. **Reject expired tokens** — Return `AUTH_TOKEN_EXPIRED` with HTTP 401.
4. **Rate-limit failed attempts** — After 5 failed auth attempts per minute from the same IP, return HTTP 429. This applies regardless of which auth provider is active.
5. **Attach identity** — On success, attach the authenticated identity to the request context for downstream handlers.

## Pocket ID Sidecar Architecture

Pocket ID runs as a bundled Go binary, managed by Core as a subprocess.

- **Binary location** — Configured via `auth.pocket_id.binary_path` in the YAML config.
- **Port** — Listens on a separate port (default `3751`), configured via `auth.pocket_id.port`.
- **Lifecycle** — Core spawns Pocket ID during startup (step 5 of the startup sequence). Core monitors the process and restarts it if it crashes. Core terminates it during shutdown.
- **Storage** — Pocket ID uses Core's data directory for its database and configuration.
- **Communication** — Core communicates with Pocket ID over localhost HTTP. No external network exposure for the sidecar.

Pocket ID capabilities:

- OIDC token issuance and validation
- Passkey/WebAuthn registration and authentication
- User registration and management
- Token refresh and revocation

## Plugin Auth

Plugins do not implement their own authentication. They inherit Core's token validation automatically.

- Every request reaching a plugin route has already been authenticated by Core's middleware
- Plugins receive the authenticated identity via the request context
- Plugins cannot access credentials directly — they use the scoped `credentials:read`/`credentials:write` capabilities
- There is no mechanism for a plugin to bypass auth

## Acceptance Criteria

- All `/api/*` routes (except `/api/system/health`) require a valid auth token
- Local token auth works end-to-end: generate token with master passphrase, use token in requests, revoke token
- Pocket ID OIDC flow completes (Phase 2): login via browser, receive tokens, access API, token refresh works silently
- Rate limiting blocks brute force attempts — 5 failed attempts per minute per IP triggers HTTP 429
- Token refresh works silently — clients receive a new access token before the old one expires
- Expired tokens are rejected with `AUTH_TOKEN_EXPIRED`
- Switching between `local-token` and `pocket-id` providers requires only a config change
- Plugin routes receive only authenticated requests — no bypass is possible
