# Workflow Engine Review

Package: `packages/workflow-engine/`
Reviewer: Workflow Engine Expert
Date: 2026-03-28

## Summary

The workflow engine is a well-structured YAML-driven pipeline execution system with three trigger types (endpoint, event, schedule), sequential step execution, error strategies (halt/skip/retry with fallback), conditional branching, and a cron scheduler with overlap prevention. It also includes a migration subsystem for versioned WASM-based data transforms.

The codebase is generally solid with good separation of concerns, thorough test coverage, and careful error handling. The most significant issues involve a race condition in the async job registration path, unbounded growth in the in-memory job registry, the `WorkflowEventEmitter` trait always emitting events at depth 0 (defeating cascade tracking), and missing validation for workflows that declare no trigger type. There are also several minor structural and correctness issues documented below.

## File-by-File Analysis

### Cargo.toml

The dependency set is appropriate. Notable dependencies include `extism` and `wasmparser` for migration WASM execution, `cron` for scheduling, and `rusqlite` for direct database access in the migration engine.

Observations:

- The direct dependency on `life-engine-storage-sqlite` and `rusqlite` tightly couples the migration engine to SQLite. This is acceptable for the current single-database architecture but will need refactoring if the storage backend becomes pluggable.
- `wasmparser` is used for export validation while `extism` is used for execution. Both are necessary but could be worth noting as a dual-WASM-runtime situation.

### src/lib.rs (WorkflowEngine)

The top-level `WorkflowEngine` struct ties together the trigger registry, pipeline executor, event bus, and scheduler. It serves as the public API surface for transports.

- **has_endpoint / handle_endpoint** — Clean routing interface. `has_endpoint` does a registry lookup, and `handle_endpoint` builds the trigger context and executes.
- The `_scheduler` field is held only to keep spawned tasks alive; this is a valid pattern but `Drop` for `Scheduler` is not implemented, meaning tasks are aborted only if `stop()` is explicitly called. If the engine is dropped without `stop()`, the tasks will be orphaned and only cleaned up when the tokio runtime shuts down.

### src/types.rs

Well-defined type hierarchy. Key observations:

- `TriggerDef` uses `Option<String>` for all three trigger fields. There is no compile-time or serde-level enforcement that at least one is set. Validation happens at runtime in `triggers/mod.rs` and `loader.rs`, but a workflow with all `None` triggers could slip through if only `load_workflows` is called without `validate_triggers`. These two validation paths are separate and could diverge.
- `ConditionDef` has `then_steps` and `else_steps` as `Vec<StepDef>` — empty vectors are valid, meaning a condition branch can silently do nothing. This is a design choice but could surprise users.
- `ErrorStrategy.fallback` is a `Box<StepDef>`, but there is no validation that fallback steps themselves don't declare error strategies or conditions. A fallback with its own retry/fallback creates an implicit nesting that is not handled.

### src/executor.rs (PipelineExecutor)

This is the core of the engine at approximately 1170 lines (code) plus approximately 1400 lines of tests. It implements sequential step execution with three error strategies, conditional branching, sync/async execution modes, validation levels, a job registry, concurrency control via semaphore, and structured execution logging.

Significant findings:

- **Race condition in `spawn()`** (lines 358-369): The job is registered as `InProgress` in a *separate* spawned task from the actual execution task. There is no ordering guarantee between these two spawns. A caller could call `spawn()`, get the job ID, then immediately query `job_status()` and get `None` because the registration task hasn't completed yet. The `execute_async()` method does this correctly (registers inline before spawning). The `spawn()` method should register the job synchronously before spawning the execution task.

- **Unbounded job registry** — The `jobs: Arc<RwLock<HashMap<Uuid, JobEntry>>>` grows indefinitely. `cleanup_expired_jobs()` exists but is never called automatically. There is no periodic cleanup task, no maximum size cap, and no eviction on query. In a long-running instance with many async workflows, this will leak memory. The cleanup should either be called periodically by the scheduler or triggered on each insertion.

- **Event emitter depth always 0** — The `WorkflowEventEmitter` trait implementation on `EventBus` (event_bus.rs:222-234) always creates events with `depth: 0`. This means events emitted by workflows that were themselves triggered by events always appear as root events, defeating the depth-based loop prevention. A workflow triggered at depth 3 that emits a completion event should produce an event at depth 4, not depth 0.

