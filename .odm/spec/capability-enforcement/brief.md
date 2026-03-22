<!--
domain: capability-enforcement
status: draft
tier: 1
updated: 2026-03-23
-->

# Capability Enforcement Spec

## Overview

This spec defines the capability enforcement system that governs plugin access to Core host functions. Capabilities are declared in the plugin's `manifest.toml`, checked at plugin load time against an approval policy, and enforced at the WASM boundary when host functions are invoked. The system follows a deny-by-default model: plugins receive no access to any host function unless they declare the capability and it is approved.

## Goals

- Enforce deny-by-default access control for all plugin host function calls
- Gate host function injection into the WASM runtime by the plugin's approved capability set
- Return a fatal `EngineError` when a plugin invokes a host function it was not granted
- Support config-based approval for third-party plugins with no install dialog or UI
- Auto-grant capabilities for first-party plugins shipped in the monorepo
- Refuse to load any plugin whose manifest declares capabilities not in its approved set

## User Stories

- As a system administrator, I want third-party plugin capabilities controlled via `config.toml` so that approval is explicit and auditable.
- As a plugin author, I want to declare capabilities in `manifest.toml` so that Core grants me access to the host functions I need.
- As a platform operator, I want unapproved capabilities to prevent plugin loading so that no plugin can silently exceed its permissions.
- As a developer, I want capability violations to return a clear `EngineError` with the plugin ID and missing capability so that I can debug permission issues quickly.

## Functional Requirements

- The system must check declared capabilities against the approval policy at plugin load time, before the WASM module is initialized.
- The system must inject only approved host functions into the WASM runtime for each plugin.
- The system must return a fatal `EngineError` when a plugin calls a host function it was not granted.
- The system must auto-grant all capabilities for first-party plugins (those in the monorepo `plugins/` directory).
- The system must require explicit `approved_capabilities` in `config.toml` for third-party plugins.
- The system must refuse to load a plugin if its manifest declares any capability not present in the approved set.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
