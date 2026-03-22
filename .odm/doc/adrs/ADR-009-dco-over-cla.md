# ADR-009: Developer Certificate of Origin over Contributor Licence Agreement

## Status
Accepted

## Context

Life Engine is an open-source project that will accept contributions from external developers. A legal framework is needed to establish that:

- Contributors have the right to submit the code they are contributing (it is their own work, not their employer's property, and not copied from a licence-incompatible source).
- The project can distribute contributions under the Apache 2.0 licence.
- There is a clear, auditable record that each contributor agreed to the terms.

Two dominant models exist for open-source contribution agreements: the Contributor Licence Agreement (CLA) and the Developer Certificate of Origin (DCO).

The choice has a direct impact on contributor friction. Requiring contributors to sign a legal document before their first pull request creates a barrier that discourages casual contributions (documentation fixes, small bug reports turned into patches) and requires legal infrastructure (a CLA management service, follow-up emails, corporate CLA review processes for employed contributors).

## Decision

Life Engine uses the Developer Certificate of Origin (DCO) version 1.1. Contributors confirm the DCO by adding a `Signed-off-by: Name <email>` line to every commit, which is enforced by CI (the `DCO` GitHub Action checks every commit in a pull request). No separate legal document needs to be signed, no account created with a CLA management service, and no email exchanged with the maintainers.

The DCO is a lightweight attestation: the contributor confirms they wrote the code, they have the right to submit it under the open-source licence, and they understand it will be incorporated into the project under that licence. This is sufficient for an Apache 2.0-licensed project.

## Consequences

Positive consequences:

- Zero contribution friction for individuals. Adding `git commit -s` (or a Git alias) is all that is required to comply.
- No legal document to review. Contributors can submit a two-line bug fix without involving their company's legal team.
- The `Signed-off-by` lines are embedded in commit history, creating a permanent, distributed record. There is no external service to lose the records.
- The DCO is used by major open-source foundations (Linux Foundation, CNCF) and is well-understood by corporate legal teams that review open-source contributions.
- Automation is straightforward. The DCO GitHub Action rejects pull requests with unsigned commits automatically, without human review.
- No renewal or expiry. A CLA signed three years ago may be considered stale by a new legal team; DCO attestations are per-commit and always current.

Negative consequences:

- The DCO is an attestation, not an assignment or exclusive licence grant. If the project ever needed to relicense, a CLA (which can include broader grants) would have been more useful. However, Life Engine has no plans to relicense from Apache 2.0.
- Contributors who commit without `-s` must amend or rebase their commits before CI passes. This is a minor friction point, especially for contributors unfamiliar with DCO.
- A CLA can include patent retaliation clauses that explicitly protect the project if a contributor later claims patent infringement. Apache 2.0 includes a patent grant, but the DCO does not add additional patent protections beyond what the licence provides.
- Multi-commit pull requests from contributors who forget to sign some commits require an interactive rebase to fix, which can be frustrating for less experienced Git users.

## Alternatives Considered

**Contributor Licence Agreement (CLA)** requires contributors to sign a legal document granting the project (or its governing body) a licence to use their contributions. CLAs are used by projects governed by foundations (Apache Software Foundation, Eclipse Foundation) and by companies that want the option to offer commercial licences alongside the open-source release. A CLA was rejected for Life Engine because:

- It creates a "CLA wall" that discourages small contributions and first-time contributors.
- It requires a CLA management service (e.g., CLA Assistant, EasyCLA) or a manual process to track signatures.
- Employed contributors typically need their employer to sign a corporate CLA, adding delay and bureaucracy.
- Life Engine has no commercial licensing plans that would require the additional IP transfer rights a CLA provides.

**No contribution agreement** was considered — simply accepting all contributions with no formal agreement. This was rejected because it leaves the project with no documented assurance that contributors had the right to submit their code, creating potential IP risk if a contributor's employer claims ownership of submitted work.
