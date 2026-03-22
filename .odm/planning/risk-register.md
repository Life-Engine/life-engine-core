---
title: "Life Engine — Risk Register"
tags:
  - life-engine
  - planning
  - risk
created: 2026-03-21
---

# Life Engine — Risk Register

This register tracks identified risks across technical, product, security, and organisational categories. Each risk is assessed for likelihood and impact, paired with a mitigation strategy, and assigned to the phase where it is most relevant. Risks are reviewed at each phase boundary and updated as the project evolves.

## Technical Risks

- **IMAP protocol inconsistency** — Likelihood: High. Impact: Medium. Different providers implement IMAP differently, leading to edge cases in parsing, folder naming, and flag handling. Mitigation: start with well-tested providers (Gmail, Fastmail, Outlook), maintain a provider compatibility matrix, and use a battle-tested IMAP library. Owner: Phase 1. Status: Open.

- **Tauri v2 mobile maturity** — Likelihood: Medium. Impact: High. Tauri v2 mobile support is relatively new and may have stability or API coverage gaps. Mitigation: defer mobile to Phase 4, validate the architecture on desktop first, and monitor Tauri v2 mobile stability through their release channels. Owner: Phase 4. Status: Open.

- **PowerSync integration complexity** — Likelihood: Medium. Impact: Medium. PowerSync is designed for specific backend patterns and may not fit cleanly with Life Engine's storage model. Mitigation: start with REST polling sync in Phase 1, introduce PowerSync in Phase 2 or 3, and ensure the SyncAdapter abstraction allows swapping implementations without upstream changes. Owner: Phase 1. Status: Open.

- **WASM performance overhead** — Likelihood: Low. Impact: Medium. WASM plugins may introduce latency compared to native code, especially for data-heavy operations. Mitigation: benchmark WASM vs native in Phase 4, allow first-party plugins to remain native if overhead is unacceptable, and require WASM only for community plugins. Owner: Phase 4. Status: Open.

- **Shadow DOM CSS limitations** — Likelihood: Medium. Impact: Low. Some CSS patterns (inherited properties, certain selectors) do not work across Shadow DOM boundaries, which may limit plugin styling flexibility. Mitigation: CSS custom properties pass through Shadow DOM, the shell design system handles 90% of styling needs, and workarounds are documented for edge cases. Owner: Phase 1. Status: Open.

- **SQLite concurrent write contention** — Likelihood: Low. Impact: Low. SQLite's single-writer model could become a bottleneck under heavy sync loads. Mitigation: WAL mode reduces contention significantly, personal-scale data makes this unlikely in practice, and PostgreSQL is available for power users in Phase 3. Owner: Phase 1. Status: Open.

- **Extism WASM host function limitations** — Likelihood: Medium. Impact: Medium. The Extism host function API may not cover all the capabilities plugins need, requiring workarounds or upstream contributions. Mitigation: design host functions conservatively, add new functions as optional capabilities with versioned negotiation, and engage with the Extism community on gaps. Owner: Phase 4. Status: Open.

## Product Risks

- **Too technical for non-technical users** — Likelihood: High. Impact: High. Self-hosting requirements, encryption passphrases, and IMAP credential entry may overwhelm users who are not technically inclined. Mitigation: sidecar mode hides the server entirely, onboarding wizard guides users through setup step by step, and OAuth is used where possible to avoid raw credential entry. Owner: Phase 1. Status: Open.

- **Plugin ecosystem doesn't materialise** — Likelihood: Medium. Impact: High. The community may not build third-party plugins, leaving Life Engine dependent on first-party development. Mitigation: build at least 5 first-party plugins to demonstrate the platform's capability, provide excellent SDK documentation and tooling, and create a "Your First Plugin" tutorial completable in under 30 minutes. Owner: Phase 3. Status: Open.

- **Competing with established tools** — Likelihood: High. Impact: Medium. Gmail, Google Calendar, Apple Notes, and similar tools are free, polished, and deeply integrated into existing ecosystems. Mitigation: do not compete on polish or feature parity. Life Engine's value proposition is unified data under the user's control, cross-source integration, and extensibility through plugins. Owner: All phases. Status: Accepted.

- **Documentation falls behind the software** — Likelihood: High. Impact: High. As a solo developer, writing docs and code simultaneously is difficult. Outdated or missing docs frustrate users and contributors, reducing adoption. Mitigation: auto-generate API and SDK reference from source code so docs update automatically. CI validates that generated docs are current — a PR that changes the API without updating the spec fails the build. Hand-written guides are scoped per phase alongside the features they document. Owner: All phases. Status: Open.