- **Conditional branch index multiplication** — In `execute_steps()` (line 856), branch steps get index `index * 100`. This is meant to create a separate index space for branch step logs, but it means a workflow with 2+ condition steps will have overlapping index ranges (e.g., step 0 branches use 0-99, step 1 branches use 100-199). If a then/else branch has more than 100 steps, indices will collide with the next condition step's range. More importantly, the `index * 100` scheme makes step logs hard to correlate with the original workflow definition.

- **validate_message is structurally weak** — The validation function (lines 226-272) always validates against `{"type": "object"}`, which is a minimal schema that only checks the payload is a JSON object. This means validation provides almost no actual data integrity checking. For CDM payloads especially, a more specific schema should be used. The comment says "CDM payloads are strongly typed" but the runtime validation doesn't leverage the CDM schemas at all.

- **Retry backoff cap is 30 seconds** — Line 1020: `std::cmp::min(1u64 << (attempt - 1), 30)`. With max_retries defaulting to 3, the maximum backoff is 4 seconds (1, 2, 4). The 30-second cap only applies if someone configures `max_retries >= 6`. The exponential backoff has no jitter, which could cause thundering herd effects if multiple workflows retry the same failing service simultaneously.

- **Warning severity skips step logging** — When a step returns a `Warning` severity error, the `continue` on line 925 bypasses step log creation entirely. The step won't appear in the `ExecutionLog.steps` vector at all, making it invisible to monitoring.

- **`let-else` chains** — The `if let Ok(ref msg) = result` with `&&` on line 738 uses an unstable `let-chains` syntax that requires `#![feature(let_chains)]` or Rust 1.87.0+. This may cause compilation issues on older toolchains.

### src/event_bus.rs (EventBus)

Clean broadcast-based event distribution with depth-based loop prevention.

- **Channel capacity of 256 may be too small** — Under heavy event load (many concurrent workflows emitting events), the broadcast channel can lag. When a subscriber falls behind by more than 256 events, it will receive `RecvError::Lagged` and lose events. There's no monitoring or logging for this condition.

- **Fire-and-forget spawns** — Event-triggered workflow executions are spawned with `tokio::spawn` (line 126) and the `JoinHandle` is discarded. If the spawned task panics, the panic will be silently swallowed. A `JoinHandle` set or at minimum a panic hook should catch these.

- **No backpressure** — The `emit()` method spawns unbounded tasks for matching workflows. If a burst of events arrives (e.g., initial data import), this could spawn thousands of concurrent workflow executions with no throttling. The `PipelineExecutor` has a semaphore, but the event bus doesn't coordinate with it for spawned tasks created directly (the semaphore is per-executor, not shared across event-triggered spawns).

- **Event name validation gap** — The event bus `emit()` method performs no validation on event names. The loader validates event names in trigger declarations, but events emitted at runtime (e.g., from `emit_system_event`) bypass this validation. An event like `"system..startup"` would be emitted without error.

### src/scheduler.rs (Scheduler)

Correct cron scheduling with overlap prevention via `ScheduleJobTracker`.

- **`to_std().unwrap_or_default()` on line 152** — If the computed `next - now` is negative (e.g., if the system clock jumps forward between computing `next` and reaching this line), `to_std()` will fail and `unwrap_or_default()` will produce `Duration::ZERO`. This means the workflow fires immediately, which is acceptable. However, repeated clock skew could cause rapid-fire executions.

- **Tracker uses `tokio::sync::Mutex`** — The `ScheduleJobTracker` uses a tokio `Mutex` wrapping a `HashSet`. Since the critical section is just an `insert`/`remove`/`contains` on a small set, a `std::sync::Mutex` would be more efficient (no `.await` overhead). The tokio `Mutex` is typically recommended only when the lock needs to be held across `.await` points, which is not the case here.

- **No missed execution detection** — If the scheduler is delayed (e.g., due to system load or the overlap prevention skip), there is no mechanism to detect or log that a scheduled execution was missed (as opposed to deliberately skipped for overlap). The debug log for overlap skip is good, but there's no equivalent for "the scheduler woke up late and missed the scheduled time."

### src/loader.rs (TriggerRegistry)

