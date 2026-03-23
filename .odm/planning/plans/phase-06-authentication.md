<!--
project: life-engine-core
phase: 6
specs: auth-and-pocket-id
updated: 2026-03-23
-->

# Phase 6 — Authentication

## Plan Overview

This phase implements the auth crate (`packages/auth/`): an independent, transport-agnostic authentication module supporting two mechanisms — Pocket ID (OIDC) for user sessions and API keys for scripting. The auth module is initialized once during Core startup and shared with all transports. It provides the `AuthProvider` trait, JWT validation with Ed25519 signatures, a validation pipeline, per-IP sliding window rate limiting, and API key CRUD operations.

This phase depends on Phase 3 (traits, crypto) and Phase 5 (storage for API key persistence). Phase 9 (Core startup) wires this module into the startup sequence.

> spec: .odm/spec/auth-and-pocket-id/brief.md

Progress: 0 / 12 work packages complete

---

## 6.1 — Auth Crate Scaffold
> spec: .odm/spec/auth-and-pocket-id/brief.md

- [x] Create the auth crate with standard layout and dependencies
  <!-- file: packages/auth/Cargo.toml -->
  <!-- file: packages/auth/src/lib.rs -->
  <!-- purpose: Set up Cargo.toml with name = "life-engine-auth", edition = "2024", dependencies on life-engine-types (workspace), life-engine-traits (workspace), life-engine-crypto (workspace), jsonwebtoken = "9" (JWT validation), reqwest = { version = "0.12", features = ["json"] } (OIDC discovery), ed25519-dalek = "2" (Ed25519 signature verification), serde (workspace), serde_json (workspace), tokio (workspace), tracing (workspace), thiserror (workspace), uuid (workspace), chrono (workspace). Create src/lib.rs with module declarations: mod config, mod error, mod types, mod handlers (containing validate, rate_limit, keys submodules), mod tests. Declare pub use for the public API: AuthProvider trait, AuthConfig, AuthIdentity, AuthError. Verify crate compiles. -->
  <!-- requirements: 1.1, 1.3 -->
  <!-- leverage: none -->

---

## 6.2 — Auth Error Types
> spec: .odm/spec/auth-and-pocket-id/brief.md

- [x] Define AuthError enum implementing EngineError trait
  <!-- file: packages/auth/src/error.rs -->
  <!-- purpose: Define AuthError enum with variants: TokenMissing (no auth header present, code "AUTH_001", Severity::Fatal), TokenExpired (JWT past exp claim, code "AUTH_002", Severity::Fatal), TokenInvalid (signature verification failed or malformed JWT, code "AUTH_003", Severity::Fatal), ProviderUnreachable (cannot reach OIDC issuer for key refresh, code "AUTH_004", Severity::Retryable), ConfigInvalid (invalid auth configuration, code "AUTH_005", Severity::Fatal), RateLimited { retry_after: u64 } (too many failed attempts from this IP, code "AUTH_006", Severity::Fatal — includes seconds until retry allowed), KeyRevoked (API key has been revoked, code "AUTH_007", Severity::Fatal), KeyInvalid (API key not found or wrong hash, code "AUTH_008", Severity::Fatal). Implement EngineError trait: code() returns the AUTH_xxx code, severity() returns the appropriate Severity, source_module() returns "auth". Implement std::error::Error and Display with human-readable messages. -->
  <!-- requirements: 1.2 -->
  <!-- leverage: packages/traits EngineError trait -->

---

## 6.3 — Auth Config and Types
> spec: .odm/spec/auth-and-pocket-id/brief.md

- [x] Define AuthConfig struct for TOML deserialization
  <!-- file: packages/auth/src/config.rs -->
  <!-- purpose: Define AuthConfig struct with serde Deserialize: provider (String — "pocket-id" or "api-key"), issuer (Option<String> — OIDC issuer URL, required for pocket-id), audience (Option<String> — expected JWT audience claim), jwks_refresh_interval (Option<u64> — seconds between JWKS key refresh, default 3600). Implement validation: if provider is "pocket-id", issuer must be present; if provider is "api-key", issuer is ignored. Add Default impl with provider = "pocket-id". -->
  <!-- requirements: 2.1 -->
  <!-- leverage: none -->

