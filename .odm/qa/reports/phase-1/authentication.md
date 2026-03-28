# Authentication and Authorization Review

## Summary

The Life Engine authentication system spans two layers: the `packages/auth` crate (transport-agnostic auth library) and the `apps/core/src/auth/` module (HTTP-specific middleware, local token provider, OIDC integration, and WebAuthn support). The design is sound at an architectural level -- there is a clean `AuthProvider` trait, proper separation between transport and auth logic, and good use of Argon2id for passphrase hashing and HMAC-SHA256 for API key verification.

However, the review uncovered several security issues ranging from critical to minor, as well as architectural concerns that should be addressed before production deployment.

---

## File-by-File Analysis

### packages/auth/Cargo.toml

Clean dependency list. Uses workspace versions for consistency. `ed25519-dalek` is included for OIDC Ed25519 JWKS support, `jsonwebtoken` for JWT validation. No unnecessary or outdated dependencies observed.

### packages/auth/src/lib.rs

- Defines the `AuthProvider` trait with `validate_token`, `validate_key`, and `revoke_key` methods
- Factory function `create_auth_provider` dispatches on `config.provider` string
- Clean design with proper error propagation
- The two auth providers ("pocket-id" and "api-key") are mutually exclusive -- there is no composite provider at this layer (unlike the core layer's `MultiAuthProvider`), which means if both JWT and API key auth are needed simultaneously, the caller must manage two providers

### packages/auth/src/config.rs

- `AuthConfig` validates that `issuer` is required for "pocket-id" provider
- No validation for `jwks_refresh_interval` being zero (a zero value would cause the cache to be refreshed on every request, which is a DoS vector against the OIDC provider)
- The `audience` field is optional with no guidance on whether it should be required in production

### packages/auth/src/error.rs

- Error messages are appropriately generic -- no internal details leak through error variants
- `KeyInvalid` and `KeyRevoked` are separate variants, which is slightly information-leaking (an attacker can distinguish between "key doesn't exist" and "key was revoked"), though this is acceptable for API key management where the caller likely knows they have a key
- Properly implements `EngineError` trait with error codes

### packages/auth/src/types.rs

- `ApiKeyRecord` stores `key_hash` and `salt` as base64-encoded strings
- `AuthIdentity.scopes` is a `Vec<String>` with no validation -- arbitrary scope strings are accepted
- The `ApiKeyRecord` derives `Serialize`, which means the `key_hash` and `salt` fields could be exposed if the record is serialized into an API response (the `list_keys` function returns full records)

### packages/auth/src/handlers/keys.rs

- API keys are generated with 32 bytes from `OsRng` -- good entropy
- Uses HMAC-SHA256 with per-key salt for key hashing -- good practice
- `hmac_verify` provides constant-time comparison -- resistant to timing attacks
- `validate_key` iterates ALL keys on every validation, decoding salt/hash for each. This is O(n) per validation and does not scale
- `revoke_key` uses a hardcoded `expected_version: 1` which will fail if the record has been updated more than once (e.g., after `last_used` is updated)
- The `permissive_schema()` returns `{}` which means no schema validation occurs on stored API key records
- `list_keys` returns full `ApiKeyRecord` objects including `key_hash` and `salt` -- these should be stripped before returning to callers
- `validate_key` returns `AuthError::KeyRevoked` for expired keys, which is semantically incorrect -- should be a distinct error or documented as intentional

### packages/auth/src/handlers/validate.rs

- OIDC discovery and JWKS fetching are properly implemented
- JWKS keys are cached with configurable refresh interval
- `decode_and_validate` properly handles key rotation by refreshing on cache miss
- `refresh_keys_if_needed` has a TOCTOU race: the staleness check happens under a read lock, then releases the lock before acquiring a write lock in `refresh_keys`. Under concurrent load, multiple threads could all decide to refresh simultaneously, causing a thundering herd of JWKS fetches
- The `find_key` method falls back to using the first key when `kid` is `None` and there is exactly one key -- this is reasonable but should log a warning since it bypasses key ID matching
- `validate_request` properly integrates rate limiting before credential validation
- Bearer token and API key schemes are cleanly separated via prefix matching
- Failed validations correctly record failures in the rate limiter

### packages/auth/src/handlers/rate_limit.rs

- Simple sliding window implementation with per-IP tracking
- Default: 5 failures per 60 seconds -- reasonable baseline
- `is_rate_limited` takes a write lock even when just checking (to prune entries). Under high traffic, this serializes all rate limit checks behind a single lock
- No maximum size for the `failures` HashMap -- memory grows unboundedly with unique attacker IPs until `cleanup` is called
- `cleanup` must be called externally from a background task, but there is no built-in mechanism to ensure this happens

### packages/auth/src/tests/

All test files demonstrate good coverage:

- `identity_test.rs` -- Verifies `AuthIdentity` survives JSON serialization round-trips through the WASM boundary
- `keys_test.rs` -- Comprehensive API key lifecycle tests with a mock storage backend
- `validate_test.rs` -- Tests the `validate_request` pipeline including rate limiting integration
- `rate_limit_test.rs` -- Thorough rate limiter tests including window expiry and cleanup
- `pocket_id_test.rs` -- End-to-end JWT validation with mock OIDC server using Ed25519 keys

### apps/core/src/auth/middleware.rs

- Auth middleware for axum HTTP router
- Maintains its own separate `RateLimiter` implementation (distinct from `packages/auth`'s rate limiter) -- code duplication
- Trusts `X-Forwarded-For` header without validation. An attacker can spoof this header to bypass IP-based rate limiting
- Exempt endpoints are hardcoded as string comparisons -- no path normalization (e.g., `/api/auth/token/` with trailing slash or `//api//auth//token` would not match the exemption)
- The `maybe_cleanup` uses a non-atomic counter (`fetch_add` with `Relaxed` ordering) for scheduling cleanups, which is fine for approximate scheduling but the `is_multiple_of` check is a nightly-only API as of stable Rust -- this may cause compilation issues
- Error responses use simple JSON with only an error code -- good, no information leakage

### apps/core/src/auth/types.rs

- `AuthIdentity` includes `household_id` and `role` fields for RBAC, but these are always `None` in the current implementations -- the permission model is scaffolded but not enforced
- `HouseholdRole` has three levels (Admin, Member, Guest) but no middleware enforces role-based access
- `TokenRequest` has no maximum length validation for `passphrase` -- extremely long passphrases could cause DoS via Argon2id

### apps/core/src/auth/local_token.rs

- Uses SHA-256 for token hashing and Argon2id for passphrase hashing -- both appropriate choices
- Token generation uses `rand::thread_rng()` which delegates to the OS CSPRNG -- secure
- First `generate_token` call sets the master passphrase with no confirmation -- the passphrase is irreversible once set
- Expired tokens are returned by `list_tokens` with `is_expired: true` but never cleaned up from storage -- unbounded growth over time
- `expires_in_days < 1` is rejected, but `expires_in_days = u32::MAX` is accepted, allowing tokens that effectively never expire

### apps/core/src/auth/jwt.rs

- JWT validation supports RS256 only in the core layer (the `packages/auth` layer also supports EdDSA)
- JWKS cache implementation with TTL is clean and uses `RwLock` for concurrent access
- `validate_jwt` properly validates `exp`, `nbf`, `iss`, and `aud` claims
- `decode_and_validate_jwt` is a convenience wrapper that combines key construction and validation
- Keys without `kid` are silently ignored by the cache -- this means if the OIDC provider does not include `kid` in its keys, the cache will be empty

### apps/core/src/auth/oidc.rs

- OIDC provider properly implements the `AuthProvider` trait
- `client_secret` is stored in `OidcConfig` as a plain `Option<String>` -- if this struct is serialized (e.g., for debugging), the secret would be exposed
- `validate_token` requires a `kid` in the JWT header -- tokens without `kid` are rejected with `InvalidCredentials` rather than attempting fallback key matching
- `generate_token`, `revoke_token`, and `list_tokens` all return errors, which is correct since OIDC tokens are managed by the identity provider
- The HTTP client for JWKS fetching has no timeout configured (unlike the `packages/auth` PocketIdProvider which has a 10-second timeout)

### apps/core/src/auth/routes.rs

- Token generation endpoint (`POST /api/auth/token`) is exempt from auth middleware -- this is correct since it is the bootstrap endpoint
- `revoke_token` and `list_tokens` check for `AuthIdentity` in request extensions -- redundant with middleware but provides defense in depth
- OIDC login/refresh/register endpoints proxy requests to the identity provider -- credentials pass through the Life Engine server

### apps/core/src/auth/webauthn_provider.rs

- WebAuthn provider wraps `webauthn-rs` for FIDO2 passkey ceremonies
- Challenge state is stored in-memory with TTL expiration
- After successful WebAuthn authentication, delegates to the local token provider for session token generation -- clean separation

### apps/core/src/auth/middleware.rs (auth bypass paths)

- `/api/storage/init` POST is exempt from auth -- this endpoint should have additional protection since it initializes storage
- `/api/auth/register` POST is exempt from auth -- registration should be rate-limited separately to prevent abuse
- WebAuthn authenticate endpoints are exempt -- correct since they are the login flow

### apps/core/src/identity.rs

- Identity credential system with encrypted claims storage
- Uses separate encryption key from main data store -- good key separation
- Disclosure tokens use HMAC-SHA256 signatures -- appropriate for self-issued tokens
- Audit logging for disclosure events -- good compliance practice

### apps/core/src/credential_store.rs

- SQLite-backed credential store with AES-256-GCM encryption
- Uses parameterized queries (`params![]`) -- immune to SQL injection
- Values are never logged -- correct security practice
- Plugin scoping enforced by composite primary key `(plugin_id, key)` -- each plugin can only access its own credentials

### apps/core/src/rate_limit.rs

- Uses `governor` crate with GCRA algorithm -- production-grade rate limiting
- Per-IP keying with `DashMap` for lock-free concurrent access
- `reconfigure` method allows hot-reloading rate limit settings
- Health endpoint exempt -- correct
- Same `X-Forwarded-For` trust issue as the auth middleware
- Returns `Retry-After` header on 429 responses -- good HTTP compliance

---

## Problems Found

### Critical

- **X-Forwarded-For header trusted without validation** (`apps/core/src/auth/middleware.rs:128-133`, `apps/core/src/rate_limit.rs:113-118`) -- An attacker can bypass rate limiting by sending a different spoofed IP in each request via the `X-Forwarded-For` header. This completely undermines the per-IP rate limiter for both the auth middleware and the general rate limiter. The server should only trust `X-Forwarded-For` when a known reverse proxy is in front of it, validated by checking the direct connection IP against a trusted proxy list.

- **API key validation is O(n) full table scan** (`packages/auth/src/handlers/keys.rs:164`) -- `validate_key` calls `list_keys` to retrieve ALL API key records, then iterates through each one comparing HMAC hashes. With thousands of keys, this creates a linear-time validation that degrades authentication latency and could be used for DoS. A prefix-based lookup or indexed hash approach would provide O(1) validation.

- **API key hashes and salts exposed via `list_keys`** (`packages/auth/src/handlers/keys.rs:103-127`) -- `list_keys` returns full `ApiKeyRecord` objects including `key_hash` and `salt` fields. If this data reaches an API response, it gives attackers the material needed for offline brute-force attacks against API keys. The function should return metadata-only records with hashes stripped.

### Major

- **Duplicate rate limiter implementations** (`packages/auth/src/handlers/rate_limit.rs` vs `apps/core/src/auth/middleware.rs:31-82`) -- Two completely separate rate limiter implementations exist: one in the `packages/auth` crate (using `RwLock<HashMap>`) and another in the core middleware (using `Mutex<HashMap>`). They track different state, have different locking strategies, and are not coordinated. This creates confusion about which rate limiter is actually protecting which endpoint.

- **JWKS thundering herd on cache miss** (`packages/auth/src/handlers/validate.rs:225-236`) -- `refresh_keys_if_needed` checks staleness under a read lock, then releases it before acquiring a write lock for the refresh. Under concurrent load, all threads that observe a stale cache will initiate their own JWKS fetch. A mutex or `once_cell` pattern should be used to ensure only one refresh occurs.

- **No passphrase length limits** (`apps/core/src/auth/local_token.rs:245-313`, `apps/core/src/auth/types.rs:40-46`) -- `TokenRequest.passphrase` has no maximum length validation. Submitting an extremely long passphrase (e.g., 1 MB) to the Argon2id hasher would consume significant CPU and memory, enabling a denial-of-service attack on the token generation endpoint.

- **OIDC HTTP client has no timeout** (`apps/core/src/auth/oidc.rs:101`) -- `OidcProvider::new` creates a default `reqwest::Client` with no timeout. If the OIDC provider becomes slow or unresponsive, JWKS fetch calls will hang indefinitely, tying up server resources. The `packages/auth` PocketIdProvider correctly sets a 10-second timeout.

- **`expected_version: 1` hardcoded in revoke_key** (`packages/auth/src/handlers/keys.rs:146`) -- `revoke_key` passes `expected_version: 1` when updating the key record. If `validate_key` has already updated the record (to set `last_used`), the version will be > 1 and the revocation will fail. This is a race condition between key usage and revocation.

- **No auth bypass path normalization** (`apps/core/src/auth/middleware.rs:103-125`) -- Auth exemptions use exact string matching (e.g., `path == "/api/auth/token"`). URL-encoded paths (`/api/auth/%74oken`), trailing slashes (`/api/auth/token/`), or double slashes (`//api//auth//token`) would bypass the exemption check and trigger auth, or more dangerously, could bypass auth if the downstream router normalizes differently.

### Minor

- **`jwks_refresh_interval` of zero not validated** (`packages/auth/src/config.rs:44-49`) -- A zero refresh interval would cause the JWKS cache to be considered stale on every request, triggering an HTTP fetch to the OIDC provider on every token validation. This should be validated as part of `AuthConfig::validate()`.

- **Expired tokens never cleaned from storage** (`apps/core/src/auth/local_token.rs:335-349`) -- `list_tokens` returns expired tokens with `is_expired: true` but they are never removed from the in-memory cache or SQLite database. Over time, this causes unbounded memory and storage growth.

- **`ApiKeyRecord` returned for expired keys uses `KeyRevoked` error** (`packages/auth/src/handlers/keys.rs:187-192`) -- When an API key has expired, the code returns `AuthError::KeyRevoked` instead of a semantically correct expiration error. This conflates two distinct conditions and may confuse API consumers.

- **`/api/storage/init` exempt from auth** (`apps/core/src/auth/middleware.rs:106-108`) -- The storage initialization endpoint bypasses authentication. If exposed to the network, an attacker could potentially re-initialize storage. This endpoint should have alternative protection (e.g., localhost-only, or a one-time setup token).

- **`/api/auth/register` exempt from auth with no separate rate limiting** (`apps/core/src/auth/middleware.rs:112-119`) -- The registration endpoint bypasses auth middleware. Without dedicated rate limiting on this endpoint, an attacker could create accounts at high speed.

- **No scope validation or enforcement** (`packages/auth/src/types.rs:15`, `apps/core/src/auth/types.rs:13-26`) -- Scopes are stored as arbitrary strings with no validation, and no middleware enforces scope-based access control. The `HouseholdRole` enum in the core auth types is never checked against incoming requests.

- **`OidcConfig.client_secret` serializable** (`apps/core/src/auth/oidc.rs:26`) -- The `client_secret` field derives `Serialize`, meaning it could be accidentally included in log output or API responses. It should be marked with `#[serde(skip_serializing)]` or wrapped in a `Secret` type.

- **Two separate `AuthProvider` trait definitions** (`packages/auth/src/lib.rs:32-41` vs `apps/core/src/auth/mod.rs:25-38`) -- The `packages/auth` crate and `apps/core` each define their own `AuthProvider` trait with different method signatures. This prevents direct interop between the two layers and increases cognitive load.

---

## Recommendations

1. **Fix X-Forwarded-For trust immediately.** Add a configuration option for trusted proxy IPs and only accept `X-Forwarded-For` when the direct connection comes from a trusted proxy. This is the most exploitable issue.

2. **Add a key prefix index for API key validation.** Store a non-secret prefix (first 8 bytes) of each API key's hash as an indexed lookup column. Validation can then filter to candidate keys by prefix before performing full HMAC verification, reducing validation from O(n) to O(1).

3. **Strip sensitive fields from `list_keys` responses.** Create a `ApiKeyMetadata` struct that excludes `key_hash` and `salt`, and return that from listing endpoints.

4. **Consolidate rate limiter implementations.** Remove the duplicate rate limiter from `apps/core/src/auth/middleware.rs` and use either the `packages/auth` rate limiter or the `governor`-based one from `apps/core/src/rate_limit.rs`, but not both.

5. **Add a mutex around JWKS refresh** to prevent the thundering herd. A `tokio::sync::Notify` or `tokio::sync::OnceCell` pattern would ensure only one concurrent refresh happens.

6. **Add passphrase length limits.** Validate `TokenRequest.passphrase` length (e.g., max 1024 bytes) before passing to Argon2id.

7. **Set HTTP timeouts on the OIDC provider's reqwest client.** Match the 10-second timeout used by the `packages/auth` PocketIdProvider.

8. **Unify or explicitly deprecate one of the two `AuthProvider` traits.** Having two competing trait definitions will cause maintenance issues as the system grows.

9. **Implement token cleanup.** Add a background task that periodically removes expired tokens from both the in-memory cache and SQLite.

10. **Add path normalization before auth bypass checks.** Use a canonical path comparison that handles trailing slashes, percent-encoding, and double slashes.
