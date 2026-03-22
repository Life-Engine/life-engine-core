<!--
domain: ci-and-cd
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Spec — CI and CD

## Contents

- [Purpose](#purpose)
- [CI Pipeline](#ci-pipeline)
- [Release Pipeline](#release-pipeline)
- [Branch Strategy](#branch-strategy)
- [Branch Protection](#branch-protection)
- [Dependency Management](#dependency-management)
- [Acceptance Criteria](#acceptance-criteria)

## Purpose

This spec defines the CI/CD pipeline for the Life Engine monorepo using GitHub Actions. The pipeline validates every pull request before merge and automates releases across all platform targets.

## CI Pipeline

The CI workflow (`ci.yml`) runs on every pull request targeting `main`. All checks must pass before the PR can be merged.

### Rust Checks

- **cargo check** — Verifies that all workspace members compile without errors
- **cargo clippy --deny warnings** — Enforces lint-level code quality. Warnings are treated as errors to prevent accumulation.
- **cargo test** — Runs the full test suite for all Rust workspace members (Core, plugin-sdk-rs, first-party Rust plugins)

### JavaScript/TypeScript Checks

- **npm ci** — Clean install of all JS dependencies from lockfile
- **eslint** — Lints all JS/TS source files against the project's ESLint configuration
- **tsc --noEmit** — Type-checks all TypeScript without producing output files
- **vitest** — Runs the full test suite for all JS packages (App, plugin-sdk-js, first-party App plugins)

### Tauri Build Check

- **Compile check** — Builds the Tauri application to verify that Rust and frontend compilation succeeds. Does not produce distributable packages (no signing, no bundling). Catches integration issues between the frontend and the Tauri backend.

### DCO Check

- **Signed-off-by verification** — Verifies that all commits in the PR include a `Signed-off-by` line, confirming Developer Certificate of Origin compliance. Required for open-source contributions.

### Dependency Auditing

- **cargo-deny** — Scans Rust dependencies for licence compliance violations and known vulnerabilities. Rejects PRs that introduce disallowed licences or dependencies with active CVEs.
- **npm audit** — Scans JS dependencies for known vulnerabilities. Reports findings as PR annotations.

## Release Pipeline

The release workflow (`release.yml`) triggers on version tag pushes (e.g. `v1.0.0`). It builds, packages, and publishes all artifacts.

### Platform Binary Builds

Builds are produced for the following targets:

- macOS arm64 (Apple Silicon)
- macOS x86_64 (Intel)
- Linux x86_64
- Linux aarch64 (ARM64, for Raspberry Pi and ARM servers)
- Windows x86_64

### Tauri Bundle Packaging

Platform-specific installers are built from the Tauri application:

- **.dmg** — macOS disk image (universal binary where possible)
- **.AppImage** — Linux portable application
- **.msi** — Windows installer package

### GitHub Release

A GitHub Release is created with:

- All platform binaries attached as assets
- All Tauri bundles attached as assets
- SHA-256 checksums for every artifact in a `checksums.txt` file
- Auto-generated release notes from merged PRs since the last tag

### SDK Publishing

- **plugin-sdk-rs** — Published to crates.io with `cargo publish`
- **plugin-sdk-js** — Published to npm with `npm publish`

SDK versions are independent from the platform version. They are published only when their source has changed since the last release.

## Branch Strategy

The project uses a simple branch model with no long-lived feature branches.

- **main** — Always releasable. Every commit on main has passed CI and review. Tagged commits trigger releases.
- **feat/** — Feature branches. Short-lived. Squash merged into main.
- **fix/** — Bug fix branches. Short-lived. Squash merged into main.
- **.odm/docs/** — Documentation-only branches. Short-lived. Squash merged into main.

No long-lived branches exist. No develop branch. No staging branch. The main branch is the single source of truth.

## Branch Protection

Branch protection rules are applied to `main`:

- **Require CI pass** — All CI checks must succeed before a PR can merge
- **Require 1 review** — At least one approving review is required (enforced when the team grows beyond a solo founder; advisory for now)
- **No direct pushes** — All changes go through pull requests
- **Squash merge only** — Keeps the main branch history linear and readable

## Dependency Management

Automated dependency updates are configured using Dependabot or Renovate (either tool is acceptable):

- **Rust dependencies** — Weekly update PRs for Cargo dependencies. Grouped by workspace member.
- **JS dependencies** — Weekly update PRs for npm dependencies. Security updates are opened immediately rather than batched.
- **GitHub Actions** — Monthly update PRs for action versions used in workflows.

Update PRs go through the standard CI pipeline. Merging is manual to allow review of changelogs and breaking changes.

## Acceptance Criteria

1. Every PR is validated by CI before it can be merged — no bypassing checks
2. The release pipeline produces platform binaries for all 5 targets (macOS arm64, macOS x86_64, Linux x86_64, Linux aarch64, Windows x86_64)
3. The release pipeline produces Tauri bundles (.dmg, .AppImage, .msi) and attaches them to the GitHub Release
4. SDK packages (plugin-sdk-rs on crates.io, plugin-sdk-js on npm) are published as part of the release when their source has changed
5. Branch protection rules prevent direct pushes to main and require CI pass
6. Dependency update PRs are created automatically on a weekly cadence
