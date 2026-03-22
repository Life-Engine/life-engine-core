<!--
domain: auth-and-pocket-id
status: draft
tier: 1
updated: 2026-03-22
-->

# Auth and Pocket ID Spec

## Overview

This spec defines the authentication mechanisms for Core and the Pocket ID OIDC sidecar. Core supports two auth mechanisms — Pocket ID (OIDC) for user sessions and API keys for local dev and scripting. Both go through the same `AuthProvider` trait and middleware stack, ensuring consistent security guarantees regardless of the auth method used.

## Goals

- Secure all API routes with token-based authentication by default
- Support swappable auth providers (local-token for Phase 1, Pocket ID OIDC for Phase 2) with no code changes
- Provide API key management for CLI tools and automation scripts
- Enforce rate limiting on failed auth attempts to prevent brute force attacks
- Ensure plugins inherit authentication automatically with no bypass path

## User Stories

- As a self-hosted user, I want to authenticate with a simple bearer token so that I can access Core without setting up OIDC.
- As a developer, I want to generate scoped API keys so that I can automate tasks via CLI scripts.
- As a multi-user household member, I want to log in via Pocket ID with passkey support so that I have a secure, passwordless experience.
- As a plugin author, I want authenticated identity passed to my route handlers automatically so that I do not need to implement auth myself.

## Functional Requirements

- The system must abstract authentication behind an `AuthProvider` trait with local-token and pocket-id implementations.
- The system must validate every `/api/*` request (except `/api/system/health`) through the auth middleware.
- The system must rate-limit failed auth attempts at 5 per minute per IP.
- The system must support scoped, revocable API keys stored as salted hashes.
- The system must manage the Pocket ID sidecar lifecycle (spawn, monitor, terminate).
- The system must pass authenticated identity to downstream handlers and plugin routes.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
