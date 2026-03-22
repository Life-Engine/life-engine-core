# Federation Protocol Specification

## Overview

The Life Engine Federation Protocol enables peer-to-peer data synchronisation between independently operated Core instances. Each instance maintains full ownership of its data while selectively sharing collections with trusted peers.

## Transport

- **Encryption** — All federation traffic uses mTLS (mutual TLS) over HTTPS.
- **Authentication** — Both sides present X.509 client certificates. Each peer verifies the other's certificate against a pre-exchanged CA or self-signed certificate.
- **No anonymous access** — Requests without a valid client certificate are rejected at the TLS handshake level.

## Sync Model

Federation uses a **pull-based** sync model:

1. The receiving instance polls the offering instance for changes.
2. The offering instance serves changes since the last known cursor.
3. The receiving instance applies changes to its local storage.

This model avoids the need for push notifications and works naturally with firewalled or intermittently connected instances.

## Peer Registration

Before syncing, peers must be explicitly registered on both instances:

- **Name** — A human-readable label for the peer.
- **Endpoint** — The peer's API base URL (e.g. `https://partner.example.com:3750`).
- **Collections** — The list of collections to sync with this peer. Only collections declared by both peers are transferred.
- **Certificates** — Paths to the CA certificate for verifying the peer, and the client certificate/key for authenticating to the peer.

## API Endpoints

The federation API consists of the following endpoints:

- `POST /api/federation/peers` — Register a new peer. Body: `PeerRequest` JSON.
- `GET /api/federation/peers` — List all registered peers.
- `DELETE /api/federation/peers/{id}` — Remove a peer by ID.
- `POST /api/federation/sync` — Trigger a sync with a specific peer. Body: `{ peer_id, collections? }`.
- `GET /api/federation/status` — Get the overall federation status including peer summaries.
- `GET /api/federation/changes/{collection}?since={cursor}` — Serve changes in a collection since the given cursor (called by remote peers).

## Change Record Format

Each change record contains:

- `id` — Record identifier.
- `collection` — The collection name.
- `operation` — One of `create`, `update`, or `delete`.
- `data` — The record payload (null for deletes).
- `version` — Optimistic concurrency version number.
- `timestamp` — ISO 8601 timestamp of the change.

## Conflict Resolution

Conflicts are resolved using **last-write-wins** per record:

- When applying a remote change, if the local record has a higher version number, the local version is kept.
- When the remote version is higher, the remote data overwrites the local record.
- All conflicts are logged for auditability.

## Selective Sync

Only collections explicitly declared in the peer's configuration are eligible for sync. When triggering a sync, an optional `collections` filter further narrows the set. A collection must appear in both the peer's declared list and the filter to be synced.

## Sync Cursors

Each peer-collection pair maintains a cursor (ISO 8601 timestamp string) representing the last successfully synced point. On each pull, only changes newer than the cursor are returned. The cursor is updated after successful application.

## Security Considerations

- **mTLS required** — All federation endpoints require mutual TLS authentication.
- **Collection scoping** — Peers can only access collections explicitly shared with them.
- **No transitive trust** — If instance A shares with B and B shares with C, A does not automatically share with C.
- **Certificate rotation** — Peers can update their certificates by re-registering with new certificate paths.
