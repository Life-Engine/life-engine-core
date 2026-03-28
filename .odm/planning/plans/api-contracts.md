<!--
project: life-engine-core
specs: api-contracts (from QA)
updated: 2026-03-28
-->

# API Contract Consistency Plan

## Plan Overview

This plan addresses 25 contract breaks identified in the phase-4 API contract consistency review. The fixes are organized into 7 work packages spanning 3 priority tiers: immediate (blocking integration), short-term (required for end-to-end workflows), and medium-term (contract hardening). The most critical issues are dual type definitions that cross crate boundaries via trait objects and JSON serialization, causing runtime failures invisible to the compiler.

**Progress:** 7 / 7 work packages complete

---

## 1.1 — Unify Capability Enum
> depends: none
> spec: .odm/qa/reports/phase-4/api-contracts.md

- [x] Add `CredentialsRead`, `CredentialsWrite`, `Logging` variants to `traits::Capability` enum [critical-fix]
  <!-- file: packages/traits/src/lib.rs -->
  <!-- purpose: Ensure all SDK-declared capabilities have runtime enforcement counterparts (CB-1) -->
  <!-- requirements: CB-1, CB-18 -->
  <!-- leverage: existing Capability enum with Display/FromStr -->
- [x] Add `from_str` mappings for the three new capabilities using colon-separated format [critical-fix]
  <!-- file: packages/traits/src/lib.rs -->
  <!-- purpose: Allow manifest parser to recognize credentials:read, credentials:write, logging capabilities (CB-17, CB-18) -->
  <!-- requirements: CB-17, CB-18 -->
  <!-- leverage: existing FromStr implementation pattern -->
- [x] Remove `plugin_sdk_rs::types::Capability` enum and re-export `traits::Capability` directly [critical-fix]
  <!-- file: packages/plugin-sdk-rs/src/types.rs -->
  <!-- purpose: Eliminate the dual-enum problem by having a single source of truth (CB-3) -->
  <!-- requirements: CB-1, CB-3 -->
  <!-- leverage: SDK already imports traits crate -->
- [x] Remove the `WasmCapability` alias from the SDK prelude [cleanup]
  <!-- file: packages/plugin-sdk-rs/src/lib.rs -->
  <!-- purpose: Eliminate confusing re-export now that there is only one Capability type (CB-3) -->
  <!-- requirements: CB-3 -->
  <!-- leverage: none -->
- [x] Update `CorePlugin::capabilities()` return type to `Vec<traits::Capability>` [critical-fix]
  <!-- file: packages/plugin-sdk-rs/src/lib.rs -->
  <!-- purpose: Ensure plugin capability declarations use the unified enum (CB-3) -->
  <!-- requirements: CB-3 -->
  <!-- leverage: existing CorePlugin trait -->
- [x] Update `StorageContext::new()` to accept the unified `traits::Capability` set without manual mapping [fix]
  <!-- file: packages/plugin-sdk-rs/src/storage.rs -->
  <!-- purpose: Allow CorePlugin plugins to construct StorageContext from their declared capabilities (CB-22) -->
  <!-- requirements: CB-22 -->
  <!-- leverage: existing StorageContext constructor -->

## 1.2 — Fix Identity Type Mismatch
> depends: none
> spec: .odm/qa/reports/phase-4/api-contracts.md

- [x] Convert `AuthIdentity` directly to `life_engine_types::identity::Identity` in auth middleware [critical-fix]
  <!-- file: packages/transport-rest/src/middleware/auth.rs -->
  <!-- purpose: Ensure auth middleware inserts the same Identity type that handlers extract (CB-4) -->
  <!-- requirements: CB-4, CB-5 -->
  <!-- leverage: existing AuthIdentity conversion code -->
- [x] Map `user_id` to `subject`, `provider` to `issuer`, `scopes` to `claims` JSON array value [critical-fix]
  <!-- file: packages/transport-rest/src/middleware/auth.rs -->
  <!-- purpose: Provide correct field mapping between auth and canonical identity shapes (CB-5) -->
  <!-- requirements: CB-5 -->
  <!-- leverage: existing field values from AuthIdentity -->
