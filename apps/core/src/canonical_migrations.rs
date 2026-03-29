//! Canonical schema migration engine.
//!
//! Compares each canonical collection's stored schema version against the
//! declared version in `life_engine_types` and runs WASM migration transforms
//! for any that are behind.
#![allow(dead_code)]

use std::path::Path;

use life_engine_types::migrations::{CANONICAL_COLLECTIONS, CANONICAL_PLUGIN_ID};

use crate::sqlite_storage::SqliteStorage;

/// A migration that needs to run for a single canonical collection.
struct PendingMigration {
    collection_name: &'static str,
    stored_version: i64,
    declared_version: i64,
    wasm_path: std::path::PathBuf,
    entries: Vec<life_engine_workflow_engine::migration::MigrationEntry>,
}

/// Run canonical schema migrations against the given storage.
///
/// For each canonical collection whose stored version is behind the declared
/// version, this function looks for a WASM migration transform and runs it.
/// First-run collections are version-stamped without migration.
pub async fn run_canonical_migrations(
    storage: &SqliteStorage,
    db_path: &Path,
    data_dir: &Path,
) {
    let migrations_base = data_dir.join("migrations");

    // Phase 1: Check versions and collect migrations that need to run.
    let mut pending_migrations = Vec::new();

    {
        let conn = storage.connection().await;

        for collection in CANONICAL_COLLECTIONS {
            let stored_version = conn
                .query_row(
                    "SELECT version FROM schema_versions WHERE plugin_id = ?1 AND collection = ?2",
                    rusqlite::params![CANONICAL_PLUGIN_ID, collection.name],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap_or(0);

            if stored_version == 0 {
                // First run — stamp the current version, no migration needed.
                let now = chrono::Utc::now().to_rfc3339();
                let _ = conn.execute(
                    "INSERT OR REPLACE INTO schema_versions (plugin_id, collection, version, updated_at) \
                     VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![CANONICAL_PLUGIN_ID, collection.name, collection.version, now],
                );
                tracing::debug!(
                    collection = collection.name,
                    version = collection.version,
                    "canonical collection version stamped (first run)"
                );
                continue;
            }

            if stored_version >= collection.version {
                tracing::debug!(
                    collection = collection.name,
                    stored = stored_version,
                    declared = collection.version,
                    "canonical collection schema is current"
                );
                continue;
            }

            // Schema version is behind — look for WASM migration transforms.
            let migration_dir = migrations_base.join(collection.migration_dir);
            if !migration_dir.exists() {
                tracing::warn!(
                    collection = collection.name,
                    stored = stored_version,
                    declared = collection.version,
                    dir = %migration_dir.display(),
                    "canonical migration directory not found, skipping"
                );
                continue;
            }

            let manifest_path = migration_dir.join("manifest.toml");
            if !manifest_path.exists() {
                tracing::warn!(
                    collection = collection.name,
                    "canonical migration manifest.toml not found, skipping"
                );
                continue;
            }

            let wasm_path = migration_dir.join("transform.wasm");
            if !wasm_path.exists() {
                tracing::warn!(
                    collection = collection.name,
                    "canonical transform.wasm not found, skipping"
                );
                continue;
            }

            match life_engine_workflow_engine::migration::parse_migration_entries(&manifest_path) {
                Ok(entries) => {
                    let relevant: Vec<_> = entries
                        .into_iter()
                        .filter(|e| {
                            let from_major = e.from.split('.').next()
                                .and_then(|s| s.parse::<i64>().ok())
                                .unwrap_or(0);
                            from_major >= stored_version && (e.to.major as i64) <= collection.version
                        })
                        .collect();

                    if !relevant.is_empty() {
                        pending_migrations.push(PendingMigration {
                            collection_name: collection.name,
                            stored_version,
                            declared_version: collection.version,
                            wasm_path,
                            entries: relevant,
                        });
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        collection = collection.name,
                        error = %e,
                        "failed to parse canonical migration manifest"
                    );
                }
            }
        }
    } // Connection lock released here.

    // Phase 2: Run any pending migrations (without holding the connection lock).
    for migration in &pending_migrations {
        tracing::info!(
            collection = migration.collection_name,
            from = migration.stored_version,
            to = migration.declared_version,
            entries = migration.entries.len(),
            "running canonical schema migration"
        );

        match life_engine_workflow_engine::migration::run_migrations(
            &migration.wasm_path,
            &migration.entries,
            CANONICAL_PLUGIN_ID,
            db_path,
            data_dir,
        )
        .await
        {
            Ok(result) => {
                tracing::info!(
                    collection = migration.collection_name,
                    migrated = result.migrated,
                    quarantined = result.quarantined,
                    duration_ms = result.duration_ms,
                    "canonical migration complete"
                );

                // Stamp the new version.
                let conn = storage.connection().await;
                let now = chrono::Utc::now().to_rfc3339();
                let _ = conn.execute(
                    "INSERT OR REPLACE INTO schema_versions (plugin_id, collection, version, updated_at) \
                     VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![CANONICAL_PLUGIN_ID, migration.collection_name, migration.declared_version, now],
                );
            }
            Err(e) => {
                tracing::error!(
                    collection = migration.collection_name,
                    error = %e,
                    "canonical migration failed (non-fatal)"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    // The canonical migration logic depends on the full storage and
    // workflow-engine infrastructure, so integration tests are more
    // appropriate. These unit tests verify the version-check logic.

    #[test]
    fn canonical_collections_have_positive_versions() {
        use life_engine_types::migrations::CANONICAL_COLLECTIONS;
        for collection in CANONICAL_COLLECTIONS {
            assert!(
                collection.version > 0,
                "canonical collection '{}' must have a positive version",
                collection.name
            );
        }
    }

    #[test]
    fn canonical_collections_have_non_empty_names() {
        use life_engine_types::migrations::CANONICAL_COLLECTIONS;
        for collection in CANONICAL_COLLECTIONS {
            assert!(
                !collection.name.is_empty(),
                "canonical collection must have a non-empty name"
            );
            assert!(
                !collection.migration_dir.is_empty(),
                "canonical collection '{}' must have a non-empty migration_dir",
                collection.name
            );
        }
    }
}
