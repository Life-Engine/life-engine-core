<!--
domain: event-bus
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Implementation Tasks — Event Bus

**Progress:** 0 / 14 tasks complete

## 1.1 — Event Struct and Error Types

- [ ] Define `Event` struct and `EventBusError` enum
  <!-- files: crates/le-event-bus/src/event.rs, crates/le-event-bus/src/error.rs -->
  <!-- purpose: Establish the canonical event shape and error types used across the bus -->
  <!-- requirements: 1.1, 1.2, 1.3, 1.4, 1.5 -->

- [ ] Add unit tests for `Event` construction and field defaults
  <!-- files: crates/le-event-bus/src/event.rs -->
  <!-- purpose: Verify timestamp, source, and depth are set correctly during construction -->
  <!-- requirements: 1.1, 1.5 -->

## 1.2 — Event Bus Core

- [ ] Implement `EventBus` struct with broadcast channel, `emit`, and `subscribe`
  <!-- files: crates/le-event-bus/src/bus.rs -->
  <!-- purpose: Provide the in-memory pub/sub backbone using Tokio broadcast -->
  <!-- requirements: 4.1, 4.4 -->

- [ ] Implement depth-based loop prevention in `EventBus::emit`
  <!-- files: crates/le-event-bus/src/bus.rs -->
  <!-- purpose: Drop events exceeding max depth and log a warning -->
  <!-- requirements: 5.1, 5.2, 5.3, 5.4, 5.5 -->

- [ ] Add unit tests for emit, subscribe, and depth limit enforcement
  <!-- files: crates/le-event-bus/tests/bus_tests.rs -->
  <!-- purpose: Verify broadcast delivery and depth-based dropping behaviour -->
  <!-- requirements: 4.1, 4.4, 5.3, 5.4 -->

## 1.3 — Crate Scaffolding

- [ ] Create `le-event-bus` crate with `Cargo.toml` and module structure
  <!-- files: crates/le-event-bus/Cargo.toml, crates/le-event-bus/src/lib.rs -->
  <!-- purpose: Establish crate boundaries, dependencies, and public API exports -->
  <!-- requirements: N/A (infrastructure) -->

## 1.4 — Manifest Validation

- [ ] Implement manifest event declaration parsing and storage
  <!-- files: crates/le-event-bus/src/manifest.rs -->
  <!-- purpose: Parse declared event names from plugin manifests and store them for runtime lookup -->
  <!-- requirements: 3.3 -->

- [ ] Implement emission validation against declared events
  <!-- files: crates/le-event-bus/src/manifest.rs -->
  <!-- purpose: Reject emission of events not declared in the emitting plugin's manifest -->
  <!-- requirements: 3.1, 3.2 -->

- [ ] Add unit tests for manifest validation (declared and undeclared events)
  <!-- files: crates/le-event-bus/tests/manifest_tests.rs -->
  <!-- purpose: Verify accept/reject behaviour for declared vs undeclared event names -->
  <!-- requirements: 3.1, 3.2, 3.3 -->

## 1.5 — Plugin Host Function

- [ ] Implement `emit_event` host function for WASM plugins
  <!-- files: crates/le-event-bus/src/host.rs -->
  <!-- purpose: Expose event emission to plugins via the host function interface with manifest validation and depth tracking -->
  <!-- requirements: 7.1, 7.2, 7.3, 2.1 -->

## 1.6 — Event Dispatcher and Workflow Integration

- [ ] Implement event dispatcher that matches events to workflow triggers
  <!-- files: crates/le-event-bus/src/dispatcher.rs -->
  <!-- purpose: Subscribe to the bus, match events by name to workflow triggers, and spawn workflow tasks -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4 -->

- [ ] Implement `build_pipeline_message` for event-triggered workflows
  <!-- files: crates/le-event-bus/src/dispatcher.rs -->
  <!-- purpose: Construct the initial PipelineMessage from a triggering event's payload and metadata -->
  <!-- requirements: 8.1, 8.2, 8.3, 8.4 -->

## 1.7 — System Events

- [ ] Implement system event emission at Core lifecycle points
  <!-- files: crates/le-event-bus/src/system_events.rs -->
  <!-- purpose: Emit system.startup, system.plugin.loaded, system.plugin.failed, system.workflow.completed, system.workflow.failed at the correct lifecycle points -->
  <!-- requirements: 6.1, 6.2, 6.3, 6.4, 6.5 -->

- [ ] Add integration tests for system event emission and workflow triggering
  <!-- files: crates/le-event-bus/tests/system_event_tests.rs -->
  <!-- purpose: Verify system events are emitted with correct payloads and that matching workflows are triggered -->
  <!-- requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 4.1, 4.2 -->
