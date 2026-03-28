# GraphQL Transport Review

## Summary

The GraphQL transport layer spans two locations: the `packages/transport-graphql/` crate (a thin workflow-translation layer) and the production GraphQL API in `apps/core/src/routes/graphql.rs` (the real async-graphql schema with queries, mutations, and subscriptions). The transport crate is well-structured but largely a stub awaiting workflow-engine integration. The core `routes/graphql.rs` is substantially more complete and provides full CRUD, subscriptions, nested relationship resolution, pagination, filtering, and sorting. However, there are critical gaps in query security (no depth/complexity limits, introspection enabled in production, no batch limits) and authentication is not integrated at the GraphQL resolver level.

## File-by-File Analysis

### packages/transport-graphql/Cargo.toml

Declares the crate with workspace-managed versions and appropriate dependencies (`async-graphql`, `async-graphql-axum`, `axum`, `chrono`, `serde`, `serde_json`, `tokio`, `thiserror`, `async-trait`, `toml`, `tracing`, `uuid`).

Observations:

- Clean dependency list, all workspace-managed
- No `[dev-dependencies]` section, so tests rely on the main dependency tree (acceptable for an internal crate)

### packages/transport-graphql/src/lib.rs

Defines `GraphqlTransportConfig` (host/port with sane defaults), `GraphqlTransport` struct, and the `Transport` trait implementation. The `start()` method only logs and returns `Ok(())` -- it does not actually bind a listener or serve requests.

Issues:

- `start()` is a no-op. The transport reports itself as started without actually serving traffic. This means the transport registry can believe GraphQL is running when it is not.
- The `from_config` method accepts a `toml::Value` and converts it, which is fine, but the error uses `Box<dyn EngineError>` which means error context is erased at the call site.
- Port `4000` default is reasonable but not documented as differing from the main REST port.

### packages/transport-graphql/src/error.rs

Three error variants: `QueryFailed`, `BindFailed`, `InvalidConfig`. Implements `EngineError` with unique error codes and severity levels.

Observations:

- `QueryFailed` is marked `Retryable`, which is questionable -- most GraphQL query failures (syntax errors, validation errors) are permanent and should not be retried. Only transient backend failures warrant retry.
- Clean, minimal error type. Good use of `thiserror`.

### packages/transport-graphql/src/config.rs

Generates GraphQL type descriptors from plugin schema declarations. Maps JSON Schema types to GraphQL scalars. Includes a `to_pascal_case` helper.

Issues:

- `json_type_to_graphql` passes unknown types through verbatim (`other => other.to_string()`). An unknown type like `"object"` or `"array"` would produce an invalid GraphQL scalar name. This should either reject unknown types or map them to a default (e.g., `JSON` scalar).
- `to_pascal_case` does not handle hyphens, numbers at word boundaries, or already-PascalCase input. For example, `"my-collection"` becomes `"my-collection"` (unchanged) because only `_` is used as a separator. Hyphens are common in identifiers.
- `PluginSchemaDeclaration.fields` is a `HashMap<String, String>`, so field ordering in the generated type is nondeterministic. This is fine for runtime but makes schema introspection results unstable across restarts.
- Generated types (`GeneratedGraphqlType`) are descriptors only -- they are never wired into an actual async-graphql schema. The config module is dead code in the current architecture.

### packages/transport-graphql/src/types.rs

Defines `GraphqlRequest` wire type, `translate_request` (GraphQL -> WorkflowRequest), response envelopes (`GraphqlSuccessResponse`, `GraphqlErrorResponse`), and `translate_response` (WorkflowResponse -> GraphQL wire format).

Issues:

- **`serde_json::to_value(body).unwrap()` on lines 98 and 119** -- These `unwrap()` calls can panic if serialization fails. While unlikely for these simple structs, a library function should not panic. Use `.unwrap_or_else()` or return `Result`.
- `translate_request` hardcodes `workflow: "graphql.query"` for all requests, including mutations. If the workflow engine dispatches based on this value, mutations will be routed incorrectly.
- All GraphQL variables are flattened to string representations in the `query` HashMap. Complex variables (nested objects, arrays) lose their structure. For example, `{"filter": {"status": "active"}}` becomes `"filter" -> "{\"status\":\"active\"}"` which the workflow engine cannot parse as structured data.
- The response does not conform to the GraphQL spec for partial success -- the spec allows `{ "data": ..., "errors": [...] }` simultaneously, but this implementation returns either `data` or `errors`, never both.
- `default_error_message` has a catch-all `_ => "Unknown error"` arm, which silently swallows new `WorkflowStatus` variants without a compiler warning if the enum gains members.

### packages/transport-graphql/src/handlers/mod.rs

Contains the Axum handler for `POST /graphql` and a response translator. The handler is a stub: it always returns the `WorkflowRequest` as JSON, using `Identity::guest()` instead of an authenticated identity.

