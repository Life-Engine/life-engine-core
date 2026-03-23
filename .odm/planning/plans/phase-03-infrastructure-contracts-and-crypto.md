<!--
project: life-engine-core
phase: 3
specs: canonical-data-models (traits), plugin-sdk-rs (traits), binary-and-startup (crypto)
updated: 2026-03-23
-->

# Phase 3 — Infrastructure Contracts and Crypto

## Plan Overview

This phase implements the two foundational crates that sit between types and all higher-level modules: `packages/traits` (infrastructure contracts) and `packages/crypto` (shared encryption primitives). The traits crate defines `EngineError`, `StorageBackend`, `Transport`, and `Plugin` — the contracts that every module implements. The crypto crate provides AES-256-GCM encryption, Argon2id key derivation, and HMAC utilities used by storage, auth, and plugins.

This phase depends on Phase 2 (types must be defined). Phases 4-9 depend on the contracts and crypto primitives defined here.

> spec: .odm/spec/canonical-data-models/brief.md, .odm/spec/plugin-sdk-rs/brief.md, .odm/spec/binary-and-startup/brief.md

Progress: 0 / 10 work packages complete

---

## 3.1 — Traits Crate Foundation
> spec: .odm/spec/plugin-sdk-rs/brief.md

- [x] Define EngineError trait with code, severity, and source_module
  <!-- file: packages/traits/src/error.rs -->
  <!-- purpose: Define the EngineError trait that all module error types must implement. Methods: fn code(&self) -> &str (structured error code like "STORAGE_001", "AUTH_002", "WORKFLOW_003"), fn severity(&self) -> Severity (Fatal, Retryable, Warning), fn source_module(&self) -> &str (module name like "storage-sqlite", "auth", "workflow-engine"). Define Severity enum with three variants: Fatal (abort pipeline, run error handler), Retryable (retry up to configured limit, then fail), Warning (log and continue). EngineError must extend std::error::Error + Send + Sync + 'static. Add Display impl for Severity. Add is_fatal(), is_retryable(), is_warning() convenience methods on Severity. Re-export from lib.rs. -->
  <!-- requirements: from plugin-sdk-rs spec 1.1 -->
  <!-- leverage: none -->

---

## 3.2 — StorageBackend Trait
> depends: 3.1
> spec: .odm/spec/data-layer/brief.md

- [x] Define StorageBackend trait with execute and mutate methods
  <!-- file: packages/traits/src/storage.rs -->
  <!-- purpose: Define the async StorageBackend trait with two methods: async fn execute(&self, query: StorageQuery) -> Result<Vec<PipelineMessage>, Box<dyn EngineError>> for reads, and async fn mutate(&self, op: StorageMutation) -> Result<(), Box<dyn EngineError>> for writes. Also define async fn init(config: toml::Value, key: [u8; 32]) -> Result<Self, Box<dyn EngineError>> as an associated function for initialization. Import StorageQuery and StorageMutation from life-engine-types. Use async-trait for async trait methods. Re-export from lib.rs. -->
  <!-- requirements: from data-layer spec 1.1 -->
  <!-- leverage: none -->

---

## 3.3 — StorageQuery and StorageMutation Types
> spec: .odm/spec/data-layer/brief.md

- [x] Define StorageQuery and StorageMutation types in the types crate
  <!-- file: packages/types/src/storage.rs -->
  <!-- purpose: Define StorageQuery struct with fields: collection (String), plugin_id (String), filters (Vec<QueryFilter>), sort (Vec<SortField>), limit (Option<u32> with max 1000), offset (Option<u32>). Define QueryFilter struct with field (String), operator (FilterOp enum: Eq, Gte, Lte, Contains, NotEq), value (serde_json::Value). Define SortField struct with field (String), direction (SortDirection enum: Asc, Desc). Define StorageMutation enum with variants: Insert { plugin_id: String, collection: String, data: PipelineMessage }, Update { plugin_id: String, collection: String, id: Uuid, data: PipelineMessage, expected_version: u64 }, Delete { plugin_id: String, collection: String, id: Uuid }. The expected_version on Update enables optimistic concurrency control. Add serde derives on all types. Re-export from packages/types/src/lib.rs. -->
  <!-- requirements: from data-layer spec 1.1, 1.2 -->
  <!-- leverage: none -->

