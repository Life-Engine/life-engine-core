<!--
domain: connector-architecture
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Requirements Document — Connector Architecture

## Introduction

Connectors are Life Engine plugins that fetch, normalise, and store data from external services. They use the standard plugin capability system (declaring `http:outbound` and `credentials:read`/`credentials:write`) rather than a special connector API. This spec covers the shared patterns all connectors follow: OAuth authentication, data normalisation, raw data retention, sync strategies, and the connector lifecycle. The IMAP email connector serves as the first implementation to validate the full pipeline.

## Alignment with Product Vision

- **Open/Closed Principle** — Connectors are regular plugins, so new services can be added without modifying Core
- **Defence in Depth** — Tokens are encrypted at rest, access tokens are memory-only, and all token operations are audit-logged
- **Explicit Over Implicit** — OAuth requirements, sync strategy, and allowed domains are declared in the connector manifest
- **Parse, Don't Validate** — Raw API responses are normalised into typed canonical collections at the boundary

## Requirements

### Requirement 1 — OAuth PKCE Authentication

**User Story:** As a user, I want to connect my external accounts via OAuth, so that Life Engine can access my data securely without storing my password.

#### Acceptance Criteria

- 1.1. WHEN a user initiates a service connection THEN the system SHALL start an OAuth 2.0 + PKCE flow by opening the provider's authorisation URL in the user's browser.
- 1.2. WHEN the OAuth callback is received with a valid authorisation code THEN the system SHALL exchange the code for access and refresh tokens.
- 1.3. WHEN tokens are received THEN the system SHALL store the refresh token encrypted via the `credentials:write` capability and hold the access token in memory only.
- 1.4. WHEN the access token approaches expiry (within 5 minutes) THEN the system SHALL use the refresh token to obtain a new access token automatically.
- 1.5. WHEN a token refresh fails with an invalid_grant error THEN the system SHALL mark the connector as requiring re-authentication and emit a `plugin.auth_expired` SSE event.

### Requirement 2 — Data Normalisation

**User Story:** As a plugin author, I want connector data normalised into canonical collections, so that I can build features on structured data without parsing raw API responses.

#### Acceptance Criteria

- 2.1. WHEN a connector fetches data from an external service THEN the system SHALL transform the response into the canonical collection schema (e.g. `emails`, `events`, `contacts`).
- 2.2. WHEN normalised data is written THEN each record SHALL include a `source` field identifying the originating service and a `source_id` field containing the provider's unique identifier.
- 2.3. WHEN a normalised record already exists with the same `source` and `source_id` THEN the system SHALL update the existing record rather than creating a duplicate.
- 2.4. WHEN normalised data fails schema validation THEN the system SHALL log the validation error and skip the invalid record without aborting the sync.

### Requirement 3 — Raw Data Storage

**User Story:** As a user, I want raw API responses stored locally, so that improved normalisation logic can reprocess my data without re-fetching from external services.

#### Acceptance Criteria

- 3.1. WHEN a connector fetches data from an external service THEN the system SHALL write the raw API response to a private `raw_data` collection namespaced to the connector plugin.
- 3.2. WHEN raw data is stored THEN each record SHALL include `source`, `source_id`, `fetched_at` (RFC 3339 timestamp), `connector_version`, and `payload` fields.
- 3.3. WHEN a connector is updated with improved normalisation logic THEN the system SHALL support reprocessing existing raw records into updated canonical records.
- 3.4. WHEN raw data is stored THEN it SHALL NOT be accessible to other plugins — only the owning connector can read its raw_data collection.

### Requirement 4 — Sync Strategies

**User Story:** As a user, I want my external data synced automatically in the background, so that it is available without manual refresh.

#### Acceptance Criteria

- 4.1. WHEN a connector uses periodic sync THEN the system SHALL schedule background fetches at the interval defined in the connector's configuration (default: every 15 minutes).
- 4.2. WHEN a periodic sync runs after the initial full sync THEN the system SHALL perform an incremental sync, fetching only data changed since the last sync timestamp.
- 4.3. WHEN a connector uses pull-on-demand sync THEN the system SHALL fetch fresh data from the external service when the user explicitly requests it.
- 4.4. WHEN a sync encounters a transient error (network timeout, 429, 5xx) THEN the system SHALL retry with exponential backoff (initial delay 1s, max delay 5 minutes, max 5 retries).
- 4.5. WHEN a sync encounters a rate limit response THEN the system SHALL respect the `Retry-After` header before making another request.

### Requirement 5 — Connector Lifecycle

**User Story:** As a user, I want to connect and disconnect services with clear lifecycle states, so that I know the status of each connection.

#### Acceptance Criteria

- 5.1. WHEN a connector plugin is installed and enabled THEN the system SHALL set its state to `registered` and present the OAuth flow to the user.
- 5.2. WHEN OAuth completes successfully THEN the system SHALL transition the connector state to `syncing` and begin the initial full sync.
- 5.3. WHEN the initial sync completes THEN the system SHALL transition the connector state to `active`.
- 5.4. WHEN a user disconnects a service THEN the system SHALL delete all local tokens, call the provider's token revocation endpoint (if supported), and transition the state to `disconnected`.
- 5.5. WHEN a connector is in the `auth_expired` state THEN the system SHALL prevent sync operations until the user re-authenticates.

### Requirement 6 — Rate Limiting and Backoff

**User Story:** As a user, I want connectors to respect external service rate limits, so that my account is not throttled or banned.

#### Acceptance Criteria

- 6.1. WHEN a connector makes outbound HTTP requests THEN the system SHALL enforce per-service rate limits as configured in the connector manifest.
- 6.2. WHEN a 429 response is received THEN the system SHALL pause requests and wait for the duration specified in the `Retry-After` header.
- 6.3. WHEN a transient failure occurs THEN the system SHALL apply exponential backoff starting at 1 second with a maximum of 5 minutes between retries.

### Requirement 7 — Audit Logging for Token Operations

**User Story:** As a user, I want all credential access logged, so that I can review what accessed my tokens and when.

#### Acceptance Criteria

- 7.1. WHEN a token is read from the credential store THEN the system SHALL record the operation in the audit log with `event_type: "credential.read"`, the plugin_id, and a timestamp.
- 7.2. WHEN a token is rotated THEN the system SHALL record the operation in the audit log with `event_type: "credential.rotate"`.
- 7.3. WHEN a token is revoked THEN the system SHALL record the operation in the audit log with `event_type: "credential.revoke"`.

### Requirement 8 — IMAP Email Connector (First Implementation)

**User Story:** As a user, I want to connect my email account so that Life Engine can fetch and display my messages from any IMAP-compatible provider.

#### Acceptance Criteria

- 8.1. WHEN the IMAP connector is configured with valid credentials THEN the system SHALL connect to the mail server and fetch the inbox message list.
- 8.2. WHEN emails are fetched THEN the system SHALL normalise them into the canonical `emails` collection with fields: `subject`, `from`, `to`, `date`, `body_text`, `source`, and `source_id`.
- 8.3. WHEN the raw email data is fetched THEN the system SHALL store the full RFC 822 message in the connector's private `raw_data` collection.
- 8.4. WHEN an incremental sync runs THEN the system SHALL use IMAP UIDVALIDITY and UID to fetch only new or changed messages since the last sync.
