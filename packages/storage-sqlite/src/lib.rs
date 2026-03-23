//! SQLCipher-backed storage backend for Life Engine.
//!
//! Provides encrypted, WAL-mode SQLite storage with automatic schema
//! initialization. The database key is a 32-byte value derived externally
//! via `packages/crypto::derive_key()` — this crate never handles
//! passphrases directly.

pub mod audit;
pub mod backend;
pub mod config;
pub mod credentials;
pub mod error;
pub mod export;
pub mod migration;
pub mod schema;
pub mod types;
pub mod validation;

use rusqlite::Connection;

pub use error::StorageError;
pub use validation::PrivateSchemaRegistry;

/// SQLite/SQLCipher storage backend.
///
/// Wraps a `rusqlite::Connection` configured with SQLCipher encryption
/// and WAL journal mode. Created via the `init` constructor.
pub struct SqliteStorage {
    conn: Connection,
    private_schemas: PrivateSchemaRegistry,
    master_key: [u8; 32],
}

impl std::fmt::Debug for SqliteStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteStorage").finish_non_exhaustive()
    }
}

impl SqliteStorage {
    /// Open (or create) a SQLCipher-encrypted database and initialize the schema.
    ///
    /// # Arguments
    ///
    /// - `config` — TOML configuration containing `database_path` (string).
    /// - `key` — 32-byte encryption key derived from the user's passphrase
    ///   via `life_engine_crypto::derive_key()`.
    ///
    /// # Errors
    ///
    /// - `StorageError::InvalidConfig` — missing or invalid `database_path`.
    /// - `StorageError::PermissionDenied` — cannot write to the database path.
    /// - `StorageError::DecryptionFailed` — wrong key or corrupted database.
    /// - `StorageError::Database` — other SQLite/rusqlite errors.
    pub fn init(config: toml::Value, key: [u8; 32]) -> Result<Self, StorageError> {
        let db_path = config
            .get("database_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                StorageError::InvalidConfig(
                    "missing or invalid 'database_path' in storage config".to_string(),
                )
            })?;

        let conn = Connection::open(db_path).map_err(|e| {
            let msg = e.to_string();
            if msg.contains("permission denied") || msg.contains("readonly") {
                StorageError::PermissionDenied(format!("{db_path}: {msg}"))
            } else {
                StorageError::Database(e)
            }
        })?;

        // Set SQLCipher encryption key (hex-encoded 32-byte key).
        let hex_key = hex::encode(key);
        conn.execute_batch(&format!("PRAGMA key = 'x\"{hex_key}\"';"))
            .map_err(|e| StorageError::InitFailed(format!("failed to set encryption key: {e}")))?;

        // Verify the key is correct by reading the database header.
        // If the key is wrong, this will fail with "file is not a database".
        match conn.execute_batch("SELECT count(*) FROM sqlite_master;") {
            Ok(()) => {}
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("not a database") || msg.contains("file is encrypted") {
                    return Err(StorageError::DecryptionFailed(
                        "unable to decrypt database — wrong key or corrupted file".to_string(),
                    ));
                }
                return Err(StorageError::Database(e));
            }
        }

        // Enable WAL journal mode for concurrent read performance.
        conn.execute_batch("PRAGMA journal_mode = WAL;")
            .map_err(StorageError::Database)?;

        // Enable foreign key enforcement.
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(StorageError::Database)?;

        // Create tables and indexes if they don't exist.
        for ddl in schema::ALL_DDL {
            conn.execute_batch(ddl).map_err(StorageError::Database)?;
        }

        Ok(SqliteStorage {
            conn,
            private_schemas: PrivateSchemaRegistry::new(),
            master_key: key,
        })
    }

    /// Returns a reference to the underlying database connection.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Returns a mutable reference to the private schema registry.
    ///
    /// Use this to register private collection schemas from plugin manifests
    /// before performing write operations on private collections.
    pub fn private_schemas_mut(&mut self) -> &mut PrivateSchemaRegistry {
        &mut self.private_schemas
    }

    /// Returns a reference to the private schema registry.
    pub fn private_schemas(&self) -> &PrivateSchemaRegistry {
        &self.private_schemas
    }

    /// Returns the master encryption key for per-credential encryption.
    pub(crate) fn master_key(&self) -> &[u8; 32] {
        &self.master_key
    }
}

#[cfg(test)]
mod tests;
