# ADR-010: Apache 2.0 licence

## Status
Accepted

## Context

Life Engine is an open-source personal data sovereignty platform. The licence choice affects:

- Who can use, modify, and distribute Life Engine and its components.
- Whether commercial products can be built on top of Life Engine.
- Whether Life Engine can depend on libraries under other licences.
- The willingness of companies to contribute to or build on Life Engine.
- The legal protection offered to the project and its contributors regarding software patents.

Life Engine has two distinct categories of downstream users:

- Self-hosters who run Life Engine for personal use. These users should face no restrictions.
- Plugin authors (first-party and third-party) who build on the plugin SDKs and distribute plugins through a registry. These authors may want to build commercial plugins, and Life Engine should not prohibit this.

The project has no current plans to offer a commercial hosted version or to dual-licence components. The goal is maximum openness and adoption.

## Decision

Life Engine is licenced under the Apache Licence 2.0. All code in the monorepo (Core, App, shared types, plugin SDKs, first-party plugins) is Apache 2.0 unless a third-party dependency introduces a more restrictive licence. Third-party dependencies must be reviewed for licence compatibility before inclusion.

Apache 2.0 was chosen over MIT primarily for its explicit patent grant. By contributing to Life Engine, contributors grant a perpetual, worldwide, royalty-free patent licence to all downstream users for their contributions. This protects self-hosters, plugin authors, and commercial users from patent claims by contributors.

## Consequences

Positive consequences:

- Permissive licence enables commercial use, including commercial plugins and hosted services built on Core. This is intentional: the goal is a thriving plugin ecosystem, not restriction.
- The patent grant is essential for enterprise adoption. Companies contributing to open-source projects or building products on them require patent retaliation protection.
- Apache 2.0 is compatible with the vast majority of open-source licences used by Rust and JavaScript dependencies (MIT, BSD-2, BSD-3, ISC).
- Apache 2.0 is one of the three licences recommended by the Open Source Initiative (OSI) for maximum compatibility and clarity.
- Well-understood by legal teams worldwide. Corporate contributors do not need to seek special legal review for Apache 2.0 contributions.
- Attribution requirements are minimal: downstream distributors must include the NOTICE file if one exists, but do not need to open-source their modifications.

Negative consequences:

- Permissive licencing means someone could build a closed-source, commercial hosted version of Life Engine without contributing changes back. This is an accepted trade-off; the project's open-source community and ease of self-hosting are the defences against pure extraction.
- Apache 2.0 is GPLv2-incompatible. If Life Engine ever wanted to incorporate GPLv2-licenced code (common in Linux-adjacent projects), this would create a compatibility problem. GPLv3 is compatible.
- The NOTICE file requirement (attribution in distributions) is a minor operational requirement for anyone packaging Life Engine in a larger distribution.

## Alternatives Considered

**MIT licence** is the simplest permissive licence and has near-universal adoption in the JavaScript ecosystem. It was rejected in favour of Apache 2.0 solely because MIT includes no patent grant. For a project involving plugin authors and commercial integrations, the absence of a patent grant creates risk for downstream users. The added complexity of Apache 2.0 over MIT (the NOTICE file requirement, the slightly longer text) is worth the patent protection.

**GNU General Public Licence v3 (GPL-3.0)** is a copyleft licence requiring that derivatives and distributions under the GPL also release their source code under the GPL. It was rejected because:

- Commercial plugin authors would not be able to build closed-source plugins on GPL-licenced SDKs. Restricting the plugin ecosystem to GPL-compatible plugins would significantly reduce commercial adoption.
- Many self-hosters run modified versions of Core for their own use. GPL would not affect them, but it would complicate any commercial embedding.

**GNU Affero General Public Licence (AGPL-3.0)** is often used by self-hosted software projects to prevent "cloud providers" from running hosted versions without contributing back. It was considered and rejected because AGPL would deter self-hosting communities and enterprise deployments that are legitimate use cases. AGPL is frequently blocked by corporate legal policies, which would prevent companies from contributing even if they wanted to. Life Engine's community strategy prioritises openness over restricting potential free-riders.
