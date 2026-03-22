<!--
domain: capability-enforcement
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Requirements Document — Capability Enforcement

## Introduction

The capability enforcement system is the security boundary between plugins and shell resources. It implements a deny-by-default model where plugins must declare capabilities in their manifest and receive user approval at install time. At runtime, every ShellAPI method call is gated by a synchronous capability check. This document specifies the enforcement logic, install-time approval flow, scoping rules, and error handling for the capability system.

## Alignment with Product Vision

- **Security by default** — Deny-by-default means zero access without explicit declaration and user approval. No implicit grants or escalation paths.
- **User trust** — The install-time approval dialog gives users visibility into what a plugin can do before granting access.
- **Developer clarity** — Descriptive `CapabilityError` messages make debugging permission issues straightforward.
- **Plugin isolation** — Scoped capabilities prevent plugins from accessing data, network, or IPC targets beyond their declared needs.

## Requirements

### Requirement 1 — Runtime Capability Checks

**User Story:** As a shell developer, I want every ShellAPI method gated by a synchronous capability check, so that no plugin can access protected resources without approval.

#### Acceptance Criteria

- 1.1. WHEN a plugin calls any ShellAPI method that requires a capability THEN the shell SHALL check the plugin's approved capability set before executing any logic.
- 1.2. WHEN the plugin has the required capability THEN the method SHALL execute normally and return the expected result.
- 1.3. WHEN the plugin does not have the required capability THEN the method SHALL throw a `CapabilityError` immediately without performing any work.
- 1.4. WHEN a `CapabilityError` is thrown THEN it SHALL include the plugin ID, the attempted operation (e.g., `data.query("contacts")`), and the missing capability string (e.g., `data:read:contacts`).
- 1.5. WHEN a capability check executes THEN it SHALL be synchronous with no async overhead, network call, or promise resolution.

---

### Requirement 2 — Install-Time Approval

**User Story:** As a user, I want to review all capabilities a plugin requests before installing it, so that I can make an informed trust decision.

#### Acceptance Criteria

- 2.1. WHEN a plugin is installed for the first time THEN the shell SHALL read its declared capabilities from `plugin.json` and present an approval dialog.
- 2.2. WHEN the approval dialog is shown THEN it SHALL list each capability with a human-readable description.
- 2.3. WHEN the user approves THEN the shell SHALL record the approved capabilities in `settings.json` and proceed with loading the plugin.
- 2.4. WHEN the user rejects THEN the shell SHALL not load the plugin and mark it as unapproved in the plugins directory.
- 2.5. WHEN a previously approved plugin is loaded on subsequent startups THEN the shell SHALL skip the approval dialog and use the stored capabilities.
- 2.6. WHEN a plugin update introduces new capabilities not previously approved THEN the shell SHALL show the approval dialog for the new capabilities only.

---

### Requirement 3 — Data Capability Scoping

**User Story:** As a security-conscious user, I want data capabilities scoped to specific collections, so that a plugin with access to my tasks cannot read my contacts.

#### Acceptance Criteria

- 3.1. WHEN a plugin declares `data:read:todos` THEN the shell SHALL allow `data.query()` and `data.subscribe()` for the `todos` collection only.
- 3.2. WHEN a plugin declares `data:write:todos` THEN the shell SHALL allow `data.create()`, `data.update()`, and `data.delete()` for the `todos` collection, and SHALL implicitly grant read access to the same collection.
- 3.3. WHEN a plugin calls `data.query("contacts")` without `data:read:contacts` THEN the shell SHALL throw a `CapabilityError`.
- 3.4. WHEN a plugin declares multiple data capabilities THEN each SHALL be enforced independently per collection.

---

### Requirement 4 — Network Capability Scoping

**User Story:** As a user, I want network access restricted to declared domains, so that a plugin cannot exfiltrate data to arbitrary servers.

#### Acceptance Criteria

- 4.1. WHEN a plugin declares `network:fetch` and lists domains in `allowedDomains` THEN the shell SHALL allow HTTP requests only to those domains.
- 4.2. WHEN a plugin attempts to fetch a URL whose domain is not in `allowedDomains` THEN the shell SHALL throw a `CapabilityError`.
- 4.3. WHEN a plugin does not declare `network:fetch` THEN all `http` namespace methods SHALL throw a `CapabilityError`.

---

### Requirement 5 — IPC Capability Scoping

**User Story:** As a plugin author, I want IPC messaging scoped to declared targets, so that plugins cannot send unsolicited messages to arbitrary plugins.

#### Acceptance Criteria

- 5.1. WHEN a plugin declares `ipc:send:com.life-engine.calendar` THEN the shell SHALL allow sending messages to the calendar plugin only.
- 5.2. WHEN a plugin attempts to send a message to a plugin not declared in its `ipc:send` capabilities THEN the shell SHALL throw a `CapabilityError`.

---

### Requirement 6 — High-Trust Capability Warnings

**User Story:** As a user, I want high-trust capabilities visually highlighted during installation, so that I notice when a plugin requests write or network access.

#### Acceptance Criteria

- 6.1. WHEN the install dialog lists a `data:write` capability THEN the dialog SHALL display a warning indicator (e.g., yellow icon) with the text: "This plugin can modify your data in the following collections: [list]."
- 6.2. WHEN the install dialog lists a `network:fetch` capability THEN the dialog SHALL display a warning indicator with the text: "This plugin can make network requests to the following domains: [list]."
- 6.3. WHEN no high-trust capabilities are requested THEN the dialog SHALL not display any warning indicators.

---

### Requirement 7 — CapabilityError Structure

**User Story:** As a developer, I want capability errors to include enough context for immediate diagnosis, so that I do not need to guess which capability is missing.

#### Acceptance Criteria

- 7.1. WHEN a `CapabilityError` is constructed THEN it SHALL include the fields: `pluginId`, `operation`, and `missingCapability`.
- 7.2. WHEN a `CapabilityError` is displayed THEN the message SHALL follow the format: `CapabilityError: Plugin "{pluginId}" attempted {operation} but does not have capability "{missingCapability}"`.
- 7.3. WHEN a `CapabilityError` is thrown THEN it SHALL NOT return `undefined`, an empty result, or silently fail — it SHALL always be an explicit throw/error.
