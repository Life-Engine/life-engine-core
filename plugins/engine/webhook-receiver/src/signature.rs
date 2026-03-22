//! HMAC-SHA256 signature verification for webhook payloads.
//!
//! Supports the common `sha256=<hex>` format used by GitHub, Stripe,
//! and many other webhook providers.

use anyhow::Result;
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Verify an HMAC-SHA256 signature against a raw body.
///
/// The signature is expected in `sha256=<hex>` format. If the signature
/// does not match, returns an error.
///
/// # Arguments
///
/// - `secret` — The shared secret key
/// - `body` — The raw request body bytes
/// - `signature` — The signature header value (e.g., `sha256=abc123...`)
pub fn verify_hmac_sha256(secret: &[u8], body: &[u8], signature: &str) -> Result<()> {
    let hex_sig = signature
        .strip_prefix("sha256=")
        .ok_or_else(|| anyhow::anyhow!("signature must start with 'sha256='"))?;

    let expected_bytes =
        hex::decode(hex_sig).map_err(|e| anyhow::anyhow!("invalid hex in signature: {}", e))?;

    let mut mac =
        HmacSha256::new_from_slice(secret).map_err(|e| anyhow::anyhow!("HMAC error: {}", e))?;
    mac.update(body);

    mac.verify_slice(&expected_bytes)
        .map_err(|_| anyhow::anyhow!("HMAC signature verification failed"))
}

/// Compute an HMAC-SHA256 signature for the given body, returning the
/// `sha256=<hex>` formatted string.
pub fn compute_hmac_sha256(secret: &[u8], body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC key should be valid");
    mac.update(body);
    let result = mac.finalize();
    format!("sha256={}", hex::encode(result.into_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_and_verify_roundtrip() {
        let secret = b"my-webhook-secret";
        let body = b"hello world payload";

        let signature = compute_hmac_sha256(secret, body);
        assert!(signature.starts_with("sha256="));

        let result = verify_hmac_sha256(secret, body, &signature);
        assert!(result.is_ok());
    }

    #[test]
    fn verify_rejects_wrong_secret() {
        let body = b"payload data";
        let signature = compute_hmac_sha256(b"correct-secret", body);

        let result = verify_hmac_sha256(b"wrong-secret", body, &signature);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("verification failed"));
    }

    #[test]
    fn verify_rejects_tampered_body() {
        let secret = b"my-secret";
        let signature = compute_hmac_sha256(secret, b"original body");

        let result = verify_hmac_sha256(secret, b"tampered body", &signature);
        assert!(result.is_err());
    }

    #[test]
    fn verify_rejects_missing_prefix() {
        let result = verify_hmac_sha256(b"secret", b"body", "no-prefix-hex");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("sha256="));
    }

    #[test]
    fn verify_rejects_invalid_hex() {
        let result = verify_hmac_sha256(b"secret", b"body", "sha256=not-valid-hex!!!");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("invalid hex"));
    }

    #[test]
    fn compute_produces_consistent_output() {
        let secret = b"consistent-key";
        let body = b"consistent-body";

        let sig1 = compute_hmac_sha256(secret, body);
        let sig2 = compute_hmac_sha256(secret, body);
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn compute_produces_different_output_for_different_inputs() {
        let secret = b"key";
        let sig1 = compute_hmac_sha256(secret, b"body-a");
        let sig2 = compute_hmac_sha256(secret, b"body-b");
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn verify_rejects_truncated_signature() {
        let secret = b"my-secret";
        let body = b"test body";
        let signature = compute_hmac_sha256(secret, body);

        // Truncate the hex portion
        let truncated = &signature[..signature.len() - 10];
        let result = verify_hmac_sha256(secret, body, truncated);
        assert!(result.is_err());
    }

    #[test]
    fn empty_body_still_produces_valid_signature() {
        let secret = b"secret";
        let body = b"";

        let signature = compute_hmac_sha256(secret, body);
        let result = verify_hmac_sha256(secret, body, &signature);
        assert!(result.is_ok());
    }
}
