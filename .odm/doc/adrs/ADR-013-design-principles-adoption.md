# ADR-013: Adoption of 11 governing design principles

## Status
Accepted

## Context

Life Engine is a complex, long-lived open-source project with multiple components (Core, App, plugin SDKs, first-party plugins, documentation site), multiple contributors, and a phased delivery plan spanning years. Without a shared decision-making framework, the project risks accumulating inconsistent architectural choices as different contributors make reasonable-sounding local decisions that conflict at the system level.

Specific failure modes that a principles framework is intended to prevent:

- Business logic creeping into Core rather than plugins, eroding the clean separation that makes the system extensible.
- Plugin API designs that grant more capabilities than necessary, weakening the security model.
- Validation logic scattered through the call stack rather than concentrated at boundaries, leading to inconsistent data states.
- New features added before existing ones are fully integrated, resulting in a wide but shallow codebase.
- Decisions being relitigated by contributors who were not present for the original discussion, wasting time and creating inconsistency.

The principles framework must be specific enough to guide concrete decisions, yet concise enough that contributors can internalise all principles without reference during day-to-day work.

## Decision

Eleven design principles are adopted as the governing framework for all architectural and implementation decisions in Life Engine. These principles are:

- Separation of Concerns — one responsibility per module, layer, and component.
- Architecture Decision Records — document the why behind key decisions in `docs/adrs/`.
- Fail-Fast with Defined States — make invalid states unrepresentable; surface errors immediately.
- Defence in Depth — every layer (transport, auth, storage, credentials, plugins) is independently secure.
- Finish Before Widening — a fully integrated system with fewer features is more valuable than many partial features.
- Principle of Least Privilege — components and plugins access only what they explicitly declare.
- Parse, Don't Validate — use the type system to prevent invalid data at boundaries.
- Open/Closed Principle — open for extension via plugins, closed to modification when features are added.
- Single Source of Truth — one canonical definition for every type, schema, and capability declaration.
- Explicit Over Implicit — declare behaviour in manifests rather than runtime logic.
- The Pit of Success — design the SDK so the easiest path for plugin authors is also the correct one.

Each principle is documented in `.odm/docs/Design/Principles.md` with concrete examples of how it applies to Life Engine's specific architecture. Compliance with the principles is a review gate: pull requests are reviewed against the applicable principles before merge.

## Consequences

Positive consequences:

- Contributors have a shared vocabulary for architectural discussions. Disagreements can be resolved by reference to the principles rather than by seniority or preference.
- The principles create a predictable, consistent architecture that new contributors can learn once and apply everywhere.
- Review gates that check for principles compliance catch architectural drift early, before it accumulates into large-scale technical debt.
- The principles are documented in `.odm/docs/Design/Principles.md`, which means they are versioned in the repository and can evolve via pull request with the same review process as code.
- The principles encode lessons from real architectural failures in comparable systems. They represent accumulated wisdom rather than aspirational ideals.
- ADRs (the second principle) ensure that when other principles conflict, the resolution is documented. The decision log is an asset, not overhead.

Negative consequences:

- Eleven principles is a significant number to internalise. Contributors who do not know the principles well may make decisions that superficially satisfy one principle while violating another.
- Principles can be interpreted narrowly or broadly. "Separation of Concerns" could justify almost any refactor. Review gates must be specific about which principle is at stake and how it applies to prevent the principles from becoming an unfalsifiable trump card.
- The principles are in tension with "Finish Before Widening" when a new feature genuinely requires a departure from existing patterns. These cases require an ADR, which adds process overhead.
- There is no automatic enforcement. The principles are enforced by humans during code review, which means their effectiveness depends on reviewer knowledge and diligence.

## Alternatives Considered

**Ad-hoc decision making** — make decisions as they arise without a formal framework. This was rejected because it reliably produces inconsistent architectures in long-lived projects with multiple contributors. Without principles, each contributor optimises for local correctness rather than systemic consistency. The cost of accumulating architectural inconsistency over multiple phases is high.

**Fewer principles (five or fewer)** was considered to reduce the internalisation burden. A shorter list was rejected because the eleven principles address genuinely distinct concerns: collapsing them would either lose important guidance or merge incompatible ideas into a vague maxim. For example, "Separation of Concerns" and "Open/Closed Principle" address related but distinct failure modes; collapsing them loses precision.

**More principles (twenty or more)** was not considered seriously. The goal is internalisation, not completeness. A 20-item checklist becomes a bureaucratic review form rather than a design philosophy. Eleven is already at the upper bound of what can be held in working memory without reference.

**Adopting an existing framework wholesale** (Domain-Driven Design, Clean Architecture, the Twelve-Factor App) was evaluated. Each existing framework was either too broad (addressing concerns Life Engine does not have) or not specific enough to Life Engine's plugin-driven, local-first, self-hosted context. The eleven principles were assembled specifically to address the failure modes identified during the design of Life Engine's architecture, making them more applicable than a generic framework.