Solid YAML loading with duplicate ID and endpoint detection.

- **Endpoint path matching is exact string comparison** — `find_endpoint` (line 300-303) creates a `path.to_string()` and looks it up in a `HashMap`. This means `/email/sync` and `/email/sync/` are different paths. There's no path normalization (trailing slashes, double slashes, URL-encoded segments). This could cause subtle routing bugs.

- **No trigger requirement validation** — The loader's `validate_workflow` function checks that steps are non-empty and validates event names, but it does *not* verify that at least one trigger type is declared. This validation exists separately in `triggers/mod.rs::validate_triggers()`. If code paths call `load_workflows` without also calling `validate_triggers`, triggerless workflows could be loaded. The `WorkflowEngine::new()` does not call `validate_triggers`.

- **`HashMap` iteration order** — `WorkflowFile.workflows` is a `HashMap<String, WorkflowDef>`, so the order workflows are processed within a single file is non-deterministic. This doesn't cause correctness issues (duplicates are caught), but means error messages for the first duplicate found may vary between runs.

### src/triggers/mod.rs

Additional trigger validation that is more strict than the loader's built-in checks. Requires at least one trigger and enforces dot-separated event names with character restrictions.

- **Duplicate event name validation** — The triggers module validates event name *format* but not uniqueness. Unlike endpoints, multiple workflows can share the same event trigger (this is by design for fan-out), so no duplicate check is needed. This is correct.

- **Strictness difference** — `triggers::validate_event_name` requires at least two segments (must contain a dot), while `loader::validate_event_name` only requires non-empty segments. A single-segment event like `"startup"` would pass the loader but fail trigger validation. This inconsistency could be confusing.

### src/error.rs (WorkflowError)

Clean error type hierarchy implementing `EngineError`.

- **Severity classification** — `StepHalted` and `ValidationFailed` are classified as `Fatal`, which is correct. `RetryExhausted` is classified as `Retryable`, which is debatable since retries have already been exhausted — the caller likely cannot retry further without changing something. `EventBusError` as `Retryable` makes sense for transient issues.

### src/schema_registry.rs (SchemaRegistry)

Plugin private collection schema registry with namespace isolation.

- **No schema validation** — Schemas are stored as raw `serde_json::Value` and never validated to confirm they are actually valid JSON Schema documents. A plugin could register `{"type": "invalid_type"}` and it would be silently accepted. Validation happens at write time in the storage layer, so an invalid schema would cause all writes to fail with cryptic errors.

- **Silent overwrite** — `register()` silently replaces an existing schema. This is tested and intentional (test `overwrite_replaces_schema`), but there is no log warning when a schema is replaced. This could mask bugs during plugin reloading.

- **`RwLock` poisoning** — `get_schema` returns `None` when the `RwLock` is poisoned (line 80: `self.schemas.read().ok()?`), which silently degrades. If the lock was poisoned due to a panic in `register()`, all subsequent `get_schema` calls will return `None` instead of surfacing the error.

### src/config.rs (WorkflowConfig)

Minimal configuration struct with just a `path` field.

- Sufficient for current needs. Could expand to include event bus capacity, scheduler concurrency, job TTL, etc.

### src/migration/ (Migration Subsystem)

The migration subsystem handles versioned data transforms via WASM sandboxes.

#### migration/mod.rs

Clean module structure with a `MigrationError` type implementing `EngineError`.

#### migration/manifest.rs

Thorough manifest parsing with validation for `from` ranges, `to` versions, transform names, collection names, overlap detection, and chain contiguity.

- **Overlap detection is O(n^2)** — The `validate_no_overlapping_ranges` function does pairwise comparison of all entries within a collection. This is acceptable for the expected small number of migration entries per plugin.

- **`from_range_min_version` edge case** — For a 4-or-more segment version string, the function returns `Version::new(0, 0, 0)`, which would pass `validate_from_less_than_to` for any positive `to` version. The `validate_from_range` function rejects such inputs, so this is unreachable in practice, but the fallback is surprising.

#### migration/runner.rs

WASM transform execution via Extism in a pure sandbox (no host functions).

