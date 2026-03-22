<!--
domain: ci-and-cd
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Requirements Document — CI and CD

## Introduction

Life Engine uses GitHub Actions for continuous integration and continuous delivery. The CI pipeline validates every pull request before merge, enforcing code quality, type safety, test coverage, licence compliance, and DCO sign-off. The release pipeline triggers on version tags and produces cross-platform binaries, Tauri desktop bundles, and SDK packages.

## Alignment with Product Vision

- **Defence in Depth** — Dependency auditing (cargo-deny, npm audit) catches licence violations and known CVEs before they reach main
- **Finish Before Widening** — CI ensures each PR is complete and correct before merging, preventing partially-wired features from accumulating
- **Single Source of Truth** — One CI workflow validates the entire monorepo; one release workflow produces all artifacts from a single tag

## Requirements

### Requirement 1 — Rust CI Checks

**User Story:** As a contributor, I want Rust code validated on every PR, so that compilation errors, lint warnings, and test failures are caught before merge.

#### Acceptance Criteria

- 1.1. WHEN a PR is opened or updated targeting `main` THEN the system SHALL run `cargo check` across all workspace members and report pass/fail status.
- 1.2. WHEN a PR is opened or updated targeting `main` THEN the system SHALL run `cargo clippy --deny warnings` and fail the check if any warnings are produced.
- 1.3. WHEN a PR is opened or updated targeting `main` THEN the system SHALL run `cargo test` for all Rust workspace members and report pass/fail status.

### Requirement 2 — JavaScript/TypeScript CI Checks

**User Story:** As a contributor, I want JS/TS code validated on every PR, so that lint errors, type errors, and test failures are caught before merge.

#### Acceptance Criteria

- 2.1. WHEN a PR is opened or updated targeting `main` THEN the system SHALL run `pnpm install --frozen-lockfile` and fail if the lockfile is out of date.
- 2.2. WHEN a PR is opened or updated targeting `main` THEN the system SHALL run `eslint` against all JS/TS source files and report violations.
- 2.3. WHEN a PR is opened or updated targeting `main` THEN the system SHALL run `tsc --noEmit` and fail the check if type errors are found.
- 2.4. WHEN a PR is opened or updated targeting `main` THEN the system SHALL run `vitest` for all JS/TS packages and report pass/fail status.

### Requirement 3 — Tauri Build Validation

**User Story:** As a contributor, I want the Tauri application build verified on every PR, so that integration issues between Rust backend and JS frontend are caught early.

#### Acceptance Criteria

- 3.1. WHEN a PR is opened or updated targeting `main` THEN the system SHALL compile the Tauri application without producing distributable bundles.
- 3.2. WHEN the Tauri compile check fails THEN the system SHALL report the failure as a required status check on the PR.

### Requirement 4 — DCO Verification

**User Story:** As a maintainer, I want all commits verified for DCO sign-off, so that the project maintains open-source licence compliance.

#### Acceptance Criteria

- 4.1. WHEN a PR is opened or updated THEN the system SHALL verify that every commit includes a valid `Signed-off-by` line.
- 4.2. WHEN any commit in the PR lacks a `Signed-off-by` line THEN the system SHALL fail the DCO check and annotate the failing commit.

### Requirement 5 — Dependency Auditing

**User Story:** As a maintainer, I want dependencies audited for vulnerabilities and licence violations on every PR, so that insecure or non-compliant dependencies do not enter the codebase.

#### Acceptance Criteria

- 5.1. WHEN a PR is opened or updated targeting `main` THEN the system SHALL run `cargo-deny` and fail if disallowed licences or active CVEs are found in Rust dependencies.
- 5.2. WHEN a PR is opened or updated targeting `main` THEN the system SHALL run `npm audit` and report findings as PR annotations.

### Requirement 6 — Release Binary Builds

**User Story:** As a maintainer, I want a version tag push to produce platform binaries for all targets, so that users on any supported OS can download the correct binary.

#### Acceptance Criteria

- 6.1. WHEN a version tag (e.g. `v1.0.0`) is pushed THEN the system SHALL build the Core binary for macOS arm64, macOS x86_64, Linux x86_64, Linux aarch64, and Windows x86_64.
- 6.2. WHEN all 5 platform binaries are built successfully THEN the system SHALL attach them as assets to the GitHub Release.
- 6.3. WHEN binaries are attached to the release THEN the system SHALL generate a `checksums.txt` file containing SHA-256 hashes for every artifact.

### Requirement 7 — Tauri Bundle Packaging

**User Story:** As a user, I want desktop installers available for my platform, so that I can install Life Engine without a command line.

#### Acceptance Criteria

- 7.1. WHEN a version tag is pushed THEN the system SHALL produce a `.dmg` bundle for macOS.
- 7.2. WHEN a version tag is pushed THEN the system SHALL produce an `.AppImage` bundle for Linux.
- 7.3. WHEN a version tag is pushed THEN the system SHALL produce an `.msi` bundle for Windows.
- 7.4. WHEN Tauri bundles are built THEN the system SHALL attach them to the same GitHub Release as the platform binaries.

### Requirement 8 — SDK Publishing

**User Story:** As a plugin author, I want SDK packages published to crates.io and npm on release, so that I can depend on stable versioned packages.

#### Acceptance Criteria

- 8.1. WHEN a version tag is pushed AND `packages/plugin-sdk-rs/` has changed since the last release THEN the system SHALL publish plugin-sdk-rs to crates.io.
- 8.2. WHEN a version tag is pushed AND `packages/plugin-sdk-js/` has changed since the last release THEN the system SHALL publish plugin-sdk-js to npm.
- 8.3. WHEN an SDK has not changed since the last release THEN the system SHALL skip publishing for that SDK.

### Requirement 9 — Branch Protection

**User Story:** As a maintainer, I want branch protection rules enforced on main, so that untested or unreviewed code cannot be merged.

#### Acceptance Criteria

- 9.1. WHEN a PR targets `main` THEN the system SHALL require all CI status checks to pass before allowing merge.
- 9.2. WHEN a PR targets `main` THEN the system SHALL require at least one approving review before allowing merge.
- 9.3. WHEN a user attempts to push directly to `main` THEN the system SHALL reject the push.
- 9.4. WHEN a PR is merged to `main` THEN the merge strategy SHALL be squash merge to maintain linear history.

### Requirement 10 — Automated Dependency Updates

**User Story:** As a maintainer, I want automated dependency update PRs, so that I can review and merge updates without manual tracking.

#### Acceptance Criteria

- 10.1. WHEN a weekly schedule triggers THEN the system SHALL create PRs for outdated Cargo dependencies, grouped by workspace member.
- 10.2. WHEN a weekly schedule triggers THEN the system SHALL create PRs for outdated npm dependencies.
- 10.3. WHEN a security vulnerability is detected in an npm dependency THEN the system SHALL create a PR immediately rather than waiting for the weekly batch.
- 10.4. WHEN a monthly schedule triggers THEN the system SHALL create PRs for outdated GitHub Actions versions.
