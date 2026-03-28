# Cryptography Review Report

Reviewed: 2026-03-28

## Scope

This review covers all cryptographic code across Life Engine Core:

- `packages/crypto/` — shared encryption primitives crate
- `apps/core/src/crypto.rs` — application-level crypto utilities
- `apps/core/src/rekey.rs` — SQLCipher rekey workflow
- `plugins/engine/backup/src/crypto.rs` — backup encryption
- `packages/storage-sqlite/src/lib.rs` — key management in SQLite storage
- `packages/auth/src/handlers/keys.rs` — API key hashing and validation
- `plugins/engine/webhook-receiver/src/signature.rs` — webhook HMAC verification

## Summary

The cryptographic foundation is well-structured. The project uses well-audited Rust crates (`aes-gcm`, `argon2`, `hmac`, `sha2`, `hkdf`) rather than hand-rolled crypto. Algorithm choices are sound: AES-256-GCM for authenticated encryption, Argon2id for password hashing, HMAC-SHA256 for message authentication, and HKDF-SHA256 for key derivation. Nonces are generated from `OsRng` (CSPRNG), and HMAC verification uses constant-time comparison via the `hmac` crate's `verify_slice`.

However, there are several issues ranging from critical to minor that should be addressed.

## Problems Found

### Critical

- **C-1: Production code uses all-zeros salt for Argon2id key derivation** — `apps/core/src/sqlite_storage.rs:111` calls `crate::rekey::derive_key()`, which uses `[0u8; SALT_LENGTH]` as a fallback salt. This means all production SQLCipher databases derived from the same passphrase will have identical encryption keys, completely defeating the purpose of salt. The comment says "callers should use `derive_key_with_salt` with `load_or_create_salt`", but the only production call site uses the zero-salt path. File: `apps/core/src/rekey.rs:88-93`.

- **C-2: Backup plugin uses hardcoded fixed salt** — `plugins/engine/backup/src/crypto.rs:17` defines `const ARGON2_SALT: &[u8; 16] = b"life-engine-salt"`. Every backup encrypted with the same passphrase will derive the same key. This enables precomputed dictionary attacks against backups. Each backup archive should use a random salt stored in its header.

- **C-3: `rekey.rs` uses `thread_rng()` instead of `OsRng` for salt generation** — `apps/core/src/rekey.rs:31` uses `rand::thread_rng().fill_bytes()` to generate the salt for database encryption. `thread_rng()` is a userspace CSPRNG seeded from the OS, but `OsRng` is preferred for security-critical material because it reads directly from the OS entropy source on every call. The `packages/crypto/` crate correctly uses `OsRng`. This inconsistency should be resolved in favor of `OsRng`.

### Major

- **M-1: No key zeroization in `packages/crypto/` crate** — The `derive_key` function in `packages/crypto/src/kdf.rs` returns `[u8; 32]` on the stack. The `encrypt`/`decrypt` functions in `packages/crypto/src/encryption.rs` accept `&[u8; 32]` keys. None of the code in this crate uses `zeroize` to clear key material from memory after use. While `packages/storage-sqlite` does implement `Zeroize` on its `master_key` field and `Drop` trait, the crypto crate itself does not depend on `zeroize` at all. Derived keys in calling code may persist in memory indefinitely.

- **M-2: No zeroization of derived keys in `apps/core/src/crypto.rs`** — The `derive_key` function returns `Vec<u8>` containing the 32-byte key. This `Vec` is never zeroized. The `credential_store.rs` holds this key as `encryption_key` for the lifetime of the struct with no `Drop` implementation to clear it. Similarly, the hex-encoded key strings in `rekey.rs` (`current_key`, `new_key`) are `String` values that are not zeroized after use — `drop()` is called on passphrases but not on derived keys.

- **M-3: `apps/core/src/crypto.rs` HKDF uses `None` salt** — `Hkdf::<Sha256>::new(None, secret.as_bytes())` at line 19 uses no salt for the HKDF extract step. While HKDF without salt is still a valid PRF (RFC 5869 says the salt is optional and defaults to a string of zeroes), using a per-installation random salt would provide stronger key independence between different Core instances that happen to use the same master secret.

- **M-4: `apps/core/src/crypto.rs` panics on invalid key length** — `encrypt()` and `decrypt()` at lines 31 and 51 use `.expect("key must be 32 bytes")` on `Aes256Gcm::new_from_slice(key)`. If a caller passes a key of wrong length, this panics instead of returning an error. The `packages/crypto/` crate handles this correctly by returning `Result`. The core crypto module should do the same.

- **M-5: Duplicated encryption implementations** — AES-256-GCM encrypt/decrypt is implemented three times: in `packages/crypto/src/encryption.rs`, `apps/core/src/crypto.rs`, and `plugins/engine/backup/src/crypto.rs`. This creates maintenance risk. Changes or fixes to one copy may not propagate. The `packages/crypto/` crate was created to centralize this, but the other two locations have not been migrated.

