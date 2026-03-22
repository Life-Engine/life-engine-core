<!--
domain: auth-and-pocket-id
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Requirements Document — Auth and Pocket ID

## Introduction

Core requires a unified authentication layer that protects every API route while remaining transparent to plugins. The system supports two swappable auth providers behind a common trait: a simple local-token provider for Phase 1 single-user deployments, and a full Pocket ID OIDC provider for Phase 2 multi-user scenarios. Both providers share the same middleware stack, rate limiting, and identity propagation.

## Alignment with Product Vision

- **Security by default** — All API routes are authenticated with no opt-out. Credentials are stored as salted hashes. Rate limiting prevents brute force attacks.
- **Self-hosted simplicity** — Local-token auth works out of the box with zero external dependencies. Pocket ID adds OIDC when multi-user support is needed.
- **Plugin ecosystem safety** — Plugins inherit auth automatically and cannot bypass or weaken it.

## Requirements

### Requirement 1 — AuthProvider Trait

**User Story:** As a Core developer, I want authentication abstracted behind a trait, so that I can swap providers without changing middleware or route code.

#### Acceptance Criteria

- 1.1. WHEN Core starts THEN the system SHALL read `auth.provider` from config and instantiate the corresponding `AuthProvider` implementation.
- 1.2. WHEN the configured provider is `local-token` THEN the system SHALL use the `LocalTokenProvider` struct.
- 1.3. WHEN the configured provider is `pocket-id` THEN the system SHALL use the `PocketIdProvider` struct.
- 1.4. WHEN an unknown provider value is configured THEN the system SHALL reject startup with a clear error message.

---

### Requirement 2 — Local Token Auth

**User Story:** As a self-hosted user, I want to generate bearer tokens using my master passphrase, so that I can authenticate API requests without OIDC infrastructure.

#### Acceptance Criteria

- 2.1. WHEN a client sends `POST /api/auth/token` with a valid master passphrase THEN the system SHALL return a new bearer token and store its salted hash in the database.
- 2.2. WHEN a client sends `POST /api/auth/token` with an invalid passphrase THEN the system SHALL return HTTP 401 with error code `AUTH_INVALID_PASSPHRASE`.
- 2.3. WHEN a token is generated THEN the system SHALL return the raw token exactly once; subsequent queries SHALL NOT expose the raw value.
- 2.4. WHEN a token has a configured expiry (default 30 days) and that expiry has passed THEN the system SHALL reject requests using that token with `AUTH_TOKEN_EXPIRED`.
- 2.5. WHEN a client sends `DELETE /api/auth/token/{id}` THEN the system SHALL revoke the token immediately and reject all subsequent requests using it.

---

### Requirement 3 — Pocket ID OIDC (Phase 2)

**User Story:** As a household member, I want to log in via Pocket ID with passkey support, so that I have a secure passwordless authentication experience.

#### Acceptance Criteria

- 3.1. WHEN the auth provider is `pocket-id` THEN the system SHALL spawn the Pocket ID Go binary as a managed subprocess during startup step 5.
- 3.2. WHEN a user initiates login THEN the system SHALL complete the OIDC authorization code flow and return a JWT access token (15-minute expiry) and a refresh token (7-day expiry).
- 3.3. WHEN a JWT access token is presented THEN the system SHALL validate it against Pocket ID's Ed25519 public key.
- 3.4. WHEN the Pocket ID process crashes THEN the system SHALL detect the exit and restart it automatically.
- 3.5. WHEN Core shuts down THEN the system SHALL terminate the Pocket ID process as part of the shutdown sequence.
- 3.6. WHEN a refresh token is presented before the access token expires THEN the system SHALL issue a new access token silently.

---

### Requirement 4 — Auth Middleware

**User Story:** As a Core operator, I want every API request validated before reaching any handler, so that unauthenticated access is impossible.

#### Acceptance Criteria

- 4.1. WHEN a request arrives at any `/api/*` route (except `/api/system/health`) THEN the middleware SHALL extract the `Authorization: Bearer <token>` header and validate it.
- 4.2. WHEN the token is missing or malformed THEN the system SHALL return HTTP 401 with error code `AUTH_TOKEN_MISSING`.
- 4.3. WHEN the token is expired THEN the system SHALL return HTTP 401 with error code `AUTH_TOKEN_EXPIRED`.
- 4.4. WHEN the token is valid THEN the middleware SHALL attach the authenticated identity (user ID, provider type) to the request context.
- 4.5. WHEN a request targets `/api/system/health` THEN the middleware SHALL skip authentication and allow the request through.

---

### Requirement 5 — Rate Limiting

**User Story:** As a Core operator, I want failed authentication attempts rate-limited, so that brute force attacks are blocked.

#### Acceptance Criteria

- 5.1. WHEN a client IP sends 5 failed auth attempts within 1 minute THEN the system SHALL return HTTP 429 for subsequent requests from that IP until the window resets.
- 5.2. WHEN rate limiting is triggered THEN the response SHALL include a `Retry-After` header indicating seconds until the limit resets.
- 5.3. WHEN the rate limit window expires THEN the system SHALL allow requests from that IP again.

---

### Requirement 6 — Plugin Auth Inheritance

**User Story:** As a plugin author, I want authentication handled before my routes are called, so that I do not need to implement auth myself.

#### Acceptance Criteria

- 6.1. WHEN a request reaches a plugin-registered route THEN the auth middleware SHALL have already validated the token and attached the identity.
- 6.2. WHEN a plugin handler executes THEN it SHALL have access to the authenticated identity from the request context.
- 6.3. WHEN a plugin attempts to access credentials directly THEN the system SHALL enforce the `credentials:read` or `credentials:write` capability check.

---

### Requirement 7 — API Key Management

**User Story:** As a developer, I want to create scoped API keys, so that I can authenticate automation scripts with least-privilege access.

#### Acceptance Criteria

- 7.1. WHEN a client sends `POST /api/auth/keys` with a scope list THEN the system SHALL generate a scoped API key and return the raw key once.
- 7.2. WHEN an API key is used in a request THEN the middleware SHALL validate it against stored salted hashes and enforce its scope restrictions.
- 7.3. WHEN a client sends `DELETE /api/auth/keys/{id}` THEN the system SHALL revoke the key immediately.
- 7.4. WHEN a client sends `GET /api/auth/keys` THEN the system SHALL return a list of active keys with metadata (id, scope, created date) but not the raw key values.
