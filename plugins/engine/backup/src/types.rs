//! Types for the backup plugin.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for the backup plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    /// The passphrase used to derive the encryption key.
    /// Same derivation as SQLCipher (Argon2id).
    #[serde(skip_serializing)]
    pub passphrase: String,

    /// Backup storage target configuration.
    pub target: BackupTarget,

    /// Backup schedule (cron expression or preset).
    pub schedule: Option<BackupSchedule>,

    /// Retention policy.
    pub retention: Option<RetentionPolicy>,

    /// Argon2 settings for key derivation.
    #[serde(default)]
    pub argon2: Argon2Params,
}

/// Argon2 parameters for backup encryption key derivation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Argon2Params {
    /// Memory cost in megabytes.
    #[serde(default = "default_memory_mb")]
    pub memory_mb: u32,
    /// Number of iterations.
    #[serde(default = "default_iterations")]
    pub iterations: u32,
    /// Degree of parallelism.
    #[serde(default = "default_parallelism")]
    pub parallelism: u32,
}

fn default_memory_mb() -> u32 { 64 }
fn default_iterations() -> u32 { 3 }
fn default_parallelism() -> u32 { 1 }

impl Default for Argon2Params {
    fn default() -> Self {
        Self {
            memory_mb: default_memory_mb(),
            iterations: default_iterations(),
            parallelism: default_parallelism(),
        }
    }
}

/// The backup storage target.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BackupTarget {
    /// Local filesystem directory.
    Local {
        /// Path to the backup directory.
        path: String,
    },
    /// S3-compatible object storage.
    S3 {
        /// S3 endpoint URL.
        endpoint: String,
        /// AWS region.
        region: String,
        /// Bucket name.
        bucket: String,
        /// Access key ID.
        access_key_id: String,
        /// Secret access key.
        secret_access_key: String,
        /// Optional key prefix.
        prefix: Option<String>,
    },
    /// WebDAV server.
    WebDav {
        /// WebDAV base URL.
        url: String,
        /// Username for authentication.
        username: String,
        /// Password for authentication.
        password: String,
    },
}

/// Backup schedule configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BackupSchedule {
    /// Run daily at the specified hour (0-23).
    Daily { hour: u32 },
    /// Run weekly on the specified day (0=Sunday, 6=Saturday) and hour.
    Weekly { day: u32, hour: u32 },
    /// Custom cron expression.
    Cron { expression: String },
}

/// Retention policy for backup cleanup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    /// Maximum number of backups to keep.
    pub max_count: usize,
}

/// Metadata about a single backup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    /// Unique backup identifier.
    pub id: String,
    /// When the backup was created.
    pub created_at: DateTime<Utc>,
    /// Whether this is a full or incremental backup.
    pub backup_type: BackupType,
    /// Collections included in this backup.
    pub collections: Vec<String>,
    /// Number of records per collection.
    pub record_counts: HashMap<String, u64>,
    /// Total size in bytes (compressed, before encryption).
    pub compressed_size: u64,
    /// SHA-256 hash of the encrypted backup payload.
    pub checksum: String,
    /// ID of the parent backup (for incremental backups).
    pub parent_id: Option<String>,
    /// Cursor timestamp for incremental tracking.
    pub cursor: Option<String>,
}

/// Type of backup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupType {
    /// Contains all records from all collections.
    Full,
    /// Contains only records changed since the parent backup.
    Incremental,
}

/// A record snapshot as stored in the backup archive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupRecord {
    /// Record ID.
    pub id: String,
    /// Plugin ID that owns this record.
    pub plugin_id: String,
    /// Collection name.
    pub collection: String,
    /// Record data.
    pub data: serde_json::Value,
    /// Record version.
    pub version: i64,
    /// User ID (if set).
    pub user_id: Option<String>,
    /// Household ID (if set).
    pub household_id: Option<String>,
    /// Created timestamp.
    pub created_at: DateTime<Utc>,
    /// Updated timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Result of a restore operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResult {
    /// Number of records restored per collection.
    pub records_restored: HashMap<String, u64>,
    /// Total records restored.
    pub total_restored: u64,
    /// Whether integrity verification passed.
    pub integrity_verified: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_target_local_serialization() {
        let target = BackupTarget::Local {
            path: "/backups".into(),
        };
        let json = serde_json::to_string(&target).unwrap();
        let restored: BackupTarget = serde_json::from_str(&json).unwrap();
        match restored {
            BackupTarget::Local { path } => assert_eq!(path, "/backups"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn backup_target_s3_serialization() {
        let target = BackupTarget::S3 {
            endpoint: "https://s3.amazonaws.com".into(),
            region: "us-east-1".into(),
            bucket: "my-bucket".into(),
            access_key_id: "AKID".into(),
            secret_access_key: "SECRET".into(),
            prefix: Some("backups/".into()),
        };
        let json = serde_json::to_string(&target).unwrap();
        let restored: BackupTarget = serde_json::from_str(&json).unwrap();
        match restored {
            BackupTarget::S3 { bucket, .. } => assert_eq!(bucket, "my-bucket"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn backup_target_webdav_serialization() {
        let target = BackupTarget::WebDav {
            url: "https://dav.example.com/backups".into(),
            username: "user".into(),
            password: "pass".into(),
        };
        let json = serde_json::to_string(&target).unwrap();
        let restored: BackupTarget = serde_json::from_str(&json).unwrap();
        match restored {
            BackupTarget::WebDav { url, .. } => {
                assert_eq!(url, "https://dav.example.com/backups")
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn backup_type_serialization() {
        assert_eq!(
            serde_json::to_string(&BackupType::Full).unwrap(),
            "\"full\""
        );
        assert_eq!(
            serde_json::to_string(&BackupType::Incremental).unwrap(),
            "\"incremental\""
        );
    }

    #[test]
    fn backup_schedule_serialization() {
        let daily = BackupSchedule::Daily { hour: 3 };
        let json = serde_json::to_string(&daily).unwrap();
        assert!(json.contains("\"daily\""));

        let weekly = BackupSchedule::Weekly { day: 0, hour: 2 };
        let json = serde_json::to_string(&weekly).unwrap();
        assert!(json.contains("\"weekly\""));
    }

    #[test]
    fn retention_policy_serialization() {
        let policy = RetentionPolicy { max_count: 10 };
        let json = serde_json::to_string(&policy).unwrap();
        let restored: RetentionPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.max_count, 10);
    }

    #[test]
    fn argon2_params_default() {
        let params = Argon2Params::default();
        assert_eq!(params.memory_mb, 64);
        assert_eq!(params.iterations, 3);
        assert_eq!(params.parallelism, 1);
    }

    #[test]
    fn backup_manifest_serialization() {
        let manifest = BackupManifest {
            id: "bk-001".into(),
            created_at: Utc::now(),
            backup_type: BackupType::Full,
            collections: vec!["tasks".into(), "contacts".into()],
            record_counts: [("tasks".into(), 100), ("contacts".into(), 50)]
                .into_iter()
                .collect(),
            compressed_size: 4096,
            checksum: "abc123".into(),
            parent_id: None,
            cursor: None,
        };
        let json = serde_json::to_string(&manifest).unwrap();
        let restored: BackupManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, "bk-001");
        assert_eq!(restored.collections.len(), 2);
    }
}
