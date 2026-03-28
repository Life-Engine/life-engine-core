//! Tests for SQLiteStorage initialization.

use crate::SqliteStorage;
use tempfile::TempDir;

fn test_config(path: &str) -> toml::Value {
    toml::Value::Table({
        let mut t = toml::map::Map::new();
        t.insert(
            "database_path".to_string(),
            toml::Value::String(path.to_string()),
        );
        t
    })
}

fn test_key() -> [u8; 32] {
    [0x42u8; 32]
}

#[test]
fn init_creates_new_database() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let config = test_config(db_path.to_str().unwrap());

    let storage = SqliteStorage::init(config, test_key()).expect("init should succeed");

    // Verify tables were created by querying sqlite_master.
    let tables: Vec<String> = storage
        .connection()
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert!(tables.contains(&"plugin_data".to_string()));
    assert!(tables.contains(&"audit_log".to_string()));
}

#[test]
fn init_opens_existing_database() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let path_str = db_path.to_str().unwrap();
    let config = test_config(path_str);
    let key = test_key();

    // Create the database.
    let storage = SqliteStorage::init(config.clone(), key).expect("first init");

    // Insert a row to verify persistence.
    storage
        .connection()
        .execute(
            "INSERT INTO plugin_data (id, plugin_id, collection, data, created_at, updated_at) \
             VALUES ('p1', 'plug', 'events', '{}', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
    drop(storage);

    // Re-open with the same key.
    let storage2 = SqliteStorage::init(config, key).expect("second init should succeed");
    let count: i64 = storage2
        .connection()
        .query_row("SELECT count(*) FROM plugin_data", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn init_rejects_wrong_key() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let path_str = db_path.to_str().unwrap();
    let config = test_config(path_str);

    // Create with one key.
    let _storage = SqliteStorage::init(config.clone(), [0x01u8; 32]).expect("create");
    drop(_storage);

    // Attempt to open with a different key.
    let result = SqliteStorage::init(config, [0x02u8; 32]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("decrypt") || msg.contains("not a database"),
        "expected decryption error, got: {msg}"
    );
}

#[test]
fn init_rejects_missing_database_path() {
    let config = toml::Value::Table(toml::map::Map::new());
    let result = SqliteStorage::init(config, test_key());
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("database_path"));
}

#[test]
fn init_sets_wal_journal_mode() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let config = test_config(db_path.to_str().unwrap());

    let storage = SqliteStorage::init(config, test_key()).unwrap();
    let mode: String = storage
        .connection()
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
}

#[test]
fn init_enables_foreign_keys() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let config = test_config(db_path.to_str().unwrap());

    let storage = SqliteStorage::init(config, test_key()).unwrap();
    let fk: i64 = storage
        .connection()
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .unwrap();
    assert_eq!(fk, 1);
}

#[test]
fn init_idempotent_schema() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let path_str = db_path.to_str().unwrap();
    let config = test_config(path_str);
    let key = test_key();

    // Init twice — schema creation should be idempotent.
    let _s1 = SqliteStorage::init(config.clone(), key).unwrap();
    drop(_s1);
    let _s2 = SqliteStorage::init(config, key).expect("second init should be idempotent");
}

#[test]
fn rekey_succeeds_and_new_key_works() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let path_str = db_path.to_str().unwrap();
    let config = test_config(path_str);
    let old_key = [0x01u8; 32];
    let new_key = [0x02u8; 32];

    // Create and populate database with old key.
    let mut storage = SqliteStorage::init(config.clone(), old_key).expect("init");
    storage
        .connection()
        .execute(
            "INSERT INTO plugin_data (id, plugin_id, collection, data, created_at, updated_at) \
             VALUES ('r1', 'plug', 'events', '{\"x\":1}', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();

    // Rotate to new key.
    storage.rekey(new_key).expect("rekey should succeed");
    drop(storage);

    // Re-open with the new key — data should be accessible.
    let storage2 = SqliteStorage::init(config.clone(), new_key).expect("open with new key");
    let count: i64 = storage2
        .connection()
        .query_row("SELECT count(*) FROM plugin_data", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);

    // Old key should no longer work.
    drop(storage2);
    let result = SqliteStorage::init(config, old_key);
    assert!(result.is_err(), "old key should be rejected after rekey");
}

