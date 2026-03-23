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
