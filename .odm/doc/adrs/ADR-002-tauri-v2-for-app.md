# ADR-002: Tauri v2 for the desktop and mobile client

## Status
Accepted

## Context

Life Engine's App component is a cross-platform client that must run on macOS, Windows, Linux, and — in a later phase — iOS and Android. It renders a plugin-driven UI, communicates with Core over a local REST API (sidecar mode) or a remote HTTPS endpoint (server mode), and reads and writes a local SQLite database for offline-first operation.

The framework must support embedding a Rust binary as a sidecar process so the user does not need to run Core separately. This sidecar mode is the primary distribution path for non-technical users. The framework also must support Web Components as the plugin rendering model: plugins are custom elements that render in the webview with Shadow DOM isolation.

Bundle size and memory footprint matter. Life Engine targets users who value self-sovereignty and are likely to be running other services on the same machine. A framework that ships 200MB of Chromium for every user is unacceptable.

The Rust-based backend in Tauri's `src-tauri/` layer allows direct integration with Core's shared types (`packages/types/`) without a serialization boundary. This also enables calling Core's Rust libraries directly from Tauri commands without going through the REST API.

## Decision

Tauri v2 is used for the App client. The Rust `src-tauri/` layer manages the application lifecycle, native system integration (menus, tray, notifications), sidecar process management, and exposes Tauri commands as the bridge between the webview and native capabilities. The webview renders the plugin UI using Web Components. Vite is used for the JS/TS build pipeline.

Tauri v2 specifically is required over v1 because v2 adds mobile (iOS and Android) targets, a new capability system with per-window permissions, and a significantly improved plugin API. Life Engine's Phase 4 mobile target depends on v2's mobile support.

## Consequences

Positive consequences:

- Dramatically smaller bundle size compared to Electron. Tauri uses the system webview (WebKit on macOS/iOS, WebView2 on Windows, WebKitGTK on Linux) rather than bundling Chromium.
- Lower memory footprint at runtime. No separate Chromium process per window.
- Sidecar process management is a first-class Tauri feature (`tauri-plugin-shell`). Core can be bundled as a sidecar binary with lifecycle tied to the App process.
- Rust in `src-tauri/` shares types with Core through `packages/types/`, preserving the single source of truth for data models.
- v2 mobile targets enable the Phase 4 iOS and Android clients without a framework change.
- Tauri's capability system (per-window permissions on IPC commands) aligns with the Principle of Least Privilege for plugins.

Negative consequences:

- System webview rendering differences between platforms (Safari/WebKit, Chrome/Blink, Firefox/Gecko) require cross-browser testing and occasional workarounds.
- WebView2 on Windows requires a separate install on older Windows versions, though it is pre-installed on Windows 11.
- The Tauri ecosystem and plugin library is smaller than Electron's. Some native integrations require custom Rust plugin authoring.
- v2's API is not fully stable at Phase 0 time; breaking changes before 2.0 stable required tracking the beta.

## Alternatives Considered

**Electron** is the most widely used cross-platform desktop framework and has the largest ecosystem. It was rejected because it bundles a full Chromium instance, resulting in 150–200MB binaries and 100–200MB of baseline RAM usage. This conflicts with the goal of running on low-resource hardware alongside Core and other services. Electron also does not share Rust code with Core, requiring a separate serialization boundary for all shared types.

**Flutter** was evaluated for its strong mobile story and single codebase across six platforms. It was rejected because Flutter's ecosystem is Dart-centric, not web-native. Web Components are not a natural primitive in Flutter. Bridging the plugin model to Dart widgets would require reimplementing the entire plugin rendering pipeline in a different paradigm. Additionally, Flutter's web target requires compiling Dart to JavaScript, adding build complexity.

**Native per-platform development** (SwiftUI, WinUI, GTK) was considered and rejected for obvious resource reasons: maintaining four separate codebases (macOS, Windows, Linux, mobile) is not viable for a small team. Even a single-platform native implementation would consume the majority of Phase 1's engineering capacity.

**NeutralinoJS** was briefly considered as a lighter Electron alternative. It was rejected because its plugin ecosystem and Rust integration story were immature at evaluation time, and it lacks mobile support.
