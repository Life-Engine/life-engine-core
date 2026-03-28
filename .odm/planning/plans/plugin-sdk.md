<!--
project: plugin-sdk
source: .odm/qa/reports/phase-2/plugin-sdk.md
updated: 2026-03-28
-->

# Plugin SDK — QA Remediation Plan

## Plan Overview

This plan addresses all findings from the Phase 2 QA review of the Plugin SDK (`packages/plugin-sdk-rs`). The critical issues are the divergent Capability enums and the two undocumented parallel plugin models. Major issues include incorrect delete capability checking, missing WASM host_call FFI bridge, undocumented statelessness in the register_plugin! macro, and overlapping context types. Minor issues cover DX improvements (error conversions, retryable flags, query builder gaps) and cleanup.

Work packages are ordered by dependency: Capability unification (1.1) must come first as it affects storage and the plugin models. The WASM bridge (1.3) and error/DX improvements (1.4) are independent. Documentation and cleanup (1.5) runs last.

**Source:** .odm/qa/reports/phase-2/plugin-sdk.md

**Progress:** 4 / 5 work packages complete

---

## 1.1 — Capability Enum Unification

> depends: none
> spec: .odm/qa/reports/phase-2/plugin-sdk.md

- [x] Add `CredentialsRead`, `CredentialsWrite`, and `Logging` variants to `traits::Capability` to match the SDK's `types::Capability` [fix]
  <!-- file: packages/traits/src/capability.rs -->
  <!-- purpose: Unify the two Capability enums so runtime enforcement knows about all declared capabilities -->
  <!-- requirements: C-001 (Dual Capability enums) -->
  <!-- leverage: existing traits::Capability enum -->
- [x] Add `Display` impl for `traits::Capability` using colon-separated format (e.g., `storage:doc:read`) to reconcile with WASM host capability strings [fix]
  <!-- file: packages/traits/src/capability.rs -->
  <!-- purpose: Reconcile capability string format between WASM host and native code -->
  <!-- requirements: C-001 -->
  <!-- leverage: existing wasm_guest.rs doc comments as reference -->
- [x] Replace `types::Capability` with a re-export of `traits::Capability` or add explicit bidirectional conversion functions with error handling for unmapped variants [fix]
  <!-- file: packages/plugin-sdk-rs/src/types.rs -->
  <!-- purpose: Eliminate silent divergence between SDK and runtime capability enums -->
  <!-- requirements: C-001 -->
  <!-- leverage: existing types::Capability enum -->
- [x] Update `StorageContext` to use the unified Capability enum consistently [fix]
  <!-- file: packages/plugin-sdk-rs/src/storage.rs -->
  <!-- purpose: Ensure storage capability checks use the same enum as plugin declarations -->
  <!-- requirements: C-001 -->
  <!-- leverage: existing StorageContext capability checks -->
- [x] Write tests verifying that all SDK-declared capabilities map to runtime-enforced capabilities [test]
  <!-- file: packages/plugin-sdk-rs/src/types.rs -->
  <!-- purpose: Prevent future divergence between SDK and runtime capability enums -->
  <!-- requirements: C-001 -->
  <!-- leverage: existing capability tests -->

## 1.2 — Storage Capability and Query Fixes

> depends: 1.1
> spec: .odm/qa/reports/phase-2/plugin-sdk.md

- [x] Fix `StorageContext::delete` to check `Capability::StorageDelete` instead of `Capability::StorageWrite` [fix]
  <!-- file: packages/plugin-sdk-rs/src/storage.rs -->
  <!-- purpose: Correct capability enforcement so write-only plugins cannot delete records -->
  <!-- requirements: M-001 (delete checks StorageWrite not StorageDelete) -->
  <!-- leverage: existing delete method at line 127 -->
- [x] Add `where_not_eq` method to `QueryBuilder` exposing the existing `FilterOp::NotEq` [feature]
  <!-- file: packages/plugin-sdk-rs/src/storage.rs -->
  <!-- purpose: Expose the NotEq filter operation that already exists in the types crate -->
  <!-- requirements: m-007 (no where_not_eq) -->
  <!-- leverage: existing FilterOp::NotEq in types crate -->
