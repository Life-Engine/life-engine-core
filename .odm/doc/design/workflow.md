---
title: Workflow Engine
type: reference
created: 2026-03-14
updated: 2026-03-28
status: active
tags:
  - life-engine
  - core
  - workflow
  - pipeline
---

# Workflow Engine

Part of [[architecture/core/overview|Core Overview]] · [[architecture/core/README|Core Documentation]]

The workflow engine is the central orchestration layer. Every request that involves plugin logic flows through a workflow — a declarative pipeline of plugin steps defined in YAML.

The workflow engine owns three trigger mechanisms (endpoints, events, schedules) and a pipeline executor. All trigger types are equivalent — the same pipeline runs regardless of how it was activated.

## Request Flow

```
Transport (REST, GraphQL, etc.)
      |
      v
+------------------+
|  Workflow Engine  |  1. Match trigger to workflow
|                   |  2. Execute plugin steps in sequence
|  Step 1 (Plugin)  |  3. Pass PipelineMessage between steps
|  Step 2 (Plugin)  |  4. Validate output at boundaries
|  Step 3 (Plugin)  |
+--------+---------+
         |
         v
   Response (back through transport)
```

## Key Concepts

- **Workflows are data** — YAML files loaded at startup, immutable at runtime
- **System workflows** — Core ships editable default workflows for generic CRUD (`collection.list`, `collection.get`, `collection.create`, `collection.update`, `collection.delete`, `graphql.query`, `system.health`). Both REST and GraphQL resolve through these same workflows.
- **Control flow** — Sequential execution, conditional branching, and per-step error handling (halt, retry, skip)
- **Sync/async modes** — Sync workflows block until complete; async workflows return a job ID immediately

## Detailed Design

- [[architecture/core/design/workflow-engine-layer/outline|Outline]] — Scope, design principles, component overview
- [[architecture/core/design/workflow-engine-layer/pipeline-executor|Pipeline Executor]] — Runtime execution model, concurrency, sync/async lifecycle
- [[architecture/core/design/workflow-engine-layer/trigger-system|Trigger System]] — How triggers activate workflows, resolution rules, registration
- [[architecture/core/design/workflow-engine-layer/event-bus|Event Bus]] — Event shape, naming, delivery model, loop prevention
- [[architecture/core/design/workflow-engine-layer/scheduler|Scheduler]] — Cron evaluation, missed ticks, overlap prevention
- [[architecture/core/design/workflow-engine-layer/control-flow|Control Flow]] — Conditional branching, error handling strategies, message passing
