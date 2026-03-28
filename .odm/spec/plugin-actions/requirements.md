<!--
domain: plugin-actions
updated: 2026-03-28
spec-brief: ./brief.md
-->

# Plugin Actions Requirements

## Requirement 1 — Action Attribute Macro

**User Story:** As a plugin author, I want to annotate a function with `#[plugin_action]` so that it becomes a callable pipeline step without writing Extism boilerplate.

#### Acceptance Criteria

- 1.1. WHEN a function is annotated with `#[plugin_action]` THEN the macro SHALL generate the Extism export wrapper so the function is callable from the host.
- 1.2. WHEN the host invokes an action THEN the macro SHALL deserialise the incoming JSON bytes into a `PipelineMessage`.
- 1.3. WHEN the host invokes an action THEN the macro SHALL construct a `PluginContext` with host function clients and pass it to the function.
- 1.4. WHEN the action returns `Ok(PipelineMessage)` THEN the macro SHALL serialise the message back to JSON bytes for the host.
- 1.5. WHEN the action returns `Err(PluginError)` THEN the macro SHALL map the error into the Extism error protocol.
- 1.6. WHEN the action function panics THEN the macro SHALL catch the panic and return an `InternalError` to the host.

## Requirement 2 — PluginContext

**User Story:** As a plugin author, I want typed access to storage, events, config, and HTTP through `PluginContext` so that I can interact with the host safely.

#### Acceptance Criteria

- 2.1. WHEN a `PluginContext` is constructed THEN it SHALL expose a `storage` field of type `StorageClient` providing document and blob storage operations.
- 2.2. WHEN a `PluginContext` is constructed THEN it SHALL expose an `events` field of type `EventClient` for event emission via `emit_event`.
- 2.3. WHEN a `PluginContext` is constructed THEN it SHALL expose a `config` field of type `ConfigClient` for plugin configuration access via `config_read`.
- 2.4. WHEN a `PluginContext` is constructed THEN it SHALL expose an `http` field of type `HttpClient` for outbound HTTP requests via `http_request`.
- 2.5. WHEN a plugin calls a host function that requires an ungranted capability THEN the client method SHALL return `Err(PluginError::CapabilityDenied)`.

## Requirement 3 — Lifecycle Hooks

**User Story:** As a plugin author, I want to declare `init` and `shutdown` hooks so that my plugin can validate config on load and flush state on teardown.

#### Acceptance Criteria

- 3.1. WHEN a plugin declares an `init` hook in its manifest THEN Core SHALL call `init` once immediately after WASM module instantiation during the Load phase.
- 3.2. WHEN a plugin declares a `shutdown` hook in its manifest THEN Core SHALL call `shutdown` once when shutting down, before unloading the module.
- 3.3. WHEN `init` or `shutdown` is annotated with `#[plugin_hook]` THEN the macro SHALL generate Extism export boilerplate that passes only `PluginContext` and expects `Result<(), PluginError>`.
- 3.4. WHEN `init` returns `Err(PluginError)` THEN Core SHALL fail the plugin load and log the error.
- 3.5. WHEN a plugin does not declare `init` or `shutdown` THEN Core SHALL skip the hook invocation without error.

## Requirement 4 — Action Timeouts

**User Story:** As a workflow author, I want per-action timeouts enforced by the host so that a misbehaving plugin cannot block the pipeline indefinitely.

#### Acceptance Criteria

- 4.1. WHEN an action declares `timeout_ms` in the manifest THEN the Extism host SHALL enforce that timeout at the WASM execution level.
- 4.2. WHEN an action omits `timeout_ms` THEN Core SHALL apply the default timeout defined in the engine configuration.
- 4.3. WHEN an action exceeds its timeout THEN Extism SHALL terminate the WASM execution.
- 4.4. WHEN an action is terminated by timeout THEN the pipeline executor SHALL mark the step as failed and apply the workflow's `on_error` strategy.

## Requirement 5 — Error Handling

**User Story:** As a workflow author, I want actions to report hard errors and soft warnings so that the pipeline executor can apply the correct `on_error` strategy.

#### Acceptance Criteria

- 5.1. WHEN an action returns `Err(PluginError)` THEN the step SHALL fail immediately and the executor SHALL apply the workflow's `on_error` strategy.
- 5.2. WHEN an action returns `Ok(msg)` with entries in `msg.metadata.warnings` THEN the step SHALL succeed but the warnings SHALL be visible to the caller.
- 5.3. WHEN a `PluginError` is created THEN it SHALL use one of the defined variants: `CapabilityDenied`, `NotFound`, `ValidationError`, `StorageError`, `NetworkError`, or `InternalError`.

## Requirement 6 — Connector Pattern

**User Story:** As a connector developer, I want a standard fetch-normalise-store-emit pattern so that all connectors integrate uniformly with the workflow engine.

#### Acceptance Criteria

- 6.1. WHEN a connector's `fetch` action executes THEN it SHALL read configuration via `ctx.config`.
- 6.2. WHEN a connector's `fetch` action executes THEN it SHALL fetch data from the external API via `ctx.http`.
- 6.3. WHEN a connector receives external data THEN it SHALL normalise the data to CDM schemas before storage.
- 6.4. WHEN normalised data is ready THEN the connector SHALL write documents to shared collections via `ctx.storage`.
- 6.5. WHEN the fetch operation completes THEN the connector SHALL emit a completion event (e.g., `connector-email.fetch.completed`) via `ctx.events`.
