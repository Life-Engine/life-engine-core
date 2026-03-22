<!--
domain: sync-layer
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Sync Layer Requirements

## 1. Local-First Operations

All data operations execute against local SQLite with no network dependency.

- **1.1** — WHEN a plugin calls `data.create()`, `data.update()`, `data.delete()`, or `data.query()`, THEN the operation SHALL complete against local SQLite and return within 50ms, regardless of network connectivity.
- **1.2** — WHEN the network is unavailable, THEN all data operations SHALL succeed locally and the user SHALL experience no functional difference from online operation.
- **1.3** — WHEN a local write completes, THEN all active subscriptions for that collection SHALL fire with the updated data before the promise resolves.

## 2. SyncAdapter Interface

The sync mechanism is abstracted behind a pluggable interface.

- **2.1** — The `SyncAdapter` interface SHALL expose `connect(config)`, `disconnect()`, `pushChanges(mutations)`, `pullChanges(since)`, and `onRemoteChange(callback)` methods.
- **2.2** — WHEN a new SyncAdapter implementation is registered, THEN the shell and all plugins SHALL continue to function without code changes.
- **2.3** — WHEN `connect()` is called with a `SyncConfig`, THEN the adapter SHALL validate the `coreUrl` and `authToken` and resolve the promise only after a successful handshake.

## 3. REST Polling Adapter (Phase 1)

The default adapter uses REST polling against Core's API.

- **3.1** — The adapter SHALL poll Core via `pullChanges()` every 30 seconds to fetch new records since the last sync cursor.
- **3.2** — WHEN the shell writes a mutation to SQLite, THEN the adapter SHALL push the mutation to Core immediately if online, or enqueue it if offline.
- **3.3** — WHEN the adapter receives changes from Core via `pullChanges()`, THEN it SHALL write them to local SQLite and fire subscription callbacks for affected collections.

## 4. Offline Queue

Mutations queue locally during offline periods and replay on reconnect.

- **4.1** — The system SHALL store pending mutations in an `_sync_queue` table in local SQLite with collection, operation, data, and a monotonically increasing sequence number.
- **4.2** — WHEN connectivity is restored, THEN the adapter SHALL replay queued mutations in sequence order to preserve causality.
- **4.3** — WHEN a queued mutation fails during replay, THEN the adapter SHALL retry with exponential backoff (1s, 2s, 4s, 8s, max 60s).
- **4.4** — WHEN a mutation is successfully acknowledged by Core, THEN it SHALL be removed from the `_sync_queue` table.

## 5. Conflict Resolution

Conflicts are resolved deterministically with full audit logging.

- **5.1** — WHEN pushing a mutation and Core's `_version` for that record is higher than the local version, THEN the Core version SHALL win (last-write-wins) and the local mutation SHALL be discarded.
- **5.2** — WHEN a conflict occurs, THEN the system SHALL log the conflict to a `_sync_conflicts` table with mutation details, the server record, and a timestamp.
- **5.3** — WHEN a plugin uses optimistic locking by submitting an update with an expected `_version`, THEN the update SHALL fail if the version has changed, and the plugin SHALL receive an error enabling retry with fresh data.

## 6. Sync Status Indicator

The shell status bar displays real-time sync state.

- **6.1** — WHEN all local changes have been pushed and the latest pull is complete, THEN the status bar SHALL display "Synced".
- **6.2** — WHEN a push or pull operation is in progress, THEN the status bar SHALL display "Syncing...".
- **6.3** — WHEN there is no connectivity to Core and the queue has pending mutations, THEN the status bar SHALL display "Offline (X changes pending)" where X is the queue count.
- **6.4** — WHEN the last sync attempt fails, THEN the status bar SHALL display "Error" with details available on hover or click. The adapter SHALL continue retrying.

## 7. Write Flow

The complete write path from plugin to Core acknowledgement.

- **7.1** — WHEN a plugin calls `data.create()`, THEN the shell SHALL insert the record into local SQLite with a generated `id`, `_version: 1`, and timestamps, and resolve the promise with the new record.
- **7.2** — WHEN the local write completes, THEN subscriptions SHALL fire so the plugin UI updates immediately.
- **7.3** — WHEN the SyncAdapter pushes the mutation and Core acknowledges it, THEN other App instances SHALL pick up the change on their next pull.

## 8. Two-Tier Collections

Collections are divided into canonical (synced) and private (local-only).

- **8.1** — Canonical collections (e.g. `tasks`, `contacts`, `events`) SHALL sync between App and Core. Multiple plugins SHALL be able to read from the same canonical collection with appropriate capabilities.
- **8.2** — Private collections (namespaced per plugin, e.g. `com.example.weather:cache`) SHALL be local-only by default and SHALL NOT sync unless the plugin explicitly declares sync capability.
- **8.3** — WHEN a plugin writes to a canonical collection, THEN the mutation SHALL be enqueued for sync. WHEN a plugin writes to a private collection without sync capability, THEN no sync SHALL occur.

## 9. Directed Backend

The sync target URL is user-configurable.

- **9.1** — WHEN the user changes the sync target URL in settings, THEN the change SHALL take effect on the next sync cycle without requiring plugin changes or an App restart.
- **9.2** — The system SHALL support local Core (`http://127.0.0.1:3750`), remote Core, and shared Core (multiple App instances syncing independently) deployment modes.
