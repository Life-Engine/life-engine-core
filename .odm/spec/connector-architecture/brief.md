<!--
domain: connector-architecture
status: draft
tier: 1
updated: 2026-03-22
-->

# Connector Architecture Spec

## Overview

This spec defines how connectors fetch, normalise, and store data from external services. A connector is a regular plugin that declares `http:outbound` and `credentials:read`/`credentials:write` capabilities. Connectors use a protocol-first approach, preferring standard protocols (IMAP, CalDAV, CardDAV) over vendor-specific APIs.

## Goals

- Protocol-first connectors — cover multiple providers with a single implementation using standard protocols
- Dual-write storage — normalised data goes to canonical collections for cross-plugin access; raw data is retained for reprocessing
- Secure credential management — OAuth PKCE tokens managed by the host with automatic refresh and encrypted storage
- Incremental sync — fetch only changed data after the initial full sync to minimise network traffic and latency

## User Stories

- As a user, I want to connect my email account via OAuth so that Life Engine can fetch and display my messages.
- As a user, I want my data synced periodically in the background so that it is available without manual refresh.
- As a user, I want to disconnect a service and have all tokens revoked so that the external service can no longer be accessed.
- As a plugin author, I want normalised data in canonical collections so that I can build features on email, calendar, and contact data without parsing raw API responses.

## Functional Requirements

- The system must authenticate with external services using OAuth 2.0 + PKCE.
- The system must store encrypted OAuth tokens via the credentials capability and refresh them automatically before expiry.
- The system must normalise external API responses into canonical collection schemas (emails, events, contacts).
- The system must store raw API responses in a private `raw_data` collection for future reprocessing.
- The system must support periodic sync as the default strategy with pull-on-demand as a fallback.
- The system must perform incremental sync after the initial full sync, fetching only changes since the last sync.
- The system must revoke tokens locally and at the provider endpoint when a user disconnects a service.
- The system must log all token operations (access, rotation, revocation) in the audit log.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