- **Website scope creep** — Likelihood: Medium. Impact: Medium. A marketing site, docs hub, blog, downloads portal, and SDK reference is a lot of surface area for a solo developer. Mitigation: Phase 0 ships a minimal landing page and docs skeleton only. Full docs grow incrementally with each phase. Astro + Starlight handles navigation, search, and layout automatically — content authoring is just MDX files. Do not over-invest in custom components or visual polish before the software exists. Owner: Phase 0. Status: Open.

## Security Risks

- **Plugin supply chain attacks** — Likelihood: Medium. Impact: High. Malicious plugins could exfiltrate user data or compromise system integrity. Mitigation: WASM sandboxing in Phase 4 restricts plugin capabilities, capability enforcement limits access to declared collections and domains, plugin signing prevents tampering, and review tiers provide graduated trust levels. Owner: Phase 4. Status: Open.

- **Credential storage compromise** — Likelihood: Low. Impact: Critical. A breach of the encrypted credential store would expose all connector credentials. Mitigation: SQLCipher encryption protects the database at rest, Argon2id key derivation resists brute-force attacks, separate encryption keys isolate credentials from other data, and audit logging tracks all credential access. Owner: Phase 1. Status: Open.

- **OAuth token leakage** — Likelihood: Low. Impact: High. Refresh tokens could be exposed through application logs, debug output, or insecure storage. Mitigation: credentials are never written to logs, access tokens are held in memory only, refresh tokens are encrypted at rest, and automatic rotation limits the window of exposure. Owner: Phase 1. Status: Open.

## Design Principles Risks

- **Principle drift under deadline pressure** — Likelihood: Medium. Impact: High. Under time pressure, developers may bypass design principles (e.g., hardcoding capabilities instead of declaring them in manifests, duplicating type definitions instead of using the shared package, adding business logic to Core instead of creating a plugin). Mitigation: the review gate checklist explicitly includes [[03 - Projects/Life Engine/Design/Principles|Design Principles]] compliance checks. No work package is complete until every applicable principle has been verified. ADRs document the *why* so decisions aren't re-litigated. Owner: All phases. Status: Open.

- **Over-engineering from principle adherence** — Likelihood: Medium. Impact: Medium. Strict adherence to principles like Separation of Concerns or Open/Closed could lead to unnecessary abstraction layers or premature generalisation. Mitigation: *Finish Before Widening* acts as a counterbalance — build the minimal integrated system first. Principles guide decisions, they do not mandate indirection. If a principle creates more complexity than it resolves at the current scale, document the deviation in an ADR. Owner: All phases. Status: Open.

## Methodology Risks

- **Stitch-generated code quality** — Likelihood: Medium. Impact: Low. AI-generated UI code may not follow project conventions or may include unnecessary complexity. Mitigation: treat Stitch output as a starting point, always refactor to use the design system, and validate with Playwright tests. All Stitch code goes through the same review gate as hand-written code. Owner: All phases. Status: Open.

- **TDD overhead on solo developer** — Likelihood: Medium. Impact: Medium. Strict TDD discipline adds upfront time cost, which may slow initial delivery velocity. Mitigation: TDD prevents regressions that cost more time later, test-first forces cleaner APIs that reduce future refactoring, and the review gate catches issues before they compound. The overhead decreases as the developer builds muscle memory. Owner: All phases. Status: Accepted.

- **Playwright flaky tests in CI** — Likelihood: Medium. Impact: Low. E2E tests against a Tauri WebView may be sensitive to timing, rendering delays, or CI environment differences. Mitigation: use Playwright's built-in auto-waiting and retry mechanisms, avoid hard-coded waits, capture traces on failure for debugging, and quarantine flaky tests for investigation rather than disabling them. Owner: Phase 1. Status: Open.

## Organisational Risks

- **Solo founder burnout** — Likelihood: High. Impact: Critical. Building every component alone is unsustainable over the multi-phase timeline. Mitigation: build in public to attract contributors early, apply for grants (NLnet, Sovereign Tech Fund) after MVP to fund additional developers, and scope each phase ruthlessly to avoid overcommitment. Owner: All phases. Status: Open.

- **Funding gap** — Likelihood: High. Impact: High. Grants require working software to apply, working software requires development time, and development time requires funding. Mitigation: set up Open Collective from day one, create build-in-public content to attract early supporters, and ensure Phase 1 is achievable by a single developer within a reasonable timeframe. Owner: Phase 0. Status: Open.

- **NLnet grant deadline pressure** — Likelihood: Medium. Impact: Medium. The April 1, 2026 NLnet application deadline may pressure rushed architectural decisions or incomplete documentation. Mitigation: Phase 0 and early Phase 1 progress strengthen the application regardless, apply with whatever is complete rather than delaying, and do not compromise architecture for deadline optics. Owner: Phase 0. Status: Open.
