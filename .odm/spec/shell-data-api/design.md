<!--
domain: app
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Spec — Shell Data API

Reference: [[03 - Projects/Life Engine/Design/App/Architecture/Shell API]]

## Purpose

This spec defines the complete ShellAPI interface injected into every plugin. It covers all seven namespaces, scoping rules, and the TypeScript type definitions that the plugin SDK exports.

## Injection

The shell sets `el.__shellAPI = scopedAPI` on the plugin's custom element before it is added to the DOM. By the time `connectedCallback` fires, the API is fully available at `this.__shellAPI`.

The API surface is scoped by the plugin's declared capabilities. A plugin that does not declare `data:read:todos` cannot call `this.__shellAPI.data.query('todos', ...)` — the call will throw.

## ShellAPI Namespaces

The ShellAPI exposes seven namespaces. Six are functional; one is metadata.

### data

All operations run against local SQLite. Sync happens in the background and is invisible to the plugin.

- `query(collection: string, filter?: QueryFilter): Promise<Record[]>` — Read records from a collection. Filter supports `where`, `orderBy`, `limit`, `offset`. Returns an array of records.
- `create(collection: string, data: object): Promise<Record>` — Insert a new record. Returns the created record with its generated `id` and `_version`.
- `update(collection: string, id: string, data: object): Promise<Record>` — Update an existing record by ID. Merges `data` into the existing record. Returns the updated record.
- `delete(collection: string, id: string): Promise<void>` — Delete a record by ID.
- `subscribe(collection: string, callback: (records: Record[]) => void): Unsubscribe` — Subscribe to changes in a collection. The callback fires immediately with current data, then again whenever local data changes (from local writes or sync). Returns an unsubscribe function.

Requires `data:read:<collection>` for query and subscribe. Requires `data:write:<collection>` for create, update, and delete.

### http

Proxied and permission-checked against the plugin's `allowedDomains` list.

- `get(url: string, options?: RequestOptions): Promise<Response>` — HTTP GET request.
- `post(url: string, body: any, options?: RequestOptions): Promise<Response>` — HTTP POST request.
- `put(url: string, body: any, options?: RequestOptions): Promise<Response>` — HTTP PUT request.
- `delete(url: string, options?: RequestOptions): Promise<Response>` — HTTP DELETE request.

Requires `network:fetch` capability and the target domain must be in the manifest's `allowedDomains` array.

### storage

Plugin-private key-value storage. Data is stored locally and not synced.

- `get(key: string): Promise<any>` — Retrieve a value by key. Returns `null` if the key does not exist.
- `set(key: string, value: any): Promise<void>` — Store a value. The value is JSON-serialised.
- `delete(key: string): Promise<void>` — Remove a key-value pair.

Requires `storage:local` capability.

### settings

Plugin settings storage. Always available (no capability required). Settings are persisted in `settings.json` and survive plugin updates.

- `get(key: string): Promise<any>` — Retrieve a setting value.
- `set(key: string, value: any): Promise<void>` — Store a setting value.
- `subscribe(key: string, callback: (value: any) => void): Unsubscribe` — Subscribe to changes on a specific setting key. Fires immediately with current value, then on each change.

### ui

Shell-level navigation and user feedback. Always available (no capability required, except `ui:overlay` for modals).

