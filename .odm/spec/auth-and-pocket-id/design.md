<!--
domain: auth-and-pocket-id
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Auth and Pocket ID

## Purpose

This spec defines the authentication module for Core. Auth is an independent crate (`packages/auth/`) initialized by Core at startup and shared with all active transports. Two auth mechanisms are supported — Pocket ID (OIDC) for user sessions and API keys for scripting. Both go through the same `AuthProvider` trait and validation pipeline.

## Crate Structure

The auth crate follows the standard crate internal layout:

```
packages/auth/src/
  lib.rs          → Public API (init, AuthModule, Config re-export)
  config.rs       → AuthConfig struct + TOML deserialization
  error.rs        → Auth error types implementing EngineError
  handlers/
    mod.rs
    validate.rs   → Token/key validation logic
    keys.rs       → API key CRUD operations
  types.rs        → AuthIdentity, AuthToken, ApiKey, Scope
  tests/
    mod.rs
    ...
```

The crate depends on `packages/types`, `packages/traits`, and `packages/crypto`. It has no dependency on any transport, storage implementation, or the core binary.

## Configuration

Auth is configured via the `[auth]` section in `config.toml`. Core reads this section at startup and passes it to the auth module for initialization:

```toml
[auth]
provider = "pocket-id"
issuer = "https://auth.local"
```

The `AuthConfig` struct in `packages/auth/src/config.rs` deserializes this section. Required fields are `provider` and `issuer`. Missing or invalid values cause Core to reject startup with a `Fatal` severity error.

## Auth Mechanisms

### Pocket ID (OIDC) — Primary Auth

Pocket ID is the primary authentication mechanism for user sessions. Core validates tokens against the configured OIDC issuer.

- Core validates JWTs against the Pocket ID issuer's public key
- JWT access tokens have a 15-minute expiry, refresh tokens have a 7-day expiry
- Ed25519 signatures for token signing
- Supports passkey/WebAuthn for passwordless login
- Supports user registration (single-user by default, multi-user configurable)
- Plugins inherit auth automatically — no direct credential access, no auth bypass possible

### API Keys — Secondary Auth

API keys provide a simpler alternative for CLI tools and automation scripts.

- Generated and managed through auth module handlers
- Same validation pipeline as OIDC tokens — API keys are not a bypass or lower-security path
- Scoped — keys can be restricted to specific capabilities
- Revocable individually
- Stored as salted hashes using `packages/crypto` — the raw key is shown once at creation and never retrievable again

## AuthProvider Trait

Core abstracts authentication behind an `AuthProvider` trait so implementations are swappable. The active provider is set in `config.toml`:

```toml
[auth]
provider = "pocket-id"
issuer = "https://auth.local"
```

- **pocket-id** — Full OIDC auth via a Pocket ID instance. The primary and default provider.
- **api-key** — API key validation against stored hashes. Used for scripting and automation.

Both providers implement the same trait and plug into the same validation pipeline. The auth module determines which provider to use based on the credential type presented (JWT vs API key prefix).

## Transport Integration

Auth is transport-agnostic. Core initializes the auth module once at startup and passes it to every active transport. Each transport is responsible for:

1. Extracting the credential from the transport-specific location (e.g., `Authorization` header for REST, connection params for GraphQL)
2. Calling the auth module's `validate` method with the extracted credential
3. Attaching the returned `AuthIdentity` to the request context
4. Skipping auth for health check endpoints

The auth module handles all validation logic — transports only extract and forward.

## Auth Validation Flow

1. **Extract credential** — Transport extracts the bearer token or API key from the request
2. **Validate** — Auth module validates the credential against the active provider (JWT verification or hash comparison)
3. **Reject expired/invalid** — Return error with `AUTH_TOKEN_EXPIRED` or `AUTH_TOKEN_MISSING` code
4. **Rate-limit failures** — After 5 failed auth attempts per minute from the same IP, reject with rate limit error
5. **Return identity** — On success, return `AuthIdentity` (user ID, provider type) to the transport

## Error Types

Auth errors implement the `EngineError` trait defined in `packages/traits`:

```rust
// packages/auth/src/error.rs
enum AuthError {
    TokenMissing,       // code: "AUTH_001", severity: Warning
    TokenExpired,       // code: "AUTH_002", severity: Warning
    TokenInvalid,       // code: "AUTH_003", severity: Warning
    ProviderUnreachable,// code: "AUTH_004", severity: Fatal
    ConfigInvalid,      // code: "AUTH_005", severity: Fatal
    RateLimited,        // code: "AUTH_006", severity: Warning
    KeyRevoked,         // code: "AUTH_007", severity: Warning
}
```

Each variant implements `code()`, `severity()`, and `source_module()` (always `"auth"`).

## Plugin Auth

Plugins do not implement their own authentication. They inherit Core's token validation automatically.

- Every request reaching a workflow pipeline has already been authenticated by the transport + auth module
- The `PipelineMessage` metadata includes the authenticated identity
- Plugins cannot access credentials directly — they use the scoped `credentials:read`/`credentials:write` capabilities
- There is no mechanism for a plugin to bypass auth

## Rate Limiting

Rate limiting is built into the auth module, not into individual transports:

- Sliding window: 5 failed attempts per minute per IP
- On trigger: reject with rate limit error and `Retry-After` header value
- On window expiry: allow requests from that IP again
- Applies regardless of which auth provider is active
