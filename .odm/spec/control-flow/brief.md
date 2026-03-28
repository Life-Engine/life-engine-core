<!--
domain: control-flow
status: draft
tier: 2
updated: 2026-03-28
-->

# Control Flow Spec

## Overview

This spec defines how the pipeline executor evaluates step order, conditional branches, and error handling within a workflow. The workflow engine supports three control flow primitives in v1: sequential execution, conditional branching, and per-step error handling. These compose within a workflow definition to handle routing, error recovery, and degraded execution.

Sequential execution is the default mode. Steps run in declaration order and the output of step N becomes the input to step N+1. Conditional branching allows a `condition` block to route execution to one of two flat step lists based on a field value in the current `PipelineMessage` payload. Error handling is declared per-step via `on_error` with three strategies: `halt` (default), `retry` with exponential backoff and optional fallback, and `skip` which logs the error and passes the pre-step message forward.

## Goals

- Linear step execution as the default and only execution model in v1 (parallel fan-out/fan-in deferred)
- Condition evaluation performed by the executor directly, with no plugin involvement
- Safe-by-default branching where missing fields take the `else` path rather than raising errors
- One level of nesting only for condition blocks, keeping workflow definitions flat and predictable
- Per-step error strategies that let workflow authors choose between halting, retrying, or skipping on failure
- Pre-step message cloning to ensure retries and skips replay from a known-good state
- Skipped step errors surfaced as warnings in `WorkflowResponse.errors` so callers can detect degradation

## User Stories

- As a workflow author, I want steps to execute sequentially by default so that I can compose simple pipelines without additional configuration.
- As a workflow author, I want to branch execution based on a field value so that different data shapes are routed to the correct handler.
- As a workflow author, I want to declare error handling per step so that transient failures are retried and non-critical steps can be skipped without halting the workflow.
- As a workflow author, I want a fallback step executed when retries are exhausted so that I can log or recover from persistent failures.
- As a Core developer, I want condition evaluation to be internal to the executor so that branching does not depend on plugin availability.
- As a caller, I want skipped step errors included in the workflow response so that I can detect degraded execution without parsing logs.

## Functional Requirements

- The executor must run steps in declaration order, passing the output `PipelineMessage` of each step as input to the next.
- The executor must evaluate `condition` blocks using a dot-separated field path into the current `PipelineMessage` payload, supporting `equals`, `not_equals`, `exists`, and `is_empty` operators.
- The executor must take the `else` branch when the condition field path does not exist in the payload.
- The executor must limit condition nesting to one level (flat step lists in `then` and `else` only).
- The executor must rejoin branches after a condition block, using the branch output as input to the next step in the parent list.
- The executor must clone the current `PipelineMessage` before each step to create a pre-step snapshot.
- The executor must default to `halt` when a step does not declare an `on_error` strategy.
- The executor must retry failed steps up to `max_retries` times with exponential backoff when the strategy is `retry`, replaying the pre-step clone on each attempt.
- The executor must execute the `fallback` step when retries are exhausted, or halt if no fallback is declared.
- The executor must halt the workflow if the fallback step itself fails.
- The executor must skip failed steps when the strategy is `skip`, passing the pre-step clone to the next step and appending the error to `WorkflowResponse.errors` as a warning.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
