---
title: Workflow Engine Layer Outline
type: adr
created: 2026-03-28
status: draft
tags:
  - architecture
  - workflow-engine
  - core
---

# Workflow Engine Layer Outline

## Scope (v1)

The workflow engine layer handles orchestration of plugin pipelines. It owns three trigger mechanisms and a pipeline executor:

- **Pipeline executor** — Runs workflow steps in sequence, passing `PipelineMessage` between them
- **Event bus** — In-memory pub/sub for plugin and system events
- **Scheduler** — Cron-based workflow activation

The following are deferred to future considerations:

- Parallel step execution (fan-out/fan-in)
- Persistent event queue
- Workflow-level timeouts
- Validation at step boundaries
- Wildcard event matching

## Request Flow

```
TriggerContext (from transport, event bus, or scheduler)
  ↓
Executor: resolve workflow ID → WorkflowDefinition
  ↓
Executor: build initial PipelineMessage from TriggerContext
  ↓
Executor: clone message, run Step 1 (plugin action)
  ↓
Executor: pass output PipelineMessage to Step 2
  ↓
  ... (sequential steps, conditional branches, error handling)
  ↓
Executor: build WorkflowResponse from final PipelineMessage
  ↓
Return to caller (transport handler, or JobRegistry for async)
```

## Design Principles

- **One path** — Every trigger type (endpoint, event, schedule) activates the same pipeline executor. No special cases per trigger.
- **Protocol-agnostic** — The workflow engine never thinks about HTTP, GraphQL, or any wire format. It receives a `TriggerContext` and returns a `WorkflowResponse`.
- **Workflows are data** — Workflow definitions are YAML files, loaded once at startup, immutable at runtime. System workflows are real editable files, not hardcoded behaviour.
- **Plugins are black boxes** — The executor calls plugin actions and receives output. It does not inspect plugin internals. The `PipelineMessage` contract is the only interface.
- **Fail predictably** — Error handling is per-step and declarative. Loop prevention, concurrency limits, and overlap detection are built into the engine with sensible defaults.

## Components

The workflow engine layer comprises six components, each documented separately:

- [[pipeline-executor]] — Runtime execution model, concurrency, sync/async lifecycle
- [[trigger-system]] — How triggers activate workflows, resolution rules, registration
- [[event-bus]] — Event shape, naming, delivery model, loop prevention
- [[scheduler]] — Cron evaluation, missed ticks, overlap prevention
- [[control-flow]] — Conditional branching, error handling strategies, message passing

## What the Workflow Engine Does Not Own

- **Transport concerns** — Protocol translation, auth, TLS, CORS. These belong to the [[architecture/core/design/transport-layer/outline|transport layer]].
- **Plugin execution internals** — WASM sandboxing, capability enforcement, host functions. These belong to the [[architecture/core/design/plugins|plugin system]].
- **Storage** — The workflow engine does not read or write storage directly. Plugins access storage through their `StorageContext`.
- **Admin panel** — Top-level Core feature with its own config and auth.