- [x] Define auth identity and token types
  <!-- file: packages/auth/src/types.rs -->
  <!-- purpose: Define AuthIdentity struct: user_id (String — subject claim from JWT or API key owner), provider (String — "pocket-id" or "api-key"), scopes (Vec<String> — authorized scopes), authenticated_at (DateTime<Utc>). Define AuthToken enum: Bearer(String) for JWT tokens, ApiKey(String) for API keys. Define ApiKeyRecord struct for storage: id (Uuid), name (String — human-readable label), key_hash (String — salted SHA-256 hash of the key), salt (String — unique salt per key), scopes (Vec<String>), created_at (DateTime<Utc>), expires_at (Option<DateTime<Utc>>), revoked (bool), last_used (Option<DateTime<Utc>>). Define Scope enum or use strings: "admin", "read", "write", "sync". All types derive Serialize, Deserialize, Debug, Clone. -->
  <!-- requirements: 8.1, 8.2, 8.3 -->
  <!-- leverage: none -->

---

## 6.4 — AuthProvider Trait
> spec: .odm/spec/auth-and-pocket-id/brief.md

- [x] Define AuthProvider trait and factory function
  <!-- file: packages/auth/src/lib.rs -->
  <!-- purpose: Define pub trait AuthProvider: Send + Sync with methods: async fn validate_token(&self, token: &str) -> Result<AuthIdentity, AuthError> for JWT validation, async fn validate_key(&self, key: &str) -> Result<AuthIdentity, AuthError> for API key validation, async fn revoke_key(&self, key_id: Uuid) -> Result<(), AuthError> for API key revocation. Define pub async fn create_auth_provider(config: AuthConfig) -> Result<Box<dyn AuthProvider>, AuthError> factory function that reads the config and returns either a PocketIdProvider or ApiKeyProvider instance. The factory function is called once during Core startup. The returned provider is wrapped in Arc for sharing across transport tasks. -->
  <!-- requirements: 2.1, 2.2, 2.3, 2.4 -->
  <!-- leverage: packages/auth/src/config.rs -->

---

## 6.5 — Pocket ID Provider Implementation
> spec: .odm/spec/auth-and-pocket-id/brief.md

- [x] Implement PocketIdProvider with JWT validation and JWKS key caching
  <!-- file: packages/auth/src/handlers/validate.rs -->
  <!-- purpose: Define PocketIdProvider struct holding: issuer URL, cached JWKS keys (RwLock<Vec<DecodingKey>>), last refresh timestamp, refresh interval. Implement AuthProvider::validate_token: (1) decode JWT header to get kid (key ID), (2) look up the matching public key from cached JWKS, (3) if key not found, attempt JWKS refresh from issuer's .well-known/openid-configuration -> jwks_uri endpoint, (4) validate JWT signature using Ed25519 (RS256 as fallback), (5) check exp claim — reject if expired, (6) check iss claim — must match configured issuer, (7) check aud claim — must match configured audience if set, (8) extract sub claim as user_id, (9) extract scope claim as scopes, (10) return AuthIdentity. Handle token refresh: JWT access tokens have 15-minute expiry, refresh tokens have 7-day expiry. If the token is a refresh token, exchange it at the issuer's token endpoint for a new access token. Cache JWKS keys for the configured interval (default 1 hour). Use reqwest for HTTP calls to the issuer. -->
  <!-- requirements: 3.1, 3.2, 3.3, 3.4, 3.5 -->
  <!-- leverage: packages/crypto for key operations -->

---

## 6.6 — Pocket ID Provider Tests
> spec: .odm/spec/auth-and-pocket-id/brief.md

