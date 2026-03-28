# Search Indexer Plugin — QA Review

## Summary

The search subsystem spans two layers: a **core search engine** (`apps/core/src/search.rs`) backed by Tantivy, and a **plugin shell** (`plugins/engine/search-indexer/`) that declares capabilities and routes but delegates all real work to the core engine. A **search processor** (`apps/core/src/search_processor.rs`) bridges the message bus to the search engine for event-driven indexing, and a **REST route** (`apps/core/src/routes/search.rs`) exposes the search API.

The core engine is well-implemented with good test coverage. The plugin shell, however, is largely a skeleton — its `steps/`, `transform/`, and `tests/` modules are empty, and its `execute()` method is a no-op passthrough. The main issues are architectural (duplication between the plugin and core, per-document commits, lack of update semantics) rather than outright bugs.

---

## File-by-File Analysis

### plugins/engine/search-indexer/Cargo.toml

Dependencies are reasonable. Declares `tantivy` as a workspace dependency but the plugin itself never uses Tantivy directly — all indexing is done by the core's `SearchEngine`. The `tantivy` dependency is dead weight in this crate.

- `crate-type = ["cdylib", "rlib"]` is correct for a dual native/WASM plugin.

### plugins/engine/search-indexer/manifest.toml

Clean declaration of three actions (`search`, `reindex`, `status`) and three capabilities (`storage_read`, `events_subscribe`, `logging`).

- Missing `storage_write` capability — if the plugin ever needs to trigger re-indexing that writes back status records, this would block it.
- The `config.schema` is inline TOML-embedded JSON. Works, but there is no validation that `commit_threshold` is > 0.

### plugins/engine/search-indexer/project.json

Standard Nx project config. The WASM build target (`wasm32-wasip1`) is correct.

### plugins/engine/search-indexer/src/lib.rs

This is the main plugin entry point. It implements both `Plugin` (WASM trait) and `CorePlugin` (native trait).

- **Dual-trait implementation**: `Plugin::id()` and `CorePlugin::id()` return the same value, but the two traits are implemented independently with copy-pasted metadata. If either drifts, it would cause identity mismatches between native and WASM execution.
- **`execute()` is a no-op**: The WASM `Plugin::execute()` method accepts actions "search", "reindex", and "status" but returns the input unchanged. No actual search or indexing happens through this code path. This is effectively a stub.
- **`handle_event()` is a no-op**: Returns `Ok(())` for all events. The real event-driven indexing is done by `search_processor.rs` in core, not by this plugin. This means the plugin's declared `EventsSubscribe` capability is unused.
- **`on_load()` ignores config**: The `PluginContext` is received but the `config` field on the struct is never populated. The `SearchIndexerConfig` is never read from disk or the context.
- **Test coverage is good** for the plugin shell itself (metadata, capabilities, routes, lifecycle, WASM actions).

### plugins/engine/search-indexer/src/config.rs

Simple config struct with `index_path` and `commit_threshold`. Has a `Default` impl.

- The `index_path` default is `"data/search-index"` but the core `SearchEngine` creates an in-RAM index (never uses a path). These are disconnected — the config exists but nothing reads it.
- No validation on `commit_threshold` (0 would mean "never commit").

### plugins/engine/search-indexer/src/types.rs

Defines `SearchResult` with `record_id`, `collection`, and `score`. This duplicates `SearchHit` from `apps/core/src/search.rs` (which has `id`, `collection`, `plugin_id`, `score`, `snippet`). The two types have different field names (`record_id` vs `id`) and the plugin type lacks `plugin_id` and `snippet`.

### plugins/engine/search-indexer/src/error.rs

Well-structured error enum with error codes. `EngineError` impl is correct. Severity mapping is reasonable (`UnknownAction` and `NotInitialized` are `Fatal`, others are `Retryable`).

### plugins/engine/search-indexer/src/steps/mod.rs

Empty module — doc comment only. No pipeline step handlers are implemented.

### plugins/engine/search-indexer/src/transform/mod.rs

Empty module — doc comment only. No input/output mapping is implemented.

### plugins/engine/search-indexer/src/tests/mod.rs

Empty module — doc comment only. No additional tests beyond those in `lib.rs`.

### apps/core/src/search.rs

This is the real search engine implementation. Uses Tantivy in-memory index.

