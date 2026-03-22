//! Backup engine — orchestrates full and incremental backup/restore operations.

use std::collections::HashMap;

use chrono::Utc;

use crate::backend::BackupBackend;
use crate::crypto;
use crate::types::{Argon2Params, BackupManifest, BackupRecord, BackupType, RestoreResult};

/// The backup archive format: manifest + records serialized as JSON.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct BackupArchive {
    manifest: BackupManifest,
    records: Vec<BackupRecord>,
}

/// Create a full backup of all provided records.
///
/// Serializes, compresses, encrypts, and stores the backup. Returns the
/// manifest describing the backup.
pub async fn create_full_backup(
    backend: &dyn BackupBackend,
    records: Vec<BackupRecord>,
    passphrase: &str,
    argon2_params: &Argon2Params,
) -> anyhow::Result<BackupManifest> {
    let now = Utc::now();
    let backup_id = format!("full-{}", now.format("%Y%m%d-%H%M%S-%3f"));

    // Compute collection stats.
    let mut collections: Vec<String> = Vec::new();
    let mut record_counts: HashMap<String, u64> = HashMap::new();
    for rec in &records {
        let col = rec.collection.clone();
        *record_counts.entry(col.clone()).or_insert(0) += 1;
        if !collections.contains(&col) {
            collections.push(col);
        }
    }

    let manifest = BackupManifest {
        id: backup_id.clone(),
        created_at: now,
        backup_type: BackupType::Full,
        collections,
        record_counts,
        compressed_size: 0,
        checksum: String::new(),
        parent_id: None,
        cursor: Some(now.to_rfc3339()),
    };

    let archive = BackupArchive {
        manifest: manifest.clone(),
        records,
    };

    // Serialize -> compress -> encrypt.
    let json = serde_json::to_vec(&archive)?;
    let compressed = crypto::compress(&json)?;
    let key = crypto::derive_key(passphrase, argon2_params)?;
    let encrypted = crypto::encrypt(&compressed, &key)?;
    let checksum = crypto::sha256_hex(&encrypted);

    // Store the encrypted backup.
    let storage_key = format!("{backup_id}.enc");
    backend.put(&storage_key, &encrypted).await?;

    // Store the manifest separately (unencrypted, for listing).
    let manifest_with_stats = BackupManifest {
        compressed_size: compressed.len() as u64,
        checksum,
        ..manifest
    };
    let manifest_json = serde_json::to_vec(&manifest_with_stats)?;
    let manifest_key = format!("{backup_id}.manifest.json");
    backend.put(&manifest_key, &manifest_json).await?;

    Ok(manifest_with_stats)
}

/// Create an incremental backup of records changed since the parent backup.
pub async fn create_incremental_backup(
    backend: &dyn BackupBackend,
    changed_records: Vec<BackupRecord>,
    parent_id: &str,
    passphrase: &str,
    argon2_params: &Argon2Params,
) -> anyhow::Result<BackupManifest> {
    let now = Utc::now();
    let backup_id = format!("incr-{}", now.format("%Y%m%d-%H%M%S-%3f"));

    let mut collections: Vec<String> = Vec::new();
    let mut record_counts: HashMap<String, u64> = HashMap::new();
    for rec in &changed_records {
        let col = rec.collection.clone();
        *record_counts.entry(col.clone()).or_insert(0) += 1;
        if !collections.contains(&col) {
            collections.push(col);
        }
    }

    let manifest = BackupManifest {
        id: backup_id.clone(),
        created_at: now,
        backup_type: BackupType::Incremental,
        collections,
        record_counts,
        compressed_size: 0,
        checksum: String::new(),
        parent_id: Some(parent_id.to_string()),
        cursor: Some(now.to_rfc3339()),
    };

    let archive = BackupArchive {
        manifest: manifest.clone(),
        records: changed_records,
    };

    let json = serde_json::to_vec(&archive)?;
    let compressed = crypto::compress(&json)?;
    let key = crypto::derive_key(passphrase, argon2_params)?;
    let encrypted = crypto::encrypt(&compressed, &key)?;
    let checksum = crypto::sha256_hex(&encrypted);

    let storage_key = format!("{backup_id}.enc");
    backend.put(&storage_key, &encrypted).await?;

    let manifest_with_stats = BackupManifest {
        compressed_size: compressed.len() as u64,
        checksum,
        ..manifest
    };
    let manifest_json = serde_json::to_vec(&manifest_with_stats)?;
    let manifest_key = format!("{backup_id}.manifest.json");
    backend.put(&manifest_key, &manifest_json).await?;

    Ok(manifest_with_stats)
}