Issues:

- **No authentication** -- The handler always uses `Identity::guest()`. No mechanism exists to extract an authenticated identity from the request (headers, cookies, or Axum extensions).
- **No dispatcher injection** -- The handler never calls a workflow dispatcher. It returns the translated request as the response, making it useless for production.
- **No input validation** -- The handler deserializes `GraphqlRequest` from JSON but performs no validation on the `query` field (length, content, nesting depth).
- The `into_graphql_response` function is dead code -- nothing calls it. It is `pub` but no other module references it.

### packages/transport-graphql/src/tests/mod.rs

Six tests covering request translation, schema generation, response envelopes, transport equivalence, and default error fallback. Tests are well-structured and cover the main contract points.

Observations:

- Good coverage of the translation pipeline
- Tests verify the structural invariant that both REST and GraphQL produce `WorkflowRequest`
- No negative tests for malformed input (empty query, missing fields, oversized payloads)
- No test for the `into_graphql_response` function
- No test for `to_pascal_case` edge cases (hyphens, empty strings, single characters)

### apps/core/src/routes/graphql.rs (Integration)

This is the production GraphQL implementation. It defines a full async-graphql schema with:

- Seven CDM types: Task, Contact, CalendarEvent, Email, Note, File, Credential
- Corresponding enums: TaskPriority, TaskStatus, CredentialType, SortDirection
- Input types: PaginationInput, SortInput, FilterInput (equality, comparison, text search)
- Generic `Connection<T>` wrapper for paginated results
- QueryRoot with list and get-by-id resolvers for all seven collections
- MutationRoot with generic `createRecord`, `updateRecord`, `deleteRecord`
- SubscriptionRoot with `recordChanges` (filtered by collection)
- Nested resolvers: `CalendarEvent.attendeeContacts`, `Email.attachmentFiles`
- Conversion helpers from `JsonValue` to typed GraphQL structs
- Schema builder with storage and message bus in context
- Playground handler (debug-only)

## Problems Found

### Critical

- **C1: No query depth limit** -- `Schema::build()` at line 1366 does not call `.limit_depth()`. A malicious client can send arbitrarily nested queries (e.g., recursive fragment spreads via aliases) to cause stack overflow or memory exhaustion. The `async-graphql` library provides `.limit_depth(N)` for this purpose.

- **C2: No query complexity limit** -- No `.limit_complexity()` is set on the schema. A single query can request all seven collections with all fields, nested resolvers, and maximum pagination, causing expensive database operations. The `async-graphql` library provides `.limit_complexity(N)`.

- **C3: No authentication at the GraphQL layer** -- The `graphql_handler` in `routes/graphql.rs` does not extract or verify any user identity. The mutations (`createRecord`, `updateRecord`, `deleteRecord`) accept any request and operate on the `"core"` plugin namespace with no authorization check. While the Axum middleware layer applies `auth_middleware` to all routes (line 658 of main.rs), the identity is not propagated into the async-graphql context -- resolvers cannot access the authenticated user to perform per-field or per-collection authorization.

- **C4: No collection validation on mutations** -- `createRecord`, `updateRecord`, and `deleteRecord` accept an arbitrary `collection` string parameter. There is no allowlist or validation. A client can target internal collections, system tables, or nonexistent collections. The storage layer may or may not reject these, but the GraphQL layer should validate at the boundary.

- **C5: Introspection enabled in production** -- The schema does not call `.disable_introspection()` for production builds. The playground is gated behind `cfg!(debug_assertions)`, but introspection queries (`__schema`, `__type`) work in all builds. This exposes the complete schema to unauthenticated attackers.

### Major

- **M1: No batch query limit** -- async-graphql supports batch requests (multiple operations in a single HTTP POST). There is no limit on the number of operations per batch, allowing a single request to execute hundreds of queries.

- **M2: transport-graphql `start()` is a no-op** -- `GraphqlTransport::start()` logs that the transport started but does not bind a listener. The transport registry will consider GraphQL running when it is not actually serving requests. This is misleading and could mask configuration errors.

- **M3: Nested resolvers vulnerable to N+1 at scale** -- `attendee_contacts` (line 203) and `attachment_files` (line 362) use OR-filter queries to batch-fetch related records within a single parent. This avoids N+1 for a single parent, but when a list of events is returned (e.g., 50 events with attendees), each event's resolver fires a separate database query. There is no DataLoader to deduplicate and batch across parents. With 50 events, this produces 50 separate database queries.

- **M4: Subscription has no connection limit or authentication** -- `SubscriptionRoot::record_changes` opens a `BroadcastStream` receiver with no per-connection resource limit. An attacker could open hundreds of WebSocket subscription connections to exhaust server memory (each `BroadcastStream` buffers messages).