- **Schema design**: Five fields (`id`, `collection`, `plugin_id`, `content`, `title`). `id`, `collection`, `plugin_id` use `STRING | STORED` (exact-match, stored). `title` uses `TEXT | STORED` (tokenized, stored). `content` uses `TEXT` only (tokenized, not stored).
- **Per-document commit in `index_record()`**: Every call to `index_record()` acquires the writer lock, adds one document, commits, and reloads. This is extremely expensive — Tantivy commits are heavy I/O operations. The `index_records_bulk()` method exists but is marked `#[allow(dead_code)]` and never called from outside tests.
- **`search_processor.rs` does delete + re-insert on updates**: For `RecordChanged`, it calls `remove()` then `index_record()`. Each of these commits separately, so an update costs two Tantivy commits. This should be a single transaction.
- **Pagination via skip in application code**: The `search()` method fetches `offset + limit` results from Tantivy and then skips the first `offset` in a loop. For deep pagination (e.g., offset=10000, limit=10), this fetches 10010 results and throws away 10000. This is a known Tantivy pattern limitation but should be documented and possibly bounded.
- **No snippet generation from content**: The `snippet` field in `SearchHit` always contains the `title` text (lines 208-211), never an actual content snippet. The field name is misleading. Tantivy provides a `SnippetGenerator` that could produce highlighted content excerpts.
- **`content` field is not STORED**: Because `content` uses `TEXT` without `STORED`, the content text cannot be retrieved from the index after indexing. This is fine for searching but means snippet generation from content is impossible without re-reading the source record.
- **`extract_text()` is collection-aware but not extensible**: The match-based extraction hardcodes six collection types. New plugin collections fall through to the generic `flatten_strings` path, which works but produces lower-quality titles (takes the first string value as title).
- **`flatten_strings` depth limit**: The `max_depth` of 10 is reasonable for preventing stack overflow on malicious/deeply-nested JSON.
- **Empty query handling**: Correctly rejects empty/whitespace queries before touching Tantivy.
- **Limit cap**: Enforces `limit.min(100)` to prevent unbounded result sets.
- **Thread safety**: `IndexWriter` is behind `Arc<Mutex<IndexWriter>>` (tokio mutex). `search()` is synchronous and uses the reader, which is lock-free. This is the correct pattern — reads never block on writes.
- **No multi-tenancy filtering**: Search results are not filtered by `user_id` or `household_id`. Any authenticated user can search all records across all plugins. This is a potential data isolation concern.
- **No index persistence**: The `Index::create_in_ram(schema)` call means the entire search index is lost on restart. Every restart requires a full re-index of all records. For large datasets, this could be a significant startup penalty.

### apps/core/src/search_processor.rs

Event-driven bridge between the message bus and search engine.

- **Lag handling**: When the broadcast channel lags, the processor logs a warning but does not trigger a re-index. Those records are silently lost from the search index until their next update. This is documented in the warning message but there is no recovery mechanism.
- **Graceful shutdown**: Well-implemented with a `watch` channel and a drain loop. The drain loop processes remaining events after shutdown signal, which is good.
- **Double commit on update**: `remove()` commits, then `index_record()` commits. Two separate Tantivy commits for one logical operation.
- **No batching**: Each event triggers a separate lock acquisition + commit cycle. Under high write load, this could become a bottleneck.
- **No error escalation**: Indexing failures are logged as warnings but never escalated. If the index becomes corrupted, failures would silently accumulate.

### apps/core/src/routes/search.rs

REST endpoint for search.

- **Good error handling**: Distinguishes between missing engine (503), empty query (400), and query parse errors (400).
- **Query injection not a concern**: Tantivy's `QueryParser` handles arbitrary query syntax safely — no injection vector.
- **No authentication check in this handler**: The handler itself does not verify auth. It relies on middleware being applied at the router level. Tests explicitly skip auth, which is noted in comments.
- **Error message leaks internal details**: The `SEARCH_QUERY_INVALID` error includes `format!("invalid search query: {e}")` which exposes Tantivy parse errors to the client. These could reveal internal index field names.

---

## Problems Found

### Critical

1. **No multi-tenancy isolation in search** (`apps/core/src/search.rs:144-227`) — Search results are not filtered by `user_id` or `household_id`. Any authenticated user can discover records belonging to other users or households. In a multi-user deployment, this is a data leak.

2. **Per-document commits create severe write amplification** (`apps/core/src/search.rs:90-111`, `search_processor.rs:62-68`) — Every `index_record()` call does a Tantivy commit and reader reload. The search processor also does a delete commit followed by an insert commit for updates (two commits per update). Under load, this will cause high I/O latency and potential write starvation. The existing `index_records_bulk()` method is never used outside tests.

### Major

3. **Plugin is a hollow shell with no real implementation** (`plugins/engine/search-indexer/src/`) — The `steps/`, `transform/`, and `tests/` modules are empty. `execute()` is a passthrough. `handle_event()` is a no-op. `on_load()` does not read config. The plugin declares capabilities it does not use. All real search logic lives in core, making the plugin purely decorative.

