<!--
domain: app
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Shell Data API — Requirements

## 1. Data Namespace

- **1.1** — WHEN `data.query(collection, filter)` is called with a declared `data:read:<collection>` capability, THEN it SHALL return records from local SQLite matching the filter.
- **1.2** — WHEN `data.query` is called with an undeclared collection, THEN it SHALL throw a `CapabilityError` identifying the plugin and collection.
- **1.3** — WHEN `data.create(collection, data)` is called with a declared `data:write:<collection>` capability, THEN it SHALL insert the record and return it with generated `id`, `_version`, `_created`, and `_updated` fields.
- **1.4** — WHEN `data.update(collection, id, data)` is called, THEN it SHALL merge the provided data into the existing record and increment `_version`.
- **1.5** — WHEN `data.delete(collection, id)` is called, THEN it SHALL remove the record and return `void`.
- **1.6** — WHEN `data.subscribe(collection, callback)` is called, THEN the callback SHALL fire immediately with current records and again on every local change (writes or background sync).
- **1.7** — WHEN the unsubscribe function returned by `data.subscribe` is called, THEN no further callbacks SHALL fire for that subscription.

## 2. HTTP Namespace

- **2.1** — WHEN `http.get(url)` is called and the URL's domain is in the plugin's `allowedDomains`, THEN the shell SHALL execute the request and return the response.
- **2.2** — WHEN any HTTP method is called with a domain not in `allowedDomains`, THEN it SHALL throw a `CapabilityError` identifying the plugin and domain.
- **2.3** — WHEN the plugin does not declare the `network:fetch` capability, THEN all HTTP namespace methods SHALL throw a `CapabilityError`.
- **2.4** — WHEN `options.timeout` is specified, THEN the request SHALL abort after the given milliseconds and reject with a timeout error.

## 3. Storage Namespace

- **3.1** — WHEN `storage.set(key, value)` is called with the `storage:local` capability, THEN the value SHALL be JSON-serialised and persisted locally.
- **3.2** — WHEN `storage.get(key)` is called for a non-existent key, THEN it SHALL return `null`.
- **3.3** — WHEN `storage.delete(key)` is called, THEN the key-value pair SHALL be removed.
- **3.4** — WHEN the plugin does not declare `storage:local`, THEN all storage namespace methods SHALL throw a `CapabilityError`.
- **3.5** — WHEN storage data is persisted, THEN it SHALL be scoped to the plugin's ID and not accessible by other plugins.

## 4. Settings Namespace

- **4.1** — WHEN `settings.set(key, value)` is called, THEN the value SHALL be persisted in `settings.json` without requiring any capability.
- **4.2** — WHEN `settings.get(key)` is called for a non-existent key, THEN it SHALL return `null`.
- **4.3** — WHEN `settings.subscribe(key, callback)` is called, THEN the callback SHALL fire immediately with the current value and again on each change.
- **4.4** — WHEN the plugin is updated, THEN settings data SHALL survive the update.

## 5. UI Namespace

- **5.1** — WHEN `ui.navigate(route)` is called, THEN the shell SHALL navigate to the specified route.
- **5.2** — WHEN `ui.back()` is called, THEN the shell SHALL navigate to the previous route.
- **5.3** — WHEN `ui.toast(message, variant)` is called, THEN the shell SHALL display a toast notification with the given message and variant. If variant is omitted, it SHALL default to `info`.
- **5.4** — WHEN `ui.openModal(element)` is called with the `ui:overlay` capability, THEN the shell SHALL display the element in a modal dialog.
- **5.5** — WHEN `ui.openModal` is called without the `ui:overlay` capability, THEN it SHALL throw a `CapabilityError`.
- **5.6** — WHEN `ui.setTitle(title)` is called, THEN the shell SHALL update the page title in the top bar.

## 6. IPC Namespace

- **6.1** — WHEN `ipc.send(targetPluginId, event, payload)` is called with a declared `ipc:send:<target>` capability, THEN the shell SHALL deliver the message to the target plugin.
- **6.2** — WHEN `ipc.send` is called targeting an undeclared plugin, THEN it SHALL throw a `CapabilityError`.
- **6.3** — WHEN `ipc.send` is called and the target plugin is not running, THEN the message SHALL be silently dropped.
- **6.4** — WHEN `ipc.on(event, handler)` is called, THEN the handler SHALL receive messages from any plugin without requiring a capability.
- **6.5** — WHEN the unsubscribe function from `ipc.on` is called, THEN no further messages SHALL be delivered to that handler.

## 7. Plugin Namespace

- **7.1** — WHEN `plugin.id` is read, THEN it SHALL return the plugin's unique identifier from its manifest.
- **7.2** — WHEN `plugin.version` is read, THEN it SHALL return the plugin's version string from its manifest.
- **7.3** — WHEN a write is attempted on `plugin.id` or `plugin.version`, THEN it SHALL be silently ignored or throw (the properties are read-only).

## 8. Injection and Scoping

- **8.1** — WHEN the shell creates a plugin's custom element, THEN it SHALL set `el.__shellAPI` before adding the element to the DOM.
- **8.2** — WHEN `connectedCallback` fires on the plugin element, THEN `this.__shellAPI` SHALL be fully initialised and usable.
- **8.3** — WHEN the shell constructs the scoped API, THEN it SHALL read the plugin's manifest and exclude methods for undeclared capabilities.
