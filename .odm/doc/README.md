---
title: Core Documentation
type: reference
created: 2026-03-14
updated: 2026-03-27
status: active
tags:
  - life-engine
  - core
  - architecture
---

# Core Documentation

All documentation for the Core backend service, organised by depth.

For a conceptual introduction to what Core is, how it works, and how it fits into Life Engine, start with the overview.

## Concept Overview (Layer 1)

- [[architecture/core/overview|Core Overview]] — What Core is, its four-layer architecture, plugin model, data model, and security posture. No code, no implementation detail.

## Design Documents (Layer 2)

How Core's subsystems work at an architectural level — data flows, boundaries, and design patterns.

- [[architecture/core/design/transport-layer/outline|Transports]] — Transport layer design: available transports, configuration, auth, middleware stack, and how transports connect to workflows
- [[architecture/core/design/workflow|Workflow]] — Workflow engine: declarative pipelines, triggers, execution modes, data flow between steps, control flow, validation, event bus, and scheduler
- [[architecture/core/design/plugins|Plugins]] — Plugin system: WASM isolation, plugin trait, discovery, lifecycle, capabilities, host functions, SDK contract, and community plugins
- [[architecture/core/design/data|Data]] — Data layer: storage abstraction, query builder, document model, collection tiers, schema validation, encryption at rest, and data export