#[test]
fn original_key_works_after_normal_close() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let path_str = db_path.to_str().unwrap();
    let config = test_config(path_str);
    let key = test_key();

    // Create a valid database.
    let storage = SqliteStorage::init(config.clone(), key).expect("init");
    storage
        .connection()
        .execute(
            "INSERT INTO plugin_data (id, plugin_id, collection, data, created_at, updated_at) \
             VALUES ('r1', 'plug', 'events', '{}', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();

    // Verify that the original key still works after dropping and re-opening
    // (no rekey was applied, so the key should be unchanged).
    drop(storage);

    let storage2 = SqliteStorage::init(config, key).expect("old key should still work");
    let count: i64 = storage2
        .connection()
        .query_row("SELECT count(*) FROM plugin_data", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn credential_encrypted_field_is_not_plaintext() {
    use crate::credentials::encrypt_credential;

    let master_key = [0x42u8; 32];
    let original = serde_json::json!({
        "id": "cred-secret-test",
        "name": "API Key",
        "credential_type": "api_key",
        "service": "stripe",
        "claims": {
            "api_key": "rk_fake_51XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
            "secret": "whsec_YYYYYYYY"
        },
        "source": "stripe-plugin",
        "source_id": "cred-1",
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-01T00:00:00Z"
    })
    .to_string();

    let encrypted = encrypt_credential(&master_key, &original).unwrap();

    // The encrypted output should NOT contain the plaintext secret values.
    assert!(
        !encrypted.contains("rk_fake_51"),
        "encrypted credential must not contain plaintext API key"
    );
    assert!(
        !encrypted.contains("whsec_YYYYYYYY"),
        "encrypted credential must not contain plaintext secret"
    );

    // The encrypted flag should be set.
    let doc: serde_json::Value = serde_json::from_str(&encrypted).unwrap();
    assert_eq!(doc["encrypted"], true);

    // The claims field should be a hex string (ciphertext), not a JSON object.
    assert!(
        doc["claims"].is_string(),
        "encrypted claims should be a hex string, not a JSON object"
    );
}

#[test]
fn credential_stored_in_db_is_not_plaintext_at_rest() {
    use crate::credentials::encrypt_credential;

    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let config = test_config(db_path.to_str().unwrap());
    let key = test_key();

    let storage = SqliteStorage::init(config, key).expect("init");

    // Encrypt and store a credential.
    let original = serde_json::json!({
        "id": "cred-at-rest-test",
        "name": "OAuth Token",
        "credential_type": "oauth_token",
        "service": "github",
        "claims": {
            "access_token": "gho_16C7e42F292c6912E7710c838347Ae178B4a",
            "token_type": "bearer"
        },
        "source": "github-plugin",
        "source_id": "cred-2",
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-01T00:00:00Z"
    })
    .to_string();

    let encrypted_json = encrypt_credential(&key, &original).unwrap();

    // Store encrypted credential in the database.
    storage
        .connection()
        .execute(
            "INSERT INTO plugin_data (id, plugin_id, collection, data, created_at, updated_at) \
             VALUES ('cred-at-rest-test', 'github-plugin', 'credentials', ?1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [&encrypted_json],
        )
        .unwrap();

    // Read it back from the database (still encrypted).
    let stored: String = storage
        .connection()
        .query_row(
            "SELECT data FROM plugin_data WHERE id = 'cred-at-rest-test'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    // Verify the stored data does not contain the plaintext token.
    assert!(
        !stored.contains("gho_16C7e42F292c6912E7710c838347Ae178B4a"),
        "stored credential must not contain plaintext access token"
    );
}

#[test]
fn credentials_remain_readable_after_rekey() {
    use crate::credentials::{decrypt_credential, encrypt_credential};

    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let path_str = db_path.to_str().unwrap();
    let config = test_config(path_str);
    let old_key = [0x01u8; 32];
    let new_key = [0x02u8; 32];

    let mut storage = SqliteStorage::init(config.clone(), old_key).expect("init");

    // Encrypt and store a credential with the old key.
    let original = serde_json::json!({
        "id": "cred-rekey-test",
        "name": "OAuth Token",
        "credential_type": "oauth_token",
        "service": "github",
        "claims": {
            "access_token": "gho_rekey_test_token_123",
            "token_type": "bearer"
        },
        "source": "github-plugin",
        "source_id": "cred-3",
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-01T00:00:00Z"
    });
    let original_json = original.to_string();
    let encrypted_json = encrypt_credential(&old_key, &original_json).unwrap();

    storage
        .connection()
        .execute(
            "INSERT INTO plugin_data (id, plugin_id, collection, data, created_at, updated_at) \
             VALUES ('cred-rekey-test', 'github-plugin', 'credentials', ?1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [&encrypted_json],
        )
        .unwrap();

    // Rotate the key — this should re-encrypt credentials.
    storage.rekey(new_key).expect("rekey should succeed");

    // Read the stored credential and decrypt with the new key.
    let stored: String = storage
        .connection()
        .query_row(
            "SELECT data FROM plugin_data WHERE id = 'cred-rekey-test'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    let decrypted = decrypt_credential(&new_key, &stored)
        .expect("credential should be decryptable with new key");
    let dec_doc: serde_json::Value = serde_json::from_str(&decrypted).unwrap();

    // Claims should match the original.
    assert_eq!(
        dec_doc["claims"]["access_token"], "gho_rekey_test_token_123",
        "access_token should survive rekey"
    );

    // Old key should no longer decrypt the credential.
    let old_result = decrypt_credential(&old_key, &stored);
    assert!(
        old_result.is_err(),
        "old key should not decrypt credential after rekey"
    );
}
