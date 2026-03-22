<!--
domain: core
status: draft
tier: 1
updated: 2026-03-22
-->

# Core Plugin System Spec

## Overview

This spec defines how Core loads, manages, and isolates plugins. Core itself is an empty orchestrator — it loads plugins, gives them scoped access to storage and the API layer, and enforces isolation. All features are provided by plugins.

## Goals

- Isolation — Every plugin runs in a WASM sandbox with no shared memory or direct I/O.
- Capability enforcement — All permissions are deny-by-default and scoped at runtime.
- Crash resilience — A failing plugin cannot crash Core; the sandbox contains the failure.
- Language agnosticism — Any language that compiles to WASM can produce a plugin.

## User Stories

- As a system administrator, I want to configure which plugins are loaded so that only approved plugins run.
- As a plugin author, I want a clear lifecycle (load, init, run, stop, unload) so that I can manage resources predictably.
- As a platform operator, I want capability enforcement so that plugins cannot exceed their declared permissions.
- As a user, I want plugin failures to be contained so that one broken plugin does not take down the system.
- As a plugin author, I want to communicate with other plugins via shared collections and events so that I can build integrations.

## Functional Requirements

- The system must load WASM plugin modules via the Extism runtime.
- The system must enforce a six-phase plugin lifecycle: Discover, Load, Init, Running, Stop, Unload.
- The system must enforce capability scoping at runtime for storage, HTTP, credentials, events, config, and logging.
- The system must export host functions for storage, credentials, config, events, logging, and HTTP.
- The system must discover plugins from configured YAML paths with explicit enable/disable.
- The system must support plugin-to-plugin communication via shared canonical collections and Core events.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
