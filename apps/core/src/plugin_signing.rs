//! Plugin signing and verification using Ed25519.
//!
//! Provides Ed25519 signing of `.wasm` plugin bundles, signature verification
//! before plugin loading, key revocation, manifest hash inclusion in signatures,
//! and verification tier classification.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::Path;
use tracing::{info, warn};

/// Verification tier displayed in the plugin store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VerificationTier {
    /// Community plugin, no review — unverified.
    Unverified,
    /// Community-reviewed, basic checks passed.
    Reviewed,
    /// Maintained by the project team.
    Official,
}

impl std::fmt::Display for VerificationTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerificationTier::Unverified => write!(f, "unverified"),
            VerificationTier::Reviewed => write!(f, "reviewed"),
            VerificationTier::Official => write!(f, "official"),
        }
    }
}

/// A plugin signature file stored alongside the `.wasm` bundle.
///
/// Contains the Ed25519 signature over `SHA-256(wasm_bytes || manifest_hash)`,
/// the public key that produced it, and the manifest hash for tamper detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSignature {
    /// Hex-encoded Ed25519 signature bytes.
    pub signature: String,
    /// Hex-encoded Ed25519 public key (32 bytes).
    pub public_key: String,
    /// Hex-encoded SHA-256 hash of the manifest content.
    pub manifest_hash: String,
    /// Verification tier assigned to this plugin.
    pub tier: VerificationTier,
}

/// Result of verifying a plugin's signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationResult {
    /// Signature is valid and the key is not revoked.
    Valid {
        tier: VerificationTier,
        public_key: String,
    },
    /// The `.wasm` file or manifest has been tampered with.
    TamperedBundle,
    /// The signature is cryptographically invalid.
    InvalidSignature,
    /// The signing key has been revoked.
    RevokedKey,
    /// No signature file found — plugin is unsigned.
    Unsigned,
    /// Signature file is malformed.
    MalformedSignature(String),
}

/// Manages revoked public keys.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RevocationList {
    /// Hex-encoded public keys that have been revoked.
    pub revoked_keys: HashSet<String>,
}

impl RevocationList {
    /// Create a new empty revocation list.
    pub fn new() -> Self {
        Self {
            revoked_keys: HashSet::new(),
        }
    }

    /// Load a revocation list from a JSON file.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let list: RevocationList = serde_json::from_str(&contents)?;
        Ok(list)
    }

    /// Save the revocation list to a JSON file.
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(path, contents)?;
        Ok(())
    }

    /// Add a public key to the revocation list.
    pub fn revoke(&mut self, public_key_hex: &str) {
        self.revoked_keys.insert(public_key_hex.to_string());
    }

    /// Check whether a public key has been revoked.
    pub fn is_revoked(&self, public_key_hex: &str) -> bool {
        self.revoked_keys.contains(public_key_hex)
    }
}

/// Configuration for plugin signature verification.
#[derive(Debug, Clone)]
pub struct SignatureVerifierConfig {
    /// Whether to allow unsigned plugins (requires explicit opt-in).
    pub allow_unsigned: bool,
    /// Revocation list of compromised keys.
    pub revocation_list: RevocationList,
}

impl Default for SignatureVerifierConfig {
    fn default() -> Self {
        Self {
            allow_unsigned: false,
            revocation_list: RevocationList::new(),
        }
    }
}

/// Compute the signing payload: `SHA-256(wasm_bytes || manifest_hash_bytes)`.
///
/// The manifest hash is included in the signed payload so that post-signing
/// tampering of capability declarations is detected.
fn compute_signing_payload(wasm_bytes: &[u8], manifest_hash: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(wasm_bytes);
    hasher.update(manifest_hash);
    hasher.finalize().to_vec()
}

/// Compute the SHA-256 hash of manifest content.
pub fn compute_manifest_hash(manifest_bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(manifest_bytes);
    hex::encode(hasher.finalize())
}

/// Sign a WASM plugin bundle with an Ed25519 signing key.
///
/// The signature covers `SHA-256(wasm_bytes || manifest_hash)` to bind the
/// WASM binary to its manifest, preventing post-signing capability tampering.
pub fn sign_plugin(
    signing_key: &SigningKey,
    wasm_bytes: &[u8],
    manifest_bytes: &[u8],
    tier: VerificationTier,
) -> PluginSignature {
    let manifest_hash = compute_manifest_hash(manifest_bytes);
    let manifest_hash_bytes = hex::decode(&manifest_hash).expect("hex decode of own hash");
    let payload = compute_signing_payload(wasm_bytes, &manifest_hash_bytes);

    let signature = signing_key.sign(&payload);
    let public_key = signing_key.verifying_key();

    PluginSignature {
        signature: hex::encode(signature.to_bytes()),
        public_key: hex::encode(public_key.to_bytes()),
        manifest_hash,
        tier,
    }
}

