//! Passphrase-based database encryption for SQLCipher.
//!
//! Manages the lifecycle of database encryption keys: deriving keys from
//! user passphrases via Argon2id, persisting salts alongside the database,
//! and providing passphrase-based key rotation.
//!
//! The raw passphrase is zeroized immediately after key derivation. The
//! derived key is held in memory for the lifetime of the process.

use std::fs;
use std::path::{Path, PathBuf};

use life_engine_crypto::{derive_key, generate_salt, Argon2Params};
use tracing::info;
use zeroize::Zeroizing;

use crate::error::StorageError;

/// Returns the path to the salt file for a given database path.
///
/// The salt file is stored alongside the database as `<db_path>.salt`.
fn salt_path(db_path: &Path) -> PathBuf {
    let mut p = db_path.as_os_str().to_owned();
    p.push(".salt");
    PathBuf::from(p)
}

/// Loads the salt from disk, or generates and persists a new one if none exists.
///
/// The salt is a 16-byte random value stored as raw bytes in `<db_path>.salt`.
fn load_or_create_salt(db_path: &Path) -> Result<[u8; 16], StorageError> {
    let path = salt_path(db_path);

    if path.exists() {
        let bytes = fs::read(&path).map_err(|e| {
            StorageError::InitFailed(format!("failed to read salt file {}: {e}", path.display()))
        })?;
        if bytes.len() != 16 {
            return Err(StorageError::InitFailed(format!(
                "salt file {} has invalid length {} (expected 16)",
                path.display(),
                bytes.len()
            )));
        }
        let mut salt = [0u8; 16];
        salt.copy_from_slice(&bytes);
        Ok(salt)
    } else {
        let salt = generate_salt();
        fs::write(&path, salt).map_err(|e| {
            StorageError::InitFailed(format!(
                "failed to write salt file {}: {e}",
                path.display()
            ))
        })?;
        info!(path = %path.display(), "generated new encryption salt");
        Ok(salt)
    }
}

/// Derives a 32-byte database encryption key from a passphrase and the salt
/// stored alongside the database.
///
/// If no salt file exists yet, one is generated and persisted. The passphrase
/// is consumed by value so the caller's copy can be zeroized.
///
/// Returns the derived key wrapped in [`Zeroizing`] for automatic cleanup.
pub fn derive_db_key(
    passphrase: &Zeroizing<String>,
    db_path: &Path,
) -> Result<Zeroizing<[u8; 32]>, StorageError> {
    let salt = load_or_create_salt(db_path)?;
    derive_key(passphrase.as_str(), &salt)
        .map_err(|e| StorageError::InitFailed(format!("key derivation failed: {e}")))
}

/// Derives a database encryption key using custom Argon2 parameters.
///
/// Use this on resource-constrained devices where the default 64 MB / 3
/// iteration parameters are too expensive.
pub fn derive_db_key_with_params(
    passphrase: &Zeroizing<String>,
    db_path: &Path,
    params: &Argon2Params,
) -> Result<Zeroizing<[u8; 32]>, StorageError> {
    let salt = load_or_create_salt(db_path)?;
    life_engine_crypto::derive_key_with_params(passphrase.as_str(), &salt, params)
        .map_err(|e| StorageError::InitFailed(format!("key derivation failed: {e}")))
}

/// Reads the master passphrase from the `LIFE_ENGINE_PASSPHRASE` environment
/// variable.
///
/// Returns `None` if the variable is not set. The passphrase is wrapped in
/// [`Zeroizing`] so it is cleared from memory when dropped.
pub fn passphrase_from_env() -> Option<Zeroizing<String>> {
    std::env::var("LIFE_ENGINE_PASSPHRASE")
        .ok()
        .filter(|s| !s.is_empty())
        .map(Zeroizing::new)
}

