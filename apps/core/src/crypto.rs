//! Domain-separated encryption — re-exports from `life_engine_crypto::domain`.
//!
//! Migrated to `packages/crypto/src/domain.rs` during architecture migration
//! (WP 10.4). This module provides backward-compatible aliases.

pub use life_engine_crypto::domain::{
    derive_domain_key as derive_key,
    domain_decrypt as decrypt,
    domain_encrypt as encrypt,
    domain_hmac as hmac_sha256,
    domain_hmac_verify as hmac_sha256_verify,
    DOMAIN_CREDENTIAL_STORE,
    DOMAIN_IDENTITY_ENCRYPT,
    DOMAIN_IDENTITY_SIGN,
};