- **M-6: `apps/core/src/crypto.rs` HMAC function lacks constant-time verification** — The `hmac_sha256` function at line 59 returns a hex-encoded `String`. There is no corresponding verification function. Any caller comparing this output with `==` on strings will perform variable-time comparison, leaking timing information. The `packages/crypto/` crate's `hmac_verify` correctly delegates to `verify_slice` (constant-time). `apps/core/src/identity.rs:597` calls `hmac_sha256` to produce signatures — how these are verified later determines whether timing attacks are possible.

### Minor

- **m-1: Inconsistent Argon2id parameters between `packages/crypto/` and `apps/core/`** — The `packages/crypto/src/kdf.rs` hardcodes `Params::new(65536, 3, 4, Some(32))` (64 MB, 3 iterations, 4 lanes). The `apps/core/src/config.rs` defines `Argon2Settings` with the same defaults but allows runtime override via config. The two KDF call sites could diverge silently — one is configurable, the other is not.

- **m-2: `packages/crypto/src/kdf.rs` does not enforce minimum salt length** — `derive_key` accepts `&[u8]` for the salt with no minimum length check. Argon2 requires at least 8 bytes of salt. While the `argon2` crate will reject salts that are too short, adding an explicit check with a descriptive error would be more helpful to callers.

- **m-3: Passphrase confirmation comparison in `rekey.rs` is not constant-time** — `apps/core/src/rekey.rs:188` uses `new_passphrase != confirm_passphrase` to compare user-entered passphrases. This is a variable-time string comparison. However, since both values come from the same user in a local terminal session, timing side-channel risk is negligible here. Noted for completeness.

- **m-4: Error messages may leak operational details** — `CryptoError::EncryptionFailed(String)` and `CryptoError::DecryptionFailed(String)` in `packages/crypto/src/error.rs` include the underlying library error as a string. In contexts where errors are returned to external callers (API responses), this could leak implementation details. The HMAC error `HmacVerificationFailed` correctly omits details.

- **m-5: `packages/crypto/` has no `#[deny(unsafe_code)]` attribute** — While no `unsafe` code is present, adding this attribute would provide a compile-time guarantee that no unsafe blocks are introduced in future changes.

- **m-6: `rand` version 0.8 is used; 0.9 is available** — The workspace uses `rand = "0.8"`. Version 0.9 includes API improvements. Not a security issue since both versions use the same underlying OS entropy sources, but worth noting for dependency hygiene.

## File-by-File Analysis

### `packages/crypto/Cargo.toml`

Dependencies are all well-maintained, widely audited crates. No `zeroize` dependency is declared — this should be added.

### `packages/crypto/src/lib.rs`

Clean module structure. Re-exports the public API: `encrypt`, `decrypt`, `derive_key`, `generate_salt`, `hmac_sign`, `hmac_verify`, `CryptoError`. No issues.

### `packages/crypto/src/encryption.rs`

- Uses `Aes256Gcm` (AES-256-GCM), correct AEAD choice
- Nonce generated via `Aes256Gcm::generate_nonce(&mut OsRng)` — correct use of CSPRNG
- Output format `nonce || ciphertext || tag` is standard and well-documented
- `NONCE_SIZE` correctly set to 12 bytes (96-bit nonce for GCM)
- Length check on decrypt input prevents panics on short input
- Key accepted as `&[u8; 32]` — type-safe, prevents wrong-length keys at compile time
- Test coverage is thorough: round-trip, empty plaintext, nonce uniqueness, wrong key, truncation, corruption, output size

### `packages/crypto/src/hmac.rs`

- Uses `Hmac<Sha256>` from the `hmac` crate — correct construction
- `hmac_verify` uses `mac.verify_slice(tag)` which performs constant-time comparison internally
- Returns `bool` rather than `Result` — acceptable for this API, prevents error-message-based oracle
- `hmac_sign` returns `Vec<u8>` (raw bytes) — callers can hex-encode as needed
- Test coverage good: consistency, verification, rejection, truncation, key/data independence

### `packages/crypto/src/kdf.rs`

- Argon2id with Version `V0x13` (latest) — correct algorithm choice
- Parameters: 64 MB memory, 3 iterations, 4 lanes — meets OWASP 2023 recommendations
- Salt generation uses `OsRng.fill_bytes()` — correct CSPRNG
- 16-byte salt — adequate (NIST SP 800-132 recommends at least 16 bytes)
- Returns `[u8; 32]` — no heap allocation, but no zeroization either
- No minimum passphrase length enforcement — may be acceptable at this layer, but callers should validate
- Test includes a "golden value" / parameter-pinning test — good for detecting accidental parameter changes

### `packages/crypto/src/error.rs`

- Uses `thiserror` for ergonomic error types
- `DecryptionFailed(String)` includes inner error detail — could leak info if surfaced to users (see m-4)
- `HmacVerificationFailed` has no message — correct, prevents oracle attacks

### `packages/crypto/src/types.rs`

