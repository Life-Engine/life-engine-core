<!--
domain: pipeline-executor
updated: 2026-03-28
-->

# Pipeline Executor Spec

## Overview

This spec defines the pipeline executor, the central runtime component of the workflow engine layer. The executor runs workflow steps in sequence, manages sync and async execution modes, builds the final response, and tracks async job lifecycle. It receives a `TriggerContext` from the transport layer (endpoint, event, or schedule), resolves the matching `WorkflowDefinition`, and walks the step list — calling plugin actions, recording `StepTrace` entries, and assembling a `WorkflowResponse`.

Async workflows run on a separate Tokio task and return a `JobId` immediately. The caller polls the `JobRegistry` for completion. A configurable concurrency limit (default 32) prevents unbounded task growth.

## Goals

- Execute workflow steps sequentially, passing a `PipelineMessage` through each step
- Support both sync (inline await) and async (spawned Tokio task) execution modes
- Convert each `TriggerContext` variant into an initial `PipelineMessage` with correct payload and metadata
- Load workflow definitions from YAML into an immutable `HashMap`, rejecting duplicate IDs at startup
- Record a `StepTrace` for every step regardless of outcome
- Build a `WorkflowResponse` with status, data, non-fatal errors, and trace metadata
- Manage async job lifecycle through a `JobRegistry` with status tracking, result storage, and TTL-based eviction
- Enforce a configurable concurrency limit on simultaneous workflow executions

## User Stories

- As a Core developer, I want a single executor entry point that handles both sync and async workflows so that transport handlers do not need separate code paths.
- As a plugin author, I want each step to receive the previous step's output as its input so that I can build composable data pipelines.
- As an operator, I want workflow definitions loaded from YAML at startup so that I can add or modify workflows without recompiling.
- As a workflow author, I want step-level tracing on every execution so that I can diagnose which step failed or slowed down.
- As a transport handler, I want async workflows to return a `JobId` immediately so that HTTP responses are not blocked by long-running work.
- As an operator, I want a concurrency limit on workflow executions so that the engine does not exhaust system resources under load.
- As a client, I want to poll async job status and retrieve results so that I can consume the output of long-running workflows.

## Functional Requirements Summary

- The system must provide `WorkflowExecutor` with `execute()` for sync workflows and `spawn()` for async workflows.
- The system must convert each `TriggerContext` variant (Endpoint, Event, Schedule) into an initial `PipelineMessage` with the correct payload and metadata mapping.
- The system must store workflow definitions in an immutable `HashMap<String, WorkflowDefinition>` loaded from YAML at startup and reject duplicate workflow IDs.
- The system must execute steps sequentially: clone the message, call the plugin action, replace the message on success, and append a `StepTrace`.
- The system must record a `StepTrace` (plugin_id, action, duration_ms, status) for every step, including skipped and failed steps.
- The system must build `WorkflowResponse` using `metadata.status_hint` (or default `Ok`), the final payload, accumulated non-fatal errors, and step traces.
- The system must manage async jobs through a `JobRegistry` with `InProgress`, `Completed`, and `Failed` statuses, exposing results via `GET /api/v1/jobs/:id`.
- The system must enforce a configurable concurrency limit (default 32) on simultaneous workflow executions, queuing excess requests.
- The system must evict completed job results after a configurable TTL (default 1 hour).

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
