//! WebDAV backup backend.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{BackupBackend, StoredBackup};

/// Configuration for WebDAV backup storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebDavConfig {
    /// Base URL of the WebDAV server (e.g. `https://dav.example.com/backups/`).
    pub url: String,
    /// Username for HTTP Basic authentication.
    pub username: String,
    /// Password for HTTP Basic authentication.
    pub password: String,
}

/// WebDAV backup backend using HTTP PUT/GET/DELETE.
pub struct WebDavBackend {
    config: WebDavConfig,
    client: reqwest::Client,
}

impl WebDavBackend {
    pub fn new(config: WebDavConfig) -> Self {
        let client = reqwest::Client::new();
        Self { config, client }
    }

    fn full_url(&self, key: &str) -> String {
        let base = self.config.url.trim_end_matches('/');
        format!("{base}/{key}")
    }
}

#[async_trait]
impl BackupBackend for WebDavBackend {
    async fn put(&self, key: &str, data: &[u8]) -> anyhow::Result<()> {
        let url = self.full_url(key);
        let resp = self
            .client
            .put(&url)
            .basic_auth(&self.config.username, Some(&self.config.password))
            .body(data.to_vec())
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!(
                "WebDAV PUT failed: {} {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            );
        }

        Ok(())
    }

    async fn get(&self, key: &str) -> anyhow::Result<Vec<u8>> {
        let url = self.full_url(key);
        let resp = self
            .client
            .get(&url)
            .basic_auth(&self.config.username, Some(&self.config.password))
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!(
                "WebDAV GET failed: {} {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            );
        }

        Ok(resp.bytes().await?.to_vec())
    }

    async fn delete(&self, key: &str) -> anyhow::Result<bool> {
        let url = self.full_url(key);
        let resp = self
            .client
            .delete(&url)
            .basic_auth(&self.config.username, Some(&self.config.password))
            .send()
            .await?;

        if resp.status().as_u16() == 404 {
            return Ok(false);
        }

        if !resp.status().is_success() {
            anyhow::bail!(
                "WebDAV DELETE failed: {} {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            );
        }

        Ok(true)
    }

    async fn list(&self, prefix: &str) -> anyhow::Result<Vec<StoredBackup>> {
        // WebDAV PROPFIND to list directory contents.
        let url = self.full_url(prefix);
        let resp = self
            .client
            .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), &url)
            .basic_auth(&self.config.username, Some(&self.config.password))
            .header("Depth", "1")
            .send()
            .await?;

        if !resp.status().is_success() && resp.status().as_u16() != 207 {
            anyhow::bail!(
                "WebDAV PROPFIND failed: {}",
                resp.status()
            );
        }

        // Parse the multi-status XML response to extract href entries.
        let body = resp.text().await?;
        let results = parse_propfind_response(&body, prefix);
        Ok(results)
    }

    async fn exists(&self, key: &str) -> anyhow::Result<bool> {
        let url = self.full_url(key);
        let resp = self
            .client
            .head(&url)
            .basic_auth(&self.config.username, Some(&self.config.password))
            .send()
            .await?;

        Ok(resp.status().is_success())
    }
}

/// Parse a WebDAV PROPFIND multi-status response to extract file entries.
fn parse_propfind_response(xml: &str, prefix: &str) -> Vec<StoredBackup> {
    let mut results = Vec::new();

    // Simple XML extraction — find <d:href> elements.
    for line in xml.lines() {
        let trimmed = line.trim();
        if let Some(href) = extract_tag_content(trimmed, "href")
            .or_else(|| extract_tag_content(trimmed, "d:href"))
            .or_else(|| extract_tag_content(trimmed, "D:href"))
        {
            let key = href.trim_start_matches('/').to_string();
            if !key.is_empty() && !key.ends_with('/') && key.contains(prefix) {
                results.push(StoredBackup {
                    key,
                    size: 0,
                    last_modified: String::new(),
                });
            }
        }
    }

    results
}

/// Extract text content between XML tags.
fn extract_tag_content<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    if let Some(start) = text.find(&open) {
        let content_start = start + open.len();
        if let Some(end) = text[content_start..].find(&close) {
            return Some(&text[content_start..content_start + end]);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webdav_config_serialization() {
        let config = WebDavConfig {
            url: "https://dav.example.com/backups".into(),
            username: "user".into(),
            password: "pass".into(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let restored: WebDavConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.url, "https://dav.example.com/backups");
    }

    #[test]
    fn full_url_construction() {
        let backend = WebDavBackend::new(WebDavConfig {
            url: "https://dav.example.com/backups/".into(),
            username: "user".into(),
            password: "pass".into(),
        });
        assert_eq!(
            backend.full_url("full-001.enc"),
            "https://dav.example.com/backups/full-001.enc"
        );
    }

    #[test]
    fn full_url_without_trailing_slash() {
        let backend = WebDavBackend::new(WebDavConfig {
            url: "https://dav.example.com/backups".into(),
            username: "user".into(),
            password: "pass".into(),
        });
        assert_eq!(
            backend.full_url("full-001.enc"),
            "https://dav.example.com/backups/full-001.enc"
        );
    }

    #[test]
    fn parse_propfind_basic() {
        let xml = r#"
        <?xml version="1.0" encoding="utf-8"?>
        <d:multistatus xmlns:d="DAV:">
          <d:response>
            <d:href>/backups/</d:href>
          </d:response>
          <d:response>
            <d:href>/backups/full-001.enc</d:href>
          </d:response>
          <d:response>
            <d:href>/backups/full-002.enc</d:href>
          </d:response>
        </d:multistatus>
        "#;
        let results = parse_propfind_response(xml, "backups/");
        assert_eq!(results.len(), 2);
        assert!(results[0].key.contains("full-001.enc"));
        assert!(results[1].key.contains("full-002.enc"));
    }

    #[test]
    fn extract_tag_content_works() {
        assert_eq!(
            extract_tag_content("<d:href>/path/file.txt</d:href>", "d:href"),
            Some("/path/file.txt")
        );
        assert_eq!(extract_tag_content("<other>data</other>", "d:href"), None);
    }
}
