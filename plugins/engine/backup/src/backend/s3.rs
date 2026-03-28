//! S3-compatible backup backend.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{BackupBackend, StoredBackup};

/// Configuration for S3-compatible backup storage.
#[derive(Clone, Serialize, Deserialize)]
pub struct S3BackupConfig {
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub access_key_id: String,
    #[serde(skip_serializing)]
    pub secret_access_key: String,
    pub prefix: Option<String>,
}

impl std::fmt::Debug for S3BackupConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3BackupConfig")
            .field("endpoint", &self.endpoint)
            .field("region", &self.region)
            .field("bucket", &self.bucket)
            .field("access_key_id", &self.access_key_id)
            .field("secret_access_key", &"[REDACTED]")
            .field("prefix", &self.prefix)
            .finish()
    }
}

/// S3-compatible backup backend.
pub struct S3Backend {
    config: S3BackupConfig,
    #[cfg(feature = "integration")]
    client: aws_sdk_s3::Client,
}

impl S3Backend {
    pub fn new(config: S3BackupConfig) -> Self {
        #[cfg(feature = "integration")]
        let client = Self::build_sdk_client_from_config(&config);
        Self {
            config,
            #[cfg(feature = "integration")]
            client,
        }
    }

    #[cfg_attr(not(feature = "integration"), allow(dead_code))]
    fn full_key(&self, key: &str) -> String {
        match &self.config.prefix {
            Some(prefix) if !prefix.is_empty() => {
                format!("{}/{}", prefix.trim_end_matches('/'), key)
            }
            _ => key.to_string(),
        }
    }

    pub fn config(&self) -> &S3BackupConfig {
        &self.config
    }
}

#[cfg(feature = "integration")]
#[async_trait]
impl BackupBackend for S3Backend {
    async fn put(&self, key: &str, data: &[u8]) -> anyhow::Result<()> {
        use aws_sdk_s3::primitives::ByteStream;

        let full_key = self.full_key(key);

        self.client
            .put_object()
            .bucket(&self.config.bucket)
            .key(&full_key)
            .body(ByteStream::from(data.to_vec()))
            .send()
            .await?;

        Ok(())
    }

    async fn get(&self, key: &str) -> anyhow::Result<Vec<u8>> {
        let full_key = self.full_key(key);

        let resp = self.client
            .get_object()
            .bucket(&self.config.bucket)
            .key(&full_key)
            .send()
            .await?;

        let body = resp.body.collect().await?;
        Ok(body.into_bytes().to_vec())
    }

    async fn delete(&self, key: &str) -> anyhow::Result<bool> {
        let full_key = self.full_key(key);

        // S3 delete on non-existent keys is a no-op, so no need to check first.
        self.client
            .delete_object()
            .bucket(&self.config.bucket)
            .key(&full_key)
            .send()
            .await?;

        Ok(true)
    }

    async fn list(&self, prefix: &str) -> anyhow::Result<Vec<StoredBackup>> {
        let full_prefix = self.full_key(prefix);
        let mut results = Vec::new();
        let mut continuation_token: Option<String> = None;

        loop {
            let mut req = self.client
                .list_objects_v2()
                .bucket(&self.config.bucket)
                .prefix(&full_prefix);

            if let Some(ref token) = continuation_token {
                req = req.continuation_token(token);
            }

            let resp = req.send().await?;

            for obj in resp.contents() {
                let key = obj.key().unwrap_or_default().to_string();
                let size = obj.size().unwrap_or_default() as u64;
                let last_modified = obj
                    .last_modified()
                    .and_then(|dt| {
                        chrono::DateTime::from_timestamp(dt.secs(), dt.subsec_nanos())
                    })
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default();

                results.push(StoredBackup {
                    key,
                    size,
                    last_modified,
                });
            }

            if resp.is_truncated() == Some(true) {
                continuation_token = resp.next_continuation_token().map(|s| s.to_string());
            } else {
                break;
            }
        }

        Ok(results)
    }

    async fn exists(&self, key: &str) -> anyhow::Result<bool> {
        let full_key = self.full_key(key);

        Ok(self.client
            .head_object()
            .bucket(&self.config.bucket)
            .key(&full_key)
            .send()
            .await
            .is_ok())
    }
}

#[cfg(feature = "integration")]
impl S3Backend {
    fn build_sdk_client_from_config(config: &S3BackupConfig) -> aws_sdk_s3::Client {
        let creds = aws_sdk_s3::config::Credentials::new(
            &config.access_key_id,
            &config.secret_access_key,
            None,
            None,
            "life-engine-backup",
        );
        let sdk_config = aws_sdk_s3::config::Builder::new()
            .endpoint_url(&config.endpoint)
            .region(aws_sdk_s3::config::Region::new(config.region.clone()))
            .credentials_provider(creds)
            .force_path_style(true)
            .behavior_version_latest()
            .build();
        aws_sdk_s3::Client::from_conf(sdk_config)
    }
}

// Non-integration stub: compile but don't actually connect.
#[cfg(not(feature = "integration"))]
#[async_trait]
impl BackupBackend for S3Backend {
    async fn put(&self, _key: &str, _data: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("S3 backend requires the `integration` feature")
    }
    async fn get(&self, _key: &str) -> anyhow::Result<Vec<u8>> {
        anyhow::bail!("S3 backend requires the `integration` feature")
    }
    async fn delete(&self, _key: &str) -> anyhow::Result<bool> {
        anyhow::bail!("S3 backend requires the `integration` feature")
    }
    async fn list(&self, _prefix: &str) -> anyhow::Result<Vec<StoredBackup>> {
        anyhow::bail!("S3 backend requires the `integration` feature")
    }
    async fn exists(&self, _key: &str) -> anyhow::Result<bool> {
        anyhow::bail!("S3 backend requires the `integration` feature")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s3_config_serialization() {
        let config = S3BackupConfig {
            endpoint: "https://s3.amazonaws.com".into(),
            region: "us-east-1".into(),
            bucket: "backups".into(),
            access_key_id: "AKID".into(),
            secret_access_key: "SECRET".into(),
            prefix: Some("life-engine/".into()),
        };
        let json = serde_json::to_string(&config).unwrap();
        // secret_access_key is redacted from serialization output
        assert!(!json.contains("SECRET"));
        // Deserialization from a complete JSON string still works
        let full_json = r#"{"endpoint":"https://s3.amazonaws.com","region":"us-east-1","bucket":"backups","access_key_id":"AKID","secret_access_key":"SECRET","prefix":"life-engine/"}"#;
        let restored: S3BackupConfig = serde_json::from_str(full_json).unwrap();
        assert_eq!(restored.bucket, "backups");
        assert_eq!(restored.secret_access_key, "SECRET");
    }

    #[test]
    fn s3_full_key_with_prefix() {
        let backend = S3Backend::new(S3BackupConfig {
            endpoint: "http://localhost:9000".into(),
            region: "us-east-1".into(),
            bucket: "test".into(),
            access_key_id: "key".into(),
            secret_access_key: "secret".into(),
            prefix: Some("backups/".into()),
        });
        assert_eq!(backend.full_key("full-001.enc"), "backups/full-001.enc");
    }

    #[test]
    fn s3_full_key_without_prefix() {
        let backend = S3Backend::new(S3BackupConfig {
            endpoint: "http://localhost:9000".into(),
            region: "us-east-1".into(),
            bucket: "test".into(),
            access_key_id: "key".into(),
            secret_access_key: "secret".into(),
            prefix: None,
        });
        assert_eq!(backend.full_key("full-001.enc"), "full-001.enc");
    }
}
