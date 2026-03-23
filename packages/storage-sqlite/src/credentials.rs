//! Per-credential encryption for the credentials collection.
//!
//! Provides defence-in-depth by encrypting individual credential `claims`
//! using AES-256-GCM on top of SQLCipher database-level encryption. Each
//! credential gets a unique derived key produced via HMAC(master_key, credential_id).

use life_engine_crypto::{encrypt, decrypt, hmac_sign};

use crate::error::StorageError;

/// Derives a per-credential encryption key from the master key and credential ID.
///
/// Uses HMAC-SHA256(master_key, credential_id_bytes) to produce a deterministic
/// 32-byte key unique to each credential.
fn derive_credential_key(master_key: &[u8; 32], credential_id: &str) -> [u8; 32] {
    let tag = hmac_sign(master_key, credential_id.as_bytes());
    let mut key = [0u8; 32];
    key.copy_from_slice(&tag[..32]);
    key
}

/// Encrypts the `claims` field of a credential JSON object before storage.
///
/// Extracts the `id` field to derive a per-credential key, encrypts the
/// `claims` value using AES-256-GCM, replaces `claims` with the hex-encoded
/// ciphertext, and sets `encrypted: true`.
///
/// Returns the modified JSON string ready for storage.
pub fn encrypt_credential(
    master_key: &[u8; 32],
    data_json: &str,
) -> Result<String, StorageError> {
    let mut doc: serde_json::Value = serde_json::from_str(data_json)?;

    let credential_id = doc
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| StorageError::CredentialEncryption {
            credential_id: "unknown".into(),
            message: "missing 'id' field in credential".into(),
        })?
        .to_string();

    let claims = &doc["claims"];
    let claims_bytes = serde_json::to_vec(claims).map_err(|e| {
        StorageError::CredentialEncryption {
            credential_id: credential_id.clone(),
            message: format!("failed to serialize claims: {e}"),
        }
    })?;

    let key = derive_credential_key(master_key, &credential_id);
    let ciphertext = encrypt(&key, &claims_bytes).map_err(|e| {
        StorageError::CredentialEncryption {
            credential_id: credential_id.clone(),
            message: format!("encryption failed: {e}"),
        }
    })?;

    // Replace claims with hex-encoded ciphertext and set encrypted flag.
    doc["claims"] = serde_json::Value::String(hex::encode(&ciphertext));
    doc["encrypted"] = serde_json::Value::Bool(true);

    serde_json::to_string(&doc).map_err(StorageError::Serialization)
}

