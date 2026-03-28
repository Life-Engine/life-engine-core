//! Shared encryption primitives for Life Engine.
//!
//! Provides AES-256-GCM encryption, Argon2id key derivation, and HMAC utilities.

pub mod credential;
pub mod encryption;
pub mod error;
pub mod hmac;
pub mod kdf;
pub mod types;

pub use encryption::{decrypt, encrypt};
pub use error::CryptoError;
pub use hmac::{hmac_sha256, hmac_sign, hmac_verify};
pub use kdf::{derive_key, derive_key_with_params, generate_salt};
pub use types::Argon2Params;

#[cfg(test)]
mod tests;
