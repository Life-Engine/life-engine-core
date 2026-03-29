//! Domain-separated key derivation and encryption.
//!
//! Uses HKDF-SHA256 to derive independent sub-keys from a master secret
//! for different subsystems (credential store, identity encryption, signing).
//! Migrated from `apps/core/src/crypto.rs` during architecture migration
//! (WP 10.4).

use hkdf::Hkdf;
use sha2::Sha256;

/// Derive a 32-byte key from a secret and domain separator using HKDF-SHA256.
///
/// Different domain separators produce independent keys from the same
/// secret, providing key isolation between subsystems. Uses HKDF (RFC 5869)
/// which provides proper domain separation via the `info` parameter.
pub fn derive_domain_key(secret: &str, domain: &str) -> Vec<u8> {
    let hk = Hkdf::<Sha256>::new(None, secret.as_bytes());
    let mut okm = vec![0u8; 32];
    hk.expand(domain.as_bytes(), &mut okm)
        .expect("32 bytes is a valid HKDF-SHA256 output length");
    okm
}

/// Encrypt data using AES-256-GCM with domain-derived key.
///
/// Returns the 12-byte nonce prepended to the ciphertext+tag.
pub fn domain_encrypt(data: &[u8], key: &[u8]) -> Result<Vec<u8>, crate::CryptoError> {
    let key_arr: [u8; 32] = key
        .try_into()
        .map_err(|_| crate::CryptoError::InvalidKeyLength)?;
    crate::encrypt(&key_arr, data)
}

/// Decrypt data produced by [`domain_encrypt`].
pub fn domain_decrypt(data: &[u8], key: &[u8]) -> Result<Vec<u8>, crate::CryptoError> {
    let key_arr: [u8; 32] = key
        .try_into()
        .map_err(|_| crate::CryptoError::InvalidKeyLength)?;
    crate::decrypt(&key_arr, data)
}

/// Compute HMAC-SHA256 of data with a signing key, returning hex-encoded MAC.
pub fn domain_hmac(key: &[u8], data: &[u8]) -> String {
    hex::encode(crate::hmac_sign(key, data))
}

/// Verify an HMAC-SHA256 tag (hex-encoded) using constant-time comparison.
pub fn domain_hmac_verify(key: &[u8], data: &[u8], hex_tag: &str) -> bool {
    match hex::decode(hex_tag) {
        Ok(tag_bytes) => crate::hmac_verify(key, data, &tag_bytes),
        Err(_) => false,
    }
}

/// Domain separator for the plugin credential store.
pub const DOMAIN_CREDENTIAL_STORE: &str = "life-engine-credential-store-v1";

/// Domain separator for identity credential encryption.
pub const DOMAIN_IDENTITY_ENCRYPT: &str = "life-engine-identity-encrypt";

/// Domain separator for identity token signing.
pub const DOMAIN_IDENTITY_SIGN: &str = "life-engine-identity-sign";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_key_produces_32_bytes() {
        let key = derive_domain_key("secret", "domain");
        assert_eq!(key.len(), 32);
    }

    #[test]
    fn derive_key_is_deterministic() {
        let k1 = derive_domain_key("secret", "domain");
        let k2 = derive_domain_key("secret", "domain");
        assert_eq!(k1, k2);
    }

    #[test]
    fn different_domains_produce_different_keys() {
        let k1 = derive_domain_key("secret", DOMAIN_CREDENTIAL_STORE);
        let k2 = derive_domain_key("secret", DOMAIN_IDENTITY_ENCRYPT);
        let k3 = derive_domain_key("secret", DOMAIN_IDENTITY_SIGN);
        assert_ne!(k1, k2);
        assert_ne!(k2, k3);
        assert_ne!(k1, k3);
    }

    #[test]
    fn different_secrets_produce_different_keys() {
        let k1 = derive_domain_key("secret-a", "domain");
        let k2 = derive_domain_key("secret-b", "domain");
        assert_ne!(k1, k2);
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = derive_domain_key("test-key", "test");
        let plaintext = b"hello world";
        let encrypted = domain_encrypt(plaintext, &key).unwrap();
        let decrypted = domain_decrypt(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn encrypt_changes_data() {
        let key = derive_domain_key("test-key", "test");
        let plaintext = b"hello world";
        let encrypted = domain_encrypt(plaintext, &key).unwrap();
        assert_ne!(&encrypted[12..], plaintext.as_slice());
    }

    #[test]
    fn encrypt_produces_unique_ciphertexts() {
        let key = derive_domain_key("test-key", "test");
        let plaintext = b"hello world";
        let e1 = domain_encrypt(plaintext, &key).unwrap();
        let e2 = domain_encrypt(plaintext, &key).unwrap();
        assert_ne!(e1, e2);
    }

    #[test]
    fn decrypt_rejects_wrong_key() {
        let key1 = derive_domain_key("key-a", "test");
        let key2 = derive_domain_key("key-b", "test");
        let encrypted = domain_encrypt(b"secret data", &key1).unwrap();
        assert!(domain_decrypt(&encrypted, &key2).is_err());
    }

    #[test]
    fn hmac_is_deterministic() {
        let key = derive_domain_key("key", "sign");
        let h1 = domain_hmac(&key, b"data");
        let h2 = domain_hmac(&key, b"data");
        assert_eq!(h1, h2);
    }

    #[test]
    fn hmac_verify_accepts_correct_tag() {
        let key = derive_domain_key("key", "sign");
        let tag = domain_hmac(&key, b"data");
        assert!(domain_hmac_verify(&key, b"data", &tag));
    }

    #[test]
    fn hmac_verify_rejects_wrong_tag() {
        let key = derive_domain_key("key", "sign");
        let tag = domain_hmac(&key, b"data");
        assert!(!domain_hmac_verify(&key, b"wrong-data", &tag));
    }
}
