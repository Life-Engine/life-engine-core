<!--
domain: ci-and-cd
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Implementation Plan — CI and CD

## Task Overview

This plan implements the CI/CD pipeline for the Life Engine monorepo. Work is split into four phases: CI workflow, release workflow, branch protection configuration, and dependency management. Each task produces a single workflow file or config change that can be tested independently.

**Progress:** 0 / 11 tasks complete

## Steering Document Compliance

- All CI checks enforce Defence in Depth by auditing dependencies and validating code quality at the boundary
- The single CI workflow per event type follows Single Source of Truth
- Branch protection enforces Finish Before Widening by requiring all checks to pass before merge

## Atomic Task Requirements

- **File Scope:** 1-3 related files maximum
- **Time Boxing:** 15-30 minutes per task
- **Single Purpose:** one testable outcome per task
- **Specific Files:** exact file paths specified
- **Agent-Friendly:** clear input/output, minimal context switching

---

## 1.1 — CI Workflow: Rust Checks

> spec: ./brief.md

- [ ] Create the CI workflow file with Rust check jobs (cargo check, clippy, test)
  <!-- file: .github/workflows/ci.yml -->
  <!-- purpose: Validate Rust code quality and tests on every PR targeting main -->
  <!-- requirements: 1.1, 1.2, 1.3 -->
  <!-- leverage: existing Cargo.toml workspace configuration -->

- [ ] Add cargo-deny and npm audit dependency scanning to the CI workflow
  <!-- file: .github/workflows/ci.yml, deny.toml -->
  <!-- purpose: Scan Rust dependencies for licence violations and CVEs via cargo-deny; scan JS dependencies via npm audit -->
  <!-- requirements: 5.1, 5.2 -->
  <!-- leverage: existing deny.toml in repo root -->

## 1.2 — CI Workflow: JS/TS Checks

> spec: ./brief.md

- [ ] Add JS/TS check jobs to the CI workflow (pnpm install, eslint, tsc, vitest)
  <!-- file: .github/workflows/ci.yml -->
  <!-- purpose: Validate JS/TS code quality, type safety, and tests on every PR -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.4 -->
  <!-- leverage: existing package.json scripts and pnpm-workspace.yaml -->

## 1.3 — CI Workflow: Tauri and DCO Checks

> spec: ./brief.md

- [ ] Add Tauri compile check job to the CI workflow
  <!-- file: .github/workflows/ci.yml -->
  <!-- purpose: Verify Tauri application compiles without producing bundles -->
  <!-- requirements: 3.1, 3.2 -->
  <!-- leverage: existing apps/app/ Tauri configuration -->

- [ ] Add DCO sign-off verification job to the CI workflow
  <!-- file: .github/workflows/ci.yml -->
  <!-- purpose: Verify all commits include Signed-off-by line for open-source compliance -->
  <!-- requirements: 4.1, 4.2 -->
  <!-- leverage: none (uses third-party DCO action) -->

## 2.1 — Release Workflow: Platform Binaries

> spec: ./brief.md

- [ ] Create the release workflow with cross-platform binary build matrix
  <!-- file: .github/workflows/release.yml -->
  <!-- purpose: Build Core binaries for all 5 platform targets on version tag push -->
  <!-- requirements: 6.1, 6.2 -->
  <!-- leverage: existing Cargo.toml workspace and apps/core/ -->

- [ ] Add SHA-256 checksum generation and release asset upload
  <!-- file: .github/workflows/release.yml -->
  <!-- purpose: Generate checksums.txt and attach all binaries to the GitHub Release -->
  <!-- requirements: 6.3 -->
  <!-- leverage: none -->

## 2.2 — Release Workflow: Tauri Bundles

> spec: ./brief.md

- [ ] Add Tauri bundle build jobs for macOS, Linux, and Windows
  <!-- file: .github/workflows/release.yml -->
  <!-- purpose: Produce .dmg, .AppImage, and .msi installers and attach to GitHub Release -->
  <!-- requirements: 7.1, 7.2, 7.3, 7.4 -->
  <!-- leverage: existing apps/app/ Tauri configuration -->

## 2.3 — Release Workflow: SDK Publishing

> spec: ./brief.md

- [ ] Add conditional SDK publish jobs for plugin-sdk-rs and plugin-sdk-js
  <!-- file: .github/workflows/release.yml -->
  <!-- purpose: Publish SDKs to crates.io and npm only when their source has changed -->
  <!-- requirements: 8.1, 8.2, 8.3 -->
  <!-- leverage: existing packages/plugin-sdk-rs/ and packages/plugin-sdk-js/ -->

## 3.1 — Branch Protection Configuration

> spec: ./brief.md

- [ ] Document branch protection rules in a repository settings script
  <!-- file: scripts/setup-branch-protection.sh -->
  <!-- purpose: Configure required status checks, review requirements, and squash merge on main -->
  <!-- requirements: 9.1, 9.2, 9.3, 9.4 -->
  <!-- leverage: none -->

## 4.1 — Dependency Management

> spec: ./brief.md

- [ ] Update Dependabot configuration for Cargo, npm, and GitHub Actions
  <!-- file: .github/dependabot.yml -->
  <!-- purpose: Configure weekly Cargo/npm updates and monthly Actions updates -->
  <!-- requirements: 10.1, 10.2, 10.3, 10.4 -->
  <!-- leverage: existing .github/dependabot.yml -->
