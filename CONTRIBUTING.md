# Contributing to Life Engine

Thank you for your interest in contributing to Life Engine! This guide will help you get started.

## Prerequisites

- [Rust toolchain](https://rustup.rs/) (stable, latest)
- [Node.js](https://nodejs.org/) 20+
- [pnpm](https://pnpm.io/) 9+
- [cargo-watch](https://crates.io/crates/cargo-watch) (for dev mode)

## Getting Started

1. Fork and clone the repository
2. Install dependencies:

```bash
cargo check --workspace
pnpm install
```

3. Run the test suite:

```bash
npx nx cargo-test
```

4. Start the development environment:

```bash
npx nx dev-core
```

## Branch Naming

Use the following prefixes for branches:

- `feat/` — New features
- `fix/` — Bug fixes
- `docs/` — Documentation changes
- `refactor/` — Code refactoring
- `test/` — Test additions or changes
- `chore/` — Build, CI, or tooling changes

## Commit Messages

We use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]
```

Types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `ci`, `style`

## Developer Certificate of Origin (DCO)

All contributions must be signed off to certify that you have the right to submit the work under the project's licence. Add a sign-off line to your commits:

```bash
git commit -s -m "feat(core): add health check endpoint"
```

This adds a `Signed-off-by` line to your commit message. Our CI checks for this on every PR.

## Pull Request Process

1. Create a branch from `main` with the appropriate prefix
2. Make your changes, following existing code patterns
3. Ensure all tests pass: `npx nx cargo-test`
4. Ensure lints pass: `npx nx cargo-lint`
5. Ensure formatting is correct: `npx nx fmt-check`
6. Submit a pull request targeting `main`
7. Fill out the PR template
8. Wait for review and CI checks

## Code of Conduct

All contributors are expected to follow our [Code of Conduct](CODE_OF_CONDUCT.md).

## Licence

By contributing, you agree that your contributions will be licensed under the Apache 2.0 licence.
