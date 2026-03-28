---
title: Control Flow Specification
type: reference
created: 2026-03-28
status: active
tags:
  - spec
  - workflow
  - control-flow
  - error-handling
---

# Control Flow Specification

This specification defines how the [[pipeline-executor]] evaluates step order, conditional branches, and error handling within a workflow.

## Sequential Execution

Sequential execution is the default mode. Steps run in order. The output of step N becomes the input to step N+1.

## Conditional Branching

A `condition` block replaces a normal step and directs execution to one of two branches:

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
```

### Evaluation Rules

- The executor evaluates conditions directly. No plugin is involved.
- `field` is a dot-separated path into the current `PipelineMessage` payload.
- Supported operators: `equals`, `not_equals`, `exists`, `is_empty`.
- Each condition block uses exactly one operator.
- Only one level of nesting is permitted. The `then` and `else` branches contain flat step lists only.
- After either branch completes, its output becomes the input to the next step in the parent list (branch rejoining).
- If the `field` path does not exist in the payload, the `else` branch is taken (safe by default).

## Error Handling Strategies

Each step may declare an `on_error` strategy. The default strategy is `halt`.

- **halt** — Stop the workflow immediately. The `WorkflowResponse` status is set to `Error`.
- **retry** — Retry the step up to `max_retries` times with exponential backoff. The pre-step clone is replayed on each attempt. On exhaustion: execute the `fallback` step if defined, otherwise halt.
- **skip** — Log the error, skip the step, and pass the pre-step `PipelineMessage` to the next step unchanged. The error is included in `WorkflowResponse.errors` as a warning.

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

## Error Handling in Branches

`on_error` is declared per-step, including steps inside conditional branches. The `condition` block itself has no error strategy because condition evaluation cannot fail.

## Fallback Steps

A fallback is a single step executed when retries are exhausted. The fallback step receives the pre-step `PipelineMessage`. If the fallback step itself fails, the workflow halts. There is no fallback-of-fallback.

## Skipped Step Errors

Errors from skipped steps are appended to `WorkflowResponse.errors` as warnings. The workflow status remains `Ok`, but the caller can inspect the errors list to detect degradation.

## Message Passing Summary

The following rules govern how `PipelineMessage` flows through the pipeline:

1. Clone the current message before each step (pre-step snapshot).
2. On success: the step's output replaces the current message.
3. On halt: the workflow stops immediately.
4. On retry: the pre-step clone is replayed on each attempt.
5. On skip: the pre-step clone is passed forward as the current message.
6. On condition: evaluate the condition, take the matching branch, and use the branch output as the current message.
