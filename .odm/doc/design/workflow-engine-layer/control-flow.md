---
title: Control Flow
type: adr
created: 2026-03-28
status: draft
tags:
  - architecture
  - workflow-engine
  - control-flow
  - error-handling
  - core
---

# Control Flow

## Overview

The workflow engine supports three control flow primitives in v1: sequential execution, conditional branching, and per-step error handling. These compose within a workflow definition to handle routing, error recovery, and degraded execution.

## Sequential Execution

The default. Steps run in order. The output `PipelineMessage` of step N becomes the input to step N+1.

```yaml
steps:
  - plugin: connector-email
    action: fetch
  - plugin: search-indexer
    action: index
```

This is the only execution model in v1. Parallel step execution (fan-out/fan-in) is deferred.

## Conditional Branching

A condition block routes execution to different steps based on the previous step's output:

```yaml
steps:
  - plugin: classifier
    action: classify
  - condition:
      field: "payload.category"
      equals: "spam"
      then:
        - plugin: spam-handler
          action: quarantine
      else:
        - plugin: email-archiver
          action: store
  - plugin: notifier
    action: notify
```

### Evaluation

The executor evaluates conditions directly — no plugin is involved. The `field` value is a dot-separated path into the current `PipelineMessage` payload.

Available operators in v1:

- `equals` — Field value matches exactly
- `not_equals` — Field value does not match
- `exists` — Field is present (any value, including `null`)
- `is_empty` — Field is absent, `null`, empty string, or empty array

Only one operator per condition block. This covers CRUD routing, error classification, and flag-based branching. Anything more complex belongs in a plugin step that outputs a classification.

### Nesting

One level only for v1. A condition block contains flat step lists in `then` and `else`. Nested conditions within branches are not supported. If deeper logic is needed, extract it into a separate workflow triggered by event.

### Branch Rejoining

Execution continues after the condition block. The output of whichever branch's final step ran becomes the input to the next step after the condition block. In the example above, `notifier.notify` receives the output of either `spam-handler.quarantine` or `email-archiver.store`, depending on which branch was taken.

This keeps the linear `PipelineMessage` passing model intact — the condition block is a detour, not a fork.

### Condition Failure

A condition block itself cannot fail. If the `field` path does not exist in the payload, the `else` branch is taken. This makes conditions safe by default — a missing field is treated as "condition not met", not as an error.

## Error Handling

Each step declares an `on_error` strategy. If no strategy is declared, the default is `halt`.

### Strategies

- **halt** (default) — Stop the entire workflow immediately. The `WorkflowResponse` status is `Error` with the step's error detail.

- **retry** — Retry the step up to `max_retries` times with exponential backoff. The pre-step `PipelineMessage` clone is replayed as input on each retry. If retries are exhausted, execute the `fallback` step (if declared) or halt.

- **skip** — Log the error, skip this step, and pass the pre-step `PipelineMessage` to the next step. The skipped step's error is included in `WorkflowResponse.errors` as a non-fatal warning.

### Declaration

```yaml
steps:
  - plugin: connector-email
    action: fetch
    on_error:
      strategy: retry
      max_retries: 3
      fallback:
        plugin: error-logger
        action: log
  - plugin: search-indexer
    action: index
    on_error:
      strategy: skip
```

### Error Handling in Branches

`on_error` is per-step, including steps inside conditional branches. The condition block itself has no error strategy because it cannot fail (see above). Each step within a `then` or `else` branch declares its own error handling independently.

```yaml
- condition:
    field: "payload.type"
    equals: "urgent"
    then:
      - plugin: fast-handler
        action: process
        on_error:
          strategy: halt
    else:
      - plugin: batch-handler
        action: queue
        on_error:
          strategy: skip
```

### Fallback Steps

A fallback is a single step executed when retries are exhausted. The fallback receives the same pre-step `PipelineMessage` that was used for retries. If the fallback itself fails, the workflow halts — there is no fallback-of-fallback.

### Skipped Step Errors in Response

When a step is skipped via `on_error: skip`, the error is not silently discarded. It is appended to `WorkflowResponse.errors` as a warning. The workflow's `status` remains `Ok` (the workflow completed), but the caller can inspect `errors` to see that execution was degraded.

## Message Passing Summary

The `PipelineMessage` lifecycle through control flow:

1. Executor clones the message before calling each step (pre-step snapshot)
2. Step succeeds → output replaces the current message
3. Step fails, `halt` → workflow stops, error returned
4. Step fails, `retry` → pre-step clone is replayed as input, up to `max_retries`
5. Step fails, `skip` → pre-step clone is passed to the next step
6. Condition block → evaluates field, takes `then` or `else` branch
7. Branch completes → branch output becomes current message, execution continues
