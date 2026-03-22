<!--
domain: sync-layer
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Sync Layer Tasks

> spec: ./brief.md

---

## 1.1 — Define SyncAdapter TypeScript Interface

> spec: ./brief.md
> depends: none

Define the `SyncAdapter` interface with `connect`, `disconnect`, `pushChanges`, `pullChanges`, and `onRemoteChange` methods. Define supporting types: `SyncConfig`, `Mutation`, `PushResult`, `PullResult`, `Change`, `Conflict`.

- **Files** — `packages/shell/src/sync/types.ts`
- **AC** — All interface methods and types are exported. Types compile with no errors.

---

## 1.2 — Create SyncAdapter Registry

> spec: ./brief.md
> depends: 1.1

Implement a registry that accepts a `SyncAdapter` implementation and makes it available to the shell. Include a `setAdapter()` and `getAdapter()` function.

- **Files** — `packages/shell/src/sync/registry.ts`
- **AC** — `setAdapter()` stores the adapter. `getAdapter()` returns the current adapter. Swapping adapters works without restart.

---

## 2.1 — Implement REST Polling Adapter Core

> spec: ./brief.md
> depends: 1.1

Create a `RestPollingSyncAdapter` class implementing `SyncAdapter`. Wire up `connect()` to store config, `disconnect()` to stop polling, and `onRemoteChange()` to register callbacks.

- **Files** — `packages/shell/src/sync/adapters/rest-polling.ts`
- **AC** — Adapter connects to Core URL, disconnects cleanly, and accepts change callbacks.

---

## 2.2 — Implement pullChanges with Polling Loop

> spec: ./brief.md
> depends: 2.1

Implement `pullChanges()` to fetch changes from Core since the last cursor. Start a 30-second polling loop on `connect()`. Write incoming changes to local SQLite and fire subscription callbacks.

- **Files** — `packages/shell/src/sync/adapters/rest-polling.ts`
- **AC** — Polling runs every 30 seconds. New records from Core appear in local SQLite. Subscription callbacks fire for affected collections.

---

## 2.3 — Implement pushChanges

> spec: ./brief.md
> depends: 2.1

Implement `pushChanges()` to send mutations to Core's REST API. Handle acknowledgements and conflict responses. Return `PushResult` with acknowledged IDs and conflicts.

- **Files** — `packages/shell/src/sync/adapters/rest-polling.ts`
- **AC** — Successful pushes return acknowledged IDs. Conflicts are returned in the `conflicts` array.

---

## 3.1 — Create _sync_queue SQLite Table

> spec: ./brief.md
> depends: none

Add migration to create the `_sync_queue` table with columns: `seq` (auto-increment), `collection`, `operation`, `record_id`, `data` (JSON), `timestamp`.

- **Files** — `packages/shell/src/db/migrations/003_sync_queue.ts`
- **AC** — Migration creates table. `seq` column is monotonically increasing.

---

## 3.2 — Implement Queue Writer and Reader

> spec: ./brief.md
> depends: 3.1

Create functions to enqueue a mutation, dequeue (read and delete) the oldest N mutations, and count pending mutations.

- **Files** — `packages/shell/src/sync/queue.ts`
- **AC** — `enqueue()` inserts a row. `dequeue()` returns mutations in seq order. `count()` returns the number of pending mutations.

---

## 3.3 — Wire Offline Queue into Adapter

> spec: ./brief.md
> depends: 2.3, 3.2

When the adapter detects offline state (push fails with network error), enqueue the mutation. On reconnect, replay queued mutations in order. Implement exponential backoff (1s, 2s, 4s, 8s, max 60s) for failed replays.

- **Files** — `packages/shell/src/sync/adapters/rest-polling.ts`, `packages/shell/src/sync/queue.ts`
- **AC** — Mutations queue when offline. Replay occurs in order on reconnect. Failed replays use exponential backoff.

---

## 4.1 — Create _sync_conflicts SQLite Table

> spec: ./brief.md
> depends: none