4. **Duplicated and divergent types** (`plugins/engine/search-indexer/src/types.rs` vs `apps/core/src/search.rs:44-57`) — `SearchResult` in the plugin has `record_id` while `SearchHit` in core has `id`. The plugin type lacks `plugin_id` and `snippet`. If the plugin ever starts returning results, these will be incompatible.

5. **No index persistence — full re-index on every restart** (`apps/core/src/search.rs:70`) — `Index::create_in_ram(schema)` means the search index is volatile. The `SearchIndexerConfig` defines an `index_path` but it is never used. For a deployment with thousands of records, re-indexing on every restart is expensive and leaves search unavailable during startup.

6. **Message bus lag causes silent data loss from search index** (`apps/core/src/search_processor.rs:78-84`) — When the broadcast channel lags, skipped records are never indexed. There is no reconciliation mechanism (e.g., periodic full re-index, or a catch-up query against storage).

7. **Snippet field is misleading** (`apps/core/src/search.rs:208-219`) — `SearchHit.snippet` always contains the document title, never an actual content snippet or highlight. The field name suggests search-term-highlighted excerpts, which would be expected by consumers of the API.

### Minor

8. **Config struct exists but is never populated** (`plugins/engine/search-indexer/src/config.rs`) — `SearchIndexerConfig` has a `Default` impl and deserialization support but `on_load()` never reads it from the `PluginContext`. The `config` field on `SearchIndexerPlugin` is always `None`.

9. **`tantivy` dependency in plugin Cargo.toml is unused** (`plugins/engine/search-indexer/Cargo.toml:25`) — The plugin crate never imports or uses Tantivy directly. This adds compile time and binary size for no benefit.

10. **Error message leaks internal field names** (`apps/core/src/routes/search.rs:68`) — Tantivy parse errors returned to the client may reveal index field names like `content`, `title`, etc. This is minor information disclosure.

11. **No `commit_threshold` validation** (`plugins/engine/search-indexer/src/config.rs:11`) — A value of 0 would mean "never commit" if the config were ever used.

12. **`content` field not STORED in Tantivy schema** (`apps/core/src/search.rs:66`) — This prevents future snippet generation from content without a schema migration. Adding `STORED` later would require a full re-index.

13. **Deep pagination is O(offset+limit)** (`apps/core/src/search.rs:179-189`) — Fetches `offset + limit` results and discards the first `offset` in application code. Not a bug, but a performance concern for large offsets.

14. **Duplicate `id()` / `display_name()` / `version()` implementations** (`plugins/engine/search-indexer/src/lib.rs:49-58`, `87-97`) — `Plugin` and `CorePlugin` return identical values via separate impls. A drift in either would cause identity mismatches.

---

## Recommendations

1. **Add user/household filtering to search** — The `search()` method should accept optional `user_id` and `household_id` parameters. Add corresponding `STRING` fields to the Tantivy schema and include `MUST` clauses in the `BooleanQuery` for tenancy isolation. The search route should extract the authenticated user from the request context and pass it through.

2. **Implement batched commits** — Replace per-document commits with a buffer-and-flush strategy. The `commit_threshold` config field already exists for this purpose. Accumulate documents in the writer and commit after N documents or after a time interval. For the search processor, batch events from the bus before committing.

3. **Use `index_records_bulk()` for the search processor** — The bulk method already exists. The search processor should buffer events and call `index_records_bulk()` periodically instead of committing per-event.

4. **Combine delete+insert into a single commit** — For record updates, call `writer.delete_term()` and `writer.add_document()` within the same lock acquisition and commit once. This halves the commit overhead for updates.

5. **Either implement the plugin or remove it** — The search-indexer plugin is a shell. Either:
   - Move the core search engine into the plugin and have it operate as a real event-driven indexer, or
   - Remove the plugin and keep search as a core-only feature (which is how it actually works today).

6. **Add index persistence** — Use `Index::create_in_dir()` with the configured `index_path` instead of `create_in_ram()`. Fall back to RAM if the path is unavailable. This eliminates the re-index-on-restart problem.

7. **Add a reconciliation mechanism for bus lag** — When the processor detects lag, schedule a background task that queries storage for recently-updated records and re-indexes them. Alternatively, add a periodic reconciliation sweep.

8. **Implement real snippet generation** — Use Tantivy's `SnippetGenerator` to produce highlighted content excerpts. This requires making the `content` field `STORED` (or keeping a separate stored copy). Rename the current `snippet` field to `title` if keeping the title-as-snippet behavior.

9. **Sanitize error messages** — Wrap Tantivy parse errors in a generic message before returning to the client. Log the detailed error server-side.

10. **Unify the result types** — Either have the plugin re-export the core `SearchHit`/`SearchResults` types, or remove the plugin's `SearchResult` type entirely since it is unused.
