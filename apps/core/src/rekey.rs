//! Database rekey (passphrase change) for SQLCipher-encrypted databases.
//!
//! Provides Argon2id key derivation and the `PRAGMA rekey` workflow for
//! changing the master passphrase on a Life Engine database file.
//!
#![allow(dead_code)]
//! # Safety
//!
//! - Passphrases are read from the terminal via `rpassword` (no echo).
//! - Passphrases are never accepted as CLI arguments.
//! - Raw passphrase strings are dropped as soon as keys are derived.
//! - The raw key format (`x'<hex>'`) is used, not passphrase mode.

use crate::config::Argon2Settings;
use crate::error::CoreError;

use argon2::Argon2;
use rand::rngs::OsRng;
use rand::RngCore;
use rusqlite::Connection;
use std::path::Path;

/// Length of the derived key in bytes (256-bit for SQLCipher).
const KEY_LENGTH: usize = 32;

/// Length of the random salt in bytes.
const SALT_LENGTH: usize = 16;

/// Generate a random 16-byte salt.
fn generate_salt() -> [u8; SALT_LENGTH] {
    let mut salt = [0u8; SALT_LENGTH];
    OsRng.fill_bytes(&mut salt);
    salt
}

/// Read the salt file for a database, or create one if it does not exist.
///
/// The salt file is stored alongside the database at `<db_path>.salt`.
pub fn load_or_create_salt(db_path: &Path) -> Result<[u8; SALT_LENGTH], CoreError> {
    let salt_path = db_path.with_extension("db.salt");
    if salt_path.exists() {
        let data = std::fs::read(&salt_path)
            .map_err(|e| CoreError::Rekey(format!("failed to read salt file: {e}")))?;
        if data.len() != SALT_LENGTH {
            return Err(CoreError::Rekey(format!(
                "salt file has wrong length: expected {SALT_LENGTH}, got {}",
                data.len()
            )));
        }
        let mut salt = [0u8; SALT_LENGTH];
        salt.copy_from_slice(&data);
        Ok(salt)
    } else {
        let salt = generate_salt();
        std::fs::write(&salt_path, salt)
            .map_err(|e| CoreError::Rekey(format!("failed to write salt file: {e}")))?;
        Ok(salt)
    }
}

/// Derive a 32-byte hex-encoded key from a passphrase and salt using Argon2id.
///
/// Returns a 64-character lowercase hex string suitable for use with
/// SQLCipher's raw key format: `PRAGMA key = "x'<hex>'"`.
pub fn derive_key_with_salt(passphrase: &str, salt: &[u8], settings: &Argon2Settings) -> Result<String, CoreError> {
    let argon2 = Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::new(
            settings.memory_mb * 1024, // KiB
            settings.iterations,
            settings.parallelism,
            Some(KEY_LENGTH),
        )
        .map_err(|e| CoreError::Rekey(format!("invalid Argon2 parameters: {e}")))?,
    );

    let mut output = [0u8; KEY_LENGTH];
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, &mut output)
        .map_err(|e| CoreError::Rekey(format!("Argon2 key derivation failed: {e}")))?;

    Ok(hex::encode(output))
}

/// Derive a key using the salt stored alongside the database file.
///
/// Loads or creates the salt file at `<db_path>.salt`, then derives the key.
/// This is the correct production path — always use this instead of
/// deriving with a static salt.
pub fn derive_key_for_db(passphrase: &str, db_path: &Path, settings: &Argon2Settings) -> Result<String, CoreError> {
    let salt = load_or_create_salt(db_path)?;
    derive_key_with_salt(passphrase, &salt, settings)
}

/// Derive a key with a zero salt. **Only for use in tests.**
///
/// Production code must use [`derive_key_for_db`] which loads or creates a
/// random per-database salt file.
#[cfg(test)]
pub fn derive_key(passphrase: &str, settings: &Argon2Settings) -> Result<String, CoreError> {
    let fallback_salt = [0u8; SALT_LENGTH];
    derive_key_with_salt(passphrase, &fallback_salt, settings)
}