- **WASI enabled** — Line 73: `.with_wasi(true)`. This grants the transform access to WASI capabilities (filesystem, environment variables, etc.). The doc comment says "no host function access" and "pure sandbox," but WASI is a form of host function access. For migration transforms that should be pure JSON-in/JSON-out, WASI should likely be disabled.

- **Per-record WASM instantiation** — `run_transform` is called per record in the migration engine (engine.rs:149). Each call creates a fresh Extism plugin instance, including WASM compilation. For large migrations (thousands of records), this could be extremely slow. The WASM module should be compiled once and reused across records.

- **Crash detection heuristic** — Lines 87-101: The distinction between `TransformFailed` and `TransformCrashed` is based on string matching ("unreachable", "trap", "panic", "wasm backtrace"). This is fragile and depends on Extism's error message format, which could change between versions.

#### migration/engine.rs

Orchestrates the full migration pipeline with backup, transform, quarantine, and logging.

- **Transaction scope issue** — The engine begins a transaction per migration entry (line 127), but quarantine writes happen inside the same transaction (line 168). If the transaction is rolled back due to a commit failure, quarantine records are also lost. More importantly, the `quarantine_record` call's error is silently discarded with `let _ = quarantine_record(...)`. If quarantining fails, the record is silently lost — not migrated and not quarantined.

- **Migration log records total counts, not per-entry counts** — Lines 212-213: `records_migrated: total_migrated as i64` and `records_quarantined: total_quarantined as i64` use the running totals, not the per-entry counts. This means the log entry for the first migration entry will show the correct counts, but subsequent entries will show inflated counts that include previous entries.

- **`run_migrations` is `async` but blocks on SQLite** — The function is async (for WASM transform calls), but all SQLite operations are synchronous and blocking. In a tokio context, this blocks the executor thread. Long migrations with many records should use `spawn_blocking` for the SQLite operations or use a connection pool.

- **Connection is not shared with backup** — The `create_backup` call (line 80) takes `db_path` and opens its own connection, while the main migration loop opens another connection (line 97). If another process writes between the backup and the start of migration, the backup may not reflect the pre-migration state.

### tests/migration_test.rs

Comprehensive integration tests covering end-to-end migration, quarantine, chain migration, backup/restore, idempotency, mixed collections, and plugin scoping.

- Well structured with clear test isolation via `TempDir`.
- Uses WAT-compiled identity transforms and trapping transforms.
- The `count_rows` helper uses string interpolation for the table name (line 77: `&format!("SELECT count(*) FROM {table}")`), which could be an SQL injection vector in production code, but is acceptable in tests with hardcoded table names.

### src/tests/mod.rs

Empty test module. All meaningful tests are colocated with their respective modules.

## Problems Found

### Critical

- **C1: Race condition in `PipelineExecutor::spawn()` job registration** — The job is registered as `InProgress` in a separate spawned task. A caller querying `job_status()` immediately after `spawn()` returns may get `None` instead of `Some(InProgress)`. This breaks the contract that the returned job ID is immediately queryable. (`executor.rs:358-369`)

- **C2: Event depth tracking is defeated** — `WorkflowEventEmitter` for `EventBus` always creates events at depth 0. Events emitted by workflows triggered by other events appear as root events, making the depth-based loop prevention ineffective for cascading workflow chains. (`event_bus.rs:222-234`)

### Major

- **M1: Unbounded job registry memory leak** — The in-memory `jobs` HashMap grows without bound. `cleanup_expired_jobs()` is never called automatically. Long-running instances with async workflows will eventually exhaust memory. (`executor.rs:278`)

- **M2: Per-record WASM instantiation in migrations** — Each record transform creates a fresh Extism plugin instance including WASM compilation. Migrations of thousands of records will be extremely slow. The WASM module should be compiled once and reused. (`migration/runner.rs` called from `migration/engine.rs:149`)

- **M3: WASI enabled in "pure sandbox" migration transforms** — Migration transforms are described as having "no host function access" but WASI is enabled, granting filesystem and environment access. This violates the stated security boundary. (`migration/runner.rs:73`)

- **M4: Quarantine failure silently discarded** — If `quarantine_record` fails, the error is ignored with `let _ = ...`. The record is neither migrated nor quarantined, resulting in silent data loss. (`migration/engine.rs:168`)

