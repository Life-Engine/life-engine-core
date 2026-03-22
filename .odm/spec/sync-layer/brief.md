<!--
domain: sync-layer
status: draft
tier: 1
updated: 2026-03-22
-->

# Sync Layer Spec

## Overview

This spec defines the local-first data architecture and the sync mechanism that keeps the App's local SQLite database in sync with Core's database. It covers the SyncAdapter interface, offline queue, conflict resolution, sync status reporting, and the two-tier collection model.

## Goals

- Execute all data operations against local SQLite with no network dependency so the App works fully offline
- Provide a pluggable SyncAdapter interface so sync implementations can be swapped without changing plugins or the shell
- Queue mutations reliably during offline periods and replay them in order when connectivity returns
- Resolve conflicts deterministically using last-write-wins with full audit logging
- Display real-time sync status so users always know the state of their data

## User Stories

- As a user, I want to create and edit data instantly without waiting for a server so that the App feels fast regardless of network conditions.
- As a user, I want to see a clear indicator of sync status so that I know whether my changes have been uploaded.
- As a user, I want my offline changes to sync automatically when I reconnect so that I do not lose work.
- As a plugin author, I want the same data API to work identically online and offline so that I do not need to handle network state.
- As an admin, I want to point the App at a different Core URL so that I can switch between local and remote servers.

## Functional Requirements

- The system must complete all data operations (query, create, update, delete) against local SQLite with no network wait.
- The system must define a `SyncAdapter` interface with `connect`, `disconnect`, `pushChanges`, `pullChanges`, and `onRemoteChange` methods.
- The system must persist pending mutations in an `_sync_queue` table and replay them in order on reconnect.
- The system must resolve conflicts using last-write-wins per record based on the `_version` field.
- The system must log all conflicts to a `_sync_conflicts` table with full context.
- The system must display sync status (Synced, Syncing, Offline with pending count, Error) in the shell status bar.
- The system must fire subscription callbacks on both local writes and incoming sync changes.
- The system must support two-tier collections: canonical (synced) and private (local-only by default).

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
