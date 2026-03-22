# ADR-012: Lit as the recommended framework for App plugin authors

## Status
Accepted

## Context

Life Engine App plugins are Web Components (ADR-003). Plugin authors have the freedom to use any JavaScript framework or library that compiles to Web Components, or no framework at all. However, the project needs a recommended framework to:

- Lower the barrier for new plugin authors. "Use whatever you want" forces every author to evaluate the Web Component framework landscape independently.
- Provide a consistent, well-tested baseline for the plugin scaffolding CLI (`create-life-engine-plugin`) and official documentation.
- Ensure the recommended path produces plugins that are compatible with the shell's shared module system. The shell can provide certain libraries at zero bundle cost to plugins that declare them as manifest dependencies; the recommended framework should be one of those libraries.
- Be small enough that plugins loading it (in cases where they bundle it rather than using the shell-provided version) do not create unacceptable bundle size overhead.

The framework must be purpose-built for Web Components. Recommending a framework that treats Web Components as a secondary output (a compile target rather than its primary model) would mean plugin authors work against their framework rather than with it.

## Decision

Lit is the recommended framework for App plugin authors. Lit is a lightweight library (~5KB minified and gzipped) built specifically for Web Components. Its `LitElement` base class provides reactive properties, declarative templates via tagged template literals, efficient rendering with DOM diffing, and a lifecycle aligned with the Web Components specification. The shell provides Lit as a shared module: plugins that declare `"sharedModules": ["lit"]` in their manifest get Lit at zero bundle cost.

The scaffolding CLI generates a `LitElement`-based plugin as the default. Documentation, tutorials, and first-party plugins all use Lit as the reference implementation.

Alternative frameworks (Svelte, vanilla JS, Angular web components) are fully supported and documented as secondary options. The recommendation is not an exclusion.

## Consequences

Positive consequences:

- Lit is purpose-built for Web Components. Its reactive property system, `@property()` decorators, and lifecycle hooks map directly to the Web Component specification without abstraction.
- At ~5KB gzipped, Lit adds negligible size to plugin bundles. Even if a plugin bundles Lit rather than using the shell-provided version, the overhead is acceptable.
- The shell provides Lit as a shared module, meaning the most common case (multiple plugins using Lit) results in exactly one copy of Lit in memory, not one per plugin.
- Lit is developed by Google's Web Components team and has strong long-term support and alignment with web platform standards.
- TypeScript support is first-class. Lit's decorators work well with TypeScript, and the `plugin-sdk-js` package provides typed wrappers for Shell API access.
- Lit's reactive property system makes it straightforward to respond to capability-gated Shell API events without complex state management.

Negative consequences:

- Lit's tagged template literal syntax (`html\`...\``) is unfamiliar to developers used to JSX or single-file component syntax (Vue, Svelte). The learning curve for JSX-native developers is real.
- Lit's component model is simpler than React's (no hooks, no context, no built-in state management). Complex plugins requiring a sophisticated state management solution need additional libraries (Zustand, XState) that are not provided as shell shared modules by default.
- Recommending Lit implicitly pressures plugin authors toward it even when a different tool would be more appropriate for their use case. The recommendation must be paired with clear documentation that other approaches are equally valid.
- Lit's diffing algorithm is not as battle-tested at large scale as React's virtual DOM. Extremely complex plugin UIs with thousands of reactive nodes may encounter performance limits not present in React.

## Alternatives Considered

**React** is the most widely used UI framework and has the largest community of developers. Building plugins with React (using custom element wrappers like `@lit-labs/react`) was considered as the recommendation. React was rejected as the primary recommendation because React is not natively a Web Components framework. It renders to a React tree, not directly to the DOM, and requires wrapper libraries to interoperate with the Shadow DOM model. This means plugin authors must understand both React and Web Components, rather than one. React's bundle size (~45KB gzipped for React + ReactDOM) also makes it costly to bundle per-plugin.

**Svelte** was a strong contender because it compiles away at build time, producing minimal runtime overhead. Svelte's Web Component support (`customElement: true` in `svelte.config.js`) allows components to be exported as custom elements. Svelte was not chosen as the primary recommendation because its Web Component support is a secondary feature, not the primary use case, leading to documented edge cases with Shadow DOM and reactive stores. Svelte remains a supported option for plugin authors who prefer its syntax.

**Vanilla JavaScript (no framework)** is always available and is documented as an advanced option for authors who want minimal overhead. It was not chosen as the default recommendation because hand-authoring Web Components without a reactive library requires boilerplate that experienced JavaScript developers can write but that creates a high barrier for plugin authors who are not web specialists.
