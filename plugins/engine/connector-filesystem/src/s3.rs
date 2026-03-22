//! S3-compatible cloud storage connector.
//!
//! Defines the `CloudStorageConnector` trait for abstracting cloud object
//! storage operations, along with S3 configuration and sync state types.
//! The actual AWS SDK implementation is behind the `integration` feature.

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use life_engine_types::FileMetadata;
use serde::{Deserialize, Serialize};

/// Configuration for an S3-compatible storage endpoint.
#[derive(Clone, Serialize, Deserialize)]
pub struct S3Config {
    /// The S3 endpoint URL (e.g. `https://s3.amazonaws.com` or MinIO URL).
    pub endpoint: String,
    /// The AWS region.
    pub region: String,
    /// The S3 bucket name.
    pub bucket: String,
    /// The access key ID for authentication.
    pub access_key_id: String,
    /// The secret access key for authentication.
    #[serde(skip_serializing)]
    pub secret_access_key: String,
    /// Optional key prefix to scope operations.
    pub prefix: Option<String>,
}

impl std::fmt::Debug for S3Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3Config")
            .field("endpoint", &self.endpoint)
            .field("region", &self.region)
            .field("bucket", &self.bucket)
            .field("access_key_id", &self.access_key_id)
            .field("secret_access_key", &"[REDACTED]")
            .field("prefix", &self.prefix)
            .finish()
    }
}

/// Tracks sync state for cloud storage operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CloudSyncState {
    /// The last sync timestamp.
    pub last_sync: Option<DateTime<Utc>>,
    /// Known objects keyed by their S3 key.
    pub known_objects: HashMap<String, ObjectState>,
}

/// State of a single known object in cloud storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectState {
    /// Object size in bytes.
    pub size: u64,
    /// Last modified timestamp from S3.
    pub last_modified: DateTime<Utc>,
    /// ETag from S3 (typically MD5 hash).
    pub etag: Option<String>,
}

/// Trait for cloud storage connectors (S3, MinIO, etc.).
///
/// Provides a common interface for listing, reading, writing, and
/// deleting objects in cloud storage.
#[async_trait]
pub trait CloudStorageConnector: Send + Sync {
    /// List objects matching the given prefix.
    async fn list_objects(&self, prefix: &str) -> anyhow::Result<Vec<FileMetadata>>;

    /// Download an object's contents by key.
    async fn get_object(&self, key: &str) -> anyhow::Result<Vec<u8>>;

    /// Upload data to the given key.
    async fn put_object(&self, key: &str, data: &[u8]) -> anyhow::Result<()>;

    /// Delete an object by key. Returns `true` if the object existed.
    async fn delete_object(&self, key: &str) -> anyhow::Result<bool>;
}

/// S3 client that manages configuration and sync state.
pub struct S3Client {
    /// Connection configuration.
    config: S3Config,
    /// Sync state tracking known objects.
    sync_state: CloudSyncState,
}

impl S3Client {
    /// Create a new S3 client with the given configuration.
    pub fn new(config: S3Config) -> Self {
        Self {
            config,
            sync_state: CloudSyncState::default(),
        }
    }

    /// Returns the S3 configuration.
    pub fn config(&self) -> &S3Config {
        &self.config
    }

    /// Returns the current sync state.
    pub fn sync_state(&self) -> &CloudSyncState {
        &self.sync_state
    }

    /// Returns a mutable reference to the sync state.
    pub fn sync_state_mut(&mut self) -> &mut CloudSyncState {
        &mut self.sync_state
    }

    /// Build the full S3 key for an object, prepending the configured prefix.
    pub fn full_key(&self, key: &str) -> String {
        match &self.config.prefix {
            Some(prefix) if !prefix.is_empty() => format!("{}/{}", prefix.trim_end_matches('/'), key),
            _ => key.to_string(),
        }
    }

    /// Update the sync state with a known object.
    pub fn track_object(&mut self, key: &str, size: u64, last_modified: DateTime<Utc>, etag: Option<String>) {
        self.sync_state.known_objects.insert(
            key.to_string(),
            ObjectState {
                size,
                last_modified,
                etag,
            },
        );
    }

