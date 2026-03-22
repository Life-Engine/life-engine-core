<!--
domain: capability-enforcement
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Spec — Capability Enforcement

Reference: [[03 - Projects/Life Engine/Design/App/Architecture/Capability System]]

## Purpose

This spec defines the capability system that governs plugin access to shell resources. Capabilities are declared in the plugin manifest, granted at install time with user approval, and enforced at runtime on every API call. It is the implementor contract for all permission checking logic.

## Principle

Deny by default. A plugin receives no access to any protected resource unless it explicitly declares the capability in its manifest and the user approves it during installation. There are no implicit grants, no escalation paths, and no way to acquire capabilities after installation without reinstalling.

## Capability Namespaces

The following capability strings are recognized by the shell:

- **data:read:collection** — Read records from a specific collection (canonical or private). Example: `data:read:todos` grants read access to the `todos` collection. The plugin can call `data.query()` and `data.subscribe()` for that collection.
- **data:write:collection** — Write, update, or delete records in a specific collection. Example: `data:write:todos` grants write access. The plugin can call `data.create()`, `data.update()`, and `data.delete()` for that collection. Write implies read for the same collection.
- **sync:pull** — Request the shell to trigger a pull sync from Core for the plugin's declared collections. Used by plugins that need fresher data than the default sync interval provides.
- **sync:push** — Request the shell to immediately push pending local mutations to Core, rather than waiting for the next sync cycle.
- **ui:overlay** — Render floating UI outside the plugin's main container. Required for `ui.openModal()`. Without this capability, the plugin can only render inside its own container.
- **notify:local** — Show local OS-level notifications via the Tauri notification API. Used for reminders, alerts, and background event notifications.
- **ipc:send:target-plugin** — Send inter-plugin messages to a specific target plugin. Example: `ipc:send:com.life-engine.calendar` allows sending messages to the calendar plugin. Each target requires a separate capability declaration.
- **storage:local** — Read and write to plugin-private key-value storage. Required for `storage.get()`, `storage.set()`, and `storage.delete()`.
- **network:fetch** — Make outbound HTTP requests. Sandboxed to the domains listed in the manifest's `allowedDomains` array. Without this capability, the `http` namespace methods throw on every call.

## High-Trust Capabilities

Two capabilities are considered high-trust and receive special treatment during installation:

- **data:write** — Writing to collections can modify shared data that other plugins and Core depend on. The install dialog highlights this capability with a warning: "This plugin can modify your data in the following collections: [list]."
- **network:fetch** — Outbound network access could exfiltrate data. The install dialog highlights this capability with a warning: "This plugin can make network requests to the following domains: [list]."

High-trust capabilities are visually distinguished in the install approval dialog (e.g. yellow warning icon, bold text) so the user can make an informed decision.

## Runtime Enforcement

The shell wraps every ShellAPI method with a capability check. Enforcement happens synchronously at the start of each method call, before any work is performed.

- If the plugin has the required capability, the method executes normally.
- If the plugin does not have the required capability, the method throws a `CapabilityError` immediately. It does not return `undefined`, return an empty result, or silently fail.

The `CapabilityError` includes the plugin ID, the attempted operation, and the missing capability, making debugging straightforward:

```text
CapabilityError: Plugin "com.example.weather" attempted data.query("contacts")
  but does not have capability "data:read:contacts"
```

Enforcement is synchronous. There is no async overhead, no network call, and no promise resolution involved in the capability check itself. The shell holds the approved capability set in memory for each loaded plugin.

## Install-Time Approval Flow

When a plugin is installed for the first time, the shell presents a capability approval dialog:

1. The shell reads the plugin's declared capabilities from `plugin.json`.
2. The dialog lists each requested capability with a human-readable description.
3. High-trust capabilities are shown with warning indicators.
4. The user can approve all capabilities or reject the installation. There is no partial approval — the plugin either gets everything it declared or it does not install.
5. On approval, the shell records the approved capabilities in `settings.json` for this plugin. Future loads skip the approval dialog.
6. On rejection, the plugin is not installed and its files remain in the `plugins/` directory but are marked as unapproved.

If a plugin update adds new capabilities, the approval dialog is shown again for the new capabilities only.

## Scoping

Capabilities are scoped to prevent overly broad access:

- **Data capabilities** are scoped to specific collection names. `data:read:todos` does not grant access to `contacts`. A plugin must declare each collection separately.
- **Network fetch** is scoped to the `allowedDomains` array in the manifest. `network:fetch` alone is not sufficient — the target domain must also be listed.
- **IPC send** is scoped to specific target plugin IDs. `ipc:send:com.life-engine.calendar` does not grant access to send messages to any other plugin.

The shell enforces scoping at the same point as capability checks — synchronously, before the operation executes.

## Acceptance Criteria

- A plugin cannot access any protected resource (data, http, storage, ipc send, ui overlay, notifications) without the corresponding capability declared and approved.
- Attempting an undeclared operation throws a `CapabilityError` with a descriptive message, not a silent failure.
- High-trust capabilities (`data:write`, `network:fetch`) trigger a visible warning in the install approval dialog.
- Runtime capability checks are synchronous with no async overhead.
- The install-time approval flow shows all requested capabilities, blocks installation on rejection, and does not re-prompt on subsequent loads unless new capabilities are added.
- Data scoping, domain scoping, and IPC target scoping are enforced at runtime.
