//! Argon2id key derivation utilities.
//!
//! Provides passphrase-based key derivation using Argon2id with configurable
//! parameters. Defaults: 64 MB memory, 3 iterations, 4 lanes.

use argon2::{Algorithm, Argon2, Params, Version};
use rand::rngs::OsRng;
use rand::TryRngCore;
use zeroize::Zeroizing;

use crate::error::CryptoError;
use crate::types::Argon2Params;

/// Derives a 32-byte encryption key from a passphrase and salt using Argon2id
/// with default parameters (64 MB memory, 3 iterations, 4 parallelism).
///
/// The returned key is wrapped in [`Zeroizing`] so it is automatically
/// cleared from memory when dropped.
pub fn derive_key(passphrase: &str, salt: &[u8]) -> Result<Zeroizing<[u8; 32]>, CryptoError> {
    derive_key_with_params(passphrase, salt, &Argon2Params::default())
}

/// Derives a 32-byte encryption key from a passphrase and salt using Argon2id
/// with the given parameters.
///
/// The returned key is wrapped in [`Zeroizing`] so it is automatically
/// cleared from memory when dropped.
pub fn derive_key_with_params(
    passphrase: &str,
    salt: &[u8],
    params: &Argon2Params,
) -> Result<Zeroizing<[u8; 32]>, CryptoError> {
    let argon2_params = Params::new(params.memory_kib, params.iterations, params.parallelism, Some(32))
        .map_err(|e| CryptoError::KeyDerivationFailed(e.to_string()))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon2_params);

    let mut key = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, &mut *key)
        .map_err(|e| CryptoError::KeyDerivationFailed(e.to_string()))?;

    Ok(key)
}

/// Generates a random 16-byte salt using `OsRng`.
pub fn generate_salt() -> [u8; 16] {
    let mut salt = [0u8; 16];
    OsRng.try_fill_bytes(&mut salt).expect("OS RNG should not fail");
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

    #[test]
    fn custom_params_produce_valid_key() {
        let params = Argon2Params {
            memory_kib: 8192,
            iterations: 1,
            parallelism: 1,
        };
        let salt = [0xBB; 16];
        let key = derive_key_with_params("test-passphrase", &salt, &params).unwrap();
        assert_eq!(key.len(), 32);
    }

    #[test]
    fn custom_params_differ_from_defaults() {
        let salt = [0xCC; 16];
        let passphrase = "same-passphrase";

        let default_key = derive_key(passphrase, &salt).unwrap();
        let custom_key = derive_key_with_params(
            passphrase,
            &salt,
            &Argon2Params {
                memory_kib: 8192,
                iterations: 1,
                parallelism: 1,
            },
        )
        .unwrap();

        assert_ne!(*default_key, *custom_key);
    }

    #[test]
    fn invalid_params_return_error() {
        let params = Argon2Params {
            memory_kib: 0,
            iterations: 0,
            parallelism: 0,
        };
        let result = derive_key_with_params("test", &[0; 16], &params);
        assert!(result.is_err());
    }

    #[test]
    fn default_params_match_expected_values() {
        let params = Argon2Params::default();
        assert_eq!(params.memory_kib, 65536);
        assert_eq!(params.iterations, 3);
        assert_eq!(params.parallelism, 4);
    }
}
