---
title: "Engine — Connectors"
tags: [life-engine, engine, connectors, oauth, sync]
created: 2026-03-14
---

# Connector Plugins

A connector is a plugin that handles the connection to one external service. It fetches data, normalises it into canonical types, and writes it to the data layer. Connectors are regular plugins that declare `http:outbound`, `credentials:read`, and `credentials:write` capabilities — there is no special connector category or trait. See [[03 - Projects/Life Engine/Design/Core/Plugins|Plugins — Capabilities]] for the full capability model.

Connectors exemplify several [[03 - Projects/Life Engine/Design/Principles|Design Principles]]: *Open/Closed Principle* (adding a new connector is adding a new plugin — Core does not change), *Parse, Don't Validate* (raw data is normalised into typed canonical records at the boundary — downstream code works with validated types), *Principle of Least Privilege* (each connector declares exactly which domains it can reach and which credential types it can access), and *Explicit Over Implicit* (OAuth requirements, sync strategy, and rate limits are declared in the manifest).

## Protocol-First Approach

Protocol connectors cover multiple providers with one implementation. Always prefer these over vendor-specific connectors.

- **IMAP/SMTP** — Email (Gmail, Fastmail, Proton, iCloud, Yahoo, any provider)
- **CalDAV** — Calendar (Google Calendar, iCloud, Fastmail, Nextcloud, any CalDAV server)
- **CardDAV** — Contacts (same providers as CalDAV)
- **WebDAV** — Files (Nextcloud, ownCloud, any WebDAV server)
- **S3-compatible** — Object storage (AWS S3, Backblaze B2, Wasabi, MinIO)

Vendor connectors are the fallback when no protocol exists (Google Drive API, GitHub API, Notion API, generic REST).

## Connector Responsibilities

Each connector plugin handles:

- **OAuth token management** — PKCE flow via the `credentials:read` / `credentials:write` capabilities. The host stores and encrypts tokens; connectors request them through host functions.
- **Rate limiting and backoff** — Per-service rate limits, exponential backoff on failures
- **Data fetching** — Pull, periodic sync, or webhook-driven (depending on what the service supports). Outbound HTTP via the `http:outbound` capability, scoped to declared domains.
- **Normalisation** — Translate raw API responses into canonical types (`Email`, `Event`, `Contact`, `File`, `Credential`)

## Normalisation and Storage

Connectors write normalised data to **canonical collections** — the platform-owned data types defined in the SDK. See [[03 - Projects/Life Engine/Design/Core/Data#Canonical Collections (platform-owned)|Data — Canonical Collections]] for the full list.

Raw and normalised data are stored separately:

- **Normalised data** → Written to the appropriate canonical collection (`events`, `emails`, `contacts`, etc.). This is what other plugins read and work with.
- **Raw data** → Written to a private `raw_data` collection namespaced to the connector plugin. Used for reprocessing when normalisation logic improves.

Example normalised record (written to canonical `events` collection):

```json
{
  "title": "Team standup",
  "start": "2026-03-14T09:00:00Z",
  "end": "2026-03-14T09:15:00Z",
  "source": "google_calendar",
  "source_id": "abc123"
}
```

Example raw record (written to private `raw_data` collection):

```json
{
  "source": "google_calendar",
  "source_id": "abc123",
  "fetched_at": "2026-03-14T10:00:00Z",
  "connector_version": "1.0.0",
  "payload": { /* original API response */ }
}
```

Raw data is never discarded. When normalisation logic improves, the connector can reprocess raw records into updated canonical records.

## Sync Strategies

Three models, matched to service capabilities:

- **Pull on demand** — Fetch when the client asks. Simple, fresh data, but slow under load.
- **Periodic sync** — Background job pulls every N minutes. Fast reads, slightly stale data.
- **Webhook-driven** — External services push changes. Real-time, but Core must be publicly addressable.

Most connectors will use periodic sync as the default, with pull on demand as a fallback.

## Connector Lifecycle

```
Register -> Auth (OAuth PKCE) -> Sync (periodic) -> Active
                                                      |
                                            Disconnect (revoke tokens)
```

1. **Register** — User selects a service to connect
2. **Auth** — OAuth PKCE flow opens in browser, user grants access, Core stores encrypted tokens
3. **Sync** — Initial full sync, then periodic incremental syncs
4. **Active** — Data available through the API, connector refreshes tokens automatically
5. **Disconnect** — User revokes access. Tokens deleted locally. Provider revocation endpoint called if supported.

If token refresh fails, the connector is marked as requiring re-auth and the user is notified.

## OAuth Token Handling

- All connectors use OAuth 2.0 + PKCE
- Tokens are managed by the host via the `credentials` capability — connectors never handle raw token storage directly
- Refresh tokens encrypted at rest, access tokens held in memory only
- Automatic rotation before expiry
- Centralised revocation and audit logging

## First Connector Plugin: Email (IMAP/SMTP)

This is the first connector plugin to build because:

- IMAP/SMTP is the most universal protocol — covers all email providers
- Email is the highest-value personal data source
- Validates the entire connector pipeline (auth, fetch, normalise, store, sync)
- OAuth + IMAP for Gmail is well-documented