    /// Mark the sync state as synced at the current time.
    pub fn mark_synced(&mut self) {
        self.sync_state.last_sync = Some(Utc::now());
    }
}

#[cfg(feature = "integration")]
impl S3Client {
    /// Build an `aws_sdk_s3::Client` from the stored `S3Config`.
    ///
    /// Uses explicit credentials, path-style addressing (required for MinIO),
    /// and the endpoint / region from the configuration.
    fn build_sdk_client(&self) -> aws_sdk_s3::Client {
        let creds = aws_sdk_s3::config::Credentials::new(
            &self.config.access_key_id,
            &self.config.secret_access_key,
            None,
            None,
            "life-engine-s3-client",
        );
        let config = aws_sdk_s3::config::Builder::new()
            .endpoint_url(&self.config.endpoint)
            .region(aws_sdk_s3::config::Region::new(self.config.region.clone()))
            .credentials_provider(creds)
            .force_path_style(true)
            .behavior_version_latest()
            .build();
        aws_sdk_s3::Client::from_conf(config)
    }
}

#[cfg(feature = "integration")]
#[async_trait]
impl CloudStorageConnector for S3Client {
    async fn list_objects(&self, prefix: &str) -> anyhow::Result<Vec<FileMetadata>> {
        use uuid::Uuid;

        let client = self.build_sdk_client();
        let full_prefix = self.full_key(prefix);

        let resp = client
            .list_objects_v2()
            .bucket(&self.config.bucket)
            .prefix(&full_prefix)
            .send()
            .await?;

        let mut results = Vec::new();
        for obj in resp.contents() {
            let key = obj.key().unwrap_or_default();
            let name = key.rsplit('/').next().unwrap_or(key).to_string();
            let size = obj.size().unwrap_or_default() as u64;
            let last_modified = obj
                .last_modified()
                .and_then(|dt| {
                    DateTime::from_timestamp(dt.secs(), dt.subsec_nanos())
                })
                .unwrap_or_else(Utc::now);

            let mime_type = crate::normalizer::detect_mime_type(std::path::Path::new(&name));

            results.push(FileMetadata {
                id: Uuid::new_v4(),
                name,
                mime_type,
                size,
                path: format!("s3://{}/{}", self.config.bucket, key),
                checksum: obj.e_tag().map(|e| format!("etag:{}", e.trim_matches('"'))),
                source: "s3".into(),
                source_id: key.to_string(),
                extensions: None,
                created_at: last_modified,
                updated_at: last_modified,
            });
        }

        Ok(results)
    }

    async fn get_object(&self, key: &str) -> anyhow::Result<Vec<u8>> {
        let client = self.build_sdk_client();
        let full_key = self.full_key(key);

        let resp = client
            .get_object()
            .bucket(&self.config.bucket)
            .key(&full_key)
            .send()
            .await?;

        let body = resp.body.collect().await?;
        Ok(body.into_bytes().to_vec())
    }

    async fn put_object(&self, key: &str, data: &[u8]) -> anyhow::Result<()> {
        use aws_sdk_s3::primitives::ByteStream;

        let client = self.build_sdk_client();
        let full_key = self.full_key(key);

        client
            .put_object()
            .bucket(&self.config.bucket)
            .key(&full_key)
            .body(ByteStream::from(data.to_vec()))
            .send()
            .await?;

        Ok(())
    }

