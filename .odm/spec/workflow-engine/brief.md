<!--
domain: workflow-engine
status: draft
tier: 1
updated: 2026-03-23
-->

# Workflow Engine Spec

## Overview

The workflow engine is the central orchestration layer in Core. Every request that involves plugin logic flows through a workflow — a declarative pipeline of plugin steps defined in YAML. The workflow engine also owns the event bus and cron scheduler, unifying all trigger mechanisms into a single pipeline system.

Workflows are YAML config files read from a configured directory at startup. There is no API for managing workflows — they are files on disk.

## Goals

- Provide a declarative YAML-based pipeline system for chaining plugin steps
- Support three trigger types: endpoint, event, and schedule (cron syntax)
- Support two execution modes: sync (await all steps) and async (return job ID, run in background)
- Pass typed data between steps using PipelineMessage envelopes with CDM or custom schema-validated payloads
- Provide configurable per-workflow validation: strict, edges (default), or none
- Support v1 control flow: sequential execution, conditional branching, and error handling (halt/skip/retry with fallback)
- Own the event bus for decoupled plugin-to-plugin communication via intermediate workflows
- Own the cron scheduler for time-triggered workflow execution
- Propagate errors using the EngineError trait with severity levels (Fatal, Retryable, Warning)

## User Stories

- As an admin, I want to define a workflow in a YAML file that chains email fetch, spam filter, and archiver plugins so that incoming email is processed automatically.
- As an admin, I want to trigger a workflow via an HTTP endpoint, an event, or a cron schedule — all using the same pipeline definition.
- As an admin, I want to choose sync or async mode per workflow so that queries return immediately while long-running operations run in the background.
- As an admin, I want to choose halt, skip, or retry per step so that non-critical steps do not block the entire pipeline.
- As a developer, I want to configure schema validation level (strict, edges, none) per workflow so that I can trade safety for performance as needed.
- As a developer, I want the workflow engine to use PipelineMessage as the standard envelope so that all data flow is typed and traceable.
- As a developer, I want plugins to emit events that trigger other workflows so that I can build decoupled, reactive pipelines.

## Functional Requirements

- The workflow engine must read YAML workflow definitions from a configured directory at startup.
- The workflow engine must support three trigger types: endpoint (HTTP path), event (named event), and schedule (cron expression).
- The workflow engine must support sync and async execution modes per workflow.
- The workflow engine must execute steps sequentially, passing PipelineMessage output of step N as input to step N+1.
- The workflow engine must support conditional branching based on output content.
- The workflow engine must support halt, skip, and retry error strategies per step, with retry supporting max_retries and fallback step.
- The workflow engine must validate data at pipeline boundaries according to the workflow's validation setting (strict, edges, none).
- The workflow engine must own the event bus — matching emitted events to workflow triggers.
- The workflow engine must own the cron scheduler — firing workflows at configured intervals.
- The workflow engine must call plugins as WASM modules via Extism.
- The workflow engine must implement the EngineError trait for error propagation with severity levels.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