- **M5: `unwrap()` calls in types.rs can panic** -- `serde_json::to_value(body).unwrap()` at lines 98 and 119 of `types.rs` will panic if serialization fails. While unlikely for these simple types, this violates the principle that library code should not panic.

- **M6: Record conversion functions silently default on missing/malformed data** -- All `record_to_*` functions use `unwrap_or_default()` and `unwrap_or_else(Utc::now)` for required fields. If a record has a missing `id` or `title`, the resolver returns an empty string rather than an error. This masks data corruption silently.

- **M7: Workflow translation hardcodes "graphql.query" for mutations** -- `translate_request` in `types.rs` sets `workflow: "graphql.query"` regardless of whether the request is a query or mutation. If the workflow engine routes based on this value, mutations will be misrouted.

### Minor

- **m1: `QueryFailed` severity is `Retryable`** -- Most query failures (syntax errors, validation errors) are permanent. Only transient backend failures should be retryable. This miscategorization could cause callers to retry requests that will never succeed.

- **m2: `json_type_to_graphql` passes unknown types through** -- Unknown JSON Schema types like `"object"` or `"array"` pass through verbatim, producing invalid GraphQL scalar names.

- **m3: `to_pascal_case` does not handle hyphens** -- Collection names with hyphens (e.g., `"my-collection"`) are not converted; only underscores are treated as word boundaries. This will produce `"my-collection"` instead of `"MyCollection"`.

- **m4: Generated schema types are dead code** -- `config.rs` generates `GeneratedGraphqlType` descriptors that are never consumed by the actual schema builder. The production schema in `routes/graphql.rs` uses hand-written types instead.

- **m5: `into_graphql_response` in handlers/mod.rs is unused** -- The function is public but never called from any module.

- **m6: No input size validation on GraphQL query string** -- Neither the transport crate handler nor the production handler validates the length of the incoming `query` string. An extremely large query string (multi-MB) could consume excessive parsing resources.

- **m7: `RecordInput.data` is a JSON string, not a JSON object** -- Mutations require the client to send `data` as a JSON-encoded string (double-serialized). This is ergonomically poor and error-prone. A `JSON` scalar or `async_graphql::Json<Value>` input would be more idiomatic.

- **m8: Filter operator string is not validated** -- `ComparisonFilterInput.operator` accepts an arbitrary string. Invalid operators are silently dropped (via `filter_map` in `convert_filter`), making the query succeed but ignore the filter condition. The client receives no feedback that their filter was ignored.

- **m9: `default_error_message` match is non-exhaustive by design** -- Uses `_ => "Unknown error"` which suppresses compiler warnings when new `WorkflowStatus` variants are added.

- **m10: Pagination `limit` cap at 1000 is only enforced at the GraphQL input layer** -- The `convert_pagination` function caps limit at 1000 via `.min(1000)`, but the nested resolvers (`attendee_contacts`, `attachment_files`) use `self.attendees.len() as u32` directly, which could exceed 1000 if a record has thousands of attendees.

## Recommendations

1. **Add query depth and complexity limits** -- Call `.limit_depth(10)` and `.limit_complexity(1000)` (or appropriate values) on `Schema::build()` in `build_schema()`. This is a one-line fix for critical security issues C1 and C2.

2. **Disable introspection in production** -- Add `.disable_introspection()` to the schema builder when not in debug mode, or gate it behind a configuration flag. Alternatively, require authentication for introspection queries.

3. **Propagate authenticated identity into GraphQL context** -- Extract the identity from the Axum request extensions (set by `auth_middleware`) and inject it into the async-graphql context via `.data(identity)` so that resolvers can perform authorization checks.

4. **Validate collection names on mutations** -- Add an allowlist of valid collection names (the seven CDM collections) and reject mutations targeting unknown collections at the GraphQL layer before reaching storage.

5. **Implement DataLoader for nested resolvers** -- Replace the per-parent OR-filter queries in `attendee_contacts` and `attachment_files` with async-graphql DataLoaders that batch across all parents in a single query.

6. **Add batch query limits** -- Configure `async-graphql-axum` to reject batch requests exceeding a reasonable limit (e.g., 10 operations per batch).

7. **Cap subscription connections** -- Add per-IP or per-user limits on concurrent WebSocket subscription connections to prevent resource exhaustion.

8. **Complete the transport crate or remove it** -- Either wire `GraphqlTransport::start()` to actually serve requests (using the production schema), or remove the crate to avoid the misleading no-op. The current state adds dead code and confusion.

9. **Replace `unwrap()` with proper error handling in types.rs** -- Use `serde_json::to_value(body).unwrap_or_else(|_| ...)` or propagate the error.

10. **Add input size limits** -- Apply Axum's `RequestBodyLimit` layer to the GraphQL route or validate query string length before parsing.
