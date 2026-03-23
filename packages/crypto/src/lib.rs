//! Shared encryption primitives for Life Engine.
//!
//! Provides AES-256-GCM encryption, Argon2id key derivation, and HMAC utilities.

pub mod encryption;
pub mod error;
pub mod hmac;
pub mod kdf;
pub mod types;

pub use encryption::{decrypt, encrypt};
pub use error::CryptoError;

#[cfg(test)]
mod tests;
