<!--
domain: trigger-system
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Trigger System — Tasks

**Progress:** 0 / 12 tasks complete

## 1.1 — Types and Data Structures

- [ ] Define `TriggerDeclaration` struct in `packages/types`
  <!-- files: packages/types/src/workflow.rs -->
  <!-- purpose: Add the TriggerDeclaration struct with optional endpoint, event, and schedule fields, and integrate it into WorkflowDefinition -->
  <!-- requirements: 1.1, 1.2, 1.3, 1.4, 1.5 -->

- [ ] Define `TriggerContext` enum in `packages/types`
  <!-- files: packages/types/src/trigger.rs -->
  <!-- purpose: Add the TriggerContext enum with Endpoint, Event, and Schedule variants -->
  <!-- requirements: 3.2, 4.6, 5.2 -->

- [ ] Define startup error and warning types
  <!-- files: packages/workflow-engine/src/triggers/error.rs -->
  <!-- purpose: Add StartupError enum (DuplicateEndpoint, InvalidEventName, InvalidCronExpression) and StartupWarning enum (UnmatchedEndpoint) with Display implementations -->
  <!-- requirements: 6.1, 7.1, 8.1 -->

## 1.2 — Validation Functions

- [ ] Implement event name validation
  <!-- files: packages/workflow-engine/src/triggers/validation.rs -->
  <!-- purpose: Add is_valid_event_name function enforcing dot-separated lowercase segments with unit tests -->
  <!-- requirements: 8.1, 8.2 -->

- [ ] Implement cron expression validation
  <!-- files: packages/workflow-engine/src/triggers/validation.rs -->
  <!-- purpose: Add is_valid_cron function using the cron crate parser with unit tests -->
  <!-- requirements: 7.1, 7.2 -->

## 1.3 — Trigger Registry

- [ ] Implement `TriggerRegistry` struct with lookup methods
  <!-- files: packages/workflow-engine/src/triggers/registry.rs -->
  <!-- purpose: Add TriggerRegistry with endpoints HashMap, events HashMap, schedules Vec, and resolve_endpoint/resolve_event methods -->
  <!-- requirements: 2.2, 2.3, 2.5, 3.1, 4.1 -->

- [ ] Implement `TriggerRegistrar` with startup registration logic
  <!-- files: packages/workflow-engine/src/triggers/registrar.rs -->
  <!-- purpose: Add TriggerRegistrar that scans workflow definitions, registers all trigger types, collects errors, and produces an immutable TriggerRegistry -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 6.1, 6.2 -->

## 1.4 — Integration with Transport Layer

- [ ] Wire endpoint trigger resolution into the HTTP handler
  <!-- files: packages/workflow-engine/src/triggers/endpoint.rs, packages/transport/src/handler.rs -->
  <!-- purpose: Call registry.resolve_endpoint() on incoming HTTP requests, build TriggerContext::Endpoint from WorkflowRequest, and pass to pipeline executor -->
  <!-- requirements: 3.1, 3.2, 3.3, 3.4 -->

## 1.5 — Integration with Event Bus

- [ ] Wire event trigger resolution into the event bus subscriber
  <!-- files: packages/workflow-engine/src/triggers/event.rs -->
  <!-- purpose: Subscribe to event bus, call registry.resolve_event() on each broadcast, build TriggerContext::Event for each match, and spawn workflows concurrently -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6 -->

## 1.6 — Integration with Scheduler

- [ ] Wire schedule trigger into the scheduler callback
  <!-- files: packages/workflow-engine/src/triggers/schedule.rs -->
  <!-- purpose: Register cron expressions with scheduler during startup, build TriggerContext::Schedule in callbacks, and pass to pipeline executor -->
  <!-- requirements: 5.1, 5.2, 5.3 -->

## 1.7 — Module Wiring and Exports

- [ ] Create triggers module and wire into workflow engine startup
  <!-- files: packages/workflow-engine/src/triggers/mod.rs, packages/workflow-engine/src/lib.rs -->
  <!-- purpose: Add mod.rs exporting registry, registrar, validation, and error submodules; call TriggerRegistrar::register_all during engine startup -->
  <!-- requirements: 2.1, 2.5 -->

## 1.8 — Integration Tests

- [ ] Add integration tests for trigger registration and resolution
  <!-- files: packages/workflow-engine/tests/trigger_system.rs -->
  <!-- purpose: Test full registration flow with valid/invalid YAML declarations, duplicate endpoint rejection, event broadcast to multiple workflows, and schedule registration -->
  <!-- requirements: 1.6, 2.1, 3.1, 4.1, 5.1, 6.1, 7.1, 8.1 -->
