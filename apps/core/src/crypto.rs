//! Shared encryption utilities for the Core application.
//!
//! Centralises key derivation and AES-256-GCM authenticated encryption
//! used across the credential store, identity store, and other subsystems.
//! Each subsystem uses a distinct domain separator to ensure key
//! independence.
//!
//! AES-256-GCM encryption and HMAC operations delegate to the canonical
//! `life_engine_crypto` crate; this module adds HKDF-based domain key
//! derivation on top.

use hkdf::Hkdf;
use sha2::Sha256;

/// Derive a 32-byte key from a secret and domain separator using HKDF-SHA256.
///
/// Different domain separators produce independent keys from the same
/// secret, providing key isolation between subsystems. Uses HKDF (RFC 5869)
/// which provides proper domain separation via the `info` parameter.
pub fn derive_key(secret: &str, domain: &str) -> Vec<u8> {
    let hk = Hkdf::<Sha256>::new(None, secret.as_bytes());
    let mut okm = vec![0u8; 32];
    hk.expand(domain.as_bytes(), &mut okm)
        .expect("32 bytes is a valid HKDF-SHA256 output length");
    okm
}

/// Encrypt data using AES-256-GCM with a random nonce.
///
/// Returns the 12-byte nonce prepended to the ciphertext+tag.
/// The key must be exactly 32 bytes.
///
/// Delegates to [`life_engine_crypto::encrypt`].
pub fn encrypt(data: &[u8], key: &[u8]) -> Result<Vec<u8>, aes_gcm::Error> {
    let key_arr: [u8; 32] = key.try_into().expect("key must be 32 bytes");
    life_engine_crypto::encrypt(&key_arr, data).map_err(|_| aes_gcm::Error)
}

/// Decrypt data produced by [`encrypt`].
///
/// Expects the first 12 bytes to be the nonce, followed by the
/// ciphertext+tag. Returns an error if the data is too short,
/// the key is wrong, or the ciphertext has been tampered with.
///
/// Delegates to [`life_engine_crypto::decrypt`].
pub fn decrypt(data: &[u8], key: &[u8]) -> Result<Vec<u8>, aes_gcm::Error> {
    let key_arr: [u8; 32] = key.try_into().expect("key must be 32 bytes");
    life_engine_crypto::decrypt(&key_arr, data).map_err(|_| aes_gcm::Error)
}

/// Compute HMAC-SHA256 of data with a signing key.
///
/// Returns the hex-encoded MAC.
///
/// Delegates to [`life_engine_crypto::hmac_sign`].
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> String {
    hex::encode(life_engine_crypto::hmac_sign(key, data))
}

/// Verify an HMAC-SHA256 tag (hex-encoded) using constant-time comparison.
///
/// Delegates to [`life_engine_crypto::hmac_verify`].
pub fn hmac_sha256_verify(key: &[u8], data: &[u8], hex_tag: &str) -> bool {
    match hex::decode(hex_tag) {
        Ok(tag_bytes) => life_engine_crypto::hmac_verify(key, data, &tag_bytes),
        Err(_) => false,
    }
}

/// Domain separator for the plugin credential store.
#[allow(dead_code)]
pub const DOMAIN_CREDENTIAL_STORE: &str = "life-engine-credential-store-v1";

/// Domain separator for identity credential encryption.
#[allow(dead_code)]
pub const DOMAIN_IDENTITY_ENCRYPT: &str = "life-engine-identity-encrypt";

/// Domain separator for identity token signing.
#[allow(dead_code)]
pub const DOMAIN_IDENTITY_SIGN: &str = "life-engine-identity-sign";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_key_produces_32_bytes() {
        let key = derive_key("secret", "domain");
        assert_eq!(key.len(), 32);
    }

    #[test]
    fn derive_key_is_deterministic() {
        let k1 = derive_key("secret", "domain");
        let k2 = derive_key("secret", "domain");
        assert_eq!(k1, k2);
    }

    #[test]
    fn different_domains_produce_different_keys() {
        let k1 = derive_key("secret", DOMAIN_CREDENTIAL_STORE);
        let k2 = derive_key("secret", DOMAIN_IDENTITY_ENCRYPT);
        let k3 = derive_key("secret", DOMAIN_IDENTITY_SIGN);
        assert_ne!(k1, k2);
        assert_ne!(k2, k3);
        assert_ne!(k1, k3);
    }

    #[test]
    fn different_secrets_produce_different_keys() {
        let k1 = derive_key("secret-a", "domain");
        let k2 = derive_key("secret-b", "domain");
        assert_ne!(k1, k2);
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = derive_key("test-key", "test");
        let plaintext = b"hello world";
        let encrypted = encrypt(plaintext, &key).unwrap();
        let decrypted = decrypt(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn encrypt_changes_data() {
        let key = derive_key("test-key", "test");
        let plaintext = b"hello world";
        let encrypted = encrypt(plaintext, &key).unwrap();
        // Skip nonce (12 bytes) when comparing
        assert_ne!(&encrypted[12..], plaintext.as_slice());
    }

    #[test]
    fn encrypt_produces_unique_ciphertexts() {
        let key = derive_key("test-key", "test");
        let plaintext = b"hello world";
        let e1 = encrypt(plaintext, &key).unwrap();
        let e2 = encrypt(plaintext, &key).unwrap();
        // Different nonces should produce different ciphertexts
        assert_ne!(e1, e2);
    }

    #[test]
    fn decrypt_rejects_wrong_key() {
        let key1 = derive_key("key-a", "test");
        let key2 = derive_key("key-b", "test");
        let encrypted = encrypt(b"secret data", &key1).unwrap();
        assert!(decrypt(&encrypted, &key2).is_err());
    }

    #[test]
    fn decrypt_rejects_tampered_data() {
        let key = derive_key("test-key", "test");
        let mut encrypted = encrypt(b"secret data", &key).unwrap();
        // Flip a byte in the ciphertext
        let last = encrypted.len() - 1;
        encrypted[last] ^= 0xff;
        assert!(decrypt(&encrypted, &key).is_err());
    }

    #[test]
    fn decrypt_rejects_short_data() {
        let key = derive_key("test-key", "test");
        assert!(decrypt(b"short", &key).is_err());
    }

    #[test]
    fn hmac_sha256_is_deterministic() {
        let key = derive_key("key", "sign");
        let h1 = hmac_sha256(&key, b"data");
        let h2 = hmac_sha256(&key, b"data");
        assert_eq!(h1, h2);
    }

    #[test]
    fn hmac_sha256_different_data_differs() {
        let key = derive_key("key", "sign");
        let h1 = hmac_sha256(&key, b"data-a");
        let h2 = hmac_sha256(&key, b"data-b");
        assert_ne!(h1, h2);
    }

    #[test]
    fn hmac_sha256_different_keys_differ() {
        let k1 = derive_key("key-a", "sign");
        let k2 = derive_key("key-b", "sign");
        let h1 = hmac_sha256(&k1, b"data");
        let h2 = hmac_sha256(&k2, b"data");
        assert_ne!(h1, h2);
    }

    #[test]
    fn hmac_sha256_verify_accepts_correct_tag() {
        let key = derive_key("key", "sign");
        let tag = hmac_sha256(&key, b"data");
        assert!(hmac_sha256_verify(&key, b"data", &tag));
    }

    #[test]
    fn hmac_sha256_verify_rejects_wrong_tag() {
        let key = derive_key("key", "sign");
        let tag = hmac_sha256(&key, b"data");
        assert!(!hmac_sha256_verify(&key, b"wrong-data", &tag));
    }
}
