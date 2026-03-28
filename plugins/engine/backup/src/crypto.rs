//! Encryption and decryption for backup archives.
//!
//! Uses Argon2id for key derivation (same algorithm as SQLCipher),
//! AES-256-GCM (via `life_engine_crypto`) for authenticated encryption,
//! and gzip for compression.

use crate::types::Argon2Params;
use anyhow::Result;
use rand::rngs::OsRng;
use rand::TryRngCore;
use sha2::{Digest, Sha256};

/// Length of the derived key in bytes (256-bit for AES-256).
const KEY_LENGTH: usize = 32;

/// Length of the random salt in bytes.
const SALT_LENGTH: usize = 16;

/// Generate a random 16-byte salt using OS entropy.
pub fn generate_salt() -> [u8; SALT_LENGTH] {
    let mut salt = [0u8; SALT_LENGTH];
    OsRng
        .try_fill_bytes(&mut salt)
        .expect("OS RNG should not fail");
    salt
}

/// Derive a 32-byte encryption key from a passphrase and salt using Argon2id.
pub fn derive_key(passphrase: &str, salt: &[u8], params: &Argon2Params) -> Result<Vec<u8>> {
    let argon2 = argon2::Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::new(
            params.memory_mb * 1024,
            params.iterations,
            params.parallelism,
            Some(KEY_LENGTH),
        )
        .map_err(|e| anyhow::anyhow!("invalid Argon2 parameters: {e}"))?,
    );

    let mut output = [0u8; KEY_LENGTH];
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, &mut output)
        .map_err(|e| anyhow::anyhow!("Argon2 key derivation failed: {e}"))?;

    Ok(output.to_vec())
}

/// Compress data using gzip.
pub fn compress(data: &[u8]) -> Result<Vec<u8>> {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    let compressed = encoder.finish()?;
    Ok(compressed)
}

/// Decompress gzip data.
pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    use flate2::read::GzDecoder;
    use std::io::Read;

    let mut decoder = GzDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

/// Encrypt data with AES-256-GCM.
///
/// The output format is: `[12-byte nonce][ciphertext+tag]`.
/// The key must be exactly 32 bytes.
///
/// Delegates to [`life_engine_crypto::encrypt`].
pub fn encrypt(plaintext: &[u8], key: &[u8]) -> Result<Vec<u8>> {
    let key_arr: [u8; 32] = key
        .try_into()
        .map_err(|_| anyhow::anyhow!("encryption key must be 32 bytes"))?;
    life_engine_crypto::encrypt(&key_arr, plaintext)
        .map_err(|e| anyhow::anyhow!("AES-256-GCM encryption failed: {e}"))
}

/// Decrypt data that was encrypted with [`encrypt`].
///
/// Verifies the AES-256-GCM authentication tag before returning plaintext.
///
/// Delegates to [`life_engine_crypto::decrypt`].
pub fn decrypt(encrypted: &[u8], key: &[u8]) -> Result<Vec<u8>> {
    let key_arr: [u8; 32] = key
        .try_into()
        .map_err(|_| anyhow::anyhow!("encryption key must be 32 bytes"))?;
    life_engine_crypto::decrypt(&key_arr, encrypted).map_err(|e| {
        anyhow::anyhow!(
            "backup integrity check failed: authentication failed (wrong passphrase or corrupted data): {e}"
        )
    })
}