- [x] Add unit tests for PocketIdProvider JWT validation
  <!-- file: packages/auth/src/tests/pocket_id_test.rs -->
  <!-- purpose: Test scenarios: (1) valid JWT with correct signature, issuer, audience, and non-expired claims returns AuthIdentity with correct user_id and scopes, (2) expired JWT returns AuthError::TokenExpired, (3) JWT with invalid signature returns AuthError::TokenInvalid, (4) JWT with wrong issuer returns AuthError::TokenInvalid, (5) JWT with wrong audience returns AuthError::TokenInvalid, (6) JWKS refresh when key not in cache — mock HTTP to return new keys, verify validation succeeds after refresh, (7) unreachable issuer returns AuthError::ProviderUnreachable with Severity::Retryable. Use a test Ed25519 keypair to generate test JWTs. Mock the OIDC discovery endpoint and JWKS endpoint using a local HTTP server or trait-based dependency injection. -->
  <!-- requirements: 3.1, 3.2, 3.3, 3.4, 3.5 -->
  <!-- leverage: packages/test-utils -->

---

## 6.7 — Auth Validation Pipeline
> spec: .odm/spec/auth-and-pocket-id/brief.md

- [x] Implement the auth validation handler function
  <!-- file: packages/auth/src/handlers/validate.rs -->
  <!-- purpose: Define pub async fn validate_request(provider: &dyn AuthProvider, auth_header: Option<&str>, rate_limiter: &RateLimiter, client_ip: &str) -> Result<AuthIdentity, AuthError>. Logic: (1) if auth_header is None, return AuthError::TokenMissing, (2) check rate_limiter — if IP is rate-limited, return AuthError::RateLimited with retry_after seconds, (3) parse the Authorization header: if starts with "Bearer ", extract token and call provider.validate_token(), if starts with "ApiKey ", extract key and call provider.validate_key(), otherwise return AuthError::TokenInvalid, (4) on validation failure, record failure in rate_limiter, then return the error, (5) on success, return AuthIdentity. This function is transport-agnostic — transports extract the Authorization header and client IP, then call this function. -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4, 4.5 -->
  <!-- leverage: packages/auth/src/types.rs -->

---

## 6.8 — Auth Validation Pipeline Tests
> spec: .odm/spec/auth-and-pocket-id/brief.md

- [x] Add unit tests for auth validation pipeline
  <!-- file: packages/auth/src/tests/validate_test.rs -->
  <!-- purpose: Test scenarios: (1) valid Bearer token returns AuthIdentity, (2) valid ApiKey returns AuthIdentity, (3) missing Authorization header returns AuthError::TokenMissing, (4) expired Bearer token returns AuthError::TokenExpired, (5) invalid Bearer token returns AuthError::TokenInvalid, (6) unknown Authorization scheme returns AuthError::TokenInvalid, (7) rate-limited IP returns AuthError::RateLimited with correct retry_after value, (8) failed validation records failure in rate limiter. Use a mock AuthProvider that returns configurable results. Create a real RateLimiter instance for rate limit tests. -->
  <!-- requirements: 4.1, 4.2, 4.3, 4.4, 4.5 -->
  <!-- leverage: packages/test-utils -->

---

## 6.9 — Rate Limiting
> spec: .odm/spec/auth-and-pocket-id/brief.md

- [x] Implement per-IP sliding window rate limiter
  <!-- file: packages/auth/src/handlers/rate_limit.rs -->
  <!-- purpose: Define RateLimiter struct with an internal HashMap<String, Vec<Instant>> tracking failure timestamps per IP address. Implement record_failure(ip: &str) that appends the current timestamp to the IP's failure list. Implement is_rate_limited(ip: &str) -> Option<u64> that: (1) removes entries older than 60 seconds from the IP's list (sliding window), (2) if remaining entries >= 5, return Some(seconds_until_oldest_entry_expires) as retry_after, (3) otherwise return None. Use RwLock for concurrent access from multiple transport tasks. The rate limiter is created during auth module initialization and shared via Arc. Add a cleanup method that periodically removes IPs with no recent failures to prevent memory growth. -->
  <!-- requirements: 5.1, 5.2, 5.3 -->
  <!-- leverage: none -->

