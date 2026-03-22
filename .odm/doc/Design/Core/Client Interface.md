---
title: "Engine — Client Interface"
tags: [life-engine, engine, client, sync, tauri]
created: 2026-03-14
---

# Client Interface

How App communicates with Core. App is local-first — it maintains its own SQLite database for instant reads and writes. Sync with Core runs in the background.

The client interface implements several [[03 - Projects/Life Engine/Design/Principles|Design Principles]]: *Separation of Concerns* (plugins never see the sync layer — they call the Shell Data API and the shell handles replication), *Open/Closed Principle* (the `SyncAdapter` abstraction allows replacing the sync implementation without changing plugin code), and *Single Source of Truth* (the REST API contract is defined in a shared `packages/api` crate — one definition consumed by all clients).

## Communication Model

```
App
  |
  +-- Local SQLite (instant reads/writes)
  |     |
  |     +-- SyncAdapter (background)
  |           |
  |           +-- Core REST API
  |
  +-- Tauri Commands (local OS operations)
```

Plugins never see the sync layer. They call the App shell's data API and the shell handles replication.

## REST API Contract

App talks to Core via the REST API defined in the shared `packages/api` crate. The API contract is the same regardless of where Core runs — localhost, LAN, or remote.

Key endpoints App uses:

- `/api/data/*` — Data sync (query, create, update, delete per collection)
- `/api/plugins/*` — Plugin management (including connector plugins)
- `/api/auth/*` — Token exchange and refresh
- `/api/system/health` — Check Core availability
- `/api/events/stream` — SSE for real-time notifications

## Sync Protocol (PowerSync)

The default sync implementation uses **PowerSync**:

- Keeps Core's database in sync with App's local SQLite
- Handles offline queuing — writes made offline are replayed in order when connectivity returns
- Partial sync via Sync Rules — each user only downloads their own data
- Open source and self-hostable (PowerSync Open Edition)

The `SyncAdapter` is an abstraction — PowerSync is the default, not a hard dependency. Replaceable with direct REST polling, libSQL sync, or a CRDT-based implementation without changing plugin code.

## Directed Backend

The sync target URL is user-configurable:

- **Local Core** — `localhost:3750` (default, bundled Core on same machine)
- **Remote Core** — `https://my-server.com:3750` (self-hosted on a VPS or home server)
- **Shared Core** — Multiple App instances on different devices pointing at the same Core

Changing the target requires no plugin changes — the data API is identical regardless of where Core lives.

## Bundled Mode

For non-technical users, Core runs as a subprocess of the App:

- Core binary bundled inside the Tauri application
- App spawns Core on startup, kills it on shutdown
- Pocket ID sidecar also managed by App
- User never sees a terminal, config file, or server setup
- Everything runs on `localhost` — no network configuration needed

## Auth Flow (Pocket ID)

1. App triggers login via a Core API call
2. Core initiates OIDC flow with the Pocket ID sidecar
3. User authenticates (passkey or passphrase)
4. Core receives and validates tokens
5. App stores session token, attaches to all API requests
6. Core silently refreshes tokens before expiry

Plugins inherit auth automatically — no per-plugin auth configuration.

## Offline-First Behaviour

- All reads and writes happen against App's local SQLite — instant, no network dependency
- Mutations made offline are stored in an ordered queue
- When connectivity returns, the SyncAdapter replays mutations in order
- Plugins see no difference — subscriptions fire normally from the local database
- Conflict resolution: last-write-wins per record (default), optimistic locking via `_version` field available

## Write Flow

1. App plugin calls `shell.data.create('todos', { title: 'Buy milk' })`
2. App shell writes to local SQLite immediately — returns instantly
3. Plugin UI updates via subscription callback
4. SyncAdapter queues the write and uploads to Core in background
5. Core confirms — other devices receive the update via sync stream

## Reconnection Strategy

- SyncAdapter monitors connectivity
- On disconnect: queue mutations locally, continue serving from local database
- On reconnect: replay queued mutations, pull latest changes from Core
- Connector re-auth: if Core reports expired connector tokens, surface re-auth prompt to user through App UI
