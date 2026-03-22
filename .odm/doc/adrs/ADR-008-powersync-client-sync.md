# ADR-008: PowerSync for client-server synchronisation

## Status
Accepted

## Context

Life Engine is a local-first application. The App client reads and writes to a local SQLite database for instant, offline-capable responses. Changes made locally must eventually synchronise to Core, and changes made in Core (by connectors fetching new emails, calendar events, etc.) must propagate to the App client. This sync system must:

- Handle conflict resolution when the same record is modified both locally and on Core before sync completes.
- Work over intermittent connectivity — the App must remain fully functional without a network connection.
- Use SQLite on both ends (App client and Core) so that the sync protocol does not require changing the storage layer.
- Not require the App to implement a custom CRDT (conflict-free replicated data type) from scratch, which is a complex and error-prone undertaking.
- Be self-hostable alongside Core without requiring a managed cloud service in the critical data path.

The sync problem is one of the hardest problems in distributed systems. The stakes are high: data loss or data corruption in a user's personal data engine would be catastrophic for trust. Using a battle-tested library rather than building from scratch is strongly preferred.

## Decision

PowerSync is used to provide the local-first sync layer between the App's local SQLite database and Core. PowerSync is a client-side synchronisation library designed specifically for SQLite-based local-first applications. The App embeds the PowerSync client SDK, which manages the local SQLite state, conflict resolution, and sync queue. Core implements the PowerSync server protocol (a simple HTTP API for download and upload), enabling the App to sync with Core's SQLite database.

PowerSync's sync buckets model allows fine-grained control over which data is synced to the App — the user's own data, shared collections, and cached read-only views can each be configured as separate sync buckets with independent sync policies.

## Consequences

Positive consequences:

- PowerSync's conflict resolution is based on last-write-wins with a client-side merge strategy. For personal data (the primary use case), last-write-wins is the correct default.
- SQLite on both sides means there is no impedance mismatch between the local schema and the server schema. Migrations are applied to both the local and remote databases in coordinated steps.
- The App remains fully functional offline. Writes are queued in the upload queue and flushed when connectivity is restored.
- PowerSync is designed for production use (used in real applications at scale). Its conflict resolution and queue management have been tested beyond Life Engine's Phase 1 scale.
- The server-side protocol is a simple HTTP API that Core implements. Core does not depend on a PowerSync-managed cloud service.
- PowerSync's client SDK handles connection management, retry logic, and back-pressure — Core does not need to implement these.

Negative consequences:

- PowerSync is a third-party dependency with its own release cadence. Breaking changes in the PowerSync SDK require coordinated updates in the App.
- The server-side protocol that Core must implement, while straightforward, adds API surface that is not part of Core's primary REST API design. This is a maintenance obligation.
- PowerSync's sync bucket model requires careful design to avoid syncing data the user has not explicitly requested. Misconfigured buckets can over-sync (privacy concern) or under-sync (data availability concern).
- PowerSync's commercial model (it offers a hosted tier in addition to the self-hosted protocol) may create uncertainty about the long-term availability of the self-hosted protocol. This is mitigated by the protocol being open and Core owning the server implementation.

## Alternatives Considered

**libSQL sync** (the Turso/libSQL replication protocol) was evaluated as a modern SQLite-based sync option. It was rejected because the sync protocol was experimental at evaluation time and the Turso team's primary focus is the hosted cloud product. Self-hosted libSQL sync was not stable enough to stake Phase 1's data integrity on.

**Custom CRDT implementation** was considered because CRDTs provide strong eventual consistency guarantees without a central authority. CRDTs were rejected because implementing a correct CRDT for structured relational data (not just simple counters or sets) requires expertise in distributed systems that the team does not have, and mistakes in a CRDT implementation lead to subtle data corruption bugs that are difficult to detect. The risk of a home-built CRDT causing data loss was considered unacceptable for a personal data sovereignty platform.

**Firebase Realtime Database / Firestore** provide battle-tested sync out of the box. They were rejected because they are cloud services that require storing user data on Google's infrastructure, which directly contradicts the data sovereignty goal of Life Engine. Even if technically excellent, they are architecturally incompatible with the project's values.