- [x] Add rate limiter tests
  <!-- file: packages/auth/src/tests/rate_limit_test.rs -->
  <!-- purpose: Test scenarios: (1) first 4 failures from same IP do not trigger rate limit, (2) 5th failure triggers rate limit with retry_after > 0, (3) after waiting for the window to expire (mock time), IP is no longer rate-limited, (4) failures from different IPs are tracked independently, (5) retry_after value decreases as the window slides, (6) cleanup removes stale IP entries. Use controlled time injection for deterministic tests rather than real sleeps. -->
  <!-- requirements: 5.1, 5.2, 5.3 -->
  <!-- leverage: packages/test-utils -->

---

## 6.10 — API Key Management
> spec: .odm/spec/auth-and-pocket-id/brief.md

- [x] Implement API key CRUD handlers
  <!-- file: packages/auth/src/handlers/keys.rs -->
  <!-- purpose: Implement pub async fn create_key(storage: &dyn StorageBackend, name: String, scopes: Vec<String>, expires_at: Option<DateTime<Utc>>) -> Result<(String, ApiKeyRecord), AuthError> that: (1) generates a cryptographically random 32-byte key, (2) encodes as URL-safe base64 string (the raw key shown to user once), (3) generates a random 16-byte salt, (4) hashes the key with SHA-256 + salt using packages/crypto, (5) creates an ApiKeyRecord with the hash and salt (never the raw key), (6) stores in the "credentials" collection via StorageBackend with CredentialType::ApiKey, (7) returns the raw key string and the record. Implement pub async fn list_keys(storage) -> Result<Vec<ApiKeyRecord>> that returns metadata only (no hashes). Implement pub async fn revoke_key(storage, key_id) -> Result<()> that sets revoked = true. Implement validate_key(storage, raw_key) -> Result<AuthIdentity> that: hashes the provided key with each stored key's salt, compares using constant-time comparison, checks not revoked and not expired, updates last_used timestamp. -->
  <!-- requirements: 7.1, 7.2, 7.3, 7.4 -->
  <!-- leverage: packages/crypto for hashing -->

---

## 6.11 — API Key Management Tests
> spec: .odm/spec/auth-and-pocket-id/brief.md

- [x] Add tests for API key lifecycle
  <!-- file: packages/auth/src/tests/keys_test.rs -->
  <!-- purpose: Test scenarios: (1) create_key returns a raw key string and a record with hashed key, (2) validate_key with correct raw key returns AuthIdentity, (3) validate_key with wrong key returns AuthError::KeyInvalid, (4) revoke_key followed by validate_key returns AuthError::KeyRevoked, (5) expired key returns AuthError::KeyRevoked, (6) list_keys returns metadata without hashes, (7) each key gets a unique salt, (8) last_used is updated on successful validation. Use MockStorageContext for storage operations. -->
  <!-- requirements: 7.1, 7.2, 7.3, 7.4 -->
  <!-- leverage: packages/test-utils, mock storage from Phase 4 -->

---

## 6.12 — Plugin Auth Inheritance
> spec: .odm/spec/auth-and-pocket-id/brief.md

- [x] Verify pipeline messages carry authenticated identity through plugin execution
  <!-- file: packages/auth/src/tests/identity_test.rs -->
  <!-- purpose: Write integration tests verifying: (1) when a transport validates a request, the resulting AuthIdentity is attached to the PipelineMessage's metadata.auth_context field as serialized JSON, (2) when the workflow engine passes PipelineMessage to a plugin step, the auth_context is preserved, (3) plugins can read the auth_context from the PipelineMessage metadata but cannot modify it (auth_context is set by the transport layer and immutable through the pipeline), (4) credential storage operations require the identity in auth_context to have appropriate scopes, (5) unauthenticated requests (health endpoint) have auth_context = None. These tests verify the end-to-end auth inheritance chain: transport → workflow → plugin. -->
  <!-- requirements: 6.1, 6.2, 6.3 -->
  <!-- leverage: packages/types PipelineMessage -->