/// Verify a plugin's signature against its WASM binary and manifest.
pub fn verify_plugin(
    plugin_sig: &PluginSignature,
    wasm_bytes: &[u8],
    manifest_bytes: &[u8],
    config: &SignatureVerifierConfig,
) -> VerificationResult {
    // Decode the public key.
    let pk_bytes = match hex::decode(&plugin_sig.public_key) {
        Ok(b) => b,
        Err(e) => {
            return VerificationResult::MalformedSignature(format!(
                "invalid public key hex: {e}"
            ));
        }
    };
    let pk_array: [u8; 32] = match pk_bytes.try_into() {
        Ok(a) => a,
        Err(_) => {
            return VerificationResult::MalformedSignature(
                "public key must be 32 bytes".to_string(),
            );
        }
    };
    let verifying_key = match VerifyingKey::from_bytes(&pk_array) {
        Ok(k) => k,
        Err(e) => {
            return VerificationResult::MalformedSignature(format!(
                "invalid public key: {e}"
            ));
        }
    };

    // Check revocation list.
    if config.revocation_list.is_revoked(&plugin_sig.public_key) {
        warn!(
            public_key = %plugin_sig.public_key,
            "plugin signed with revoked key"
        );
        return VerificationResult::RevokedKey;
    }

    // Decode the signature.
    let sig_bytes = match hex::decode(&plugin_sig.signature) {
        Ok(b) => b,
        Err(e) => {
            return VerificationResult::MalformedSignature(format!(
                "invalid signature hex: {e}"
            ));
        }
    };
    let sig_array: [u8; 64] = match sig_bytes.try_into() {
        Ok(a) => a,
        Err(_) => {
            return VerificationResult::MalformedSignature(
                "signature must be 64 bytes".to_string(),
            );
        }
    };
    let signature = Signature::from_bytes(&sig_array);

    // Verify the manifest hash matches the actual manifest.
    let actual_manifest_hash = compute_manifest_hash(manifest_bytes);
    if actual_manifest_hash != plugin_sig.manifest_hash {
        warn!("manifest hash mismatch — plugin manifest has been tampered with");
        return VerificationResult::TamperedBundle;
    }

    // Recompute the signing payload and verify.
    let manifest_hash_bytes =
        hex::decode(&plugin_sig.manifest_hash).expect("hex decode of validated hash");
    let payload = compute_signing_payload(wasm_bytes, &manifest_hash_bytes);

    match verifying_key.verify(&payload, &signature) {
        Ok(()) => {
            info!(
                public_key = %plugin_sig.public_key,
                tier = %plugin_sig.tier,
                "plugin signature verified"
            );
            VerificationResult::Valid {
                tier: plugin_sig.tier,
                public_key: plugin_sig.public_key.clone(),
            }
        }
        Err(_) => VerificationResult::TamperedBundle,
    }
}

