<!--
domain: auth-and-pocket-id
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Implementation Plan — Auth and Pocket ID

## Task Overview

This plan implements the authentication layer for Core. Work begins with the `AuthProvider` trait definition, then builds the local-token provider (Phase 1 default), adds the middleware stack, wires up API key management endpoints, and finally prepares the Pocket ID sidecar integration for Phase 2. Each task targets 1-3 files and produces a testable outcome.

**Progress:** 0 / 14 tasks complete

## Steering Document Compliance

- Auth middleware runs on all `/api/*` routes except `/api/system/health`
- Local-token is the Phase 1 default; Pocket ID is Phase 2
- Plugins inherit auth with no bypass path
- Credentials stored as salted hashes, never in plaintext

## Atomic Task Requirements

- **File Scope:** 1-3 related files maximum
- **Time Boxing:** 15-30 minutes per task
- **Single Purpose:** one testable outcome per task
- **Specific Files:** exact file paths specified
- **Agent-Friendly:** clear input/output, minimal context switching

---

## 1.1 — AuthProvider Trait and Types
> spec: ./brief.md

- [ ] Define AuthProvider trait and identity types
  <!-- file: apps/core/src/auth/mod.rs -->
  <!-- file: apps/core/src/auth/types.rs -->
  <!-- purpose: Define the AuthProvider trait with validate_token and revoke_token methods, plus AuthIdentity and AuthError types -->
  <!-- requirements: 1.1, 1.2, 1.3, 1.4 -->
  <!-- leverage: existing apps/core/src/auth/types.rs -->

---

## 1.2 — Local Token Provider
> spec: ./brief.md

- [ ] Implement LocalTokenProvider with token generation
  <!-- file: apps/core/src/auth/local_token.rs -->
  <!-- purpose: Implement AuthProvider for local-token: generate tokens from master passphrase, store salted hashes, validate bearer tokens -->
  <!-- requirements: 2.1, 2.2, 2.3 -->
  <!-- leverage: existing apps/core/src/auth/local_token.rs -->

- [ ] Add token expiry and revocation logic
  <!-- file: apps/core/src/auth/local_token.rs -->
  <!-- file: apps/core/src/sqlite_storage.rs -->
  <!-- purpose: Add expiry checking and DELETE revocation for local tokens, with database queries for token lifecycle -->
  <!-- requirements: 2.4, 2.5 -->
  <!-- leverage: existing apps/core/src/auth/local_token.rs -->

- [ ] Add local token integration tests
  <!-- file: tests/auth/local_token_test.rs -->
  <!-- purpose: Test generate, validate, expire, and revoke flows for local tokens -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.4, 2.5 -->
  <!-- leverage: packages/test-utils -->

---

## 1.3 — Auth Middleware Stack
> spec: ./brief.md

- [ ] Implement auth middleware extractor
  <!-- file: apps/core/src/auth/middleware.rs -->
  <!-- purpose: Create axum middleware that extracts Bearer token, delegates to active AuthProvider, attaches identity to request extensions -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4 -->
  <!-- leverage: existing apps/core/src/auth/middleware.rs -->

- [ ] Add health endpoint bypass
  <!-- file: apps/core/src/auth/middleware.rs -->
  <!-- file: apps/core/src/routes/health.rs -->
  <!-- purpose: Skip auth for /api/system/health route; verify health endpoint remains publicly accessible -->
  <!-- requirements: 4.5 -->
  <!-- leverage: existing apps/core/src/routes/health.rs -->

- [ ] Add auth middleware tests
  <!-- file: tests/auth/middleware_test.rs -->
  <!-- purpose: Test valid token, missing token, expired token, and health bypass scenarios -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4, 4.5 -->
  <!-- leverage: packages/test-utils -->

---

## 1.4 — Rate Limiting
> spec: ./brief.md

- [ ] Implement auth rate limiter
  <!-- file: apps/core/src/rate_limit.rs -->
  <!-- file: apps/core/src/auth/middleware.rs -->
  <!-- purpose: Add per-IP sliding window rate limiter (5 failures/minute) that returns HTTP 429 with Retry-After header -->
  <!-- requirements: 5.1, 5.2, 5.3 -->
  <!-- leverage: existing apps/core/src/rate_limit.rs -->

- [ ] Add rate limiter tests
  <!-- file: tests/auth/rate_limit_test.rs -->
  <!-- purpose: Test that 5 failures trigger 429, window reset allows new attempts, and Retry-After header is present -->
  <!-- requirements: 5.1, 5.2, 5.3 -->
  <!-- leverage: packages/test-utils -->

---

## 1.5 — API Key Endpoints
> spec: ./brief.md

- [ ] Implement API key CRUD routes
  <!-- file: apps/core/src/auth/routes.rs -->
  <!-- file: apps/core/src/auth/local_token.rs -->
  <!-- purpose: Add POST /api/auth/keys (create scoped key), GET /api/auth/keys (list), DELETE /api/auth/keys/{id} (revoke) -->
  <!-- requirements: 7.1, 7.3, 7.4 -->
  <!-- leverage: existing apps/core/src/auth/routes.rs -->

- [ ] Add API key scope enforcement in middleware
  <!-- file: apps/core/src/auth/middleware.rs -->
  <!-- purpose: When an API key is used, enforce that the request path matches the key's allowed scope -->
  <!-- requirements: 7.2 -->
  <!-- leverage: existing apps/core/src/auth/middleware.rs -->

---

## 1.6 — Plugin Auth Inheritance
> spec: ./brief.md

- [ ] Verify plugin routes receive authenticated identity
  <!-- file: apps/core/src/plugin_loader.rs -->
  <!-- file: apps/core/src/routes/plugins.rs -->
  <!-- purpose: Ensure plugin-registered routes are nested under the auth middleware layer and handlers receive AuthIdentity from request extensions -->
  <!-- requirements: 6.1, 6.2, 6.3 -->
  <!-- leverage: existing apps/core/src/plugin_loader.rs -->

---

## 1.7 — Pocket ID Sidecar Integration (Phase 2)
> spec: ./brief.md

- [ ] Implement Pocket ID process manager
  <!-- file: apps/core/src/auth/oidc.rs -->
  <!-- purpose: Spawn Pocket ID binary, monitor process health, restart on crash, terminate on shutdown -->
  <!-- requirements: 3.1, 3.4, 3.5 -->
  <!-- leverage: existing apps/core/src/auth/oidc.rs -->

- [ ] Implement PocketIdProvider with JWT validation
  <!-- file: apps/core/src/auth/oidc.rs -->
  <!-- file: apps/core/src/auth/jwt.rs -->
  <!-- purpose: Implement AuthProvider for pocket-id: validate JWTs against Ed25519 public key, handle token refresh -->
  <!-- requirements: 3.2, 3.3, 3.6 -->
  <!-- leverage: existing apps/core/src/auth/jwt.rs -->
