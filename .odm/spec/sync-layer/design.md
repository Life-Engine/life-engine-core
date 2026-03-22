<!--
domain: sync-layer
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Spec — Sync Layer

Reference: [[03 - Projects/Life Engine/Design/App/Architecture/Data & Sync]]

## Purpose

This spec defines the local-first data architecture and the sync mechanism that keeps the App's local SQLite database in sync with Core's database. It is the implementor contract for the SyncAdapter interface, offline queue, conflict resolution, and sync status reporting.

## Local-First Model

All data operations execute against local SQLite. The network is never on the critical path for any user-facing operation. When a plugin calls `data.create()`, the record is written to SQLite immediately and the plugin's UI updates via its subscription callback. Sync with Core happens in the background via the SyncAdapter.

This means the App works fully offline. The user can create, read, update, and delete records without any network connectivity. Changes queue locally and sync when connectivity is restored.

## Data Flow

The data flow from plugin to Core follows this path:

1. **Plugin** calls a ShellAPI data method (e.g. `data.create('todos', { title: 'Buy milk' })`)
2. **Shell Data API** validates capabilities and writes to local SQLite
3. **Local SQLite** stores the record immediately; subscriptions fire
4. **SyncAdapter** (background) detects the pending mutation and uploads it to Core
5. **Core REST API** receives the mutation, applies it to the database, and acknowledges

Incoming changes follow the reverse path: SyncAdapter polls Core, receives new/changed records, writes them to local SQLite, and subscription callbacks fire to update plugin UIs.

## SyncAdapter Interface

The SyncAdapter is an abstraction that decouples the sync mechanism from the shell. Different implementations can be swapped without changing the shell or plugin code.

```typescript
interface SyncAdapter {
  connect(config: SyncConfig): Promise<void>;
  disconnect(): Promise<void>;
  pushChanges(mutations: Mutation[]): Promise<PushResult>;
  pullChanges(since: string): Promise<PullResult>;
  onRemoteChange(callback: (changes: Change[]) => void): Unsubscribe;
}

interface SyncConfig {
  coreUrl: string;
  authToken: string;
  collections: string[];
}

interface Mutation {
  id: string;
  collection: string;
  operation: 'create' | 'update' | 'delete';
  data: object | null;
  timestamp: string;
}

interface PushResult {
  acknowledged: string[];
  conflicts: Conflict[];
}

interface PullResult {
  changes: Change[];
  cursor: string;
}

interface Change {
  collection: string;
  id: string;
  data: object | null;
  operation: 'create' | 'update' | 'delete';
  timestamp: string;
  version: number;
}

interface Conflict {
  mutationId: string;
  reason: string;
  serverRecord: object;
}
```

## Default Implementation — REST Polling SyncAdapter (Phase 1 MVP)

The Phase 1 sync adapter uses simple REST polling against Core's API:

- **Poll interval** — Every 30 seconds, the adapter calls `pullChanges()` to fetch new records from Core since the last sync cursor.
- **Push on write** — When the shell writes a mutation to SQLite, it also enqueues it for the adapter. The adapter pushes mutations to Core immediately if online, or queues them if offline.
- **Offline queue** — Mutations are stored in an ordered queue table in SQLite. When connectivity is restored, the adapter replays queued mutations in order.
- **Conflict resolution** — Last-write-wins per record. If Core has a newer version than the local mutation, the Core version is accepted and the local mutation is discarded. The adapter logs the conflict for debugging.

## PowerSync Implementation (Phase 2+)

A future sync adapter built on PowerSync for more robust sync:

- Keeps Core's PostgreSQL and App's SQLite in sync automatically
- Handles offline queuing with guaranteed delivery
- Supports partial sync via Sync Rules (only sync collections/records the user needs)
- Open source and self-hostable, aligning with Life Engine's self-hosted philosophy

The PowerSync adapter implements the same `SyncAdapter` interface. Switching from REST polling to PowerSync requires no changes to the shell, plugins, or ShellAPI.