/// Check whether an unsigned plugin should be allowed to load.
///
/// Returns `Ok(())` if unsigned plugins are allowed via configuration,
/// otherwise returns an error describing the requirement.
pub fn check_unsigned_policy(config: &SignatureVerifierConfig) -> anyhow::Result<()> {
    if config.allow_unsigned {
        warn!("loading unsigned plugin — user has explicitly opted in");
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "unsigned plugins are not allowed — set allow_unsigned_plugins = true to opt in"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    /// Helper: generate a fresh Ed25519 signing key.
    fn generate_key() -> SigningKey {
        let mut rng = rand::thread_rng();
        SigningKey::generate(&mut rng)
    }

    /// Helper: create sample WASM bytes (not real WASM, just test data).
    fn sample_wasm() -> Vec<u8> {
        b"\x00asm\x01\x00\x00\x00sample-plugin-bytes".to_vec()
    }

    /// Helper: create sample manifest bytes.
    fn sample_manifest() -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "id": "com.example.test",
            "display_name": "Test Plugin",
            "version": "1.0.0",
            "capabilities": ["StorageRead"]
        }))
        .unwrap()
    }

    // ── Test 1: Valid signature passes verification ──────────────

    #[test]
    fn valid_signature_passes_verification() {
        let key = generate_key();
        let wasm = sample_wasm();
        let manifest = sample_manifest();
        let config = SignatureVerifierConfig::default();

        let sig = sign_plugin(&key, &wasm, &manifest, VerificationTier::Official);
        let result = verify_plugin(&sig, &wasm, &manifest, &config);

        assert_eq!(
            result,
            VerificationResult::Valid {
                tier: VerificationTier::Official,
                public_key: hex::encode(key.verifying_key().to_bytes()),
            }
        );
    }

    #[test]
    fn valid_signature_with_reviewed_tier() {
        let key = generate_key();
        let wasm = sample_wasm();
        let manifest = sample_manifest();
        let config = SignatureVerifierConfig::default();

        let sig = sign_plugin(&key, &wasm, &manifest, VerificationTier::Reviewed);
        let result = verify_plugin(&sig, &wasm, &manifest, &config);

        assert!(matches!(
            result,
            VerificationResult::Valid {
                tier: VerificationTier::Reviewed,
                ..
            }
        ));
    }

    // ── Test 2: Tampered WASM file rejected ─────────────────────

    #[test]
    fn tampered_wasm_rejected() {
        let key = generate_key();
        let wasm = sample_wasm();
        let manifest = sample_manifest();
        let config = SignatureVerifierConfig::default();

        let sig = sign_plugin(&key, &wasm, &manifest, VerificationTier::Official);

        // Tamper with the WASM bytes.
        let mut tampered_wasm = wasm.clone();
        tampered_wasm.push(0xFF);

        let result = verify_plugin(&sig, &tampered_wasm, &manifest, &config);
        assert_eq!(result, VerificationResult::TamperedBundle);
    }

    #[test]
    fn tampered_manifest_rejected() {
        let key = generate_key();
        let wasm = sample_wasm();
        let manifest = sample_manifest();
        let config = SignatureVerifierConfig::default();

        let sig = sign_plugin(&key, &wasm, &manifest, VerificationTier::Official);

        // Tamper with the manifest (e.g., adding a capability).
        let tampered_manifest = serde_json::to_vec(&serde_json::json!({
            "id": "com.example.test",
            "display_name": "Test Plugin",
            "version": "1.0.0",
            "capabilities": ["StorageRead", "StorageWrite"]
        }))
        .unwrap();

        let result = verify_plugin(&sig, &wasm, &tampered_manifest, &config);
        assert_eq!(result, VerificationResult::TamperedBundle);
    }

    // ── Test 3: Unsigned plugin triggers warning and requires opt-in ──

    #[test]
    fn unsigned_plugin_rejected_without_opt_in() {
        let config = SignatureVerifierConfig {
            allow_unsigned: false,
            ..Default::default()
        };

        let result = check_unsigned_policy(&config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("unsigned plugins are not allowed"));
    }

    #[test]
    fn unsigned_plugin_allowed_with_opt_in() {
        let config = SignatureVerifierConfig {
            allow_unsigned: true,
            ..Default::default()
        };

        let result = check_unsigned_policy(&config);
        assert!(result.is_ok());
    }

    // ── Test 4: Revoked key rejects previously signed plugin ────

    #[test]
    fn revoked_key_rejects_plugin() {
        let key = generate_key();
        let wasm = sample_wasm();
        let manifest = sample_manifest();
        let public_key_hex = hex::encode(key.verifying_key().to_bytes());

        let sig = sign_plugin(&key, &wasm, &manifest, VerificationTier::Official);

        // Create a revocation list containing this key.
        let mut revocation = RevocationList::new();
        revocation.revoke(&public_key_hex);

        let config = SignatureVerifierConfig {
            allow_unsigned: false,
            revocation_list: revocation,
        };

        let result = verify_plugin(&sig, &wasm, &manifest, &config);
        assert_eq!(result, VerificationResult::RevokedKey);
    }

    #[test]
    fn non_revoked_key_passes() {
        let key = generate_key();
        let wasm = sample_wasm();
        let manifest = sample_manifest();

        // Revoke a different key.
        let other_key = generate_key();
        let mut revocation = RevocationList::new();
        revocation.revoke(&hex::encode(other_key.verifying_key().to_bytes()));

        let sig = sign_plugin(&key, &wasm, &manifest, VerificationTier::Reviewed);
        let config = SignatureVerifierConfig {
            allow_unsigned: false,
            revocation_list: revocation,
        };

        let result = verify_plugin(&sig, &wasm, &manifest, &config);
        assert!(matches!(result, VerificationResult::Valid { .. }));
    }

    // ── Test 5: Manifest hash included in signature ─────────────

    #[test]
    fn manifest_hash_stored_in_signature() {
        let key = generate_key();
        let wasm = sample_wasm();
        let manifest = sample_manifest();

        let sig = sign_plugin(&key, &wasm, &manifest, VerificationTier::Official);

        // The signature must include the manifest hash.
        assert!(!sig.manifest_hash.is_empty());

        // The manifest hash must match a SHA-256 of the manifest bytes.
        let expected_hash = compute_manifest_hash(&manifest);
        assert_eq!(sig.manifest_hash, expected_hash);
    }

    #[test]
    fn different_manifest_produces_different_hash() {
        let manifest_a = serde_json::to_vec(&serde_json::json!({
            "id": "com.example.a",
            "capabilities": ["StorageRead"]
        }))
        .unwrap();
        let manifest_b = serde_json::to_vec(&serde_json::json!({
            "id": "com.example.a",
            "capabilities": ["StorageRead", "StorageWrite"]
        }))
        .unwrap();

        let hash_a = compute_manifest_hash(&manifest_a);
        let hash_b = compute_manifest_hash(&manifest_b);

        assert_ne!(hash_a, hash_b);
    }

    #[test]
    fn signature_binds_wasm_to_manifest() {
        let key = generate_key();
        let wasm = sample_wasm();
        let manifest = sample_manifest();
        let config = SignatureVerifierConfig::default();

        let sig = sign_plugin(&key, &wasm, &manifest, VerificationTier::Official);

        // Valid with correct wasm + manifest.
        assert!(matches!(
            verify_plugin(&sig, &wasm, &manifest, &config),
            VerificationResult::Valid { .. }
        ));

        // Swapping in different WASM bytes with same manifest fails.
        let other_wasm = b"different-wasm-content".to_vec();
        assert_eq!(
            verify_plugin(&sig, &other_wasm, &manifest, &config),
            VerificationResult::TamperedBundle
        );
    }

    // ── Verification tier tests ─────────────────────────────────

    #[test]
    fn verification_tier_serialization() {
        assert_eq!(
            serde_json::to_string(&VerificationTier::Unverified).unwrap(),
            "\"unverified\""
        );
        assert_eq!(
            serde_json::to_string(&VerificationTier::Reviewed).unwrap(),
            "\"reviewed\""
        );
        assert_eq!(
            serde_json::to_string(&VerificationTier::Official).unwrap(),
            "\"official\""
        );
    }

    #[test]
    fn verification_tier_deserialization() {
        let tier: VerificationTier = serde_json::from_str("\"reviewed\"").unwrap();
        assert_eq!(tier, VerificationTier::Reviewed);
    }

    #[test]
    fn verification_tier_display() {
        assert_eq!(VerificationTier::Unverified.to_string(), "unverified");
        assert_eq!(VerificationTier::Reviewed.to_string(), "reviewed");
        assert_eq!(VerificationTier::Official.to_string(), "official");
    }

    // ── Revocation list persistence ─────────────────────────────

    #[test]
    fn revocation_list_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("revocation.json");

        let mut list = RevocationList::new();
        list.revoke("aabbccdd");
        list.revoke("11223344");
        list.save(&path).unwrap();

        let loaded = RevocationList::load(&path).unwrap();
        assert!(loaded.is_revoked("aabbccdd"));
        assert!(loaded.is_revoked("11223344"));
        assert!(!loaded.is_revoked("deadbeef"));
    }

    // ── Edge cases ──────────────────────────────────────────────

    #[test]
    fn wrong_key_produces_invalid_signature() {
        let key_a = generate_key();
        let key_b = generate_key();
        let wasm = sample_wasm();
        let manifest = sample_manifest();
        let config = SignatureVerifierConfig::default();

        // Sign with key_a.
        let sig = sign_plugin(&key_a, &wasm, &manifest, VerificationTier::Official);

        // Replace public key with key_b's public key — signature won't match.
        let forged_sig = PluginSignature {
            public_key: hex::encode(key_b.verifying_key().to_bytes()),
            ..sig
        };

        let result = verify_plugin(&forged_sig, &wasm, &manifest, &config);
        assert_eq!(result, VerificationResult::TamperedBundle);
    }

    #[test]
    fn malformed_public_key_returns_error() {
        let sig = PluginSignature {
            signature: hex::encode([0u8; 64]),
            public_key: "not-valid-hex!!!".to_string(),
            manifest_hash: hex::encode([0u8; 32]),
            tier: VerificationTier::Unverified,
        };
        let config = SignatureVerifierConfig::default();

        let result = verify_plugin(&sig, &[], &[], &config);
        assert!(matches!(
            result,
            VerificationResult::MalformedSignature(_)
        ));
    }

    #[test]
    fn malformed_signature_returns_error() {
        let key = generate_key();
        let sig = PluginSignature {
            signature: "tooshort".to_string(),
            public_key: hex::encode(key.verifying_key().to_bytes()),
            manifest_hash: hex::encode([0u8; 32]),
            tier: VerificationTier::Unverified,
        };
        let config = SignatureVerifierConfig::default();

        let result = verify_plugin(&sig, &[], &[], &config);
        assert!(matches!(
            result,
            VerificationResult::MalformedSignature(_)
        ));
    }
}
