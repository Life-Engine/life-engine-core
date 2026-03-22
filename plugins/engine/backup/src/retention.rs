//! Retention policy enforcement for backup cleanup.

use crate::backend::BackupBackend;
use crate::types::{BackupManifest, RetentionPolicy};

/// Apply retention policy: delete old backups beyond the max count.
///
/// Keeps the most recent `policy.max_count` backups and deletes the rest.
/// Returns the number of backups deleted.
pub async fn enforce_retention(
    backend: &dyn BackupBackend,
    manifests: &[BackupManifest],
    policy: &RetentionPolicy,
) -> anyhow::Result<usize> {
    if manifests.len() <= policy.max_count {
        return Ok(0);
    }

    // Manifests should already be sorted newest-first.
    let to_delete = &manifests[policy.max_count..];
    let mut deleted = 0;

    for manifest in to_delete {
        // Delete the encrypted backup file.
        let enc_key = format!("{}.enc", manifest.id);
        backend.delete(&enc_key).await?;

        // Delete the manifest file.
        let manifest_key = format!("{}.manifest.json", manifest.id);
        backend.delete(&manifest_key).await?;

        deleted += 1;
    }

    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::local::LocalBackend;
    use crate::engine::{create_full_backup, list_backups};
    use crate::types::{Argon2Params, BackupRecord};
    use chrono::Utc;
    use tempfile::TempDir;

    fn test_params() -> Argon2Params {
        Argon2Params {
            memory_mb: 1,
            iterations: 1,
            parallelism: 1,
        }
    }

    fn one_record() -> Vec<BackupRecord> {
        vec![BackupRecord {
            id: "rec-1".into(),
            plugin_id: "test".into(),
            collection: "tasks".into(),
            data: serde_json::json!({"title": "test"}),
            version: 1,
            user_id: None,
            household_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }]
    }

    #[tokio::test]
    async fn retention_deletes_old_backups() {
        let tmp = TempDir::new().unwrap();
        let backend = LocalBackend::new(tmp.path());
        let params = test_params();

        // Create 5 backups.
        for _ in 0..5 {
            create_full_backup(&backend, one_record(), "pass", &params)
                .await
                .unwrap();
            // Small delay to ensure different timestamps.
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let manifests = list_backups(&backend).await.unwrap();
        assert_eq!(manifests.len(), 5);

        // Keep only 2.
        let policy = RetentionPolicy { max_count: 2 };
        let deleted = enforce_retention(&backend, &manifests, &policy)
            .await
            .unwrap();
        assert_eq!(deleted, 3);

        // Verify only 2 remain.
        let remaining = list_backups(&backend).await.unwrap();
        assert_eq!(remaining.len(), 2);
    }

    #[tokio::test]
    async fn retention_does_nothing_when_under_limit() {
        let tmp = TempDir::new().unwrap();
        let backend = LocalBackend::new(tmp.path());
        let params = test_params();

        create_full_backup(&backend, one_record(), "pass", &params)
            .await
            .unwrap();

        let manifests = list_backups(&backend).await.unwrap();
        let policy = RetentionPolicy { max_count: 5 };
        let deleted = enforce_retention(&backend, &manifests, &policy)
            .await
            .unwrap();
        assert_eq!(deleted, 0);
    }

    #[tokio::test]
    async fn retention_keeps_newest_backups() {
        let tmp = TempDir::new().unwrap();
        let backend = LocalBackend::new(tmp.path());
        let params = test_params();

        let mut ids = Vec::new();
        for _ in 0..4 {
            let m = create_full_backup(&backend, one_record(), "pass", &params)
                .await
                .unwrap();
            ids.push(m.id.clone());
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let manifests = list_backups(&backend).await.unwrap();
        let policy = RetentionPolicy { max_count: 2 };
        enforce_retention(&backend, &manifests, &policy)
            .await
            .unwrap();

        let remaining = list_backups(&backend).await.unwrap();
        assert_eq!(remaining.len(), 2);

        // The newest two should remain (manifests are sorted newest first).
        let remaining_ids: Vec<&str> = remaining.iter().map(|m| m.id.as_str()).collect();
        assert!(remaining_ids.contains(&manifests[0].id.as_str()));
        assert!(remaining_ids.contains(&manifests[1].id.as_str()));
    }

    #[tokio::test]
    async fn retention_with_empty_list() {
        let tmp = TempDir::new().unwrap();
        let backend = LocalBackend::new(tmp.path());

        let policy = RetentionPolicy { max_count: 5 };
        let deleted = enforce_retention(&backend, &[], &policy).await.unwrap();
        assert_eq!(deleted, 0);
    }

    #[tokio::test]
    async fn retention_max_count_zero_deletes_all() {
        let tmp = TempDir::new().unwrap();
        let backend = LocalBackend::new(tmp.path());
        let params = test_params();

        for _ in 0..3 {
            create_full_backup(&backend, one_record(), "pass", &params)
                .await
                .unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let manifests = list_backups(&backend).await.unwrap();
        let policy = RetentionPolicy { max_count: 0 };
        let deleted = enforce_retention(&backend, &manifests, &policy)
            .await
            .unwrap();
        assert_eq!(deleted, 3);

        let remaining = list_backups(&backend).await.unwrap();
        assert!(remaining.is_empty());
    }
}
