<!--
domain: auth-and-pocket-id
updated: 2026-03-23
spec-brief: ./brief.md
-->

# Implementation Plan — Auth and Pocket ID

## Task Overview

This plan implements the auth crate (`packages/auth/`). Work begins with the crate scaffold and `AuthProvider` trait, then builds the Pocket ID OIDC provider, adds the validation pipeline, wires up API key management, and integrates rate limiting. Each task targets 1-3 files and produces a testable outcome.

**Progress:** 0 / 12 tasks complete

## Steering Document Compliance

- Auth is an independent crate following the standard layout (lib.rs, config.rs, error.rs, handlers/, types.rs, tests/)
- Error types implement EngineError trait (code, severity, source_module)
- Config is a TOML section: `[auth] provider = "pocket-id", issuer = "https://auth.local"`
- Auth is transport-agnostic — initialized by Core, shared with all transports
- Plugins inherit auth with no bypass path

## Atomic Task Requirements

- **File Scope:** 1-3 related files maximum
- **Time Boxing:** 15-30 minutes per task
- **Single Purpose:** one testable outcome per task
- **Specific Files:** exact file paths specified
- **Agent-Friendly:** clear input/output, minimal context switching

---

## 1.1 — Auth Crate Scaffold
> spec: ./brief.md

- [ ] Create the auth crate with standard layout and Cargo.toml
  <!-- file: packages/auth/Cargo.toml -->
  <!-- file: packages/auth/src/lib.rs -->
  <!-- purpose: Scaffold the auth crate with dependencies on packages/types, packages/traits, and packages/crypto; create lib.rs with module declarations -->
  <!-- requirements: 1.1, 1.3 -->
  <!-- leverage: none -->

---

## 1.2 — Auth Error Types
> spec: ./brief.md

- [ ] Define auth error types implementing EngineError
  <!-- file: packages/auth/src/error.rs -->
  <!-- purpose: Define AuthError enum with variants (TokenMissing, TokenExpired, TokenInvalid, ProviderUnreachable, ConfigInvalid, RateLimited, KeyRevoked) implementing EngineError trait -->
  <!-- requirements: 1.2 -->
  <!-- leverage: packages/traits EngineError trait -->

---

## 1.3 — Auth Config and Types
> spec: ./brief.md

- [ ] Define AuthConfig struct and auth types
  <!-- file: packages/auth/src/config.rs -->
  <!-- file: packages/auth/src/types.rs -->
  <!-- purpose: Define AuthConfig with provider and issuer fields for TOML deserialization; define AuthIdentity, AuthToken, ApiKey, and Scope types -->
  <!-- requirements: 2.1, 8.1, 8.2, 8.3 -->
  <!-- leverage: none -->

---

## 2.1 — AuthProvider Trait
> spec: ./brief.md

- [ ] Define AuthProvider trait with validate and identity methods
  <!-- file: packages/auth/src/lib.rs -->
  <!-- purpose: Define the AuthProvider trait with validate_token, validate_key, and revoke methods; add factory function that reads config and returns the correct implementation -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.4 -->
  <!-- leverage: packages/auth/src/config.rs -->

---

## 2.2 — Pocket ID Provider
> spec: ./brief.md

- [ ] Implement PocketIdProvider with JWT validation
  <!-- file: packages/auth/src/handlers/validate.rs -->
  <!-- purpose: Implement AuthProvider for PocketIdProvider: validate JWTs against issuer public key, handle token refresh, check expiry -->
  <!-- requirements: 3.1, 3.2, 3.3, 3.4, 3.5 -->
  <!-- leverage: packages/crypto for key operations -->

---

## 2.3 — Pocket ID Provider Tests
> spec: ./brief.md

- [ ] Add unit tests for PocketIdProvider
  <!-- file: packages/auth/src/tests/pocket_id_test.rs -->
  <!-- purpose: Test JWT validation with valid tokens, expired tokens, invalid signatures, and unreachable issuer scenarios -->
  <!-- requirements: 3.1, 3.2, 3.3, 3.4, 3.5 -->
  <!-- leverage: packages/test-utils -->

---

## 3.1 — Auth Validation Pipeline
> spec: ./brief.md

- [ ] Implement the auth validation handler
  <!-- file: packages/auth/src/handlers/validate.rs -->
  <!-- purpose: Create validation function that extracts credential type, delegates to correct provider, returns AuthIdentity or AuthError -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4, 4.5 -->
  <!-- leverage: packages/auth/src/types.rs -->

---

## 3.2 — Auth Validation Tests
> spec: ./brief.md

- [ ] Add unit tests for auth validation pipeline
  <!-- file: packages/auth/src/tests/validate_test.rs -->
  <!-- purpose: Test valid token, missing token, expired token, and health bypass scenarios -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4, 4.5 -->
  <!-- leverage: packages/test-utils -->

---

## 4.1 — Rate Limiting
> spec: ./brief.md

- [ ] Implement per-IP sliding window rate limiter in the auth module
  <!-- file: packages/auth/src/handlers/rate_limit.rs -->
  <!-- purpose: Add per-IP sliding window rate limiter (5 failures/minute) that returns AuthError::RateLimited with Retry-After value -->
  <!-- requirements: 5.1, 5.2, 5.3 -->
  <!-- leverage: none -->

- [ ] Add rate limiter tests
  <!-- file: packages/auth/src/tests/rate_limit_test.rs -->
  <!-- purpose: Test that 5 failures trigger rate limit, window reset allows new attempts, and Retry-After value is correct -->
  <!-- requirements: 5.1, 5.2, 5.3 -->
  <!-- leverage: packages/test-utils -->

---

## 5.1 — API Key Management
> spec: ./brief.md

- [ ] Implement API key CRUD handlers
  <!-- file: packages/auth/src/handlers/keys.rs -->
  <!-- purpose: Implement create (scoped key generation, salted hash storage), list (metadata only), and revoke operations for API keys -->
  <!-- requirements: 7.1, 7.2, 7.3, 7.4 -->
  <!-- leverage: packages/crypto for hashing -->

---

## 5.2 — Plugin Auth Inheritance
> spec: ./brief.md

- [ ] Verify pipeline messages carry authenticated identity
  <!-- file: packages/auth/src/tests/identity_test.rs -->
  <!-- purpose: Test that AuthIdentity is correctly attached to PipelineMessage metadata after auth validation; verify capability enforcement for credentials access -->
  <!-- requirements: 6.1, 6.2, 6.3 -->
  <!-- leverage: packages/types PipelineMessage -->
