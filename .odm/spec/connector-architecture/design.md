<!--
domain: connector-architecture
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Connector Architecture

## Purpose

This spec defines how connectors fetch, normalise, and store data from external services. A connector is a regular plugin that declares `http:outbound` and `credentials:read`/`credentials:write` capabilities. There is no special connector trait or category — connectors are just plugins that happen to talk to external APIs.

## Protocol-First Approach

Protocol connectors cover multiple providers with a single implementation. Always prefer protocol connectors over vendor-specific ones.

- **IMAP/SMTP** — Email (Gmail, Fastmail, Proton, iCloud, Yahoo, any provider supporting IMAP)
- **CalDAV** — Calendar (Google Calendar, iCloud, Fastmail, Nextcloud, any CalDAV server)
- **CardDAV** — Contacts (same providers as CalDAV)
- **WebDAV** — Files (Nextcloud, ownCloud, any WebDAV server)
- **S3-compatible** — Object storage (AWS S3, Backblaze B2, Wasabi, MinIO)

Vendor-specific connectors are the fallback when no standard protocol exists — Google Drive API, GitHub API, Notion API, or generic REST APIs.

## Connector Responsibilities

Each connector plugin handles:

- **OAuth token management** — PKCE flow via the `credentials:read`/`credentials:write` capabilities. The host stores and encrypts tokens; connectors request them through host functions. Connectors never handle raw token storage directly.
- **Rate limiting and backoff** — Per-service rate limits respected. Exponential backoff on transient failures.
- **Data fetching** — Pull on demand, periodic sync, or webhook-driven, depending on what the service supports. Outbound HTTP via the `http:outbound` capability, scoped to declared domains.
- **Normalisation** — Translate raw API responses into canonical types (`Email`, `Event`, `Contact`, `File`, `Credential`). See [Normalisation and Storage](#normalisation-and-storage).

## Normalisation and Storage

Connectors write to two locations. Normalised data goes to canonical collections where it is accessible to all plugins. Raw data goes to a private collection for reprocessing when normalisation logic improves.

**Normalised data** is written to the appropriate canonical collection (`events`, `emails`, `contacts`, etc.). This is what other plugins read and work with.

Example normalised record written to the canonical `events` collection:

```json
{
  "title": "Team standup",
  "start": "2026-03-14T09:00:00Z",
  "end": "2026-03-14T09:15:00Z",
  "source": "google_calendar",
  "source_id": "abc123"
}
```

**Raw data** is written to a private `raw_data` collection namespaced to the connector plugin. Raw data is never discarded — it enables reprocessing.

Example raw record written to the private `raw_data` collection:

```json
{
  "source": "google_calendar",
  "source_id": "abc123",
  "fetched_at": "2026-03-14T10:00:00Z",
  "connector_version": "1.0.0",
  "payload": { "raw": "/* original API response */" }
}
```

When normalisation logic improves in a connector update, the connector can reprocess raw records into updated canonical records without re-fetching from the external service.

## Sync Strategies

Three models, matched to what each service supports:

- **Pull on demand** — Fetch when the client explicitly requests it. Simple and always fresh, but adds latency to the request.
- **Periodic sync** — Background job pulls data every N minutes via the scheduler. Fast reads from local storage, but data may be slightly stale. This is the default strategy for most connectors.
- **Webhook-driven** — External services push changes to Core in real time. Requires Core to be publicly addressable. Best for services that support it and environments where Core is internet-facing.

Most connectors use periodic sync as the default, with pull on demand as a fallback for services that do not support incremental sync.

## Connector Lifecycle

```text
Register -> Auth (OAuth PKCE) -> Sync (periodic) -> Active
                                                      |
                                            Disconnect (revoke tokens)
```

1. **Register** — User selects a service to connect. The connector plugin is installed and enabled.
2. **Auth** — OAuth PKCE flow opens in the browser. The user grants access. Core stores encrypted tokens via the credentials capability.
3. **Sync** — Initial full sync fetches all available data. Subsequent syncs are incremental (fetching only changes since the last sync).
4. **Active** — Data is available through the REST API. The connector refreshes tokens automatically before expiry.
5. **Disconnect** — User revokes access. Tokens are deleted locally. The provider revocation endpoint is called if the service supports it.

If a token refresh fails, the connector is marked as requiring re-authentication. The user is notified via an SSE event (`plugin.auth_expired`).

## OAuth Token Handling

All connectors use OAuth 2.0 + PKCE for authentication. Token management is centralised in Core via the credentials capability.

- Tokens are managed by the host — connectors never handle raw token storage directly
- Refresh tokens are encrypted at rest within the encrypted database
- Access tokens are held in memory only and never persisted to disk
- Automatic rotation before expiry prevents service interruptions
- Centralised revocation — revoking a connector revokes all its tokens in one operation
- All token access (reads, writes, rotations, revocations) is recorded in the audit log

## First Connector: Email (IMAP/SMTP)

Email is the first connector to build. It validates the entire connector pipeline:

- IMAP/SMTP is the most universal protocol — covers all email providers
- Email is the highest-value personal data source
- Validates auth (OAuth + IMAP for Gmail), data fetching, normalisation, storage, and sync
- Exercises all connector responsibilities end-to-end

## Acceptance Criteria

- A connector plugin connects to an external service via OAuth PKCE and receives valid tokens
- Normalised data is written to the correct canonical collection with the expected schema
- Raw data is stored in the connector's private `raw_data` collection for reprocessing
- Token refresh works automatically before expiry without user intervention
- Token revocation deletes local tokens and calls the provider revocation endpoint
- Incremental sync fetches only data changed since the last sync
- Rate limiting and exponential backoff handle transient failures gracefully
- All token operations (access, rotation, revocation) are recorded in the audit log
