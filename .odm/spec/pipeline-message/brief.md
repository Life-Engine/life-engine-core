<!--
domain: pipeline-message
updated: 2026-03-28
-->

# Pipeline Message Spec

## Overview

This spec defines the `PipelineMessage` struct — the universal data envelope passed between workflow steps in the Life Engine pipeline. Every plugin action receives a `PipelineMessage` as input and returns a modified `PipelineMessage` as output. This uniform shape makes all actions composable within the workflow engine.

The message consists of two parts: a `payload` (`serde_json::Value`) containing the step's primary data, and a `PipelineMetadata` struct carrying contextual information about the request, identity, execution trace, and plugin-writable hints. Plugins have restricted write access — they may modify `payload`, `status_hint`, `warnings`, and `extra`, while `request_id`, `trigger_type`, `identity`, `params`, `query`, and `traces` are read-only and enforced by the SDK.

The `PipelineMessage` is serialised as JSON when crossing the WASM boundary. The plugin SDK handles deserialisation on entry and serialisation on exit so that plugins work with native language types.

## Goals

- Define a single, stable message struct that all workflow steps consume and produce
- Enforce clear write-permission boundaries between executor-owned and plugin-writable fields
- Carry full execution context (request ID, trigger type, identity, route params, query params, traces) without requiring plugins to manage it
- Support plugin-to-plugin communication through `extra` metadata and `payload` modification
- Serialise cleanly across the WASM boundary as JSON with no manual parsing required by plugins
- Enable the executor to build the initial message from `TriggerContext` and extract the final response from the last step's output

## User Stories

- As a plugin author, I want a well-defined message struct so that I can read inputs and write outputs without knowing which step runs before or after me.
- As a plugin author, I want to set `status_hint` so that my action can influence the HTTP response status code returned to the caller.
- As a plugin author, I want to append warnings so that I can surface non-fatal issues without aborting the pipeline.
- As a plugin author, I want to store arbitrary metadata in `extra` so that downstream steps can read context I provide.
- As a workflow engine developer, I want read-only enforcement on executor-owned fields so that plugins cannot tamper with request identity, trace history, or routing parameters.
- As a workflow engine developer, I want the message to serialise as JSON across the WASM boundary so that the SDK can handle marshalling transparently.

## Functional Requirements Summary

- The system must define `PipelineMessage` with `payload` and `metadata` fields in `packages/types`.
- The system must define `PipelineMetadata` with `request_id`, `trigger_type`, `identity`, `params`, `query`, `traces`, `status_hint`, `warnings`, and `extra` fields.
- The system must define `IdentitySummary` with `subject` and `issuer` fields.
- The system must enforce plugin write permissions: `payload`, `status_hint`, `warnings`, and `extra` are writable; all other metadata fields are read-only.
- The SDK must restore executor-owned fields after plugin execution so that plugin modifications to read-only fields have no effect.
- The system must serialise `PipelineMessage` as JSON across the WASM boundary with SDK-managed deserialisation and serialisation.
- The executor must build the initial `PipelineMessage` from `TriggerContext` (route params, query params, request body, identity).
- The executor must append a `StepTrace` to `metadata.traces` after each step completes.
- The final message's `payload` must become the `data` field of the `WorkflowResponse`.
- If any step sets `status_hint`, the executor must use it as the HTTP response status code; otherwise default status codes apply.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
