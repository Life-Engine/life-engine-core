//! Database rekey (passphrase change) — re-exports from packages.
//!
//! Key derivation is now in `life_engine_crypto` (Argon2id KDF) and
//! `life_engine_storage_sqlite::encryption` (salt management, PRAGMA rekey).
//! This module provides backward-compatible re-exports.
//!
//! Migrated during architecture migration (WP 10.4).

#![allow(dead_code)]

pub use life_engine_storage_sqlite::encryption::{
    derive_db_key, derive_db_key_with_params, derive_rekey_pair,
};
pub use life_engine_crypto::{derive_key, derive_key_with_params, generate_salt, Argon2Params};