---

## 3.4 — Transport Trait
> depends: 3.1
> spec: .odm/spec/binary-and-startup/brief.md

- [x] Define Transport trait for protocol-specific entry points
  <!-- file: packages/traits/src/transport.rs -->
  <!-- purpose: Define the async Transport trait. Methods: async fn start(&self, config: toml::Value) -> Result<(), Box<dyn EngineError>> to bind and begin serving, async fn stop(&self) -> Result<(), Box<dyn EngineError>> for graceful shutdown, fn name(&self) -> &str returning the transport identifier (e.g., "rest", "graphql", "caldav"). Transports receive a reference to the workflow engine and auth module at construction time so they can route requests through workflows and validate authentication. Define TransportConfig struct with common fields: bind_address (String, default "127.0.0.1"), port (u16), tls (Option<TlsConfig> with cert_path and key_path). Re-export from lib.rs. -->
  <!-- requirements: from binary-and-startup spec -->
  <!-- leverage: none -->

---

## 3.5 — Plugin Trait
> depends: 3.1
> spec: .odm/spec/plugin-system/brief.md

- [ ] Define Plugin trait for WASM plugin contracts
  <!-- file: packages/traits/src/plugin.rs -->
  <!-- purpose: Define the Plugin trait with methods: fn id(&self) -> &str (unique plugin identifier), fn display_name(&self) -> &str (human-readable name), fn version(&self) -> &str (semver version string), fn actions(&self) -> Vec<Action> (list of declared actions), fn execute(&self, action: &str, input: PipelineMessage) -> Result<PipelineMessage, Box<dyn EngineError>> (execute a named action). Define Action struct with fields: name (String), description (String), input_schema (Option<String> — JSON Schema for input validation), output_schema (Option<String> — JSON Schema for output validation). Add serde derives on Action. The Plugin trait is what WASM modules implement via the SDK. Re-export Plugin and Action from lib.rs. -->
  <!-- requirements: from plugin-system spec, plugin-sdk-rs spec 1.6 -->
  <!-- leverage: none -->

---

## 3.6 — Capability Types
> depends: 3.1
> spec: .odm/spec/capability-enforcement/brief.md

- [ ] Define Capability enum and CapabilityViolation error
  <!-- file: packages/traits/src/capability.rs -->
  <!-- purpose: Define Capability enum with six variants: StorageRead, StorageWrite, HttpOutbound, EventsEmit, EventsSubscribe, ConfigRead. Implement Display (lowercase colon-separated: "storage:read", "storage:write", "http:outbound", "events:emit", "events:subscribe", "config:read") and FromStr (parse the display strings back to enum values). Define CapabilityViolation error struct with fields: capability (Capability), plugin_id (String), context (String — what the plugin was trying to do). Implement EngineError for CapabilityViolation: code() returns "CAP_001" for load-time violations and "CAP_002" for runtime violations, severity() returns Severity::Fatal, source_module() returns "capability-enforcement". Implement std::error::Error and Display. Add unit tests for Display/FromStr round-trip and error field correctness. Re-export from lib.rs. -->
  <!-- requirements: from capability-enforcement spec 4.1, 4.2, 4.3, 4.4, 6.1 -->
  <!-- leverage: none -->

---

## 3.7 — Traits Crate Public API and Tests
> depends: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6
> spec: .odm/spec/plugin-sdk-rs/brief.md