/// Restore all records from a backup.
///
/// Downloads, decrypts, decompresses, and deserializes the backup archive.
/// Verifies the checksum before returning the records.
pub async fn restore_full(
    backend: &dyn BackupBackend,
    backup_id: &str,
    passphrase: &str,
    argon2_params: &Argon2Params,
) -> anyhow::Result<(Vec<BackupRecord>, RestoreResult)> {
    let storage_key = format!("{backup_id}.enc");
    let encrypted = backend.get(&storage_key).await?;

    // Verify checksum.
    let actual_checksum = crypto::sha256_hex(&encrypted);

    // Load manifest for checksum comparison.
    let manifest_key = format!("{backup_id}.manifest.json");
    let manifest_data = backend.get(&manifest_key).await?;
    let manifest: BackupManifest = serde_json::from_slice(&manifest_data)?;

    if actual_checksum != manifest.checksum {
        anyhow::bail!(
            "backup integrity check failed: checksum mismatch (expected {}, got {})",
            manifest.checksum,
            actual_checksum
        );
    }

    // Decrypt -> decompress -> deserialize.
    let key = crypto::derive_key(passphrase, argon2_params)?;
    let compressed = crypto::decrypt(&encrypted, &key)?;
    let json = crypto::decompress(&compressed)?;
    let archive: BackupArchive = serde_json::from_slice(&json)?;

    let mut records_restored: HashMap<String, u64> = HashMap::new();
    for rec in &archive.records {
        *records_restored.entry(rec.collection.clone()).or_insert(0) += 1;
    }
    let total_restored = archive.records.len() as u64;

    let result = RestoreResult {
        records_restored,
        total_restored,
        integrity_verified: true,
    };

    Ok((archive.records, result))
}

/// Restore only specific collections from a backup.
pub async fn restore_partial(
    backend: &dyn BackupBackend,
    backup_id: &str,
    collections: &[String],
    passphrase: &str,
    argon2_params: &Argon2Params,
) -> anyhow::Result<(Vec<BackupRecord>, RestoreResult)> {
    let (all_records, _) = restore_full(backend, backup_id, passphrase, argon2_params).await?;

    let filtered: Vec<BackupRecord> = all_records
        .into_iter()
        .filter(|r| collections.contains(&r.collection))
        .collect();

    let mut records_restored: HashMap<String, u64> = HashMap::new();
    for rec in &filtered {
        *records_restored.entry(rec.collection.clone()).or_insert(0) += 1;
    }
    let total_restored = filtered.len() as u64;

    let result = RestoreResult {
        records_restored,
        total_restored,
        integrity_verified: true,
    };

    Ok((filtered, result))
}

