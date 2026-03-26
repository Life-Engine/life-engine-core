# ADR-007: Extism for WASM plugin isolation in Core

## Status
Accepted

## Context

Core is a plugin-driven orchestrator: all business logic (connectors, processing, search indexing) lives in plugins, not in Core itself. Plugins will be authored by first-party and third-party developers and installed by users. Plugins must run within a security boundary that:

- Prevents a compromised or malicious plugin from accessing the host file system, environment variables, or network resources beyond what it has declared in its manifest.
- Prevents a buggy plugin from corrupting Core's memory or crashing the host process.
- Allows Core to expose a controlled set of host functions (database access, HTTP outbound, logging) that plugins call through a typed interface.
- Supports plugins written in any language that compiles to WebAssembly (Rust, Go, AssemblyScript, etc.).
- Is compatible with all deployment modes — plugins run inside the Core process, not as separate processes.

The Phase 1 implementation uses native Rust traits (not WASM) because the plugin contract needs to stabilise through real usage before the isolation boundary is introduced. However, the design must be compatible with WASM migration in Phase 4. The WASM runtime chosen now must be capable of hosting the eventual Phase 4 isolation layer.

## Decision

Extism is adopted as the WASM runtime for Core plugins in Phase 4. Extism is a WASM plugin framework built on top of Wasmtime that provides:

- A `Plugin` type wrapping a WASM module with structured function call and return value serialisation.
- Host function registration via the `host_fn!` macro, allowing Core to expose typed Rust functions that plugins call across the WASM boundary.
- PDK (Plugin Development Kit) libraries for Rust, Go, Python, TypeScript, and other languages, so plugin authors work with idiomatic code rather than raw WASM memory.
- Per-call memory isolation: each plugin function call operates on a fresh memory view, preventing inter-call data leakage.

In Phase 1, plugins implement the `CorePlugin` Rust trait and are compiled into Core's binary. The trait interface is intentionally designed to be equivalent to what Extism host functions will expose in Phase 4. Migration in Phase 4 replaces the trait dispatch with Extism plugin loading without changing the host function signatures visible to plugin authors.

## Consequences

Positive consequences:

- WASM sandbox prevents a malicious plugin from performing arbitrary system calls. The only capabilities a plugin has are the host functions Core explicitly registers.
- Extism's PDK means plugin authors write idiomatic Rust (or Go, or TypeScript) rather than hand-writing WASM imports and memory management.
- Multi-language support widens the plugin author ecosystem beyond Rust developers.
- Extism's structured serialisation (MessagePack or JSON over the WASM linear memory) means Core and plugins exchange typed values without manual pointer arithmetic.
- Wasmtime (Extism's underlying runtime) is backed by the Bytecode Alliance and is regularly audited. Core benefits from the security work of a well-resourced foundation.
- The Phase 1 trait design is forward-compatible: migrating to Extism in Phase 4 is a Core-internal change that does not break existing plugin source code.

Negative consequences:

- WASM execution has overhead compared to native Rust function calls. Plugin invocations cross the WASM boundary with serialisation on each call. For high-frequency operations (e.g., indexing thousands of emails) this overhead is measurable.
- The WASM sandbox complicates debugging. Stack traces from inside a WASM module are less readable than native Rust backtraces. Extism provides a debug build mode but production debugging remains harder.
- Plugin authors must compile to WASM targets (`wasm32-wasi` or `wasm32-unknown-unknown`). This restricts which crates they can use — crates with native dependencies (ring, OpenSSL) may not compile to WASM without feature flags or replacements.
- Extism is a younger project than Wasmtime. Its API has had breaking changes between versions.

## Alternatives Considered

**Wasmtime directly** was evaluated. Wasmtime is the underlying runtime Extism uses and is the most mature standalone WASM runtime in the Rust ecosystem. It was not chosen as the primary interface because Wasmtime's API requires significant boilerplate for host function registration and memory management that Extism abstracts away. Using Wasmtime directly would mean re-implementing what Extism provides, adding maintenance burden without material benefit.

**Wasmer** is an alternative WASM runtime that competes with Wasmtime. It was evaluated and rejected because it has a smaller ecosystem and community than Wasmtime at evaluation time, and Extism already handles the host function abstraction that would be the primary reason to use Wasmer directly.

**V8 isolates** (as used by Cloudflare Workers) were considered for JavaScript plugin isolation. They were rejected because Core is a Rust binary and embedding V8 in a Rust process is complex and adds ~200MB to the binary size. V8 isolates also only support JavaScript and WebAssembly, not the multi-language plugin ecosystem that Extism's PDK enables. JavaScript plugins are available in the App layer (Web Components) where V8 is already provided by the system webview.
