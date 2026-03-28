---
title: "ADR-011: Pipeline Message as Universal Step Contract"
type: adr
created: 2026-03-28
status: active
---

# ADR-011: Pipeline Message as Universal Step Contract

## Status

Accepted

## Context

Life Engine workflows chain multiple plugin steps in sequence. Each step receives input and produces output. The system needs a standardised envelope that carries data between steps so that any plugin's output can be another plugin's input — making plugins composable without requiring them to know about each other.

The alternative approaches are: typed per-plugin interfaces (each plugin defines its own input/output types), raw JSON blobs with no structure, or a protocol-buffer-style schema. Typed interfaces break composability — a contacts plugin's output type cannot be passed to a generic logging plugin without an adapter. Raw JSON provides no metadata for tracing, error signalling, or identity propagation. Protocol buffers add a compilation dependency to every plugin author.

## Decision

All plugin actions receive and return a `PipelineMessage` — a JSON-serialisable envelope with two top-level fields:

- `payload` — The business data. An arbitrary JSON value. The workflow engine passes it through without inspection. Only the sending and receiving plugins interpret its contents.
- `metadata` — Structured context that the workflow engine and plugins use for routing, tracing, and signalling. Metadata fields include: `request_id`, `trigger_type`, `identity` (authenticated user info), `params` (route parameters), `query` (query string), `traces` (step execution log), `status_hint` (suggested HTTP status), `warnings` (non-fatal issues), and `extra` (arbitrary key-value pairs for plugin-to-plugin communication).

The workflow engine clones the outgoing `PipelineMessage` from each step and passes the clone as input to the next step. The `payload` is replaced by the output of the previous step. Metadata accumulates — traces grow, warnings append, and `extra` values persist unless overwritten.

Plugins that have no meaningful output return the input `PipelineMessage` with the payload unchanged. Plugins that need to signal an error set `status_hint` and return the message — the workflow engine reads `status_hint` to determine the response status code.

## Consequences

Positive consequences:

- Any plugin can be composed with any other plugin. A contacts normaliser followed by a deduplication step followed by a logging step all use the same message type. No adapters or type converters needed.
- Metadata propagation (request ID, identity, traces) happens automatically. Plugins do not need to manually thread context through the pipeline.
- The `status_hint` mechanism lets plugins influence the HTTP response status without being aware of the transport layer. The transport handler reads the hint and translates it.
- The `extra` field provides an escape hatch for plugin-to-plugin data passing that does not fit in the payload.

Negative consequences:

- The `payload` field is untyped JSON. There is no compile-time guarantee that a plugin receives the shape of data it expects. Schema mismatches between steps are detected at runtime, not at definition time.
- Metadata accumulation means the message grows over the lifetime of a pipeline. Long pipelines with many steps produce large trace arrays. No truncation or sampling is applied in v1.
- The universal envelope is a lowest-common-denominator approach. Plugins that could benefit from strongly-typed interfaces (e.g., a Rust struct shared between two Rust plugins) must still serialise through JSON.
- The `status_hint` field is advisory. If multiple steps in a pipeline set conflicting hints, the last one wins. There is no merge strategy.
