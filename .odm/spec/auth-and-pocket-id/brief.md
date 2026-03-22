<!--
domain: auth-and-pocket-id
status: draft
tier: 1
updated: 2026-03-23
-->

# Auth and Pocket ID Spec

## Overview

This spec defines the authentication module for Life Engine Core. Auth is extracted to its own crate (`packages/auth/`) and initialized by Core at startup, then shared with all active transports. Auth is transport-agnostic — every transport uses the same auth module. Two auth mechanisms are supported: Pocket ID (OIDC) as the primary provider for user sessions, and API keys as a secondary provider for CLI tools and scripting. Both go through the same `AuthProvider` trait, ensuring consistent security guarantees regardless of the auth method used.

## Goals

- Extract auth into an independent crate (`packages/auth/`) following the standard crate layout
- Secure all transport endpoints with token-based authentication by default
- Use Pocket ID (OIDC) as the primary auth provider for user sessions
- Provide API key management as a secondary auth mechanism for scripting and automation
- Enforce rate limiting on failed auth attempts to prevent brute force attacks
- Ensure plugins inherit authentication automatically with no bypass path
- Implement error types using the `EngineError` trait (code, severity, source_module)

## User Stories

- As a household member, I want to log in via Pocket ID with passkey support so that I have a secure, passwordless experience.
- As a developer, I want to generate scoped API keys so that I can automate tasks via CLI scripts.
- As a plugin author, I want authenticated identity passed to my pipeline steps automatically so that I do not need to implement auth myself.
- As a Core developer, I want auth in its own crate so that transports and other modules can depend on it without circular dependencies.

## Functional Requirements

- The system must abstract authentication behind an `AuthProvider` trait with `pocket-id` and `api-key` implementations.
- The system must validate every transport request (except health checks) through the auth module.
- The system must rate-limit failed auth attempts at 5 per minute per IP.
- The system must support scoped, revocable API keys stored as salted hashes.
- The system must pass authenticated identity to downstream workflow steps and plugin routes.
- The system must configure auth via the `[auth]` section in `config.toml`.
- The system must implement auth errors using the `EngineError` trait with code, severity, and source_module.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
