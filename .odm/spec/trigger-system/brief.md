<!--
domain: trigger-system
updated: 2026-03-28
-->

# Trigger System Spec

## Overview

The trigger system is the entry point to the workflow engine. It connects external stimuli — HTTP requests, event bus broadcasts, and cron schedules — to workflow execution. All three trigger types are unified: each produces a `TriggerContext` and invokes the same pipeline executor.

Workflows declare triggers in their YAML definition. A single workflow may declare one or more trigger types (endpoint, event, schedule). All triggers are registered once at startup by scanning workflow definitions. The trigger registry is immutable at runtime.

## Goals

- Provide a single registration and resolution mechanism for all trigger types (endpoint, event, schedule)
- Validate all trigger declarations at startup so that misconfiguration is caught before the engine accepts traffic
- Enforce one-to-one mapping for endpoint triggers (one route maps to exactly one workflow)
- Support one-to-many mapping for event triggers (one event can activate multiple workflows concurrently)
- Produce a uniform `TriggerContext` regardless of trigger type so that the pipeline executor has a single entry point
- Keep the trigger registry immutable at runtime for consistency with router, plugin, and workflow definition loading

## User Stories

- As a workflow author, I want to declare an endpoint trigger in my workflow YAML so that an HTTP request activates my workflow.
- As a workflow author, I want to declare an event trigger so that my workflow fires whenever a matching event is broadcast on the event bus.
- As a workflow author, I want to declare a schedule trigger with a cron expression so that my workflow runs on a recurring schedule.
- As a workflow author, I want to declare multiple trigger types on a single workflow so that the same pipeline can be activated by different stimuli.
- As an operator, I want startup validation to reject duplicate endpoint triggers so that routing ambiguity is caught before the engine serves traffic.
- As an operator, I want startup validation to warn when a workflow's endpoint trigger does not match a route in the listener configuration.
- As a Core developer, I want the trigger registry to be immutable at runtime so that trigger resolution is lock-free and predictable.

## Functional Requirements Summary

- The system must support three trigger types: endpoint, event, and schedule.
- A workflow may declare any combination of trigger types in its YAML definition.
- Endpoint triggers must map one-to-one: no two workflows may claim the same route.
- Event triggers must support one-to-many: one event name may activate multiple workflows concurrently.
- Schedule triggers must accept valid cron expressions and delegate to the scheduler.
- All triggers must be registered at startup by scanning loaded workflow definitions.
- The trigger registry must be immutable at runtime.
- Startup validation must reject duplicate endpoint triggers, invalid cron expressions, and malformed event names.
- Startup validation must warn (not reject) when an endpoint trigger references a route not present in the listener configuration.
- Each trigger resolution must produce a `TriggerContext` variant (`Endpoint`, `Event`, or `Schedule`) for the pipeline executor.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
