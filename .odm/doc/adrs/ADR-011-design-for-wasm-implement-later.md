# ADR-011: Design for WASM plugin isolation now, implement in Phase 4

## Status
Accepted

## Context

Life Engine's Core plugin system will eventually run third-party plugins in WASM sandboxes for security isolation (ADR-007). However, implementing WASM isolation in Phase 1 would significantly increase implementation complexity before the plugin contract has been validated through real usage.

The core tension is:

- Security: Third-party plugins require sandboxing. Without it, a malicious plugin could access the host's file system, memory, or network arbitrarily.
- Velocity: Implementing the WASM boundary adds compilation steps, serialisation at every host function call, and WASM-specific debugging to every plugin. In Phase 1, where all plugins are first-party and trusted, this overhead slows development without providing any security benefit.
- Design correctness: The WASM boundary must be designed before Phase 1 plugins are written, so that the Rust trait interfaces they implement today are forward-compatible with the Extism host function interface they will call in Phase 4.

The "Finish Before Widening" principle argues against adding Phase 4 infrastructure to Phase 1. But the risk is that a Phase 1 design without WASM in mind could produce a plugin API that cannot be implemented efficiently as WASM host functions, requiring a breaking change in Phase 4.

## Decision

Core plugins in Phase 1 implement a `CorePlugin` Rust trait and are compiled directly into the Core binary. There is no runtime isolation boundary in Phase 1. However, the trait is explicitly designed to mirror the Extism host function contract that will exist in Phase 4: function names, parameter types, and return types are chosen to be serialisable across the WASM boundary without alteration.

Specifically:

- All plugin-to-host function calls pass data as values that implement `serde::Serialize + serde::Deserialize`, not as raw pointers or Rust-specific types.
- Host function signatures avoid Rust-specific types (lifetimes, `Box<dyn Trait>`, closures) that cannot be expressed as WASM imports.
- Phase 1 plugins are compiled and tested in a way that validates their API surface, so that the Phase 4 migration replaces the dispatch mechanism (trait call → Extism function call) without changing the API surface.

The trait design is reviewed against the Extism PDK model before Phase 1 implementation begins to confirm forward-compatibility.

## Consequences

Positive consequences:

- Phase 1 development velocity is not impacted by WASM compilation toolchains, `wasm32-wasi` target configuration, or Extism runtime overhead.
- Plugin authors (first-party in Phase 1) write idiomatic Rust without WASM-specific constraints during the period when the plugin API is being designed. This makes it easier to iterate on the API shape.
- Phase 4 migration is a Core-internal change. Existing plugin source code (`.rs` files) does not change; only the compilation target and host dispatch mechanism change.
- The "design now, implement later" decision is documented in an ADR, so Phase 4 contributors have an explicit mandate to migrate rather than discovering the gap as technical debt.
- Security risk is acceptable in Phase 1 because all Phase 1 plugins are first-party and reviewed as part of the monorepo. No third-party plugin installation is supported until Phase 2 or later.

Negative consequences:

- Phase 1 ships with an unmitigated plugin isolation gap. If Phase 2 third-party plugin support launches before Phase 4 WASM isolation is complete, the isolation gap becomes a real security risk.
- If the Phase 1 trait design does not perfectly anticipate the Extism host function constraints, a breaking change is required at Phase 4. The forward-compatibility review mitigates but does not eliminate this risk.
- Contributors who write Phase 1 plugins must understand and respect the "keep types serialisable" constraint even though it is not enforced by the compiler at Phase 1 time. Code review must catch violations.

## Alternatives Considered

**WASM isolation from Phase 1** was evaluated. This would provide full security from the first plugin. It was rejected because Phase 1's single end-to-end email flow requires writing and iterating on the plugin API rapidly. Every API change requires recompiling to WASM, updating host function registrations, debugging across the WASM boundary, and maintaining two sets of test infrastructure (native and WASM). This overhead would slow Phase 1 to a crawl and the security benefit is zero (all Phase 1 plugins are first-party).

**Never use WASM (permanent Rust trait model)** was considered as a simpler long-term architecture. It was rejected because it makes the third-party plugin ecosystem impossible: compiled-in Rust plugins require trusting plugin authors with arbitrary code execution in Core's process. A malicious plugin compiled into Core is indistinguishable from Core itself. WASM sandboxing is a non-negotiable requirement for third-party plugin safety.