/// Open a SQLCipher database with the given hex key and verify it is readable.
fn open_encrypted(path: &Path, hex_key: &str) -> Result<Connection, CoreError> {
    let conn = Connection::open(path)
        .map_err(|e| CoreError::Rekey(format!("failed to open database: {e}")))?;

    let pragma_key = format!("PRAGMA key = \"x'{hex_key}'\";");
    conn.execute_batch(&pragma_key)
        .map_err(|e| CoreError::Rekey(format!("failed to set key: {e}")))?;

    // Verify the key is correct by reading from the database.
    conn.execute_batch("SELECT count(*) FROM sqlite_master;")
        .map_err(|e| CoreError::Rekey(format!("database is not readable (wrong key?): {e}")))?;

    Ok(conn)
}

/// Change the encryption key on a SQLCipher database.
///
/// Opens the database with `current_hex_key`, verifies it is readable,
/// then runs `PRAGMA rekey` to re-encrypt with `new_hex_key`. After the
/// rekey, the connection is closed and the database is reopened with the
/// new key to verify the operation succeeded.
pub fn rekey_database(
    db_path: &Path,
    current_hex_key: &str,
    new_hex_key: &str,
) -> Result<(), CoreError> {
    // 1. Open with current key and verify readability.
    let conn = open_encrypted(db_path, current_hex_key)?;

    // 2. Rekey to the new key.
    let pragma_rekey = format!("PRAGMA rekey = \"x'{new_hex_key}'\";");
    conn.execute_batch(&pragma_rekey)
        .map_err(|e| CoreError::Rekey(format!("PRAGMA rekey failed: {e}")))?;

    // 3. Close the connection.
    conn.close()
        .map_err(|(_, e)| CoreError::Rekey(format!("failed to close after rekey: {e}")))?;

    // 4. Verify by reopening with the new key.
    let verify_conn = open_encrypted(db_path, new_hex_key)?;
    verify_conn
        .close()
        .map_err(|(_, e)| CoreError::Rekey(format!("verification close failed: {e}")))?;

    Ok(())
}