Add migration to create the `_sync_conflicts` table with columns: `id`, `mutation_id`, `collection`, `operation`, `local_data` (JSON), `server_data` (JSON), `reason`, `timestamp`.

- **Files** — `packages/shell/src/db/migrations/004_sync_conflicts.ts`
- **AC** — Migration creates table. All columns present and typed correctly.

---

## 4.2 — Implement Last-Write-Wins Resolver

> spec: ./brief.md
> depends: 4.1, 2.3

When `pushChanges()` returns a conflict (Core's `_version` is higher), accept Core's record, discard the local mutation, write the server record to local SQLite, and log the conflict to `_sync_conflicts`.

- **Files** — `packages/shell/src/sync/conflict-resolver.ts`
- **AC** — Core version wins on version mismatch. Local SQLite updated with server record. Conflict logged with full context.

---

## 4.3 — Implement Optimistic Locking Support

> spec: ./brief.md
> depends: 4.2

When a plugin submits an update with an expected `_version` and the version has changed, return an error to the plugin with the current server record so it can retry.

- **Files** — `packages/shell/src/sync/conflict-resolver.ts`
- **AC** — Version mismatch returns a typed error. The error includes the current server record.

---

## 5.1 — Create Sync Status State Machine

> spec: ./brief.md
> depends: 1.1, 3.2

Implement a reactive state machine with four states: `synced`, `syncing`, `offline` (with pending count), `error` (with message). Expose a subscribe function for UI binding.

- **Files** — `packages/shell/src/sync/status.ts`
- **AC** — State transitions correctly between all four states. Subscribers receive updates on every transition.

---

## 5.2 — Build Sync Status Bar Component

> spec: ./brief.md
> depends: 5.1

Create a Lit component that subscribes to sync status and renders the appropriate indicator: green check (synced), spinner (syncing), yellow offline badge with count, red error with hover details.

- **Files** — `packages/shell/src/components/sync-status.ts`
- **AC** — Component renders all four states correctly. Hovering on error shows details.

---

## 6.1 — Integrate Sync into Shell Data API Writes

> spec: ./brief.md
> depends: 2.3, 3.2, 5.1

After a local SQLite write completes, enqueue the mutation for sync, update sync status to `syncing`, push if online or queue if offline. On acknowledgement, update status to `synced`.

- **Files** — `packages/shell/src/data/api.ts`, `packages/shell/src/sync/adapters/rest-polling.ts`
- **AC** — Every data write triggers a sync mutation. Online writes push immediately. Offline writes queue. Status updates reflect current state.

---

## 6.2 — Wire Incoming Sync Changes to Subscriptions

> spec: ./brief.md
> depends: 2.2

When `pullChanges()` writes new records to local SQLite, fire subscription callbacks for all affected collections so plugin UIs update.

- **Files** — `packages/shell/src/data/api.ts`, `packages/shell/src/sync/adapters/rest-polling.ts`
- **AC** — Incoming changes fire subscription callbacks. Plugin UIs reflect remote changes within one poll cycle.

---

## 6.3 — Implement Directed Backend URL Switching

> spec: ./brief.md
> depends: 2.1

When the user changes the sync target URL in settings, disconnect the current adapter, update the config, and reconnect on the next sync cycle. No plugin changes required.

- **Files** — `packages/shell/src/sync/registry.ts`, `packages/shell/src/settings/sync-settings.ts`
- **AC** — Changing the URL takes effect on the next sync cycle. Plugins continue to work without changes.

---

## 7.1 — Implement Collection Tier Classifier

> spec: ./brief.md
> depends: 6.1

Add logic to classify collections as canonical (synced) or private (local-only). Canonical collections enqueue mutations for sync. Private collections skip sync unless the plugin declares sync capability.

- **Files** — `packages/shell/src/sync/collection-tier.ts`, `packages/shell/src/data/api.ts`
- **AC** — Writes to canonical collections enqueue for sync. Writes to private collections without sync capability do not enqueue. Classification is correct for known canonical names and plugin-namespaced names.
