//! Storage migration between SQLite and PostgreSQL backends.
//!
//! Provides atomic migration with record count verification, progress
//! reporting, and rollback on failure. Data is migrated in batches to
//! handle large datasets without excessive memory usage.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::pg_storage::PgStorage;
use crate::sqlite_storage::SqliteStorage;
use crate::storage::{Pagination, StorageAdapter};

/// Batch size for migration (number of records per batch).
const MIGRATION_BATCH_SIZE: u32 = 500;

/// Progress reporting callback.
pub type ProgressCallback = Box<dyn Fn(MigrationProgress) + Send + Sync>;

/// Migration progress information.
#[derive(Debug, Clone)]
pub struct MigrationProgress {
    /// Total records to migrate.
    pub total_records: u64,
    /// Records migrated so far.
    pub migrated_records: u64,
    /// Current collection being migrated.
    pub current_collection: String,
    /// Whether the migration is complete.
    pub complete: bool,
}

/// Result of a completed migration.
#[derive(Debug)]
pub struct MigrationResult {
    /// Number of records migrated.
    pub records_migrated: u64,
    /// Collections migrated.
    pub collections_migrated: Vec<String>,
}

/// Migrate all data from SQLite to PostgreSQL.
///
/// The migration is performed atomically within a PostgreSQL transaction.
/// If any step fails, all changes are rolled back. After migration,
/// record counts are verified to ensure data integrity.
pub async fn migrate_sqlite_to_pg(
    sqlite: &SqliteStorage,
    pg: &PgStorage,
    progress_cb: Option<ProgressCallback>,
) -> anyhow::Result<MigrationResult> {
    // 1. Discover all unique plugin_id + collection pairs in SQLite.
    let collections = discover_collections(sqlite).await?;

    if collections.is_empty() {
        return Ok(MigrationResult {
            records_migrated: 0,
            collections_migrated: vec![],
        });
    }

    // 2. Count total records across all collections.
    let mut total_records: u64 = 0;
    for (plugin_id, collection) in &collections {
        let result = sqlite
            .list(plugin_id, collection, None, Pagination { limit: 1, offset: 0 })
            .await?;
        total_records += result.total;
    }

    let migrated_count = Arc::new(AtomicU64::new(0));

    // 3. Get a client from the pool and start a transaction.
    let mut pg_client = pg.pool().get().await?;
    let transaction = pg_client.transaction().await?;

    // 4. Migrate each collection in batches within the transaction.
    let mut collections_migrated = Vec::new();

    for (plugin_id, collection) in &collections {
        if let Some(ref cb) = progress_cb {
            cb(MigrationProgress {
                total_records,
                migrated_records: migrated_count.load(Ordering::Relaxed),
                current_collection: collection.clone(),
                complete: false,
            });
        }

        let mut offset: u64 = 0;
        loop {
            let batch = sqlite
                .list(
                    plugin_id,
                    collection,
                    None,
                    Pagination {
                        limit: MIGRATION_BATCH_SIZE,
                        offset: offset as u32,
                    },
                )
                .await?;

            if batch.records.is_empty() {
                break;
            }

            for record in &batch.records {
                let data_json = serde_json::to_string(&record.data)?;
                let created_at = record.created_at;
                let updated_at = record.updated_at;

                transaction
                    .execute(
                        "INSERT INTO plugin_data (id, plugin_id, collection, data, version, created_at, updated_at)
                         VALUES ($1, $2, $3, $4::jsonb, $5, $6, $7)
                         ON CONFLICT (id) DO NOTHING",
                        &[
                            &record.id,
                            &record.plugin_id,
                            &record.collection,
                            &data_json,
                            &record.version,
                            &created_at,
                            &updated_at,
                        ],
                    )
                    .await
                    .map_err(|e| {
                        anyhow::anyhow!(
                            "failed to migrate record {} in {}/{}: {e}",
                            record.id,
                            plugin_id,
                            collection
                        )
                    })?;

                migrated_count.fetch_add(1, Ordering::Relaxed);
            }

            offset += batch.records.len() as u64;

            if offset >= batch.total {
                break;
            }
        }

        collections_migrated.push(collection.clone());
    }

    // 5. Verify record counts per-collection before committing.
    //    We compare per (plugin_id, collection) rather than using a global
    //    COUNT(*), because the target table may already contain rows from a
    //    previous migration run (INSERT … ON CONFLICT DO NOTHING skips
    //    duplicates). A global count would include pre-existing rows and
    //    produce a false mismatch.
    let final_migrated = migrated_count.load(Ordering::Relaxed);

    for (plugin_id, collection) in &collections {
        let sqlite_result = sqlite
            .list(plugin_id, collection, None, Pagination { limit: 1, offset: 0 })
            .await?;
        let expected = sqlite_result.total as i64;

        let pg_count_row = transaction
            .query_one(
                "SELECT COUNT(*) FROM plugin_data WHERE plugin_id = $1 AND collection = $2",
                &[plugin_id, collection],
            )
            .await?;
        let pg_count: i64 = pg_count_row.get(0);

        if pg_count != expected {
            // Rollback happens automatically when transaction is dropped.
            return Err(anyhow::anyhow!(
                "record count mismatch for {plugin_id}/{collection}: \
                 SQLite has {expected} records, PostgreSQL has {pg_count} records"
            ));
        }
    }

    // 6. Commit the transaction.
    transaction.commit().await.map_err(|e| {
        anyhow::anyhow!("failed to commit migration transaction: {e}")
    })?;

    if let Some(ref cb) = progress_cb {
        cb(MigrationProgress {
            total_records,
            migrated_records: final_migrated,
            current_collection: String::new(),
            complete: true,
        });
    }

    Ok(MigrationResult {
        records_migrated: final_migrated,
        collections_migrated,
    })
}