/// Decrypts the `claims` field of a credential JSON object after reading.
///
/// Checks the `encrypted` flag; if `true`, decrypts the hex-encoded
/// ciphertext in `claims` back to the original JSON value.
///
/// Returns the modified JSON string with plaintext claims and `encrypted`
/// removed for the caller.
pub fn decrypt_credential(
    master_key: &[u8; 32],
    data_json: &str,
) -> Result<String, StorageError> {
    let mut doc: serde_json::Value = serde_json::from_str(data_json)?;

    // Only decrypt if the encrypted flag is set.
    let is_encrypted = doc
        .get("encrypted")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !is_encrypted {
        return Ok(data_json.to_string());
    }

    let credential_id = doc
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| StorageError::CredentialEncryption {
            credential_id: "unknown".into(),
            message: "missing 'id' field in credential".into(),
        })?
        .to_string();

    let claims_hex = doc
        .get("claims")
        .and_then(|v| v.as_str())
        .ok_or_else(|| StorageError::CredentialEncryption {
            credential_id: credential_id.clone(),
            message: "missing or non-string 'claims' field in encrypted credential".into(),
        })?;

    let ciphertext = hex::decode(claims_hex).map_err(|e| {
        StorageError::CredentialEncryption {
            credential_id: credential_id.clone(),
            message: format!("invalid hex in encrypted claims: {e}"),
        }
    })?;

    let key = derive_credential_key(master_key, &credential_id);
    let plaintext = decrypt(&key, &ciphertext).map_err(|e| {
        StorageError::CredentialEncryption {
            credential_id: credential_id.clone(),
            message: format!("decryption failed: {e}"),
        }
    })?;

    let claims_value: serde_json::Value =
        serde_json::from_slice(&plaintext).map_err(|e| {
            StorageError::CredentialEncryption {
                credential_id: credential_id.clone(),
                message: format!("failed to deserialize decrypted claims: {e}"),
            }
        })?;

    // Restore plaintext claims and remove the encrypted flag.
    doc["claims"] = claims_value;
    doc.as_object_mut()
        .map(|obj| obj.remove("encrypted"));

    serde_json::to_string(&doc).map_err(StorageError::Serialization)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_credential_json(id: &str) -> String {
        serde_json::json!({
            "id": id,
            "name": "Test OAuth Token",
            "credential_type": "oauth_token",
            "service": "google",
            "claims": {
                "access_token": "ya29.a0AfH6SMB",
                "refresh_token": "1//0eXyz",
                "token_type": "Bearer",
                "expires_in": 3600
            },
            "source": "google-calendar-plugin",
            "source_id": "cred-1",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        })
        .to_string()
    }

    #[test]
    fn encrypt_decrypt_round_trip() {
        let master_key = [0x42u8; 32];
        let id = "550e8400-e29b-41d4-a716-446655440000";
        let original = sample_credential_json(id);

        let encrypted = encrypt_credential(&master_key, &original).unwrap();

        // Verify encrypted flag is set.
        let enc_doc: serde_json::Value = serde_json::from_str(&encrypted).unwrap();
        assert_eq!(enc_doc["encrypted"], true);
        // Claims should be a hex string, not the original object.
        assert!(enc_doc["claims"].is_string());

        let decrypted = decrypt_credential(&master_key, &encrypted).unwrap();
        let dec_doc: serde_json::Value = serde_json::from_str(&decrypted).unwrap();
        let orig_doc: serde_json::Value = serde_json::from_str(&original).unwrap();

        // Claims should match the original.
        assert_eq!(dec_doc["claims"], orig_doc["claims"]);
        // Encrypted flag should be removed.
        assert!(dec_doc.get("encrypted").is_none());
    }

    #[test]
    fn decrypt_with_wrong_key_fails() {
        let master_key = [0x42u8; 32];
        let wrong_key = [0x99u8; 32];
        let id = "550e8400-e29b-41d4-a716-446655440000";
        let original = sample_credential_json(id);

        let encrypted = encrypt_credential(&master_key, &original).unwrap();
        let result = decrypt_credential(&wrong_key, &encrypted);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, StorageError::CredentialEncryption { .. }));
    }

    #[test]
    fn unencrypted_credential_passes_through() {
        let master_key = [0x42u8; 32];
        let original = sample_credential_json("some-id");

        // No encrypted flag — should pass through unchanged.
        let result = decrypt_credential(&master_key, &original).unwrap();
        let orig_doc: serde_json::Value = serde_json::from_str(&original).unwrap();
        let res_doc: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(orig_doc["claims"], res_doc["claims"]);
    }

    #[test]
    fn different_credentials_get_different_keys() {
        let master_key = [0x42u8; 32];
        let id_a = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
        let id_b = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";

        let json_a = sample_credential_json(id_a);
        let json_b = sample_credential_json(id_b);

        let enc_a = encrypt_credential(&master_key, &json_a).unwrap();
        let enc_b = encrypt_credential(&master_key, &json_b).unwrap();

        let doc_a: serde_json::Value = serde_json::from_str(&enc_a).unwrap();
        let doc_b: serde_json::Value = serde_json::from_str(&enc_b).unwrap();

        // Encrypted claims should differ due to different derived keys (and random nonces).
        assert_ne!(doc_a["claims"], doc_b["claims"]);
    }
}