- [ ] Finalize traits crate public API and add comprehensive tests
  <!-- file: packages/traits/src/lib.rs -->
  <!-- file: packages/traits/src/tests/mod.rs -->
  <!-- purpose: Ensure lib.rs re-exports all public types: EngineError, Severity, StorageBackend, StorageQuery, StorageMutation (from types), Transport, TransportConfig, Plugin, Action, Capability, CapabilityViolation. Write unit tests verifying: Severity display formatting, EngineError trait is object-safe, Capability FromStr/Display round-trip for all 6 variants, CapabilityViolation error codes are correct, Action struct serialization/deserialization. Ensure the crate compiles with no warnings under clippy -D warnings. -->
  <!-- requirements: from plugin-sdk-rs spec 1.1 -->
  <!-- leverage: none -->

---

## 3.8 — AES-256-GCM Encryption Primitives
> spec: .odm/spec/binary-and-startup/brief.md

- [ ] Implement AES-256-GCM encrypt and decrypt functions
  <!-- file: packages/crypto/src/encryption.rs -->
  <!-- purpose: Implement pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>> that generates a random 12-byte nonce, encrypts using AES-256-GCM, and prepends the nonce to the ciphertext (output format: nonce || ciphertext || tag). Implement pub fn decrypt(key: &[u8; 32], ciphertext: &[u8]) -> Result<Vec<u8>> that splits the nonce from the ciphertext and decrypts. Use the aes-gcm crate. Never reuse nonces — always generate fresh random nonces via OsRng. Define CryptoError enum with variants: EncryptionFailed, DecryptionFailed, InvalidKeyLength, InvalidCiphertext. Implement std::error::Error for CryptoError. Re-export encrypt and decrypt from lib.rs. -->
  <!-- requirements: from data-layer spec 6.1 -->
  <!-- leverage: none -->

---

## 3.9 — Argon2id Key Derivation
> spec: .odm/spec/binary-and-startup/brief.md

- [ ] Implement Argon2id key derivation function
  <!-- file: packages/crypto/src/kdf.rs -->
  <!-- purpose: Implement pub fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; 32]> using Argon2id with parameters: memory_cost = 65536 (64 MB), time_cost = 3 iterations, parallelism = 4 lanes, output length = 32 bytes. These parameters match the ARCHITECTURE.md and binary-and-startup spec requirements. Also implement pub fn generate_salt() -> [u8; 16] using OsRng. Use the argon2 crate. Add unit tests: same passphrase + salt produces same key (deterministic), different passphrases produce different keys, different salts produce different keys, output is exactly 32 bytes. Re-export from lib.rs. -->
  <!-- requirements: from binary-and-startup spec 3.1, 3.2 -->
  <!-- leverage: none -->

---

## 3.10 — HMAC Utilities and Crypto Tests
> spec: .odm/spec/binary-and-startup/brief.md

- [ ] Implement HMAC-SHA256 sign and verify functions
  <!-- file: packages/crypto/src/hmac.rs -->
  <!-- purpose: Implement pub fn hmac_sign(key: &[u8], data: &[u8]) -> Vec<u8> that produces an HMAC-SHA256 tag. Implement pub fn hmac_verify(key: &[u8], data: &[u8], tag: &[u8]) -> bool that verifies an HMAC tag using constant-time comparison. Use the hmac and sha2 crates. Add unit tests: sign produces consistent output, verify accepts correct tag, verify rejects wrong tag, verify rejects truncated tag. Re-export from lib.rs. -->
  <!-- requirements: from auth-and-pocket-id spec (webhook signature verification) -->
  <!-- leverage: none -->

- [ ] Finalize crypto crate public API
  <!-- file: packages/crypto/src/lib.rs -->
  <!-- purpose: Ensure lib.rs re-exports: encrypt, decrypt from encryption module; derive_key, generate_salt from kdf module; hmac_sign, hmac_verify from hmac module; CryptoError from error module. Verify the crate compiles with clippy -D warnings. Ensure no unsafe code is used anywhere in the crate. -->
  <!-- requirements: from binary-and-startup spec -->
  <!-- leverage: none -->