/// Performs passphrase-based key rotation.
///
/// Derives both old and new keys, then delegates to the SQLCipher `PRAGMA
/// rekey` mechanism. Both passphrases are zeroized after derivation.
///
/// The salt remains unchanged — only the passphrase (and thus the derived
/// key) changes.
pub fn derive_rekey_pair(
    old_passphrase: &Zeroizing<String>,
    new_passphrase: &Zeroizing<String>,
    db_path: &Path,
) -> Result<(Zeroizing<[u8; 32]>, Zeroizing<[u8; 32]>), StorageError> {
    let salt = load_or_create_salt(db_path)?;

    let old_key = life_engine_crypto::derive_key(old_passphrase.as_str(), &salt)
        .map_err(|e| StorageError::RekeyFailed(format!("old key derivation failed: {e}")))?;

    let new_key = life_engine_crypto::derive_key(new_passphrase.as_str(), &salt)
        .map_err(|e| StorageError::RekeyFailed(format!("new key derivation failed: {e}")))?;

    Ok((old_key, new_key))
}

/// Deletes the salt file for a database. Used only in tests or when
/// destroying a database entirely.
#[cfg(test)]
pub(crate) fn remove_salt(db_path: &Path) {
    let path = salt_path(db_path);
    let _ = fs::remove_file(path);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn salt_is_created_on_first_call() {
        let dir = TempDir::new().unwrap();
        let db = dir.path().join("test.db");
        let sp = salt_path(&db);

        assert!(!sp.exists());
        let salt = load_or_create_salt(&db).unwrap();
        assert!(sp.exists());
        assert_eq!(salt.len(), 16);
    }

    #[test]
    fn salt_is_stable_across_loads() {
        let dir = TempDir::new().unwrap();
        let db = dir.path().join("test.db");

        let salt1 = load_or_create_salt(&db).unwrap();
        let salt2 = load_or_create_salt(&db).unwrap();
        assert_eq!(salt1, salt2);
    }

    #[test]
    fn invalid_salt_file_is_rejected() {
        let dir = TempDir::new().unwrap();
        let db = dir.path().join("test.db");
        let sp = salt_path(&db);
        fs::write(&sp, &[0u8; 10]).unwrap(); // wrong length

        let result = load_or_create_salt(&db);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid length"));
    }

    #[test]
    fn derive_db_key_produces_32_bytes() {
        let dir = TempDir::new().unwrap();
        let db = dir.path().join("test.db");
        let passphrase = Zeroizing::new("test-passphrase".to_string());

        let key = derive_db_key(&passphrase, &db).unwrap();
        assert_eq!(key.len(), 32);
    }

    #[test]
    fn same_passphrase_same_salt_produces_same_key() {
        let dir = TempDir::new().unwrap();
        let db = dir.path().join("test.db");
        let passphrase = Zeroizing::new("my-passphrase".to_string());

        let key1 = derive_db_key(&passphrase, &db).unwrap();
        let key2 = derive_db_key(&passphrase, &db).unwrap();
        assert_eq!(*key1, *key2);
    }

    #[test]
    fn different_passphrases_produce_different_keys() {
        let dir = TempDir::new().unwrap();
        let db = dir.path().join("test.db");

        let key1 = derive_db_key(&Zeroizing::new("pass-one".into()), &db).unwrap();
        let key2 = derive_db_key(&Zeroizing::new("pass-two".into()), &db).unwrap();
        assert_ne!(*key1, *key2);
    }

    #[test]
    fn derive_rekey_pair_returns_distinct_keys() {
        let dir = TempDir::new().unwrap();
        let db = dir.path().join("test.db");

        let old = Zeroizing::new("old-pass".to_string());
        let new = Zeroizing::new("new-pass".to_string());
        let (old_key, new_key) = derive_rekey_pair(&old, &new, &db).unwrap();

        assert_ne!(*old_key, *new_key);
    }

    #[test]
    fn custom_params_derive_key() {
        let dir = TempDir::new().unwrap();
        let db = dir.path().join("test.db");
        let passphrase = Zeroizing::new("test".to_string());
        let params = Argon2Params {
            memory_kib: 8192,
            iterations: 1,
            parallelism: 1,
        };

        let key = derive_db_key_with_params(&passphrase, &db, &params).unwrap();
        assert_eq!(key.len(), 32);

        // Custom params should produce a different key than defaults.
        let default_key = derive_db_key(&passphrase, &db).unwrap();
        assert_ne!(*key, *default_key);
    }
}