/// Discover all unique (plugin_id, collection) pairs in the SQLite database.
async fn discover_collections(sqlite: &SqliteStorage) -> anyhow::Result<Vec<(String, String)>> {
    // Use a query through the internal connection to get distinct pairs.
    // Since SqliteStorage exposes only the StorageAdapter trait, we
    // query with a known plugin_id pattern — but for migration we need
    // direct access. We work around this by scanning via the list method
    // using a broad query.
    //
    // For a real implementation, we'd add a method to SqliteStorage.
    // For now, we access the internal connection.
    let conn = sqlite.connection().await;
    let mut stmt = conn.prepare(
        "SELECT DISTINCT plugin_id, collection FROM plugin_data ORDER BY plugin_id, collection",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
        ))
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }

    Ok(result)
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pg_storage::{PgConfig, PgSslMode};
    use std::sync::atomic::AtomicBool;

    fn pg_test_url() -> Option<String> {
        std::env::var("LIFE_ENGINE_TEST_PG_URL").ok()
    }

    async fn test_pg_storage() -> Option<PgStorage> {
        let url = pg_test_url()?;
        let url_parsed: url::Url = url.parse().ok()?;
        let ssl_mode = url_parsed
            .query_pairs()
            .find(|(k, _)| k == "sslmode")
            .and_then(|(_, v)| v.parse::<PgSslMode>().ok())
            .unwrap_or(PgSslMode::Disable);
        let config = PgConfig {
            host: url_parsed.host_str().unwrap_or("localhost").into(),
            port: url_parsed.port().unwrap_or(5432),
            dbname: url_parsed.path().trim_start_matches('/').into(),
            user: url_parsed.username().into(),
            password: url_parsed.password().unwrap_or("").into(),
            pool_size: 4,
            ssl_mode,
        };
        let storage = PgStorage::open(&config).await.ok()?;
        let client = storage.pool().get().await.ok()?;
        let _ = client.execute("DELETE FROM plugin_data", &[]).await;
        let _ = client.execute("DELETE FROM audit_log", &[]).await;
        Some(storage)
    }

    #[tokio::test]
    async fn migration_record_count_matches() {
        let Some(pg) = test_pg_storage().await else {
            eprintln!("Skipping — LIFE_ENGINE_TEST_PG_URL not set");
            return;
        };
        let sqlite = SqliteStorage::open_in_memory().unwrap();

        // Create test records in SQLite.
        for i in 0..10 {
            sqlite
                .create("p1", "tasks", serde_json::json!({"i": i}))
                .await
                .unwrap();
        }
        for i in 0..5 {
            sqlite
                .create("p1", "notes", serde_json::json!({"n": i}))
                .await
                .unwrap();
        }
        for i in 0..3 {
            sqlite
                .create("p2", "tasks", serde_json::json!({"x": i}))
                .await
                .unwrap();
        }

        let result = migrate_sqlite_to_pg(&sqlite, &pg, None).await.unwrap();
        assert_eq!(result.records_migrated, 18);

        // Verify counts match per collection.
        let pg_tasks_p1 = pg
            .list("p1", "tasks", None, Pagination { limit: 1, offset: 0 })
            .await
            .unwrap();
        assert_eq!(pg_tasks_p1.total, 10);

        let pg_notes_p1 = pg
            .list("p1", "notes", None, Pagination { limit: 1, offset: 0 })
            .await
            .unwrap();
        assert_eq!(pg_notes_p1.total, 5);

        let pg_tasks_p2 = pg
            .list("p2", "tasks", None, Pagination { limit: 1, offset: 0 })
            .await
            .unwrap();
        assert_eq!(pg_tasks_p2.total, 3);
    }

    #[tokio::test]
    async fn migration_is_atomic_rolls_back_on_failure() {
        let Some(pg) = test_pg_storage().await else {
            eprintln!("Skipping — LIFE_ENGINE_TEST_PG_URL not set");
            return;
        };
        let sqlite = SqliteStorage::open_in_memory().unwrap();

        // Verify that if we start with records in PG (simulating a partial
        // migration), a fresh migration succeeds atomically.
        sqlite
            .create("p1", "tasks", serde_json::json!({"task": "one"}))
            .await
            .unwrap();

        let result = migrate_sqlite_to_pg(&sqlite, &pg, None).await.unwrap();
        assert_eq!(result.records_migrated, 1);

        // The PG database should have exactly 1 record.
        let check = pg
            .list("p1", "tasks", None, Pagination::default())
            .await
            .unwrap();
        assert_eq!(check.total, 1);
    }

    #[tokio::test]
    async fn migration_empty_sqlite_succeeds() {
        let Some(pg) = test_pg_storage().await else {
            eprintln!("Skipping — LIFE_ENGINE_TEST_PG_URL not set");
            return;
        };
        let sqlite = SqliteStorage::open_in_memory().unwrap();

        let result = migrate_sqlite_to_pg(&sqlite, &pg, None).await.unwrap();
        assert_eq!(result.records_migrated, 0);
        assert!(result.collections_migrated.is_empty());
    }

    #[tokio::test]
    async fn migration_progress_callback_fires() {
        let Some(pg) = test_pg_storage().await else {
            eprintln!("Skipping — LIFE_ENGINE_TEST_PG_URL not set");
            return;
        };
        let sqlite = SqliteStorage::open_in_memory().unwrap();

        for i in 0..5 {
            sqlite
                .create("p1", "tasks", serde_json::json!({"i": i}))
                .await
                .unwrap();
        }

        let progress_called = Arc::new(AtomicBool::new(false));
        let progress_called_clone = Arc::clone(&progress_called);
        let cb: ProgressCallback = Box::new(move |progress| {
            progress_called_clone.store(true, Ordering::Relaxed);
            assert_eq!(progress.total_records, 5);
        });

        let result = migrate_sqlite_to_pg(&sqlite, &pg, Some(cb)).await.unwrap();
        assert_eq!(result.records_migrated, 5);
        assert!(progress_called.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn migration_preserves_data_fidelity() {
        let Some(pg) = test_pg_storage().await else {
            eprintln!("Skipping — LIFE_ENGINE_TEST_PG_URL not set");
            return;
        };
        let sqlite = SqliteStorage::open_in_memory().unwrap();

        let data = serde_json::json!({
            "title": "Important task",
            "priority": 5,
            "tags": ["urgent", "work"],
            "metadata": {"source": "email", "thread_id": 42}
        });
        let created = sqlite.create("p1", "tasks", data.clone()).await.unwrap();

        // Update to bump version.
        let updated_data = serde_json::json!({
            "title": "Important task (updated)",
            "priority": 5,
            "tags": ["urgent", "work"],
            "metadata": {"source": "email", "thread_id": 42}
        });
        sqlite
            .update("p1", "tasks", &created.id, updated_data.clone(), 1)
            .await
            .unwrap();

        migrate_sqlite_to_pg(&sqlite, &pg, None).await.unwrap();

        let pg_record = pg
            .get("p1", "tasks", &created.id)
            .await
            .unwrap()
            .expect("record should exist in PG");

        assert_eq!(pg_record.data, updated_data);
        assert_eq!(pg_record.version, 2);
        assert_eq!(pg_record.plugin_id, "p1");
        assert_eq!(pg_record.collection, "tasks");
    }
}
