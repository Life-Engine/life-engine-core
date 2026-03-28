//! HMAC-SHA256 signing and verification utilities.

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Produce an HMAC-SHA256 tag as a fixed-size 32-byte array.
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut mac =
        HmacSha256::new_from_slice(key).expect("HMAC-SHA256 accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().into()
}

/// Produce an HMAC-SHA256 tag for the given data.
pub fn hmac_sign(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac =
        HmacSha256::new_from_slice(key).expect("HMAC-SHA256 accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

/// Verify an HMAC-SHA256 tag using constant-time comparison.
///
/// Returns `true` if the tag is valid, `false` otherwise.
pub fn hmac_verify(key: &[u8], data: &[u8], tag: &[u8]) -> bool {
    let mut mac =
        HmacSha256::new_from_slice(key).expect("HMAC-SHA256 accepts any key length");
    mac.update(data);
    mac.verify_slice(tag).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_produces_consistent_output() {
        let key = b"test-secret-key";
        let data = b"hello world";
        let tag1 = hmac_sign(key, data);
        let tag2 = hmac_sign(key, data);
        assert_eq!(tag1, tag2);
        assert_eq!(tag1.len(), 32); // SHA-256 output is 32 bytes
    }

    #[test]
    fn verify_accepts_correct_tag() {
        let key = b"test-secret-key";
        let data = b"hello world";
        let tag = hmac_sign(key, data);
        assert!(hmac_verify(key, data, &tag));
    }

    #[test]
    fn verify_rejects_wrong_tag() {
        let key = b"test-secret-key";
        let data = b"hello world";
        let mut tag = hmac_sign(key, data);
        tag[0] ^= 0xff; // flip bits in the first byte
        assert!(!hmac_verify(key, data, &tag));
    }

    #[test]
    fn verify_rejects_truncated_tag() {
        let key = b"test-secret-key";
        let data = b"hello world";
        let tag = hmac_sign(key, data);
        let truncated = &tag[..16];
        assert!(!hmac_verify(key, data, truncated));
    }

    #[test]
    fn different_keys_produce_different_tags() {
        let data = b"hello world";
        let tag1 = hmac_sign(b"key-one", data);
        let tag2 = hmac_sign(b"key-two", data);
        assert_ne!(tag1, tag2);
    }

    #[test]
    fn different_data_produces_different_tags() {
        let key = b"test-secret-key";
        let tag1 = hmac_sign(key, b"data one");
        let tag2 = hmac_sign(key, b"data two");
        assert_ne!(tag1, tag2);
    }
}
