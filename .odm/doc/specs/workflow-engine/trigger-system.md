---
title: Trigger System Specification
type: reference
created: 2026-03-28
status: active
tags:
  - spec
  - trigger
  - workflow
---

# Trigger System Specification

The trigger system connects external stimuli (HTTP requests, events, cron schedules) to workflow execution. All trigger types produce a [[pipeline-executor#TriggerContext|TriggerContext]] and invoke the same [[pipeline-executor|pipeline executor]].

## Trigger Types

Workflows declare triggers in their YAML definition. A single workflow may declare one or more trigger types:

```yaml
trigger:
  endpoint: "POST /email/sync"
  event: "webhook.email.received"
  schedule: "*/5 * * * *"
```

## Endpoint Triggers

- Each HTTP route maps to a workflow by name.
- The route configuration in the listener is the source of truth for routing.
- The `trigger.endpoint` value in the workflow YAML is validated against the router at startup. A mismatch produces a warning log.
- One route must map to exactly one workflow. Duplicate endpoint triggers cause a startup error.

## Event Triggers

- The [[event-bus|event bus]] broadcasts events to all workflows whose `trigger.event` matches the event name.
- One event can activate multiple workflows (broadcast model).
- Each matching workflow fires independently and concurrently, receiving its own copy of the payload.
- There is no ordering guarantee between workflows triggered by the same event.
- A failure in one workflow does not affect others triggered by the same event.

## Schedule Triggers

- The [[scheduler]] evaluates cron expressions and fires matching workflows at the scheduled time.
- Schedule triggers produce a `TriggerContext::Schedule` containing only the workflow ID.

## Registration

All triggers are registered once at startup by scanning workflow definitions. The trigger registry is immutable at runtime.

## Startup Validation

The following checks must pass at startup:

- No duplicate endpoint triggers across all workflows.
- Every endpoint trigger references an existing route in the listener configuration.
- All cron expressions are syntactically valid.
- Event names follow the dot-separated naming convention (see [[event-bus#Event Naming]]).
