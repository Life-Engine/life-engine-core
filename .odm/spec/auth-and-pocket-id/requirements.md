<!--
domain: auth-and-pocket-id
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Requirements Document — Auth and Pocket ID

## Introduction

Core requires a unified authentication module extracted into its own crate (`packages/auth/`). The auth module is initialized by Core at startup and shared with all active transports. It is transport-agnostic — REST, GraphQL, CalDAV, CardDAV, and Webhook transports all use the same auth module. Two providers are supported behind the `AuthProvider` trait: Pocket ID (OIDC) as the primary provider for user sessions, and API keys for scripting and automation. Both providers share the same validation pipeline, rate limiting, and identity propagation.

## Alignment with Product Vision

- **Defence in Depth** — All transport endpoints are authenticated with no opt-out. Credentials are stored as salted hashes. Rate limiting prevents brute force attacks.
- **Separation of Concerns** — Auth is an independent crate, not embedded in any transport. Core initializes it once and shares it with all transports.
- **Plugin ecosystem safety** — Plugins inherit auth automatically and cannot bypass or weaken it.
- **Explicit Over Implicit** — Auth provider and configuration are declared in `config.toml`, not inferred.

## Requirements

### Requirement 1 — Auth Crate Structure

**User Story:** As a Core developer, I want auth extracted to its own crate following the standard layout, so that transports and other modules can depend on it without circular dependencies.

#### Acceptance Criteria

- 1.1. WHEN the `packages/auth/` crate is built THEN it SHALL follow the standard crate layout: `lib.rs`, `config.rs`, `error.rs`, `handlers/`, `types.rs`, `tests/`.
- 1.2. WHEN `packages/auth/` defines error types THEN they SHALL implement the `EngineError` trait with `code()`, `severity()`, and `source_module()` methods.
- 1.3. WHEN `packages/auth/` is used THEN it SHALL depend only on `packages/types`, `packages/traits`, and `packages/crypto` — never on transports or storage.

---

### Requirement 2 — AuthProvider Trait

**User Story:** As a Core developer, I want authentication abstracted behind a trait, so that I can swap providers without changing transport or middleware code.

#### Acceptance Criteria

- 2.1. WHEN Core starts THEN the system SHALL read the `[auth]` section from `config.toml` and instantiate the corresponding `AuthProvider` implementation.
- 2.2. WHEN the configured provider is `pocket-id` THEN the system SHALL use the `PocketIdProvider` struct.
- 2.3. WHEN an API key is presented THEN the system SHALL validate it through the `ApiKeyProvider`.
- 2.4. WHEN an unknown provider value is configured THEN the system SHALL reject startup with a clear error message and `Fatal` severity.

---

### Requirement 3 — Pocket ID OIDC (Primary Auth)

**User Story:** As a household member, I want to log in via Pocket ID with passkey support, so that I have a secure passwordless authentication experience.

#### Acceptance Criteria

- 3.1. WHEN the auth provider is `pocket-id` THEN the system SHALL validate tokens against the configured OIDC issuer URL.
- 3.2. WHEN a user initiates login THEN the system SHALL complete the OIDC authorization code flow and return a JWT access token (15-minute expiry) and a refresh token (7-day expiry).
- 3.3. WHEN a JWT access token is presented THEN the system SHALL validate it against the Pocket ID issuer's public key.
- 3.4. WHEN a refresh token is presented before the access token expires THEN the system SHALL issue a new access token silently.
- 3.5. WHEN the OIDC issuer is unreachable at startup THEN the system SHALL fail with a `Fatal` severity error.

---

### Requirement 4 — Auth Validation

**User Story:** As a Core operator, I want every transport request validated before reaching any handler, so that unauthenticated access is impossible.

#### Acceptance Criteria

- 4.1. WHEN a request arrives at any transport endpoint (except health checks) THEN the auth module SHALL extract and validate the bearer token or API key.
- 4.2. WHEN the token is missing or malformed THEN the system SHALL return an error with code `AUTH_TOKEN_MISSING`.
- 4.3. WHEN the token is expired THEN the system SHALL return an error with code `AUTH_TOKEN_EXPIRED`.
- 4.4. WHEN the token is valid THEN the auth module SHALL return the authenticated identity (user ID, provider type) for attachment to the request context.
- 4.5. WHEN a request targets a health check endpoint THEN the auth module SHALL skip authentication.

---

### Requirement 5 — Rate Limiting

**User Story:** As a Core operator, I want failed authentication attempts rate-limited, so that brute force attacks are blocked.

#### Acceptance Criteria

- 5.1. WHEN a client IP sends 5 failed auth attempts within 1 minute THEN the system SHALL reject subsequent requests from that IP until the window resets.
- 5.2. WHEN rate limiting is triggered THEN the response SHALL include a `Retry-After` header indicating seconds until the limit resets.
- 5.3. WHEN the rate limit window expires THEN the system SHALL allow requests from that IP again.

---

### Requirement 6 — Plugin Auth Inheritance

**User Story:** As a plugin author, I want authentication handled before my pipeline steps execute, so that I do not need to implement auth myself.

#### Acceptance Criteria

- 6.1. WHEN a request reaches a workflow pipeline THEN the auth module SHALL have already validated the token and attached the identity.
- 6.2. WHEN a plugin step executes THEN the `PipelineMessage` metadata SHALL include the authenticated identity from the auth context.
- 6.3. WHEN a plugin attempts to access credentials directly THEN the system SHALL enforce the `credentials:read` or `credentials:write` capability check.

---

### Requirement 7 — API Key Management

**User Story:** As a developer, I want to create scoped API keys, so that I can authenticate automation scripts with least-privilege access.

#### Acceptance Criteria

- 7.1. WHEN an API key is created with a scope list THEN the system SHALL generate a scoped API key and return the raw key once.
- 7.2. WHEN an API key is used in a request THEN the auth module SHALL validate it against stored salted hashes and enforce its scope restrictions.
- 7.3. WHEN an API key is revoked THEN the system SHALL reject all subsequent requests using it.
- 7.4. WHEN API keys are listed THEN the system SHALL return metadata (id, scope, created date) but not the raw key values.

---

### Requirement 8 — Configuration

**User Story:** As a deployer, I want auth configured via a TOML section, so that it integrates naturally with the rest of Core's configuration.

#### Acceptance Criteria

- 8.1. WHEN Core reads `config.toml` THEN the system SHALL parse the `[auth]` section with at minimum `provider` and `issuer` fields.
- 8.2. WHEN the `[auth]` section is missing required fields THEN Core SHALL refuse to start with a validation error.
- 8.3. WHEN the auth config is parsed THEN it SHALL be deserialized into the `AuthConfig` struct defined in `packages/auth/src/config.rs`.
