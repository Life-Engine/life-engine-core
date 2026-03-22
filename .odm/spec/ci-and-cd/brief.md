<!--
domain: ci-and-cd
status: draft
tier: 1
updated: 2026-03-22
-->

# CI and CD Spec

## Overview

This spec defines the CI/CD pipeline for the Life Engine monorepo using GitHub Actions. The pipeline validates every pull request before merge and automates releases across all platform targets, including Tauri desktop bundles and SDK publishing.

## Goals

- Automated quality gates — every PR is validated by CI before merge, preventing regressions from reaching main
- Cross-platform releases — a single tag push produces binaries, Tauri bundles, and checksums for all 5 targets
- SDK publishing — plugin-sdk-rs and plugin-sdk-js are published to their respective registries when source changes
- Dependency hygiene — automated weekly dependency update PRs catch security vulnerabilities and stale dependencies

## User Stories

- As a contributor, I want CI to validate my PR automatically so that I get fast feedback on code quality and test failures.
- As a maintainer, I want a tag push to produce all platform artifacts so that I do not need to build releases manually.
- As a plugin author, I want SDK packages published to crates.io and npm so that I can depend on stable, versioned releases.
- As a maintainer, I want automated dependency update PRs so that I can review and merge security patches without manual tracking.

## Functional Requirements

- The system must run Rust checks (cargo check, clippy, test) on every PR targeting main.
- The system must run JS/TS checks (eslint, tsc, vitest) on every PR targeting main.
- The system must verify DCO sign-off on all commits in a PR.
- The system must audit Rust and JS dependencies for licence violations and known CVEs.
- The system must build platform binaries for macOS arm64/x86_64, Linux x86_64/aarch64, and Windows x86_64 on tag push.
- The system must produce Tauri bundles (.dmg, .AppImage, .msi) and attach them to the GitHub Release.
- The system must create automated dependency update PRs on a weekly cadence.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