- [x] Add default limit of 1000 when `QueryBuilder::execute` is called without explicit `limit()` [fix]
  <!-- file: packages/plugin-sdk-rs/src/storage.rs -->
  <!-- purpose: Prevent unbounded queries when backend doesn't enforce its own limit -->
  <!-- requirements: m-008 (no default limit) -->
  <!-- leverage: existing limit cap logic -->
- [x] Update `MockStorageContext` to use record ID instead of `correlation_id` for update/delete lookups [fix]
  <!-- file: packages/plugin-sdk-rs/src/test/mock_storage.rs -->
  <!-- purpose: Align mock behavior with production storage to prevent false-passing tests -->
  <!-- requirements: m-004 (mock uses correlation_id) -->
  <!-- leverage: existing mock_storage.rs -->
- [x] Add `where_not_eq` to `MockQueryBuilder` to match the real `QueryBuilder` [fix]
  <!-- file: packages/plugin-sdk-rs/src/test/mock_storage.rs -->
  <!-- purpose: Keep mock query builder in sync with real query builder API -->
  <!-- requirements: m-007 -->
  <!-- leverage: existing MockQueryBuilder -->
- [x] Write tests for delete capability enforcement, where_not_eq, default limit, and mock ID behavior [test]
  <!-- file: packages/plugin-sdk-rs/src/storage.rs, packages/plugin-sdk-rs/src/test/mock_storage.rs -->
  <!-- purpose: Verify corrected storage behavior -->
  <!-- requirements: M-001, m-007, m-008, m-004 -->
  <!-- leverage: existing storage tests -->

## 1.3 — WASM Host Call Bridge

> depends: none
> spec: .odm/qa/reports/phase-2/plugin-sdk.md

- [x] Implement `host_call(request: &HostRequest) -> Result<HostResponse, PluginError>` function in `wasm_guest.rs` [feature]
  <!-- file: packages/plugin-sdk-rs/src/wasm_guest.rs -->
  <!-- purpose: Complete the guest-side FFI bridge so WASM plugins can call host functions -->
  <!-- requirements: M-002 (no host_call FFI) -->
  <!-- leverage: existing HostRequest/HostResponse types -->
- [x] Add convenience wrapper functions for common host operations (doc_read, doc_write, emit_event, etc.) [feature]
  <!-- file: packages/plugin-sdk-rs/src/wasm_guest.rs -->
  <!-- purpose: Provide ergonomic API for WASM plugins to call host functions without manual serialization -->
  <!-- requirements: M-002 -->
  <!-- leverage: existing HostRequest variants -->
- [x] Add `Serialize`/`Deserialize` derives to `HttpResponse` for WASM boundary crossing [fix]
  <!-- file: packages/plugin-sdk-rs/src/context.rs -->
  <!-- purpose: Enable HttpResponse to cross the WASM boundary -->
  <!-- requirements: m-006 (HttpResponse not serializable) -->
  <!-- leverage: existing HttpResponse struct -->
- [x] Write tests for host_call serialization round-trip and convenience wrappers [test]
  <!-- file: packages/plugin-sdk-rs/src/wasm_guest.rs -->
  <!-- purpose: Verify WASM host call bridge works correctly -->
  <!-- requirements: M-002 -->
  <!-- leverage: existing HostRequest/HostResponse tests -->

## 1.4 — Error Handling and DX Improvements

> depends: none
> spec: .odm/qa/reports/phase-2/plugin-sdk.md

- [x] Add `From<serde_json::Error>` and `From<std::io::Error>` impls for `PluginError`, mapping to `InternalError` [feature]
  <!-- file: packages/plugin-sdk-rs/src/error.rs -->
  <!-- purpose: Reduce boilerplate error conversion in plugin code -->
  <!-- requirements: m-002 (no From impls) -->
  <!-- leverage: existing PluginError enum -->
- [x] Add `PluginError::from_anyhow(e: anyhow::Error)` convenience method [feature]
  <!-- file: packages/plugin-sdk-rs/src/error.rs -->
  <!-- purpose: Provide ergonomic anyhow error conversion for plugin authors -->
  <!-- requirements: m-002 -->
  <!-- leverage: existing PluginError enum -->