/// Run the interactive rekey workflow.
///
/// Prompts the user for the current passphrase, derives the current key,
/// prompts for a new passphrase (with confirmation), derives the new key,
/// and calls [`rekey_database`].
///
/// # Errors
///
/// Returns `CoreError::Rekey` if passphrases do not match, key derivation
/// fails, or the database rekey operation fails.
pub fn run_rekey(db_path: &Path, argon2_settings: &Argon2Settings) -> Result<(), CoreError> {
    // 0. Verify database file exists.
    if !db_path.exists() {
        return Err(CoreError::Rekey(format!(
            "database file not found: {}",
            db_path.display()
        )));
    }

    eprintln!();
    eprintln!("=== Life Engine — Database Rekey ===");
    eprintln!();
    eprintln!("WARNING: Back up your database before proceeding.");
    eprintln!("         File: {}", db_path.display());
    eprintln!();

    // 1. Read current passphrase.
    let current_passphrase = rpassword::prompt_password("Current passphrase: ")
        .map_err(|e| CoreError::Rekey(format!("failed to read current passphrase: {e}")))?;

    if current_passphrase.is_empty() {
        return Err(CoreError::Rekey("passphrase must not be empty".into()));
    }

    // 2. Read new passphrase with confirmation.
    let new_passphrase = rpassword::prompt_password("New passphrase: ")
        .map_err(|e| CoreError::Rekey(format!("failed to read new passphrase: {e}")))?;

    if new_passphrase.is_empty() {
        return Err(CoreError::Rekey("new passphrase must not be empty".into()));
    }

    let confirm_passphrase = rpassword::prompt_password("Confirm new passphrase: ")
        .map_err(|e| CoreError::Rekey(format!("failed to read confirmation: {e}")))?;

    if new_passphrase != confirm_passphrase {
        return Err(CoreError::Rekey(
            "new passphrase and confirmation do not match".into(),
        ));
    }

    // 3. Load or create salt, then derive keys (drop passphrase strings immediately after).
    eprintln!("Deriving keys...");
    let salt = load_or_create_salt(db_path)?;
    let current_key = derive_key_with_salt(&current_passphrase, &salt, argon2_settings)?;
    drop(current_passphrase);

    let new_key = derive_key_with_salt(&new_passphrase, &salt, argon2_settings)?;
    drop(new_passphrase);
    drop(confirm_passphrase);

    // 4. Perform the rekey.
    eprintln!("Rekeying database...");
    rekey_database(db_path, &current_key, &new_key)?;

    eprintln!("Database rekeyed successfully.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    /// Helper: create an encrypted database with the given hex key and a test table.
    fn create_encrypted_db(path: &Path, hex_key: &str) {
        let conn = Connection::open(path).unwrap();
        let pragma_key = format!("PRAGMA key = \"x'{hex_key}'\";");
        conn.execute_batch(&pragma_key).unwrap();
        conn.execute_batch("CREATE TABLE test (id INTEGER PRIMARY KEY, value TEXT);")
            .unwrap();
        conn.execute_batch("INSERT INTO test (value) VALUES ('hello');")
            .unwrap();
        conn.close().unwrap();
    }

    /// Helper: verify that a database can be opened with the given hex key and
    /// contains expected data.
    fn verify_db_readable(path: &Path, hex_key: &str) -> bool {
        let conn = match Connection::open(path) {
            Ok(c) => c,
            Err(_) => return false,
        };
        let pragma_key = format!("PRAGMA key = \"x'{hex_key}'\";");
        if conn.execute_batch(&pragma_key).is_err() {
            return false;
        }
        conn.execute_batch("SELECT count(*) FROM sqlite_master;")
            .is_ok()
    }

    /// Test Argon2 settings with low cost for fast tests.
    fn test_argon2_settings() -> Argon2Settings {
        Argon2Settings {
            memory_mb: 1, // 1 MiB — fast for tests
            iterations: 1,
            parallelism: 1,
        }
    }

    // ── derive_key tests ───────────────────────────────────────────

    #[test]
    fn derive_key_produces_64_hex_chars() {
        let settings = test_argon2_settings();
        let key = derive_key("test-passphrase", &settings).unwrap();
        assert_eq!(key.len(), 64);
        assert!(key.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn derive_key_is_deterministic() {
        let settings = test_argon2_settings();
        let k1 = derive_key("my-pass", &settings).unwrap();
        let k2 = derive_key("my-pass", &settings).unwrap();
        assert_eq!(k1, k2);
    }

    #[test]
    fn derive_key_different_passphrases_produce_different_keys() {
        let settings = test_argon2_settings();
        let k1 = derive_key("pass-a", &settings).unwrap();
        let k2 = derive_key("pass-b", &settings).unwrap();
        assert_ne!(k1, k2);
    }

    #[test]
    fn derive_key_different_settings_produce_different_keys() {
        let s1 = Argon2Settings {
            memory_mb: 1,
            iterations: 1,
            parallelism: 1,
        };
        let s2 = Argon2Settings {
            memory_mb: 2,
            iterations: 1,
            parallelism: 1,
        };
        let k1 = derive_key("same-pass", &s1).unwrap();
        let k2 = derive_key("same-pass", &s2).unwrap();
        assert_ne!(k1, k2);
    }

    // ── rekey_database tests ───────────────────────────────────────

    #[test]
    fn rekey_succeeds_with_correct_passphrase() {
        let settings = test_argon2_settings();
        let old_key = derive_key("old-pass", &settings).unwrap();
        let new_key = derive_key("new-pass", &settings).unwrap();

        let tmp = NamedTempFile::new().unwrap();
        create_encrypted_db(tmp.path(), &old_key);

        // Rekey should succeed.
        rekey_database(tmp.path(), &old_key, &new_key).unwrap();

        // Old key should no longer work.
        assert!(!verify_db_readable(tmp.path(), &old_key));

        // New key should work.
        assert!(verify_db_readable(tmp.path(), &new_key));
    }

    #[test]
    fn rekey_fails_with_wrong_current_passphrase() {
        let settings = test_argon2_settings();
        let correct_key = derive_key("correct", &settings).unwrap();
        let wrong_key = derive_key("wrong", &settings).unwrap();
        let new_key = derive_key("new", &settings).unwrap();

        let tmp = NamedTempFile::new().unwrap();
        create_encrypted_db(tmp.path(), &correct_key);

        // Rekey with wrong current key should fail.
        let result = rekey_database(tmp.path(), &wrong_key, &new_key);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("not readable") || err_msg.contains("wrong key"),
            "unexpected error: {err_msg}"
        );

        // Original key should still work.
        assert!(verify_db_readable(tmp.path(), &correct_key));
    }

    #[test]
    fn verification_confirms_new_key_works_after_rekey() {
        let settings = test_argon2_settings();
        let key_a = derive_key("pass-a", &settings).unwrap();
        let key_b = derive_key("pass-b", &settings).unwrap();

        let tmp = NamedTempFile::new().unwrap();
        create_encrypted_db(tmp.path(), &key_a);

        rekey_database(tmp.path(), &key_a, &key_b).unwrap();

        // Explicitly verify the new key works by reading data.
        let conn = open_encrypted(tmp.path(), &key_b).unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM test", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
        conn.close().unwrap();
    }

    #[test]
    fn rekey_database_nonexistent_file() {
        let result = rekey_database(
            Path::new("/tmp/nonexistent-life-engine-test.db"),
            "aabbccdd",
            "eeff0011",
        );
        // Opening a nonexistent file creates it, but it won't have the
        // expected schema, so the SELECT on sqlite_master may succeed on
        // an empty DB. Either way the test documents the behavior.
        // The key point is it does not panic.
        let _ = result;
    }

    #[test]
    fn open_encrypted_with_correct_key() {
        let settings = test_argon2_settings();
        let key = derive_key("my-pass", &settings).unwrap();

        let tmp = NamedTempFile::new().unwrap();
        create_encrypted_db(tmp.path(), &key);

        let conn = open_encrypted(tmp.path(), &key).unwrap();
        conn.close().unwrap();
    }

    #[test]
    fn open_encrypted_with_wrong_key_fails() {
        let settings = test_argon2_settings();
        let correct_key = derive_key("correct", &settings).unwrap();
        let wrong_key = derive_key("wrong", &settings).unwrap();

        let tmp = NamedTempFile::new().unwrap();
        create_encrypted_db(tmp.path(), &correct_key);

        let result = open_encrypted(tmp.path(), &wrong_key);
        assert!(result.is_err());
    }

    // ── salt uniqueness tests ──────────────────────────────────────

    #[test]
    fn generate_salt_produces_unique_values() {
        let s1 = generate_salt();
        let s2 = generate_salt();
        assert_ne!(s1, s2, "two consecutive salts should differ");
    }

    #[test]
    fn derive_key_with_different_salts_produces_different_keys() {
        let settings = test_argon2_settings();
        let salt1 = [0x01u8; SALT_LENGTH];
        let salt2 = [0x02u8; SALT_LENGTH];
        let k1 = derive_key_with_salt("same-pass", &salt1, &settings).unwrap();
        let k2 = derive_key_with_salt("same-pass", &salt2, &settings).unwrap();
        assert_ne!(k1, k2, "same passphrase with different salts should produce different keys");
    }

    // ── run_rekey cannot be tested non-interactively (requires tty) ──
    // The interactive flow is covered by the unit tests above for
    // derive_key and rekey_database. Integration testing of run_rekey
    // would require a pseudo-terminal or test harness.

    #[test]
    fn run_rekey_rejects_nonexistent_db() {
        let settings = test_argon2_settings();
        let result = run_rekey(
            Path::new("/tmp/does-not-exist-life-engine-rekey-test.db"),
            &settings,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    // ── double rekey test ──────────────────────────────────────────

    #[test]
    fn double_rekey_works() {
        let settings = test_argon2_settings();
        let key_a = derive_key("alpha", &settings).unwrap();
        let key_b = derive_key("bravo", &settings).unwrap();
        let key_c = derive_key("charlie", &settings).unwrap();

        let tmp = NamedTempFile::new().unwrap();
        create_encrypted_db(tmp.path(), &key_a);

        // First rekey: A -> B
        rekey_database(tmp.path(), &key_a, &key_b).unwrap();
        assert!(verify_db_readable(tmp.path(), &key_b));
        assert!(!verify_db_readable(tmp.path(), &key_a));

        // Second rekey: B -> C
        rekey_database(tmp.path(), &key_b, &key_c).unwrap();
        assert!(verify_db_readable(tmp.path(), &key_c));
        assert!(!verify_db_readable(tmp.path(), &key_b));
    }
}
