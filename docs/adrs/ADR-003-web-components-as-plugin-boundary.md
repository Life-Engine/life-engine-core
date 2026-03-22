# ADR-003: Web Components as the App plugin boundary

## Status
Accepted

## Context

Life Engine's App is a plugin-driven shell: all user-facing features are provided by plugins, not the shell itself. Plugins are authored by first-party and third-party developers and installed by the user at runtime. This requires a clear, enforceable boundary between the shell and each plugin so that:

- Plugins cannot access the DOM of the shell or of other plugins.
- Plugins cannot observe or modify shell state through global JavaScript objects.
- CSS from one plugin cannot affect another plugin or the shell.
- The shell can load, unload, and update plugins without page refresh.
- Plugin authors can use any JavaScript framework or library they choose.

The boundary mechanism must be enforced by the browser/webview itself, not by convention or by auditing plugin source code. Third-party plugins cannot be trusted to self-police.

The mechanism must also be a web standard, not a proprietary abstraction. Life Engine is open source and plugin authors should be able to use standard tooling, documentation, and skills. Depending on a proprietary sandbox layer would create a long-term maintenance burden and reduce the plugin author ecosystem.

## Decision

App plugins are Web Components — custom HTML elements registered in the browser's custom element registry. Each plugin's UI is rendered inside one or more custom elements whose internals are enclosed in a closed Shadow DOM. Closed (not open) Shadow DOM is used so that external JavaScript cannot traverse into the plugin's shadow root via `element.shadowRoot`.

The shell places plugin custom elements inside a `<plugin-container>` that manages lifecycle events, passes props via HTML attributes, and relays Shell API calls through a proxy that enforces manifest-declared capabilities. Plugin-to-shell communication uses `CustomEvent` (dispatched upward) and a `postMessage`-style bridge provided by the SDK. Plugin-to-plugin communication is not permitted directly; it must go through canonical data collections in the shell.

## Consequences

Positive consequences:

- Style encapsulation is enforced by the Shadow DOM at the browser level. No CSS leakage between plugins or between plugins and the shell.
- Custom elements are a platform standard. Plugin authors can use any framework that compiles to Web Components (Lit, Svelte, Angular, vanilla JS) or none at all.
- The closed Shadow DOM prevents external JavaScript from inspecting or mutating plugin internals at runtime.
- Custom elements have a defined lifecycle (`connectedCallback`, `disconnectedCallback`, `attributeChangedCallback`) that the shell can use to manage plugin mount and unmount cleanly.
- Shared modules (Lit, React) can be provided by the shell host and declared as dependencies in the plugin manifest, reducing plugin bundle size.
- The Web Component model requires no build-time knowledge of which plugins exist. New plugins are loaded at runtime by registering their custom elements.

Negative consequences:

- Closed Shadow DOM means plugin authors cannot use global stylesheets or CSS custom properties from the host without the shell explicitly forwarding design tokens as CSS variables on the plugin container element. This requires discipline in the shell design system.
- Plugins that need to render portals (tooltips, modals above the shadow root) must use a provided Shell API method rather than creating DOM elements directly. This adds API surface.
- Testing Web Components requires a browser environment or a headless browser test runner. Pure unit testing with jsdom has limited support for Shadow DOM.
- Cross-plugin clipboard and drag-and-drop interactions require explicit shell-mediated coordination rather than direct DOM interaction.

## Alternatives Considered

**iframes** provide the strongest isolation (separate browsing context, separate origin if using `sandbox` attribute) but were rejected because they are heavyweight (each iframe is a full document), cross-frame communication via `postMessage` is verbose and serializes all data, shared UI (e.g., a modal overlay spanning the page) is impossible across frame boundaries, and styling across frame boundaries requires duplicating stylesheets. The performance and UX cost of one iframe per plugin view was considered unacceptable.

**React components as the plugin format** was rejected because it locks the plugin author into React (or a React-compatible framework) and requires the shell to manage React's virtual DOM across plugin boundaries. It also creates a tight coupling between the shell's React version and the plugin's expected version, leading to "dependency hell" as the ecosystem matures. Web Components are framework-agnostic.

**Custom sandboxing built on top of raw JavaScript** (e.g., running plugins in a realm with a proxy-wrapped global) was evaluated. It was rejected because it is difficult to implement correctly, requires ongoing maintenance to close new browser API escape hatches, and provides weaker guarantees than native browser isolation. Any security issue would be a first-party bug rather than a browser bug.
