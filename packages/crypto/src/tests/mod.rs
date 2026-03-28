//! Integration tests for crypto primitives.

use crate::{decrypt, derive_key, encrypt, hmac_sign, hmac_verify};

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

    let tag = hmac_sign(&key, data);
    assert!(hmac_verify(&key, data, &tag));

    // Tampered data fails
    assert!(!hmac_verify(&key, b"tampered data", &tag));
}
