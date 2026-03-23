//! AES-256-GCM encryption utilities.
//!
//! Provides encrypt and decrypt functions using AES-256-GCM with random nonces.
//! Output format: `nonce (12 bytes) || ciphertext || tag (16 bytes)`.

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, AeadCore, Nonce,
};

use crate::error::CryptoError;

/// The size of the AES-256-GCM nonce in bytes.
const NONCE_SIZE: usize = 12;

/// Encrypts plaintext using AES-256-GCM with a random nonce.
///
/// Returns `nonce || ciphertext || tag` as a single `Vec<u8>`.
/// A fresh 12-byte nonce is generated for every call using `OsRng`.
pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;

    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;

    let mut output = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    output.extend_from_slice(&nonce);
    output.extend_from_slice(&ciphertext);

    Ok(output)
}

/// Decrypts ciphertext produced by [`encrypt`].
///
/// Expects the input format: `nonce (12 bytes) || ciphertext || tag`.
pub fn decrypt(key: &[u8; 32], ciphertext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if ciphertext.len() < NONCE_SIZE {
        return Err(CryptoError::DecryptionFailed(
            "ciphertext too short to contain nonce".to_string(),
        ));
    }

    let (nonce_bytes, encrypted) = ciphertext.split_at(NONCE_SIZE);
    let nonce = Nonce::from_slice(nonce_bytes);

    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| CryptoError::DecryptionFailed(e.to_string()))?;

    cipher
        .decrypt(nonce, encrypted)
        .map_err(|e| CryptoError::DecryptionFailed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        [0xAB; 32]
    }

    #[test]
    fn encrypt_decrypt_round_trip() {
        let key = test_key();
        let plaintext = b"hello, life engine";

        let encrypted = encrypt(&key, plaintext).unwrap();
        let decrypted = decrypt(&key, &encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn encrypt_decrypt_empty_plaintext() {
        let key = test_key();
        let plaintext = b"";

        let encrypted = encrypt(&key, plaintext).unwrap();
        let decrypted = decrypt(&key, &encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn different_encryptions_produce_different_output() {
        let key = test_key();
        let plaintext = b"same input";

        let enc1 = encrypt(&key, plaintext).unwrap();
        let enc2 = encrypt(&key, plaintext).unwrap();

        // Different nonces mean different ciphertext
        assert_ne!(enc1, enc2);

        // But both decrypt to the same plaintext
        assert_eq!(decrypt(&key, &enc1).unwrap(), plaintext);
        assert_eq!(decrypt(&key, &enc2).unwrap(), plaintext);
    }

    #[test]
    fn decrypt_with_wrong_key_fails() {
        let key = test_key();
        let wrong_key = [0xCD; 32];
        let plaintext = b"secret data";

        let encrypted = encrypt(&key, plaintext).unwrap();
        let result = decrypt(&wrong_key, &encrypted);

        assert!(result.is_err());
    }

    #[test]
    fn decrypt_truncated_ciphertext_fails() {
        let result = decrypt(&test_key(), &[0u8; 5]);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("too short")
        );
    }

    #[test]
    fn decrypt_corrupted_ciphertext_fails() {
        let key = test_key();
        let plaintext = b"important data";

        let mut encrypted = encrypt(&key, plaintext).unwrap();
        // Flip a byte in the ciphertext portion
        let last = encrypted.len() - 1;
        encrypted[last] ^= 0xFF;

        assert!(decrypt(&key, &encrypted).is_err());
    }

    #[test]
    fn output_contains_nonce_prefix() {
        let key = test_key();
        let encrypted = encrypt(&key, b"test").unwrap();

        // Output must be at least nonce (12) + tag (16) bytes
        assert!(encrypted.len() >= NONCE_SIZE + 16);
    }
}