- [x] Remove the private `middleware::auth::Identity` struct [cleanup]
  <!-- file: packages/transport-rest/src/middleware/auth.rs -->
  <!-- purpose: Eliminate the duplicate Identity type that causes Axum extension mismatch (CB-4) -->
  <!-- requirements: CB-4 -->
  <!-- leverage: none -->
- [x] Insert a guest `Identity` (subject: "anonymous") on public route bypass [fix]
  <!-- file: packages/transport-rest/src/middleware/auth.rs -->
  <!-- purpose: Ensure handlers can unconditionally extract Extension<Identity> on public routes (CB-4) -->
  <!-- requirements: CB-4 -->
  <!-- leverage: existing Identity::guest() constructor -->

## 1.3 — Fix SDK Delete Capability Check
> depends: none
> spec: .odm/qa/reports/phase-4/api-contracts.md

- [x] Change `StorageContext::delete()` to check `Capability::StorageDelete` instead of `Capability::StorageWrite` [critical-fix]
  <!-- file: packages/plugin-sdk-rs/src/storage.rs -->
  <!-- purpose: Align SDK-side capability check with host function enforcement (CB-2) -->
  <!-- requirements: CB-2 -->
  <!-- leverage: existing require() call pattern -->

## 1.4 — Standardize Event Field Naming
> depends: none
> spec: .odm/qa/reports/phase-4/api-contracts.md

- [x] Rename `EmitRequest.event_name` to `event_type` in host events module [fix]
  <!-- file: packages/plugin-system/src/host_functions/events.rs -->
  <!-- purpose: Align host function field naming with SDK CoreEvent and WASM HostRequest (CB-20) -->
  <!-- requirements: CB-20 -->
  <!-- leverage: existing EmitRequest struct -->
- [x] Add `timestamp` field to enriched event payload in `host_events_emit` [fix]
  <!-- file: packages/plugin-system/src/host_functions/events.rs -->
  <!-- purpose: Ensure enriched events deserialize correctly into SDK CoreEvent type (CB-21) -->
  <!-- requirements: CB-21 -->
  <!-- leverage: existing payload enrichment code -->

## 2.1 — Bridge PluginError Types and Consolidate TriggerContext
> depends: 1.1
> spec: .odm/qa/reports/phase-4/api-contracts.md

- [x] Implement `EngineError` for `plugin_sdk_rs::error::PluginError` with mapped error codes [fix]
  <!-- file: packages/plugin-sdk-rs/src/error.rs -->
  <!-- purpose: Allow SDK errors to participate in the structured error system (CB-7) -->
  <!-- requirements: CB-7 -->
  <!-- leverage: existing EngineError trait pattern from plugin-system -->
- [x] Define shared error serialization format for the WASM boundary using `serde(tag = "code")` [fix]
  <!-- file: packages/plugin-system/src/error.rs -->
  <!-- purpose: Align host-side error deserialization with SDK serialization format (CB-6) -->
  <!-- requirements: CB-6 -->
  <!-- leverage: SDK's existing serde(tag = "code") pattern -->
- [x] Replace `PluginOutput::Error` severity `String` with `Severity` enum or validated parsing [fix]
  <!-- file: packages/plugin-sdk-rs/src/lib.rs -->
  <!-- purpose: Prevent runtime classification failures from malformed severity strings (CB-8) -->
  <!-- requirements: CB-8 -->
  <!-- leverage: existing Severity enum in traits -->
- [x] Remove `types::identity::TriggerContext` and use workflow engine's version as source of truth [fix]
  <!-- file: packages/types/src/identity.rs -->
  <!-- purpose: Eliminate stale, unused TriggerContext that diverges from the runtime version (CB-9) -->
  <!-- requirements: CB-9 -->
  <!-- leverage: workflow_engine::types::TriggerContext already in use -->

## 2.2 — Wire REST Router to Handlers and Add PATCH Support
> depends: none
> spec: .odm/qa/reports/phase-4/api-contracts.md

