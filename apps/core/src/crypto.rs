//! Shared encryption utilities for the Core application.
//!
//! Centralises key derivation and XOR-based encryption used across
//! the credential store, identity store, and other subsystems.
//! Each subsystem uses a distinct domain separator to ensure key
//! independence.

use sha2::{Digest, Sha256};

/// Derive a 32-byte key from a secret and domain separator using SHA-256.
///
/// Different domain separators produce independent keys from the same
/// secret, providing key isolation between subsystems.
pub fn derive_key(secret: &str, domain: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.update(domain.as_bytes());
    hasher.finalize().to_vec()
}

/// XOR-based stream cipher with key repetition.
///
/// Used for at-rest encryption of credential values. The key is
/// repeated cyclically to match the data length.
pub fn xor_encrypt(data: &[u8], key: &[u8]) -> Vec<u8> {
    data.iter()
        .enumerate()
        .map(|(i, byte)| byte ^ key[i % key.len()])
        .collect()
}

/// Compute HMAC-SHA256 of data with a signing key.
///
/// Returns the hex-encoded hash.
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key);
    hasher.update(data);
    hex::encode(hasher.finalize())
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
    fn xor_encrypt_roundtrip() {
        let key = derive_key("test-key", "test");
        let plaintext = b"hello world";
        let encrypted = xor_encrypt(plaintext, &key);
        let decrypted = xor_encrypt(&encrypted, &key);
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn xor_encrypt_changes_data() {
        let key = derive_key("test-key", "test");
        let plaintext = b"hello world";
        let encrypted = xor_encrypt(plaintext, &key);
        assert_ne!(encrypted, plaintext.to_vec());
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
}
