# ADR-001: Rust for Core backend

## Status
Accepted

## Context

Life Engine's Core backend is responsible for loading and running WASM plugins, managing an encrypted SQLite database, handling concurrent HTTP requests, and acting as a long-lived server process on self-hosted hardware. The language chosen for Core must support all of these responsibilities without introducing a managed runtime that would bloat deployment packages or consume excessive memory on low-end home servers (Raspberry Pi, NAS devices, etc.).

The backend also needed to serve as both a standalone process and an embedded library callable from Tauri's sidecar mode. This dual-mode requirement ruled out languages that assume a long-lived process with a dedicated event loop that cannot be embedded cleanly.

Safety was a core requirement: the process handles encrypted user data and executes third-party plugin code. Memory unsafety vulnerabilities (use-after-free, buffer overflows) are unacceptable in this context. The language must eliminate these categories of bugs at compile time.

Finally, the WASM runtime for plugin isolation (Extism, Wasmtime) has its primary SDK in Rust. Embedding Extism in a non-Rust host is possible but introduces FFI complexity that adds maintenance burden and potential unsafety at the boundary.

## Decision

Rust is the implementation language for the Core backend. The HTTP layer is `axum` (tower-based, async). The async runtime is `tokio`. Database access uses `rusqlite` with the `SQLCipher` feature for encrypted storage. The WASM plugin runtime is `Extism`. TLS is provided by `rustls` (no OpenSSL dependency).

This stack is chosen as a cohesive unit: axum, tokio, and rusqlite are the dominant choices in the Rust ecosystem for their respective roles, have active maintainers, and integrate well with each other. Using Rust throughout eliminates FFI at every internal boundary.

## Consequences

Positive consequences:

- Memory safety is guaranteed by the compiler. An entire class of CVEs is structurally prevented.
- Zero-cost abstractions mean Core can run on constrained hardware without a garbage collector pause or JIT warm-up period.
- First-class Extism SDK means WASM plugin integration is idiomatic, not an FFI workaround.
- Single binary deployment with no runtime dependency. Self-hosters install one file.
- `rustls` eliminates the OpenSSL dependency and its associated CVE surface.
- `cargo` workspace enables sharing types with the Tauri client through `packages/types/`, creating a single source of truth for data models.

Negative consequences:

- Rust has a steeper learning curve than Go or TypeScript for contributors new to the language. Onboarding new contributors takes longer.
- Compile times are longer than Go. Incremental builds are acceptable but clean builds on CI require caching.
- The Rust ecosystem for some domains (e.g., CalDAV parsing) is less mature than Go or Python. First-party connectors may require more implementation work.
- Async Rust's `Send + Sync` bounds, lifetime rules, and the distinction between `async fn` traits and boxed futures add cognitive overhead for contributors unfamiliar with the model.

## Alternatives Considered

**Go** was evaluated as the primary alternative. Go has excellent concurrency primitives, fast compile times, and a mature HTTP and database ecosystem. The main rejections were: Go's type system is structurally weaker than Rust's (no sum types, no exhaustive match), making it harder to enforce "Parse, Don't Validate"; Go has no first-class Extism SDK comparable to Rust's; and Go's garbage collector introduces unpredictable pauses that are undesirable in a long-lived server process on constrained hardware. Go's binary size is also larger than Rust's for equivalent functionality.

**TypeScript/Node.js** was considered for ecosystem familiarity, since many potential contributors know TypeScript. It was rejected because Node.js has high baseline memory usage (~50MB for a minimal process), requires the V8 runtime in the deployment package, and is not suitable for embedded sidecar use. Memory safety is also not enforced at the language level.

**Python** was briefly considered for rapid prototyping but rejected for the same runtime-dependency and performance reasons as Node, and additionally for lack of meaningful WASM integration tooling.
