//! WebDAV backup backend.

use async_trait::async_trait;
use quick_xml::events::Event;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};

use super::{BackupBackend, StoredBackup};

/// Configuration for WebDAV backup storage.
#[derive(Clone, Serialize, Deserialize)]
pub struct WebDavConfig {
    /// Base URL of the WebDAV server (e.g. `https://dav.example.com/backups/`).
    pub url: String,
    /// Username for HTTP Basic authentication.
    pub username: String,
    /// Password for HTTP Basic authentication.
    #[serde(skip_serializing)]
    pub password: String,
}

impl std::fmt::Debug for WebDavConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebDavConfig")
            .field("url", &self.url)
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .finish()
    }
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
        let encoded_key = urlencoding::encode(key);
        format!("{base}/{encoded_key}")
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

/// Parse a WebDAV PROPFIND multi-status response using quick-xml to extract file entries.
fn parse_propfind_response(xml: &str, prefix: &str) -> Vec<StoredBackup> {
    let mut results = Vec::new();
    let mut reader = Reader::from_str(xml);
    let mut in_href = false;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local_name = e.local_name();
                if local_name.as_ref() == b"href" {
                    in_href = true;
                }
            }
            Ok(Event::Text(ref e)) if in_href => {
                if let Ok(text) = e.unescape() {
                    let key = text.trim_start_matches('/').to_string();
                    if !key.is_empty() && !key.ends_with('/') && key.contains(prefix) {
                        results.push(StoredBackup {
                            key,
                            size: 0,
                            last_modified: String::new(),
                        });
                    }
                }
                in_href = false;
            }
            Ok(Event::End(ref e)) => {
                if e.local_name().as_ref() == b"href" {
                    in_href = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webdav_config_debug_redacts_password() {
        let config = WebDavConfig {
            url: "https://dav.example.com/backups".into(),
            username: "user".into(),
            password: "super-secret".into(),
        };
        let debug_output = format!("{:?}", config);
        assert!(!debug_output.contains("super-secret"));
        assert!(debug_output.contains("[REDACTED]"));
    }

    #[test]
    fn webdav_config_serialization_skips_password() {
        let config = WebDavConfig {
            url: "https://dav.example.com/backups".into(),
            username: "user".into(),
            password: "pass".into(),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("pass"));
        // Can still deserialize with password provided
        let json_with_pass = r#"{"url":"https://dav.example.com/backups","username":"user","password":"pass"}"#;
        let restored: WebDavConfig = serde_json::from_str(json_with_pass).unwrap();
        assert_eq!(restored.url, "https://dav.example.com/backups");
        assert_eq!(restored.password, "pass");
    }

    #[test]
    fn full_url_encodes_key() {
        let backend = WebDavBackend::new(WebDavConfig {
            url: "https://dav.example.com/backups/".into(),
            username: "user".into(),
            password: "pass".into(),
        });
        // Normal key
        assert_eq!(
            backend.full_url("full-001.enc"),
            "https://dav.example.com/backups/full-001.enc"
        );
        // Key with path traversal characters gets encoded
        let url = backend.full_url("../../../etc/passwd");
        assert!(!url.contains("../"));
        assert!(url.contains("%2F"));
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
    fn parse_propfind_unprefixed_namespaces() {
        let xml = r#"
        <?xml version="1.0" encoding="utf-8"?>
        <multistatus xmlns="DAV:">
          <response>
            <href>/backups/full-001.enc</href>
          </response>
        </multistatus>
        "#;
        let results = parse_propfind_response(xml, "backups/");
        assert_eq!(results.len(), 1);
        assert!(results[0].key.contains("full-001.enc"));
    }

    #[test]
    fn parse_propfind_malformed_xml_no_panic() {
        let xml = "<<<not valid xml>>>";
        let results = parse_propfind_response(xml, "backups/");
        assert!(results.is_empty());
    }
}