- [x] Add `fn is_retryable(&self) -> bool` method to `PluginError` [feature]
  <!-- file: packages/plugin-sdk-rs/src/error.rs -->
  <!-- purpose: Enable pipeline executor to determine retry behavior without matching on variant names -->
  <!-- requirements: m-003 (no retryable indicator) -->
  <!-- leverage: existing PluginError variants -->
- [x] Add optional jitter to `RetryState` exponential backoff [feature]
  <!-- file: packages/plugin-sdk-rs/src/retry.rs -->
  <!-- purpose: Prevent thundering herd when multiple plugins retry simultaneously -->
  <!-- requirements: m-005 (no jitter) -->
  <!-- leverage: existing RetryState implementation -->
- [x] Make `RetryState` fields private with getter methods, keeping a public constructor [refactor]
  <!-- file: packages/plugin-sdk-rs/src/retry.rs -->
  <!-- purpose: Prevent plugins from bypassing record_failure/record_success by directly mutating fields -->
  <!-- requirements: m-009 (public fields) -->
  <!-- leverage: existing RetryState struct -->
- [x] Add `RetryState` to the SDK prelude [feature]
  <!-- file: packages/plugin-sdk-rs/src/lib.rs -->
  <!-- purpose: Make retry state easily accessible for connector plugins -->
  <!-- requirements: m-prelude-missing -->
  <!-- leverage: existing prelude -->
- [x] Write tests for From impls, is_retryable, jitter, and RetryState encapsulation [test]
  <!-- file: packages/plugin-sdk-rs/src/error.rs, packages/plugin-sdk-rs/src/retry.rs -->
  <!-- purpose: Verify error conversion, retry classification, and encapsulation -->
  <!-- requirements: m-002, m-003, m-005, m-009 -->
  <!-- leverage: existing error and retry tests -->

## 1.5 — Documentation and Cleanup

> depends: 1.1, 1.3
> spec: .odm/qa/reports/phase-2/plugin-sdk.md

- [ ] Add crate-level documentation section explaining when to use `CorePlugin` vs `Plugin`, which context type each uses, and the planned convergence path [docs]
  <!-- file: packages/plugin-sdk-rs/src/lib.rs -->
  <!-- purpose: Guide plugin authors to the correct plugin model for their use case -->
  <!-- requirements: C-002 (two plugin models without migration path) -->
  <!-- leverage: existing crate-level docs -->
- [ ] Add prominent doc comments to `register_plugin!` macro explaining statelessness and per-call instantiation [docs]
  <!-- file: packages/plugin-sdk-rs/src/macros.rs -->
  <!-- purpose: Prevent plugin authors from expecting state persistence across WASM calls -->
  <!-- requirements: M-003 (register_plugin creates new instance per call) -->
  <!-- leverage: existing macro docs -->
- [ ] Document the relationship between `PluginContext` and `ActionContext` [docs]
  <!-- file: packages/plugin-sdk-rs/src/types.rs, packages/plugin-sdk-rs/src/context.rs -->
  <!-- purpose: Clarify which context type is used by which plugin model -->
  <!-- requirements: M-004 (overlapping contexts) -->
  <!-- leverage: existing context types -->
- [ ] Document the relationship between `CredentialStore` and `CredentialAccess` traits [docs]
  <!-- file: packages/plugin-sdk-rs/src/credential_store.rs -->
  <!-- purpose: Clarify layering: CredentialStore is Core's impl, CredentialAccess is the plugin-facing interface -->
  <!-- requirements: m-credential-overlap -->
  <!-- leverage: existing trait definitions -->
- [ ] Update or remove stale "Phase 1" comment in `PluginRoute` [fix]
  <!-- file: packages/plugin-sdk-rs/src/types.rs -->
  <!-- purpose: Remove outdated TODO reference that creates confusion about project state -->
  <!-- requirements: m-001 (stale comment) -->
  <!-- leverage: none -->
- [ ] Add or verify `.cargo/config.toml` referenced in crate-level docs, or remove the reference [fix]
  <!-- file: packages/plugin-sdk-rs/src/lib.rs -->
  <!-- purpose: Fix documentation reference to potentially non-existent file -->
  <!-- requirements: m-010 (referenced config may not exist) -->
  <!-- leverage: none -->