/// List all backup manifests from the backend.
pub async fn list_backups(backend: &dyn BackupBackend) -> anyhow::Result<Vec<BackupManifest>> {
    let stored = backend.list("").await?;
    let mut manifests = Vec::new();

    for entry in stored {
        if entry.key.ends_with(".manifest.json") {
            let data = backend.get(&entry.key).await?;
            if let Ok(manifest) = serde_json::from_slice::<BackupManifest>(&data) {
                manifests.push(manifest);
            }
        }
    }

    manifests.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(manifests)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::local::LocalBackend;
    use tempfile::TempDir;

    fn test_params() -> Argon2Params {
        Argon2Params {
            memory_mb: 1,
            iterations: 1,
            parallelism: 1,
        }
    }

    fn sample_records() -> Vec<BackupRecord> {
        vec![
            BackupRecord {
                id: "rec-1".into(),
                plugin_id: "com.life-engine.todos".into(),
                collection: "tasks".into(),
                data: serde_json::json!({"title": "Buy groceries", "status": "pending"}),
                version: 1,
                user_id: None,
                household_id: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            BackupRecord {
                id: "rec-2".into(),
                plugin_id: "com.life-engine.todos".into(),
                collection: "tasks".into(),
                data: serde_json::json!({"title": "Walk the dog", "status": "completed"}),
                version: 2,
                user_id: None,
                household_id: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            BackupRecord {
                id: "rec-3".into(),
                plugin_id: "com.life-engine.contacts".into(),
                collection: "contacts".into(),
                data: serde_json::json!({"name": "Alice", "email": "alice@example.com"}),
                version: 1,
                user_id: None,
                household_id: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        ]
    }

    // ── Full backup encrypt/restore tests ────────────────────────────

    #[tokio::test]
    async fn full_backup_and_restore_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let backend = LocalBackend::new(tmp.path());
        let records = sample_records();
        let params = test_params();

        // Create backup.
        let manifest = create_full_backup(&backend, records.clone(), "test-pass", &params)
            .await
            .unwrap();

        assert_eq!(manifest.backup_type, BackupType::Full);
        assert_eq!(manifest.collections.len(), 2);
        assert!(manifest.record_counts.get("tasks").unwrap() == &2);
        assert!(manifest.record_counts.get("contacts").unwrap() == &1);
        assert!(!manifest.checksum.is_empty());
        assert!(manifest.compressed_size > 0);
        assert!(manifest.parent_id.is_none());

        // Restore backup.
        let (restored_records, result) =
            restore_full(&backend, &manifest.id, "test-pass", &params)
                .await
                .unwrap();

        assert_eq!(restored_records.len(), 3);
        assert_eq!(result.total_restored, 3);
        assert!(result.integrity_verified);
        assert_eq!(result.records_restored.get("tasks").unwrap(), &2);
        assert_eq!(result.records_restored.get("contacts").unwrap(), &1);

        // Verify data integrity.
        let task_1 = restored_records.iter().find(|r| r.id == "rec-1").unwrap();
        assert_eq!(task_1.data["title"], "Buy groceries");
    }

    #[tokio::test]
    async fn full_backup_restore_with_wrong_passphrase_fails() {
        let tmp = TempDir::new().unwrap();
        let backend = LocalBackend::new(tmp.path());
        let records = sample_records();
        let params = test_params();

        let manifest = create_full_backup(&backend, records, "correct-pass", &params)
            .await
            .unwrap();

        let result = restore_full(&backend, &manifest.id, "wrong-pass", &params).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("authentication failed"));
    }

    #[tokio::test]
    async fn full_backup_checksum_verified_on_restore() {
        let tmp = TempDir::new().unwrap();
        let backend = LocalBackend::new(tmp.path());
        let records = sample_records();
        let params = test_params();

        let manifest = create_full_backup(&backend, records, "test-pass", &params)
            .await
            .unwrap();

        // Tamper with the encrypted file.
        let storage_key = format!("{}.enc", manifest.id);
        let mut encrypted = backend.get(&storage_key).await.unwrap();
        if encrypted.len() > 20 {
            encrypted[15] ^= 0xFF;
        }
        backend.put(&storage_key, &encrypted).await.unwrap();

        let result = restore_full(&backend, &manifest.id, "test-pass", &params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("checksum mismatch"));
    }

    #[tokio::test]
    async fn full_backup_empty_records() {
        let tmp = TempDir::new().unwrap();
        let backend = LocalBackend::new(tmp.path());
        let params = test_params();

        let manifest = create_full_backup(&backend, vec![], "test-pass", &params)
            .await
            .unwrap();

        assert!(manifest.collections.is_empty());
        assert_eq!(manifest.backup_type, BackupType::Full);

        let (restored, result) = restore_full(&backend, &manifest.id, "test-pass", &params)
            .await
            .unwrap();
        assert!(restored.is_empty());
        assert_eq!(result.total_restored, 0);
    }

    // ── Incremental backup tests ─────────────────────────────────────

    #[tokio::test]
    async fn incremental_backup_only_includes_changed_records() {
        let tmp = TempDir::new().unwrap();
        let backend = LocalBackend::new(tmp.path());
        let params = test_params();

        // Full backup first.
        let full_manifest = create_full_backup(&backend, sample_records(), "test-pass", &params)
            .await
            .unwrap();

        // Only one record changed.
        let changed = vec![BackupRecord {
            id: "rec-1".into(),
            plugin_id: "com.life-engine.todos".into(),
            collection: "tasks".into(),
            data: serde_json::json!({"title": "Buy groceries", "status": "completed"}),
            version: 2,
            user_id: None,
            household_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }];

        let incr_manifest = create_incremental_backup(
            &backend,
            changed,
            &full_manifest.id,
            "test-pass",
            &params,
        )
        .await
        .unwrap();

        assert_eq!(incr_manifest.backup_type, BackupType::Incremental);
        assert_eq!(incr_manifest.parent_id.as_deref(), Some(full_manifest.id.as_str()));
        assert_eq!(incr_manifest.record_counts.get("tasks").unwrap(), &1);

        // Restore incremental.
        let (records, result) = restore_full(&backend, &incr_manifest.id, "test-pass", &params)
            .await
            .unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(result.total_restored, 1);
        assert_eq!(records[0].data["status"], "completed");
    }

    // ── Partial restore tests ────────────────────────────────────────

    #[tokio::test]
    async fn partial_restore_filters_by_collection() {
        let tmp = TempDir::new().unwrap();
        let backend = LocalBackend::new(tmp.path());
        let params = test_params();

        let manifest = create_full_backup(&backend, sample_records(), "test-pass", &params)
            .await
            .unwrap();

        // Restore only contacts.
        let (records, result) = restore_partial(
            &backend,
            &manifest.id,
            &["contacts".into()],
            "test-pass",
            &params,
        )
        .await
        .unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(result.total_restored, 1);
        assert_eq!(records[0].collection, "contacts");
        assert_eq!(records[0].data["name"], "Alice");
    }

    #[tokio::test]
    async fn partial_restore_nonexistent_collection_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let backend = LocalBackend::new(tmp.path());
        let params = test_params();

        let manifest = create_full_backup(&backend, sample_records(), "test-pass", &params)
            .await
            .unwrap();

        let (records, result) = restore_partial(
            &backend,
            &manifest.id,
            &["nonexistent".into()],
            "test-pass",
            &params,
        )
        .await
        .unwrap();

        assert!(records.is_empty());
        assert_eq!(result.total_restored, 0);
    }

    #[tokio::test]
    async fn partial_restore_does_not_affect_other_collections() {
        let tmp = TempDir::new().unwrap();
        let backend = LocalBackend::new(tmp.path());
        let params = test_params();

        let manifest = create_full_backup(&backend, sample_records(), "test-pass", &params)
            .await
            .unwrap();

        // Restore only tasks.
        let (records, result) = restore_partial(
            &backend,
            &manifest.id,
            &["tasks".into()],
            "test-pass",
            &params,
        )
        .await
        .unwrap();

        assert_eq!(records.len(), 2);
        assert_eq!(result.total_restored, 2);
        assert!(records.iter().all(|r| r.collection == "tasks"));
    }

    // ── List backups test ────────────────────────────────────────────

    #[tokio::test]
    async fn list_backups_returns_manifests() {
        let tmp = TempDir::new().unwrap();
        let backend = LocalBackend::new(tmp.path());
        let params = test_params();

        create_full_backup(&backend, sample_records(), "test-pass", &params)
            .await
            .unwrap();
        create_full_backup(&backend, sample_records(), "test-pass", &params)
            .await
            .unwrap();

        let manifests = list_backups(&backend).await.unwrap();
        assert_eq!(manifests.len(), 2);
        // Sorted newest first.
        assert!(manifests[0].created_at >= manifests[1].created_at);
    }
}
