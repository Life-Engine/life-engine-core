<!--
domain: app
status: draft
tier: 1
updated: 2026-03-22
-->

# Shell Data API Spec

## Overview

This spec defines the complete ShellAPI interface injected into every plugin. It covers all seven namespaces (data, http, storage, settings, ui, ipc, plugin), scoping rules, and the TypeScript type definitions that the plugin SDK exports.

## Goals

- Complete API surface — Plugins access all platform services through a single injected object.
- Scoping enforcement — Every operation is checked against the plugin's declared capabilities at call time.
- Local-first data — Data operations run against local SQLite with sync invisible to the plugin.
- Framework agnosticism — The API is injected as a plain object on the custom element, usable from any framework.

## User Stories

- As a plugin author, I want to query and write data via `this.__shellAPI.data` so that I can manage records without direct database access.
- As a plugin author, I want to make HTTP requests via `this.__shellAPI.http` so that I can call external APIs within my allowed domains.
- As a plugin author, I want private key-value storage via `this.__shellAPI.storage` so that I can persist plugin-specific state.
- As a plugin author, I want to show toasts and navigate via `this.__shellAPI.ui` so that I can provide feedback and move between views.
- As a plugin author, I want to send messages to other plugins via `this.__shellAPI.ipc` so that I can build cross-plugin integrations.

## Functional Requirements

- The shell must inject a scoped `ShellAPI` object onto the plugin's custom element before `connectedCallback` fires.
- The data namespace must support query, create, update, delete, and subscribe operations against local SQLite.
- The http namespace must proxy all requests through the shell with domain-level scoping enforcement.
- The storage namespace must provide plugin-private key-value get, set, and delete.
- The settings namespace must provide persistent get, set, and subscribe without requiring a capability.
- The ui namespace must provide navigate, back, toast, openModal, closeModal, and setTitle methods.
- The ipc namespace must provide send (capability-gated) and on (open to all plugins) methods.
- The plugin namespace must expose read-only id and version metadata.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
