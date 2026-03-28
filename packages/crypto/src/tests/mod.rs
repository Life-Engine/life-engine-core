//! Integration tests for crypto primitives.

use crate::{decrypt, derive_key, derive_key_with_params, encrypt, hmac_sha256, hmac_sign, hmac_verify, Argon2Params};

#[test]
fn derive_key_then_encrypt_decrypt_round_trip() {
    let salt = [0x01u8; 16];
    let key = derive_key("integration-test-passphrase", &salt).unwrap();
    let plaintext = b"sensitive credential data";

    let ciphertext = encrypt(&key, plaintext).unwrap();
    let recovered = decrypt(&key, &ciphertext).unwrap();

    assert_eq!(recovered, plaintext);
}

#[test]
fn wrong_derived_key_fails_decryption() {
    let salt = [0x01u8; 16];
    let key = derive_key("correct-passphrase", &salt).unwrap();
    let wrong_key = derive_key("wrong-passphrase", &salt).unwrap();

    let ciphertext = encrypt(&key, b"secret").unwrap();
    assert!(decrypt(&wrong_key, &ciphertext).is_err());
}

#[test]
fn hmac_integrity_with_derived_key() {
    let salt = [0x02u8; 16];
    let key = derive_key("hmac-test-passphrase", &salt).unwrap();
    let data = b"data to authenticate";

    let tag = hmac_sign(key.as_ref(), data);
    assert!(hmac_verify(key.as_ref(), data, &tag));

    // Tampered data fails
    assert!(!hmac_verify(key.as_ref(), b"tampered data", &tag));
}

#[test]
fn hmac_sha256_returns_32_bytes() {
    let tag = hmac_sha256(b"key", b"data");
    assert_eq!(tag.len(), 32);
}

#[test]
fn hmac_sha256_consistent_with_hmac_sign() {
    let key = b"shared-key";
    let data = b"shared-data";
    let tag_fixed = hmac_sha256(key, data);
    let tag_vec = hmac_sign(key, data);
    assert_eq!(tag_fixed.as_slice(), tag_vec.as_slice());
}

#[test]
fn custom_params_encrypt_decrypt_round_trip() {
    let params = Argon2Params {
        memory_kib: 8192,
        iterations: 1,
        parallelism: 1,
    };
    let salt = [0x03u8; 16];
    let key = derive_key_with_params("custom-params-passphrase", &salt, &params).unwrap();
    let plaintext = b"encrypted with custom params";

    let ciphertext = encrypt(&key, plaintext).unwrap();
    let recovered = decrypt(&key, &ciphertext).unwrap();

    assert_eq!(recovered, plaintext);
}
