//! Pre-migration backup and restore for SQLite databases.
//!
//! Before every migration run the caller creates a timestamped backup
//! using SQLite's online backup API. The backup path is recorded in the
//! migration log so that an admin can restore if a migration produces
//! undesirable results.

use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::Connection;
use tracing::{info, warn};

use crate::error::StorageError;

/// Create a consistent backup of the SQLite database before a migration.
///
/// The backup is written to `data_dir/backups/pre-migration-{timestamp}.db`
/// using SQLite's backup API, which does not block concurrent readers.
/// After writing, the backup is verified with `PRAGMA integrity_check`.
///
/// Returns the path to the newly created backup file.
pub fn create_backup(db_path: &Path, data_dir: &Path) -> Result<PathBuf, StorageError> {
    let backups_dir = data_dir.join("backups");
    std::fs::create_dir_all(&backups_dir)?;

    let timestamp = Utc::now().format("%Y-%m-%d-%H%M%S");
    let backup_filename = format!("pre-migration-{timestamp}.db");
    let backup_path = backups_dir.join(&backup_filename);

    // Open the source database and perform the backup.
    let src = Connection::open(db_path).map_err(StorageError::Database)?;
    let mut dst = Connection::open(&backup_path).map_err(StorageError::Database)?;

    let backup =
        rusqlite::backup::Backup::new(&src, &mut dst).map_err(StorageError::Database)?;

    // Copy all pages in one step (-1 = all remaining pages).
    backup
        .step(-1)
        .map_err(StorageError::Database)?;

    // Drop the backup handle and source connection before verifying.
    drop(backup);
    drop(src);
    drop(dst);

    // Verify the backup by opening it and running an integrity check.
    let verify_conn = Connection::open(&backup_path).map_err(StorageError::Database)?;
    let integrity: String = verify_conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(StorageError::Database)?;

    if integrity != "ok" {
        warn!(path = %backup_path.display(), result = %integrity, "backup integrity check failed");
        // Remove the corrupt backup.
        let _ = std::fs::remove_file(&backup_path);
        return Err(StorageError::InitFailed(format!(
            "backup integrity check failed: {integrity}"
        )));
    }

    info!(path = %backup_path.display(), "pre-migration backup created and verified");
    Ok(backup_path)
}

/// Restore a database from a previously created backup.
///
/// This is a manual admin action — the migration system never calls it
/// automatically. It replaces the database at `db_path` with the contents
/// of the backup file.
pub fn restore_backup(backup_path: &Path, db_path: &Path) -> Result<(), StorageError> {
    if !backup_path.exists() {
        return Err(StorageError::NotFound(format!(
            "backup file not found: {}",
            backup_path.display()
        )));
    }

    // Verify the backup before restoring.
    let verify_conn = Connection::open(backup_path).map_err(StorageError::Database)?;
    let integrity: String = verify_conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(StorageError::Database)?;
    drop(verify_conn);

    if integrity != "ok" {
        return Err(StorageError::InitFailed(format!(
            "backup integrity check failed before restore: {integrity}"
        )));
    }

    std::fs::copy(backup_path, db_path)?;
    info!(
        backup = %backup_path.display(),
        target = %db_path.display(),
        "database restored from backup"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Create a small test database with one table and one row.
    fn create_test_db(path: &Path) {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            "CREATE TABLE test_data (id INTEGER PRIMARY KEY, value TEXT);
             INSERT INTO test_data (id, value) VALUES (1, 'hello');",
        )
        .unwrap();
    }

    #[test]
    fn create_backup_produces_valid_copy() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        create_test_db(&db_path);

        let backup_path = create_backup(&db_path, dir.path()).unwrap();

        // Backup file should exist in a backups/ subdirectory.
        assert!(backup_path.exists());
        assert!(backup_path
            .to_string_lossy()
            .contains("backups/pre-migration-"));

        // Backup should contain the same data.
        let conn = Connection::open(&backup_path).unwrap();
        let value: String = conn
            .query_row("SELECT value FROM test_data WHERE id = 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(value, "hello");
    }

    #[test]
    fn create_backup_creates_backups_directory() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        create_test_db(&db_path);

        let backups_dir = dir.path().join("backups");
        assert!(!backups_dir.exists());

        let _ = create_backup(&db_path, dir.path()).unwrap();
        assert!(backups_dir.exists());
    }

    #[test]
    fn restore_backup_replaces_database() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        create_test_db(&db_path);

        let backup_path = create_backup(&db_path, dir.path()).unwrap();

        // Modify the original database.
        let conn = Connection::open(&db_path).unwrap();
        conn.execute("UPDATE test_data SET value = 'modified' WHERE id = 1", [])
            .unwrap();
        drop(conn);

        // Restore from backup.
        restore_backup(&backup_path, &db_path).unwrap();

        // Original value should be back.
        let conn = Connection::open(&db_path).unwrap();
        let value: String = conn
            .query_row("SELECT value FROM test_data WHERE id = 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(value, "hello");
    }

    #[test]
    fn restore_backup_fails_for_missing_file() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let missing = dir.path().join("nonexistent.db");

        let result = restore_backup(&missing, &db_path);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("backup file not found"));
    }
}
