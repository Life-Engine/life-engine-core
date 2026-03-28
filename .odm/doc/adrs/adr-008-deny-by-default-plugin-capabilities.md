---
title: "ADR-008: Deny-by-Default Plugin Capability Model"
type: adr
created: 2026-03-28
status: active
---

# ADR-008: Deny-by-Default Plugin Capability Model

## Status

Accepted

## Context

Life Engine runs third-party WASM plugins that handle sensitive personal data — emails, contacts, credentials, calendar events. The plugin system must enforce strict access control so that a plugin cannot read data it does not need, make network requests without authorisation, or escalate its privileges at runtime.

Mobile operating systems (Android, iOS) and browser extensions use a capability model where applications declare the permissions they need and the platform grants or denies them. This model has proven effective at limiting the blast radius of compromised or malicious code.

The alternative — granting all plugins full access and relying on code review — does not scale to a third-party plugin ecosystem where the platform cannot audit every plugin.

## Decision

Plugin capabilities are deny-by-default. Every capability a plugin requires must be explicitly declared in its `manifest.toml`. Core reads the manifest at startup and grants or denies each capability. Undeclared capabilities are rejected at runtime — the host function returns an error and the operation is not performed.

The v1 capability set is:

- `storage:doc:read`, `storage:doc:write`, `storage:doc:delete` — Document storage access
- `storage:blob:read`, `storage:blob:write`, `storage:blob:delete` — Blob storage access
- `http:outbound` — Outbound HTTP requests
- `events:emit`, `events:subscribe` — Event bus access
- `config:read` — Read own configuration section

First-party plugins (those shipped with Core) have capabilities auto-granted. Third-party plugins require explicit approval in Core's configuration file. This makes the default posture secure while allowing self-hosters to grant trust when they choose to install a plugin.

The `StorageContext` API enforces capability checks at every storage operation. The workflow engine enforces event bus capabilities. The HTTP host function enforces outbound access. There is no layer where a plugin can bypass capability checks.

## Consequences

Positive consequences:

- A compromised or malicious plugin can only access what it declared and the user approved. The blast radius is bounded by the manifest.
- Users can inspect a plugin's manifest before installation to understand exactly what data it accesses and what external connections it makes.
- The capability model is enforced at the host function boundary — inside the WASM sandbox, the plugin has no mechanism to bypass it. This is a structural guarantee, not a code-review-dependent one.
- Adding new capabilities in future versions (e.g., `system:exec`, `storage:watch`) requires only extending the enum and adding the enforcement check.

Negative consequences:

- V1 capabilities are coarse-grained. `storage:doc:read` grants read access to all collections the plugin has declared, not per-collection. Fine-grained per-collection ACLs are deferred.
- V1 authentication is all-or-nothing (authenticated = authorised). There are no per-user or per-role capability overrides. A plugin's capabilities are the same regardless of which user triggered the workflow.
- Plugin authors must understand the capability model and declare all required capabilities upfront. Missing a capability declaration results in a runtime error, not a compile-time error — the feedback loop is longer than ideal.
- First-party auto-grant creates a trust asymmetry. If a first-party plugin is compromised (e.g., supply chain attack on a dependency), it has full access without user approval.