- [x] Replace router inline debug closures with calls to `handle_with_body`/`handle_without_body` [fix]
  <!-- file: packages/transport-rest/src/router/mod.rs -->
  <!-- purpose: Connect the router to the proper handler implementations (CB-12) -->
  <!-- requirements: CB-12 -->
  <!-- leverage: existing handlers/mod.rs functions -->
- [x] Add `routing::patch` handler to the router method match [fix]
  <!-- file: packages/transport-rest/src/router/mod.rs -->
  <!-- purpose: Prevent PATCH-configured routes from being silently dropped (CB-11) -->
  <!-- requirements: CB-11 -->
  <!-- leverage: existing GET/POST/PUT/DELETE match arms -->
- [x] Add HTTP method validation in route config to reject unknown methods [fix]
  <!-- file: packages/transport-rest/src/router/mod.rs -->
  <!-- purpose: Reject invalid method strings like "FROBNICATE" at config time (CB-13) -->
  <!-- requirements: CB-13 -->
  <!-- leverage: existing match arm pattern -->
- [x] Validate GraphQL mutation collection names against CDM allowlist [fix]
  <!-- file: packages/transport-graphql/src/handlers/mod.rs -->
  <!-- purpose: Prevent mutations targeting internal tables or nonexistent collections (CB-15) -->
  <!-- requirements: CB-15 -->
  <!-- leverage: existing CDM collection definitions -->
- [x] Fix `translate_request` to use distinct workflow names for queries vs mutations [fix]
  <!-- file: packages/transport-graphql/src/lib.rs -->
  <!-- purpose: Prevent mutations from being misrouted as "graphql.query" (CB-14) -->
  <!-- requirements: CB-14 -->
  <!-- leverage: existing translate_request function -->

## 3.1 — Contract Hardening
> depends: 1.1, 2.1
> spec: .odm/qa/reports/phase-4/api-contracts.md

- [x] Rename `traits::index_hints::SchemaError` to `IndexHintError` [cleanup]
  <!-- file: packages/traits/src/index_hints.rs -->
  <!-- purpose: Resolve public name collision between two SchemaError types (CB-10) -->
  <!-- requirements: CB-10 -->
  <!-- leverage: none -->
- [x] Consolidate REST config types: remove `RestTransportConfig` and use `ListenerConfig` throughout [cleanup]
  <!-- file: packages/transport-rest/src/lib.rs -->
  <!-- purpose: Eliminate three competing address/port config shapes (CB-24) -->
  <!-- requirements: CB-24, CB-25 -->
  <!-- leverage: existing ListenerConfig -->
- [x] Add `From<StorageError> for plugin_system::PluginError` conversion [fix]
  <!-- file: packages/plugin-system/src/error.rs -->
  <!-- purpose: Make error propagation across storage and plugin boundaries type-safe (CB-6) -->
  <!-- requirements: CB-6 -->
  <!-- leverage: existing error types -->
- [x] Validate `events.subscribe` declarations at runtime in `host_events_subscribe` [fix]
  <!-- file: packages/plugin-system/src/host_functions/events.rs -->
  <!-- purpose: Enforce subscription declarations matching the existing emit enforcement (CB-19) -->
  <!-- requirements: CB-19 -->
  <!-- leverage: existing declared_emit_events validation pattern -->
- [x] Remove or integrate dead `GeneratedGraphqlType` code in transport-graphql config [cleanup]
  <!-- file: packages/transport-graphql/src/config.rs -->
  <!-- purpose: Remove dead code that generates invalid GraphQL scalar names (CB-16) -->
  <!-- requirements: CB-16 -->
  <!-- leverage: none -->
- [x] Fix `Transport::start()` signature to not require `toml::Value` parameter [cleanup]
  <!-- file: packages/traits/src/lib.rs -->
  <!-- purpose: Decouple transport trait from TOML format dependency (CB-25) -->
  <!-- requirements: CB-25 -->
  <!-- leverage: existing from_config() already handles parsing -->