## Directed Backend

The sync target URL is user-configurable in the settings page. This supports three deployment scenarios:

- **Local Core** — `http://127.0.0.1:3750` (default). Core runs as a Tauri sidecar on the same machine. Zero-latency sync over loopback.
- **Remote Core** — `https://my-server.com:3750`. Core runs on a user's personal server or VPS. App syncs over the network.
- **Shared Core** — Multiple App instances (e.g. desktop and mobile) sync through the same Core. Each instance has its own local SQLite and syncs independently.

Changing the sync target does not affect plugins. They continue calling `data.query()` and `data.create()` as before — only the SyncAdapter's target URL changes.

## Write Flow

The complete write flow in 5 steps:

1. **Plugin calls create** — `this.__shellAPI.data.create('todos', { title: 'Buy milk' })` returns a promise.
2. **Shell writes locally** — The shell inserts the record into local SQLite with a generated `id`, `_version: 1`, and timestamps. The promise resolves with the new record.
3. **Subscriptions fire** — Any plugin subscribed to the `todos` collection receives the updated record list via its callback. The UI updates immediately.
4. **SyncAdapter queues and uploads** — The mutation is added to the sync queue. If online, the adapter pushes it to Core immediately. If offline, it waits in the queue.
5. **Core confirms** — Core acknowledges the mutation. Other App instances pick up the change on their next pull. If there is a conflict, the adapter handles it per the conflict resolution strategy.

## Offline Queue

- Mutations are stored in an `_sync_queue` table in local SQLite.
- Each entry records the collection, operation, data, and a monotonically increasing sequence number.
- On reconnect, the adapter replays mutations in sequence order. This preserves causality (e.g. create before update).
- Failed mutations are retried with exponential backoff (1s, 2s, 4s, 8s, max 60s).
- The plugin sees no difference between online and offline operation. The ShellAPI behaves identically in both states.

## Conflict Resolution

- **Default strategy** — Last-write-wins per record. Each record has a `_version` field. When pushing a mutation, if Core's `_version` is higher than the local version, Core's record wins and the local mutation is discarded.
- **Optimistic locking** — For plugins that need stricter consistency, the `_version` field can be used for optimistic locking. The plugin reads the current version, makes changes, and submits the update with the expected version. If the version has changed, the update fails and the plugin can retry with the latest data.
- **Conflict logging** — All conflicts are logged to a `_sync_conflicts` table in local SQLite with the mutation details, the server's record, and a timestamp. This allows debugging and potential manual resolution.

## Sync Status Indicator

The shell status bar displays the current sync state:

- **Synced** — All local changes have been pushed to Core and the latest pull is complete.
- **Syncing...** — A push or pull operation is currently in progress.
- **Offline (X changes pending)** — No connectivity to Core. X is the number of mutations in the offline queue.
- **Error** — The last sync attempt failed. Hovering or clicking shows the error details. The adapter continues retrying.

## Two-Tier Collections

Collections are divided into two tiers, mirroring Core's data model:

- **Canonical collections** — Shared, platform-owned collections (e.g. `tasks`, `contacts`, `events`). These sync between App and Core. Multiple plugins can read from the same canonical collection (with appropriate capabilities).
- **Private collections** — Namespaced per plugin (e.g. `com.example.weather:cache`). These are local-only by default and do not sync unless the plugin explicitly declares sync capability. Used for plugin-specific data that has no meaning outside that plugin.

## Acceptance Criteria

- Data operations (query, create, update, delete) complete instantly against local SQLite with no network wait, even when offline.
- The offline queue replays mutations correctly in order when connectivity is restored.
- Sync status in the status bar accurately reflects the current state (Synced, Syncing, Offline with count, Error).
- Changing the sync target URL in settings takes effect on the next sync cycle without requiring any plugin changes.
- Conflicts are resolved via last-write-wins by default and logged to the conflicts table.
- Subscriptions fire on both local writes and incoming sync changes.
