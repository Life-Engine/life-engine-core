<!--
domain: plugin-manifest
status: draft
tier: 1
updated: 2026-03-28
-->

# Plugin Manifest Spec

## Overview

Every plugin must ship a `manifest.toml` at the root of its plugin directory. The manifest declares the plugin's identity, actions, capabilities, collection access, events, configuration schema, and route bindings. Core validates the manifest at load time and rejects plugins with invalid or incomplete manifests.

The manifest is the single source of truth for what a plugin can do and what resources it requires. It drives capability enforcement (deny-by-default), collection provisioning, event wiring, and trust model decisions.

## Goals

- Single declarative file — one `manifest.toml` fully describes a plugin's identity, actions, capabilities, collections, events, and configuration
- Deny-by-default capabilities — only capabilities explicitly declared in the manifest and approved by trust rules are granted
- Strict validation at load time — Core rejects any plugin whose manifest is missing required fields, references unresolvable schemas, or contains unknown sections
- Trust-model awareness — first-party plugins auto-grant declared capabilities; third-party plugins require explicit approval in Core's configuration
- Consistent naming conventions — plugin IDs use kebab-case, event names use dot-separated format, extension fields use `ext.<plugin-id>.<field>` namespacing

## User Stories

- As a plugin author, I want to declare my plugin's identity in `manifest.toml` so that Core can uniquely identify and display it.
- As a plugin author, I want to declare actions with optional timeouts so that workflows can invoke my plugin's entry points with bounded execution time.
- As a plugin author, I want to declare capabilities so that Core grants only the host functions my plugin needs.
- As a plugin author, I want to declare collections with schema references and access levels so that Core provisions storage and enforces access control.
- As a plugin author, I want to declare events I emit and subscribe to so that the event bus and trigger system wire my plugin correctly.
- As a plugin author, I want to declare a configuration schema so that Core validates my plugin's runtime config at load time.
- As a Core maintainer, I want unknown manifest sections to be rejected so that typos and unsupported fields do not silently pass validation.
- As a user, I want third-party plugin capabilities to require explicit approval so that untrusted plugins cannot access resources without my consent.

## Functional Requirements

- The manifest must contain a `[plugin]` section with required fields: `id`, `name`, `version`.
- The manifest must declare at least one action in `[actions.<name>]`.
- Capabilities in `[capabilities]` follow deny-by-default — omitted capabilities are denied.
- Collections in `[collections.<name>]` must declare `schema` and `access`.
- Events in `[events.emit]` and `[events.subscribe]` must follow the dot-separated naming convention.
- Configuration schema in `[config]` must resolve to a valid JSON Schema file.
- First-party plugins auto-grant all declared capabilities; third-party plugins require explicit approval.
- Core must reject manifests with unknown top-level sections.
- Core must log specific validation errors when a manifest fails to load.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
