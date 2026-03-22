<!--
domain: app
status: draft
tier: 1
updated: 2026-03-22
-->

# Plugin Loader Spec

## Overview

Defines the 11-step plugin loading lifecycle, manifest validation rules, shared module hosting, plugin unloading behaviour, and sidebar registration. This is the implementor contract for all code that loads, validates, mounts, and removes plugins in the App shell.

## Goals

- **Reliable loading** — Guarantee that every valid plugin loads through a deterministic 11-step lifecycle with clear failure points.
- **Security by default** — Validate manifests and require user approval of capabilities before any plugin code executes.
- **Efficient resource sharing** — Host shared modules (Lit, React) once and distribute them to all plugins via import maps.
- **Clean teardown** — Ensure plugin unloading removes all DOM elements, subscriptions, and sidebar entries without leaks.
- **Extensible sidebar** — Allow plugins to declare sidebar presence with configurable icons, labels, and sort order.

## User Stories

- As a user, I want to see a capability approval dialog before a plugin loads so that I understand what permissions it requires.
- As a user, I want invalid plugins to show clear error messages so that I know why a plugin failed to load.
- As a plugin author, I want shared modules loaded by the shell so that my bundle stays small.
- As a user, I want deactivated plugins fully cleaned up so that they do not consume memory or show stale UI.
- As a plugin author, I want my plugin to appear in the sidebar with a custom icon and label so that users can navigate to it.

## Functional Requirements

- The system must validate plugin manifests for required fields, element name format, version compatibility, and bundle size.
- The system must execute all 11 lifecycle steps in order, aborting on failure at any step.
- The system must present a capability approval dialog with warnings for high-trust capabilities before first load.
- The system must register import maps for shared modules declared in the manifest.
- The system must dynamically import the plugin entry and wait up to 10 seconds for custom element definition.
- The system must create a scoped ShellAPI instance locked to the plugin's approved capabilities.
- The system must remove DOM elements, clean up subscriptions, and remove sidebar items on unload.
- The system must sort sidebar items by order value ascending, then alphabetically by label.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
