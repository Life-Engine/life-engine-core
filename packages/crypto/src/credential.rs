//! Per-record credential encryption utilities.
//!
//! Provides HKDF-based key derivation for individual credential records,
//! plus encrypt/decrypt helpers that operate on the `claims` field of
//! credential documents. The derived key is distinct from the database
//! encryption key even though both originate from the same master passphrase.

use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::encryption;
use crate::error::CryptoError;

/// HKDF info context string — ensures credential keys are domain-separated
/// from all other keys derived from the same master secret.
const CREDENTIAL_HKDF_INFO: &[u8] = b"life-engine-credential-encryption-v1";

/// Derive a 32-byte encryption key for a specific credential record.
///
/// Uses HKDF-SHA256 (RFC 5869) with:
/// - IKM: the master key (32 bytes, derived from the user's passphrase)
/// - Salt: the credential ID (provides per-record uniqueness)
/// - Info: a fixed domain separator
///
/// The output is deterministic: the same master key + credential ID always
/// produces the same derived key, enabling transparent decryption on read.
pub fn derive_credential_key(
    master_key: &[u8; 32],
    credential_id: &str,
) -> Zeroizing<[u8; 32]> {
    let hk = Hkdf::<Sha256>::new(Some(credential_id.as_bytes()), master_key);
    let mut okm = Zeroizing::new([0u8; 32]);
    hk.expand(CREDENTIAL_HKDF_INFO, &mut *okm)
        .expect("32 bytes is a valid HKDF-SHA256 output length");
    okm
}

/// Encrypt a credential's claims field.
///
/// Serialises the `claims` JSON value to bytes, encrypts with AES-256-GCM
/// using a key derived from the master key and credential ID, and returns
/// the ciphertext as a base64-encoded string.
pub fn encrypt_claims(
    master_key: &[u8; 32],
    credential_id: &str,
    claims: &serde_json::Value,
) -> Result<String, CryptoError> {
    let key = derive_credential_key(master_key, credential_id);
    let plaintext = serde_json::to_vec(claims)
        .map_err(|e| CryptoError::EncryptionFailed(format!("failed to serialize claims: {e}")))?;
    let ciphertext = encryption::encrypt(&key, &plaintext)?;
    Ok(base64_encode(&ciphertext))
}

/// Decrypt a credential's claims field.
///
/// Decodes the base64 ciphertext, decrypts with AES-256-GCM using a key
/// derived from the master key and credential ID, and deserialises the
/// plaintext back to a JSON value.
pub fn decrypt_claims(
    master_key: &[u8; 32],
    credential_id: &str,
    encoded_ciphertext: &str,
) -> Result<serde_json::Value, CryptoError> {
    let key = derive_credential_key(master_key, credential_id);
    let ciphertext = base64_decode(encoded_ciphertext)
        .map_err(|e| CryptoError::DecryptionFailed(format!("invalid base64: {e}")))?;
    let plaintext = encryption::decrypt(&key, &ciphertext)?;
    serde_json::from_slice(&plaintext)
        .map_err(|e| CryptoError::DecryptionFailed(format!("failed to deserialize claims: {e}")))
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn base64_decode(s: &str) -> Result<Vec<u8>, base64::DecodeError> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_master_key() -> [u8; 32] {
        [0xAB; 32]
    }

    #[test]
    fn derive_key_is_deterministic() {
        let mk = test_master_key();
        let k1 = derive_credential_key(&mk, "cred-001");
        let k2 = derive_credential_key(&mk, "cred-001");
        assert_eq!(*k1, *k2);
    }

    #[test]
    fn different_credential_ids_produce_different_keys() {
        let mk = test_master_key();
        let k1 = derive_credential_key(&mk, "cred-001");
        let k2 = derive_credential_key(&mk, "cred-002");
        assert_ne!(*k1, *k2);
    }

    #[test]
    fn different_master_keys_produce_different_keys() {
        let k1 = derive_credential_key(&[0xAA; 32], "cred-001");
        let k2 = derive_credential_key(&[0xBB; 32], "cred-001");
        assert_ne!(*k1, *k2);
    }

    #[test]
    fn encrypt_decrypt_claims_round_trip() {
        let mk = test_master_key();
        let claims = json!({
            "access_token": "ghp_secret123",
            "refresh_token": "ghr_refresh456",
            "token_type": "Bearer"
        });

        let encrypted = encrypt_claims(&mk, "cred-001", &claims).unwrap();
        let decrypted = decrypt_claims(&mk, "cred-001", &encrypted).unwrap();

        assert_eq!(decrypted, claims);
    }

    #[test]
    fn decrypt_with_wrong_credential_id_fails() {
        let mk = test_master_key();
        let claims = json!({"secret": "value"});

        let encrypted = encrypt_claims(&mk, "cred-001", &claims).unwrap();
        let result = decrypt_claims(&mk, "cred-002", &encrypted);

        assert!(result.is_err());
    }

    #[test]
    fn decrypt_with_wrong_master_key_fails() {
        let claims = json!({"secret": "value"});

        let encrypted = encrypt_claims(&[0xAA; 32], "cred-001", &claims).unwrap();
        let result = decrypt_claims(&[0xBB; 32], "cred-001", &encrypted);

        assert!(result.is_err());
    }

    #[test]
    fn encrypt_produces_different_ciphertexts() {
        let mk = test_master_key();
        let claims = json!({"token": "abc"});

        let e1 = encrypt_claims(&mk, "cred-001", &claims).unwrap();
        let e2 = encrypt_claims(&mk, "cred-001", &claims).unwrap();

        // Different random nonces → different ciphertexts
        assert_ne!(e1, e2);

        // Both decrypt correctly
        assert_eq!(decrypt_claims(&mk, "cred-001", &e1).unwrap(), claims);
        assert_eq!(decrypt_claims(&mk, "cred-001", &e2).unwrap(), claims);
    }

    #[test]
    fn decrypt_invalid_base64_fails() {
        let mk = test_master_key();
        let result = decrypt_claims(&mk, "cred-001", "not-valid-base64!!!");
        assert!(result.is_err());
    }

    #[test]
    fn empty_claims_round_trip() {
        let mk = test_master_key();
        let claims = json!({});

        let encrypted = encrypt_claims(&mk, "cred-001", &claims).unwrap();
        let decrypted = decrypt_claims(&mk, "cred-001", &encrypted).unwrap();

        assert_eq!(decrypted, claims);
    }
}
