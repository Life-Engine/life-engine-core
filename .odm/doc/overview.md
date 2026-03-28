---
title: Core Overview
type: reference
created: 2026-03-27
status: active
tags:
  - life-engine
  - core
  - overview
---

# Core Overview

Core is the self-hosted backend at the centre of Life Engine. It aggregates personal data from external services, stores it locally with encryption, and exposes it through configurable protocol endpoints. Core contains no business logic of its own — every feature is provided by plugins, and Core's job is to wire them together securely.

## What Core Does

Core solves one problem: your personal data is scattered across dozens of external services, each with its own API, its own format, and its own terms. Core pulls that data in, normalises it into a shared set of types, and gives you a single API to access all of it — on your hardware, under your control.

From a user's perspective, Core is invisible. It runs in the background on a home server, a Raspberry Pi, or a Docker container. The [[architecture/client/README|Client]] app connects to it for data and sync. Native calendar and contacts apps can connect directly via standard protocols.

## Four-Layer Architecture

Core is organised into four independent layers. Each layer has a single responsibility, and they communicate through well-defined contracts.

- **Transport** — Protocol-specific entry points that receive requests from the outside world. REST, GraphQL, CalDAV, CardDAV, and webhooks are all transports. Each one authenticates incoming requests, translates them into a standard internal message, and hands them to the workflow engine. The admin chooses which transports are active — Core starts only those.

- **Workflow Engine** — The orchestration layer. Workflows are declarative pipelines that chain plugin steps together. When a request arrives from a transport, a scheduled timer fires, or an internal event is emitted, the workflow engine finds the matching pipeline and runs its steps in sequence. It owns the event bus and cron scheduler.

- **Plugins** — All logic lives here. Plugins are sandboxed modules loaded at runtime. A connector plugin fetches emails from IMAP; a search plugin indexes content; a transform plugin normalises dates. Each plugin declares the capabilities it needs, and Core grants or denies them. Plugins communicate only through workflows and shared data — never directly with each other.

- **Data** — Persistent storage behind an abstract interface. Plugins interact with storage through a query builder, never through direct database access. The current implementation uses SQLite with full-database encryption. The storage layer is swappable without changing any plugin code.

These layers form a clean pipeline: a request enters through a transport, flows through a workflow of plugin steps, and reads or writes data through the storage layer. Each layer can be understood, tested, and replaced independently.

## The Plugin Model

Core's defining characteristic is that it ships with no built-in features beyond orchestration. Every capability — email sync, calendar access, search indexing, data transformation — is provided by a plugin.

Plugins are isolated through WebAssembly sandboxing. A plugin cannot access the filesystem, network, or other plugins directly. It declares the capabilities it needs in a manifest, and Core enforces those grants at runtime. A plugin that requests network access but is only approved for storage access will be refused at load time.

This model means adding a feature to Core never requires changing Core itself. Install a plugin, approve its capabilities, and define a workflow that uses it.

## Canonical Data Model

Core defines seven shared data types — events, tasks, contacts, notes, emails, files, and credentials — that form the common language of the platform. Every connector normalises external data into these types, and every plugin reads and writes through them.

This shared model is what makes plugins composable. A calendar connector writes events; a notification plugin reads events; a search plugin indexes events. None of them need to know about each other — they all speak the same data language.

Plugins can also define private data types for internal use, and can extend canonical types with plugin-specific fields through a namespaced extension mechanism that prevents collisions.

## Security Model

Security is structural, not bolted on.

- **Encrypted at rest** — The database is fully encrypted. The encryption key is derived from a user-provided passphrase and never stored.
- **Deny by default** — Plugins receive no capabilities unless explicitly granted. All grants are enforced at runtime.
- **Sandboxed execution** — Plugins run in WebAssembly isolation. A compromised plugin cannot access the host system.
- **Auth on every request** — Every transport enforces authentication before routing to the workflow engine.
- **Localhost by default** — Core binds to localhost only. External access requires explicit configuration and enables mandatory TLS and rate limiting.

## How Core Fits Into Life Engine

Core is one half of the platform. The [[architecture/client/README|Client]] is the other — a native desktop app that provides the user interface. The Client maintains its own local database for instant reads and writes, and syncs with Core in the background.

Core can also serve native calendar and contacts apps directly through its CalDAV and CardDAV transports, and any tool that speaks REST or GraphQL can connect to it.

For the governing principles behind Core's design, see [[design-principles]].