- **M5: Missing trigger validation in WorkflowEngine::new()** — `validate_triggers()` is exported but never called in the engine initialization path. A workflow with no triggers could be loaded and registered, causing it to never execute. (`lib.rs:57-59`)

- **M6: Warning severity bypasses step logging** — Steps that return `Warning` severity errors are not recorded in the `ExecutionLog.steps` vector, making them invisible to monitoring and debugging. (`executor.rs:925`)

- **M7: Fire-and-forget spawns swallow panics** — Event-triggered workflow tasks are spawned and immediately forgotten. Panics in these tasks are silently swallowed with no logging. (`event_bus.rs:126`)

### Minor

- **m1: Scheduler uses `tokio::sync::Mutex` unnecessarily** — The `ScheduleJobTracker` lock is never held across `.await` points, so `std::sync::Mutex` would be more efficient. (`scheduler.rs:16,29`)

- **m2: No path normalization for endpoint matching** — Endpoint paths are matched as exact strings. Trailing slashes, double slashes, and case differences are not normalized. (`loader.rs:300-303`)

- **m3: Event name validation inconsistency** — `triggers::validate_event_name` requires at least two segments (must contain a dot), while `loader::validate_event_name` allows single-segment names that happen to not contain dots (it only checks for empty segments). (`triggers/mod.rs:54` vs `loader.rs:206`)

- **m4: Conditional branch step index collision** — The `index * 100` scheme for branch step indices creates potential collisions with branches having 100+ steps, and makes log correlation with workflow definitions difficult. (`executor.rs:856`)

- **m5: No Scheduler Drop implementation** — If the `WorkflowEngine` is dropped without explicitly calling `stop()`, scheduler tasks remain running until the tokio runtime shuts down. (`scheduler.rs:59-62`)

- **m6: Migration log uses cumulative totals** — Per-entry migration log records contain total cumulative counts rather than per-entry counts. (`migration/engine.rs:212-213`)

- **m7: Retry backoff lacks jitter** — Exponential backoff is deterministic, which can cause thundering herd effects when multiple workflows retry the same failing service. (`executor.rs:1020-1021`)

- **m8: Schema registry silently degrades on RwLock poison** — `get_schema` returns `None` when the lock is poisoned, indistinguishable from "schema not registered." (`schema_registry.rs:80`)

- **m9: No broadcast channel lag monitoring** — The event bus broadcast channel (capacity 256) will silently drop events for slow subscribers with no logging. (`event_bus.rs:21`)

- **m10: Blocking SQLite operations in async migration function** — Synchronous SQLite calls block the tokio executor thread during migrations. (`migration/engine.rs`)

- **m11: `validate_message` only checks `{"type": "object"}`** — Pipeline message validation is nearly a no-op, providing minimal data integrity checking even in `Strict` mode. (`executor.rs:246`)

## Recommendations

1. **Fix the spawn() race condition** — Register the job inline in `spawn()` before spawning the execution task, matching how `execute_async()` handles it.

2. **Propagate event depth through workflow execution** — Pass the triggering event's depth into the executor and increment it when emitting downstream events. The `WorkflowEventEmitter` trait should accept depth as a parameter, or events should carry their lineage.

3. **Add automatic job cleanup** — Either schedule periodic cleanup via the scheduler, cap the job map size with LRU eviction, or trigger cleanup on each `execute_async` call.

4. **Compile WASM once per migration** — Load the Extism plugin outside the per-record loop and reuse it across all records in a migration entry.

5. **Disable WASI for migration transforms** — Change `.with_wasi(true)` to `.with_wasi(false)` to enforce the documented pure-sandbox constraint.

6. **Surface quarantine failures** — At minimum, log a warning when quarantining fails. Consider returning an error or adding the failure to the migration result.

7. **Call `validate_triggers` in `WorkflowEngine::new()`** — Add this call after `load_workflows` to ensure all loaded workflows have at least one trigger.

8. **Add step logs for warning-severity steps** — Create a `StepStatus::Warning` variant or reuse `Skipped` so these steps appear in execution logs.

9. **Implement `Drop` for `Scheduler`** — Abort spawned tasks on drop to prevent leaking background tasks when the engine is dropped.

10. **Add jitter to retry backoff** — Use randomized jitter (e.g., `duration * (0.5 + random(0.0, 0.5))`) to prevent thundering herd effects.