    async fn delete_object(&self, key: &str) -> anyhow::Result<bool> {
        let client = self.build_sdk_client();
        let full_key = self.full_key(key);

        // Check if object exists first
        let exists = client
            .head_object()
            .bucket(&self.config.bucket)
            .key(&full_key)
            .send()
            .await
            .is_ok();

        if exists {
            client
                .delete_object()
                .bucket(&self.config.bucket)
                .key(&full_key)
                .send()
                .await?;
        }

        Ok(exists)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s3_config_serialization() {
        let config = S3Config {
            endpoint: "https://s3.amazonaws.com".into(),
            region: "us-east-1".into(),
            bucket: "my-bucket".into(),
            access_key_id: "AKID".into(),
            secret_access_key: "SECRET".into(),
            prefix: Some("files/".into()),
        };
        let json = serde_json::to_string(&config).expect("serialize");
        // secret_access_key is redacted from serialization output
        assert!(!json.contains("SECRET"));
        // Deserialization from a complete JSON string still works
        let full_json = r#"{"endpoint":"https://s3.amazonaws.com","region":"us-east-1","bucket":"my-bucket","access_key_id":"AKID","secret_access_key":"SECRET","prefix":"files/"}"#;
        let restored: S3Config = serde_json::from_str(full_json).expect("deserialize");
        assert_eq!(restored.endpoint, "https://s3.amazonaws.com");
        assert_eq!(restored.region, "us-east-1");
        assert_eq!(restored.bucket, "my-bucket");
        assert_eq!(restored.prefix, Some("files/".into()));
    }

    #[test]
    fn s3_client_construction() {
        let config = S3Config {
            endpoint: "http://localhost:9000".into(),
            region: "us-east-1".into(),
            bucket: "test-bucket".into(),
            access_key_id: "minioadmin".into(),
            secret_access_key: "minioadmin".into(),
            prefix: None,
        };
        let client = S3Client::new(config);
        assert_eq!(client.config().bucket, "test-bucket");
        assert!(client.sync_state().last_sync.is_none());
        assert!(client.sync_state().known_objects.is_empty());
    }

    #[test]
    fn s3_full_key_with_prefix() {
        let config = S3Config {
            endpoint: "http://localhost:9000".into(),
            region: "us-east-1".into(),
            bucket: "test".into(),
            access_key_id: "key".into(),
            secret_access_key: "secret".into(),
            prefix: Some("uploads/".into()),
        };
        let client = S3Client::new(config);
        assert_eq!(client.full_key("document.pdf"), "uploads/document.pdf");
    }

    #[test]
    fn s3_full_key_without_prefix() {
        let config = S3Config {
            endpoint: "http://localhost:9000".into(),
            region: "us-east-1".into(),
            bucket: "test".into(),
            access_key_id: "key".into(),
            secret_access_key: "secret".into(),
            prefix: None,
        };
        let client = S3Client::new(config);
        assert_eq!(client.full_key("document.pdf"), "document.pdf");
    }

    #[test]
    fn s3_track_object() {
        let config = S3Config {
            endpoint: "http://localhost:9000".into(),
            region: "us-east-1".into(),
            bucket: "test".into(),
            access_key_id: "key".into(),
            secret_access_key: "secret".into(),
            prefix: None,
        };
        let mut client = S3Client::new(config);
        let now = Utc::now();

        client.track_object("file.txt", 1024, now, Some("abc123".into()));

        let state = client.sync_state().known_objects.get("file.txt");
        assert!(state.is_some());
        let obj = state.unwrap();
        assert_eq!(obj.size, 1024);
        assert_eq!(obj.etag, Some("abc123".into()));
    }

    #[test]
    fn s3_mark_synced() {
        let config = S3Config {
            endpoint: "http://localhost:9000".into(),
            region: "us-east-1".into(),
            bucket: "test".into(),
            access_key_id: "key".into(),
            secret_access_key: "secret".into(),
            prefix: None,
        };
        let mut client = S3Client::new(config);
        assert!(client.sync_state().last_sync.is_none());

        client.mark_synced();
        assert!(client.sync_state().last_sync.is_some());
    }

    #[test]
    fn cloud_sync_state_serialization() {
        let mut state = CloudSyncState {
            last_sync: Some(Utc::now()),
            ..Default::default()
        };
        state.known_objects.insert(
            "test.txt".into(),
            ObjectState {
                size: 512,
                last_modified: Utc::now(),
                etag: Some("etag123".into()),
            },
        );

        let json = serde_json::to_string(&state).expect("serialize");
        let restored: CloudSyncState = serde_json::from_str(&json).expect("deserialize");
        assert!(restored.last_sync.is_some());
        assert_eq!(restored.known_objects.len(), 1);
    }

    #[test]
    fn cloud_storage_connector_trait_is_object_safe() {
        // This test verifies the trait is object-safe by constructing a trait object type.
        // If this compiles, the trait is object-safe.
        fn _assert_object_safe(_: &dyn CloudStorageConnector) {}
    }
}
