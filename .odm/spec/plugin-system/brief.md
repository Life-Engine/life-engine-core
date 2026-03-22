<!--
domain: core
status: draft
tier: 1
updated: 2026-03-23
-->

# Core Plugin System Spec

## Overview

This spec defines how Core discovers, loads, manages, and isolates plugins. Core is a thin orchestrator — it scans a plugins directory, loads WASM modules at runtime via Extism, grants scoped capabilities through host functions, and enforces isolation. All features are provided by plugins. Core never compiles against any plugin.

This spec absorbs the previously separate plugin-loader spec. Plugin loading, discovery, and manifest parsing are all covered here.

## Goals

- Isolation — Every plugin runs in a WASM sandbox with no shared memory or direct I/O.
- Capability enforcement — All permissions are deny-by-default and scoped at runtime via host functions.
- Crash resilience — A failing plugin cannot crash Core; the WASM sandbox contains the failure.
- Language agnosticism — Any language that compiles to WASM can produce a plugin.
- Runtime loading — Plugins are discovered and loaded at runtime from a configured directory. No compiled-in Rust traits.
- Declarative manifests — Each plugin declares its identity, actions, capabilities, and config schema in a `manifest.toml`.

## User Stories

- As a system administrator, I want Core to scan a plugins directory at startup so that adding a plugin is as simple as dropping a folder into the directory.
- As a plugin author, I want a clear lifecycle (Discover, Load, Init, Running, Stop, Unload) so that I can manage resources predictably.
- As a platform operator, I want capability enforcement so that plugins cannot exceed their declared permissions.
- As a user, I want plugin failures to be contained so that one broken plugin does not take down the system.
- As a plugin author, I want to communicate with other plugins via workflow chaining and shared collections so that I can build integrations without direct plugin-to-plugin calls.
- As a community developer, I want to compile my plugin to WASM, drop it into the plugins directory, and have Core load it without forking the monorepo.

## Functional Requirements

- The system must load WASM plugin modules at runtime via the Extism runtime.
- The system must discover plugins by scanning a configured directory where each plugin is a folder containing `plugin.wasm` and `manifest.toml`.
- The system must parse `manifest.toml` for plugin identity (id, name, version), actions (with input/output schemas), required capabilities, and config schema.
- The system must enforce a six-phase plugin lifecycle: Discover, Load, Init, Running, Stop, Unload.
- The system must enforce capability scoping at runtime via host functions gated by the capability set.
- The system must auto-grant capabilities for first-party plugins and require explicit approval in config for third-party plugins.
- The system must refuse to load a plugin whose manifest declares an unapproved capability.
- The system must support plugin-to-plugin communication via workflow output-to-input chaining and shared canonical collections only. No direct plugin-to-plugin calls.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