Empty module. No issues.

### `packages/crypto/src/tests/mod.rs`

Integration tests cover the full pipeline: derive key then encrypt/decrypt, wrong key rejection, HMAC with derived key. Adequate coverage.

### `apps/core/src/crypto.rs`

- Duplicates `encrypt`/`decrypt` from `packages/crypto/` (see M-5)
- `derive_key` uses `Hkdf::<Sha256>::new(None, ...)` — no salt (see M-3)
- Domain separation via `info` parameter is well-designed: `DOMAIN_CREDENTIAL_STORE`, `DOMAIN_IDENTITY_ENCRYPT`, `DOMAIN_IDENTITY_SIGN`
- `hmac_sha256` returns hex-encoded string with no verify counterpart (see M-6)
- `encrypt`/`decrypt` panic on wrong key length instead of returning errors (see M-4)
- Returns `Vec<u8>` for derived key with no zeroization (see M-2)
- Good test coverage for all functions

### `apps/core/src/rekey.rs`

- Argon2id key derivation is correct: `Algorithm::Argon2id`, `Version::V0x13`, configurable parameters
- `generate_salt` uses `thread_rng()` instead of `OsRng` (see C-3)
- `derive_key` fallback with zero salt is dangerous (see C-1)
- `load_or_create_salt` stores salt in `<db_path>.db.salt` — correct separation from DB file
- `run_rekey` drops passphrases after derivation — good practice
- `run_rekey` does NOT zeroize derived hex keys after use
- SQLCipher PRAGMA format uses `x'<hex>'` raw key syntax — correct, avoids passphrase mode
- Rekey workflow: open with old key, `PRAGMA rekey`, close, verify with new key — correct and robust
- Hex key validation in `sqlite_storage.rs:116-121` before interpolation into PRAGMA — good injection prevention
- Good test coverage including double-rekey scenario

### `plugins/engine/backup/src/crypto.rs`

- Hardcoded salt `b"life-engine-salt"` (see C-2)
- Otherwise sound: Argon2id, AES-256-GCM with OsRng nonces, proper length checking on decrypt
- SHA-256 checksum function is straightforward and correct
- Compression (gzip) is applied *before* encryption — correct order (compress-then-encrypt)
- Full pipeline test covers compress-encrypt-decrypt-decompress

### `packages/storage-sqlite/src/lib.rs`

- Implements `Zeroize` on `master_key` field — good
- `Drop` implementation calls `self.master_key.zeroize()` — good
- `rekey` method zeroizes old key before setting new one — correct
- Key material handled as `[u8; 32]` — type-safe
- `master_key()` returns `&[u8; 32]` via `pub(crate)` — appropriately restricted visibility
- `Debug` implementation uses `finish_non_exhaustive()` to avoid printing key material — good

### `packages/auth/src/handlers/keys.rs`

- API keys hashed with `hmac_sign(&salt, raw_key.as_bytes())` — HMAC used as a keyed hash, reasonable approach
- Verification uses `hmac_verify` (constant-time) — correct
- Salt is per-key and stored base64-encoded — correct
- Linear scan over all keys is O(n) — acceptable for reasonable key counts, but does reveal total key count via timing

### `plugins/engine/webhook-receiver/src/signature.rs`

- Uses `mac.verify_slice()` for constant-time comparison — correct
- Supports `sha256=<hex>` format — standard for GitHub/Stripe webhooks
- Error messages are generic ("verification failed") — no oracle
- Good test coverage including prefix validation, hex validation, truncation

## Recommendations

1. **Fix the zero-salt fallback immediately.** Make `open_encrypted` in `sqlite_storage.rs` accept a database path and use `load_or_create_salt` to get a proper random salt. Remove or gate the `derive_key` zero-salt convenience function behind `#[cfg(test)]`.

2. **Replace the hardcoded backup salt** with a random salt stored in the backup archive header. Each backup should derive its key independently.

3. **Replace `thread_rng()` with `OsRng`** in `apps/core/src/rekey.rs:31` for salt generation.

4. **Add `zeroize` as a dependency to `packages/crypto/`** and use `Zeroizing<[u8; 32]>` as the return type for `derive_key`. Audit all call sites for key lifetime.

5. **Consolidate encryption to `packages/crypto/`**. Remove duplicate implementations in `apps/core/src/crypto.rs` and `plugins/engine/backup/src/crypto.rs`. Add HKDF-based domain separation to the shared crate if needed.

6. **Add a constant-time HMAC verification function** to `apps/core/src/crypto.rs` or migrate all callers to `packages/crypto::hmac_verify`.

7. **Replace panics with `Result` returns** in `apps/core/src/crypto.rs` encrypt/decrypt for invalid key lengths.

8. **Add `#![deny(unsafe_code)]`** to `packages/crypto/src/lib.rs`.

9. **Consider adding a HKDF salt** (random, per-installation) to the `apps/core/src/crypto.rs` `derive_key` function for stronger key independence across Core instances.
