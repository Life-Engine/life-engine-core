//! Argon2id key derivation utilities.
//!
//! Provides passphrase-based key derivation using Argon2id with parameters
//! tuned for Life Engine: 64 MB memory, 3 iterations, 4 lanes.

use argon2::{Algorithm, Argon2, Params, Version};
use rand::rngs::OsRng;
use rand::RngCore;

use crate::error::CryptoError;

/// Derives a 32-byte encryption key from a passphrase and salt using Argon2id.
///
/// Parameters: memory_cost = 65536 (64 MB), time_cost = 3, parallelism = 4.
pub fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; 32], CryptoError> {
    let params = Params::new(65536, 3, 4, Some(32))
        .map_err(|e| CryptoError::KeyDerivationFailed(e.to_string()))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = [0u8; 32];
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|e| CryptoError::KeyDerivationFailed(e.to_string()))?;

    Ok(key)
}

/// Generates a random 16-byte salt using `OsRng`.
pub fn generate_salt() -> [u8; 16] {
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    salt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_passphrase_and_salt_produces_same_key() {
        let salt = [0x42u8; 16];
        let key1 = derive_key("my-passphrase", &salt).unwrap();
        let key2 = derive_key("my-passphrase", &salt).unwrap();
        assert_eq!(key1, key2);
    }

    #[test]
    fn different_passphrases_produce_different_keys() {
        let salt = [0x42u8; 16];
        let key1 = derive_key("passphrase-one", &salt).unwrap();
        let key2 = derive_key("passphrase-two", &salt).unwrap();
        assert_ne!(key1, key2);
    }

    #[test]
    fn different_salts_produce_different_keys() {
        let salt1 = [0x01u8; 16];
        let salt2 = [0x02u8; 16];
        let key1 = derive_key("same-passphrase", &salt1).unwrap();
        let key2 = derive_key("same-passphrase", &salt2).unwrap();
        assert_ne!(key1, key2);
    }

    #[test]
    fn output_is_exactly_32_bytes() {
        let salt = generate_salt();
        let key = derive_key("test", &salt).unwrap();
        assert_eq!(key.len(), 32);
    }

    #[test]
    fn generate_salt_produces_unique_values() {
        let salt1 = generate_salt();
        let salt2 = generate_salt();
        assert_ne!(salt1, salt2);
    }
}