- `navigate(route: string): void` — Navigate to a shell route (e.g. another plugin's main view).
- `back(): void` — Navigate to the previous route.
- `toast(message: string, variant?: 'info' | 'success' | 'warning' | 'error'): void` — Show a toast notification. Defaults to `info` variant.
- `openModal(element: HTMLElement): void` — Open a modal dialog containing the given element. Requires `ui:overlay` capability.
- `closeModal(): void` — Close the currently open modal.
- `setTitle(title: string): void` — Set the page title displayed in the top bar.

### ipc

Inter-plugin messaging. Requires `ipc:send:<target-plugin-id>` capability for sending.

- `send(targetPluginId: string, event: string, payload?: any): void` — Send a message to another plugin. The target must be running. If it is not, the message is silently dropped.
- `on(event: string, handler: (payload: any, senderId: string) => void): Unsubscribe` — Listen for incoming messages on an event name. Any plugin can listen; only sending requires a capability. Returns an unsubscribe function.

### plugin

Plugin metadata. Always available, read-only.

- `id: string` — The plugin's unique identifier from its manifest.
- `version: string` — The plugin's version string from its manifest.

## Data API Scoping

The shell checks every data operation against the plugin's declared collections:

- If a plugin declares `data:read:todos` and `data:write:todos`, it can query, subscribe, create, update, and delete records in the `todos` collection.
- If a plugin declares only `data:read:contacts`, it can query and subscribe but not write.
- Accessing an undeclared collection throws an error: `CapabilityError: Plugin "com.example.plugin" does not have access to collection "secrets"`.

## HTTP API Scoping

The shell checks every outbound HTTP request against the plugin's `allowedDomains` array:

- If a plugin declares `allowedDomains: ["api.weather.com"]`, it can make requests to any path on `api.weather.com`.
- Requests to undeclared domains throw: `CapabilityError: Plugin "com.example.plugin" is not allowed to access domain "evil.com"`.

## IPC Scoping

The shell checks every `ipc.send()` call against the plugin's declared IPC targets:

- If a plugin declares `ipc:send:com.life-engine.calendar`, it can send messages to the calendar plugin only.
- Sending to an undeclared target throws: `CapabilityError: Plugin "com.example.plugin" cannot send IPC to "com.life-engine.notes"`.

## TypeScript Types

The plugin SDK (`@life-engine/plugin-sdk`) exports the following type definitions:

```typescript
interface ShellAPI {
  data: {
    query(collection: string, filter?: QueryFilter): Promise<Record[]>;
    create(collection: string, data: object): Promise<Record>;
    update(collection: string, id: string, data: object): Promise<Record>;
    delete(collection: string, id: string): Promise<void>;
    subscribe(collection: string, callback: (records: Record[]) => void): Unsubscribe;
  };
  http: {
    get(url: string, options?: RequestOptions): Promise<Response>;
    post(url: string, body: any, options?: RequestOptions): Promise<Response>;
    put(url: string, body: any, options?: RequestOptions): Promise<Response>;
    delete(url: string, options?: RequestOptions): Promise<Response>;
  };
  storage: {
    get(key: string): Promise<any>;
    set(key: string, value: any): Promise<void>;
    delete(key: string): Promise<void>;
  };
  settings: {
    get(key: string): Promise<any>;
    set(key: string, value: any): Promise<void>;
    subscribe(key: string, callback: (value: any) => void): Unsubscribe;
  };
  ui: {
    navigate(route: string): void;
    back(): void;
    toast(message: string, variant?: ToastVariant): void;
    openModal(element: HTMLElement): void;
    closeModal(): void;
    setTitle(title: string): void;
  };
  ipc: {
    send(targetPluginId: string, event: string, payload?: any): void;
    on(event: string, handler: (payload: any, senderId: string) => void): Unsubscribe;
  };
  plugin: {
    readonly id: string;
    readonly version: string;
  };
}

type ToastVariant = 'info' | 'success' | 'warning' | 'error';
type Unsubscribe = () => void;

interface QueryFilter {
  where?: Record<string, any>;
  orderBy?: { field: string; direction: 'asc' | 'desc' };
  limit?: number;
  offset?: number;
}

interface RequestOptions {
  headers?: Record<string, string>;
  timeout?: number;
}

interface Record {
  id: string;
  _version: number;
  _created: string;
  _updated: string;
  [key: string]: any;
}
```

## Acceptance Criteria

- All seven namespaces (data, http, storage, settings, ui, ipc, plugin) are accessible from `this.__shellAPI` inside a plugin's `connectedCallback`.
- Data operations (query, create, update, delete) complete instantly against local SQLite with no network wait.
- Scoping is enforced for data (collection-level), http (domain-level), and ipc (target-level). Undeclared access throws `CapabilityError`.
- Data subscriptions fire immediately with current state and again on every local change (from local writes or background sync).
- UI namespace methods (toast, navigate, setTitle) work without any capability declaration.
- IPC `on()` works without capability; only `send()` requires `ipc:send:<target>`.
