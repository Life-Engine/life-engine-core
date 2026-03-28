---
title: Trigger System
type: adr
created: 2026-03-28
status: draft
tags:
  - architecture
  - workflow-engine
  - triggers
  - core
---

# Trigger System

## Overview

The trigger system is the entry point to the workflow engine. It resolves incoming signals — HTTP requests, events, cron ticks — into workflow executions. All three trigger types are unified: they all produce a `TriggerContext` and call the same [[pipeline-executor]].

## Trigger Types

A workflow can declare one or more triggers in its YAML definition. All trigger types are equivalent — the same pipeline runs regardless of how it was activated.

```yaml
trigger:
  endpoint: "POST /email/sync"
  event: "webhook.email.received"
  schedule: "*/5 * * * *"
```

A workflow can declare all three, any combination, or just one.

### Endpoint Triggers

An HTTP route in the transport layer maps to a workflow by name. When a request matches the route, the transport handler builds a `WorkflowRequest` and wraps it in `TriggerContext::Endpoint`.

The router config in the [[listener|listener configuration]] is the source of truth for endpoint triggers. The `trigger.endpoint` field in the workflow YAML is validated against the router at startup — if the declared route doesn't exist in the router config, Core logs a warning. But the route config is what actually wires the request to the workflow. This avoids two competing route registries.

An HTTP route maps to exactly one workflow. Duplicate endpoint triggers (two workflows claiming the same route) are a startup error.

### Event Triggers

The [[event-bus]] broadcasts events to all workflows with a matching `trigger.event`. Unlike endpoint triggers, one event can activate multiple workflows — this is the natural broadcast model of the event bus.

All matching workflows fire independently and concurrently. Each gets its own copy of the trigger payload. No ordering guarantee. If one fails, the others are unaffected.

### Schedule Triggers

The [[scheduler]] evaluates cron expressions and fires matching workflows at the specified interval. Schedule triggers produce `TriggerContext::Schedule` with only the workflow ID — there is no input data.

## Registration

All triggers are registered once at startup by scanning loaded workflow definitions. The trigger registry is immutable at runtime — adding or changing a workflow requires restarting Core. This is consistent with the router, plugin loading, and workflow definition loading.

At startup, the trigger system:

1. Scans all loaded `WorkflowDefinition` entries
2. Registers endpoint triggers — validates against the router config, rejects duplicates
3. Registers event triggers — builds a map of event name → list of workflow IDs
4. Registers schedule triggers — passes cron expressions to the scheduler

## Resolution

When a signal arrives, the trigger system resolves it to one or more workflow executions:

- **Endpoint** — The router resolves the route to a single workflow ID. One-to-one.
- **Event** — The event bus looks up the event name in the trigger map. One-to-many. All matching workflows are spawned.
- **Schedule** — The scheduler fires the specific workflow associated with the cron expression. One-to-one per schedule entry.

## Startup Validation

The trigger system validates the following at startup:

- No two workflows declare the same endpoint trigger
- Every endpoint trigger references a route that exists in the router config
- Every schedule trigger has a valid cron expression
- Event trigger names follow dot-separated naming convention

Validation failures produce clear error messages and prevent startup.