/// Compute SHA-256 checksum of data.
pub fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_params() -> Argon2Params {
        Argon2Params {
            memory_mb: 1,
            iterations: 1,
            parallelism: 1,
        }
    }

    fn test_salt() -> [u8; 16] {
        [0x42u8; 16]
    }

    #[test]
    fn derive_key_produces_32_bytes() {
        let key = derive_key("test-passphrase", &test_salt(), &test_params()).unwrap();
        assert_eq!(key.len(), 32);
    }

    #[test]
    fn derive_key_is_deterministic() {
        let salt = test_salt();
        let k1 = derive_key("my-pass", &salt, &test_params()).unwrap();
        let k2 = derive_key("my-pass", &salt, &test_params()).unwrap();
        assert_eq!(k1, k2);
    }

    #[test]
    fn derive_key_different_passphrases_differ() {
        let salt = test_salt();
        let k1 = derive_key("pass-a", &salt, &test_params()).unwrap();
        let k2 = derive_key("pass-b", &salt, &test_params()).unwrap();
        assert_ne!(k1, k2);
    }

    #[test]
    fn derive_key_different_salts_differ() {
        let salt1 = [0x01u8; 16];
        let salt2 = [0x02u8; 16];
        let k1 = derive_key("same-pass", &salt1, &test_params()).unwrap();
        let k2 = derive_key("same-pass", &salt2, &test_params()).unwrap();
        assert_ne!(k1, k2);
    }

    #[test]
    fn generate_salt_produces_unique_values() {
        let s1 = generate_salt();
        let s2 = generate_salt();
        assert_ne!(s1, s2);
    }

    #[test]
    fn compress_decompress_roundtrip() {
        let data = b"Hello, World! This is test data for compression.";
        let compressed = compress(data).unwrap();
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn compress_reduces_size_for_repetitive_data() {
        let data = "AAAA".repeat(1000);
        let compressed = compress(data.as_bytes()).unwrap();
        assert!(compressed.len() < data.len());
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = derive_key("test-pass", &test_salt(), &test_params()).unwrap();
        let plaintext = b"secret backup data";
        let encrypted = encrypt(plaintext, &key).unwrap();
        assert_ne!(encrypted, plaintext);
        let decrypted = decrypt(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn encrypt_decrypt_large_data() {
        let key = derive_key("test-pass", &test_salt(), &test_params()).unwrap();
        let plaintext: Vec<u8> = (0..10000).map(|i| (i % 256) as u8).collect();
        let encrypted = encrypt(&plaintext, &key).unwrap();
        let decrypted = decrypt(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn decrypt_with_wrong_key_fails() {
        let salt = test_salt();
        let key1 = derive_key("correct-pass", &salt, &test_params()).unwrap();
        let key2 = derive_key("wrong-pass", &salt, &test_params()).unwrap();
        let plaintext = b"secret data";
        let encrypted = encrypt(plaintext, &key1).unwrap();
        let result = decrypt(&encrypted, &key2);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("authentication failed"));
    }

    #[test]
    fn decrypt_tampered_data_fails() {
        let key = derive_key("test-pass", &test_salt(), &test_params()).unwrap();
        let plaintext = b"secret data";
        let mut encrypted = encrypt(plaintext, &key).unwrap();
        // Tamper with ciphertext.
        if encrypted.len() > 20 {
            encrypted[15] ^= 0xFF;
        }
        let result = decrypt(&encrypted, &key);
        assert!(result.is_err());
    }

    #[test]
    fn decrypt_too_short_data_fails() {
        let key = derive_key("test-pass", &test_salt(), &test_params()).unwrap();
        let result = decrypt(&[0u8; 10], &key);
        assert!(result.is_err());
    }

    #[test]
    fn sha256_hex_produces_64_chars() {
        let hash = sha256_hex(b"test data");
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn sha256_hex_is_deterministic() {
        let h1 = sha256_hex(b"same data");
        let h2 = sha256_hex(b"same data");
        assert_eq!(h1, h2);
    }

    #[test]
    fn sha256_hex_different_data_differs() {
        let h1 = sha256_hex(b"data-a");
        let h2 = sha256_hex(b"data-b");
        assert_ne!(h1, h2);
    }

    #[test]
    fn full_pipeline_compress_encrypt_decrypt_decompress() {
        let key = derive_key("pipeline-test", &test_salt(), &test_params()).unwrap();
        let original = serde_json::json!({
            "records": [
                {"id": "1", "data": {"title": "Test"}},
                {"id": "2", "data": {"title": "Another"}}
            ]
        });
        let json_bytes = serde_json::to_vec(&original).unwrap();

        // Compress -> Encrypt
        let compressed = compress(&json_bytes).unwrap();
        let encrypted = encrypt(&compressed, &key).unwrap();

        // Decrypt -> Decompress
        let decrypted = decrypt(&encrypted, &key).unwrap();
        let decompressed = decompress(&decrypted).unwrap();
        let restored: serde_json::Value = serde_json::from_slice(&decompressed).unwrap();

        assert_eq!(original, restored);
    }
}
