//! HTTP outbound request host function for WASM plugins.
//!
//! This host function allows plugins to make outbound HTTP requests.
//! It checks the plugin's `HttpOutbound` capability before executing
//! the request via `reqwest`.

use std::collections::HashMap;
use std::time::Duration;

use life_engine_traits::Capability;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::capability::ApprovedCapabilities;
use crate::error::PluginError;

/// Maximum request timeout in seconds.
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// Maximum response body size in bytes (10 MB).
const MAX_RESPONSE_BODY_SIZE: usize = 10 * 1024 * 1024;

/// Context passed to the HTTP host function, containing the plugin's identity,
/// approved capabilities, and an HTTP client.
#[derive(Clone)]
pub struct HttpHostContext {
    /// The plugin ID making the HTTP call.
    pub plugin_id: String,
    /// The plugin's approved capabilities.
    pub capabilities: ApprovedCapabilities,
    /// Shared HTTP client (connection pooling).
    pub client: Client,
    /// Domains this plugin is allowed to contact, declared in the manifest's
    /// `http_outbound` list. If `None`, all domains are allowed (backwards compat).
    /// If `Some(vec)`, only the listed domains are permitted.
    pub allowed_domains: Option<Vec<String>>,
}

/// Request payload for an outbound HTTP request from a plugin.
#[derive(Debug, Deserialize, Serialize)]
pub struct HttpRequest {
    /// HTTP method (GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS).
    pub method: String,
    /// The target URL.
    pub url: String,
    /// Optional request headers.
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Optional request body.
    #[serde(default)]
    pub body: Option<String>,
}

/// Response payload returned to the plugin after an HTTP request.
#[derive(Debug, Deserialize, Serialize)]
pub struct HttpResponse {
    /// HTTP status code.
    pub status: u16,
    /// Response headers.
    pub headers: HashMap<String, String>,
    /// Response body as a string.
    pub body: String,
}

/// Executes an outbound HTTP request on behalf of a plugin.
///
/// Deserializes the request from JSON bytes, checks the `HttpOutbound` capability,
/// executes the request via `reqwest`, and serializes the response back to JSON bytes.
pub async fn host_http_request(
    ctx: &HttpHostContext,
    input: &[u8],
) -> Result<Vec<u8>, PluginError> {
    // Check capability
    if !ctx.capabilities.has(Capability::HttpOutbound) {
        warn!(
            plugin_id = %ctx.plugin_id,
            "http:outbound capability violation"
        );
        return Err(PluginError::RuntimeCapabilityViolation(format!(
            "plugin '{}' lacks capability 'http:outbound'",
            ctx.plugin_id
        )));
    }

    // Deserialize the HTTP request from WASM input
    let request: HttpRequest = serde_json::from_slice(input).map_err(|e| {
        PluginError::ExecutionFailed(format!(
            "failed to deserialize HTTP request from plugin '{}': {e}",
            ctx.plugin_id
        ))
    })?;

    // Validate URL scheme — only HTTP/HTTPS allowed
    let scheme = request
        .url
        .split("://")
        .next()
        .unwrap_or("")
        .to_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err(PluginError::ExecutionFailed(format!(
            "plugin '{}': only http:// and https:// schemes are allowed, got '{scheme}://'",
            ctx.plugin_id
        )));
    }

    // Validate domain against the declared http_outbound list
    if let Some(ref allowed) = ctx.allowed_domains {
        let domain = request
            .url
            .split("://")
            .nth(1)
            .and_then(|rest| rest.split('/').next())
            .and_then(|host_port| host_port.split(':').next())
            .unwrap_or("");

        if !allowed.iter().any(|d| d == domain) {
            warn!(
                plugin_id = %ctx.plugin_id,
                domain = %domain,
                "HTTP request to undeclared domain"
            );
            return Err(PluginError::RuntimeCapabilityViolation(format!(
                "plugin '{}' attempted HTTP request to undeclared domain '{domain}'",
                ctx.plugin_id
            )));
        }
    }

    // Parse the HTTP method
    let method: reqwest::Method = request.method.parse().map_err(|_| {
        PluginError::ExecutionFailed(format!(
            "plugin '{}': invalid HTTP method '{}'",
            ctx.plugin_id, request.method
        ))
    })?;

    debug!(
        plugin_id = %ctx.plugin_id,
        method = %request.method,
        url = %request.url,
        "executing outbound HTTP request"
    );

    // Build the request
    let mut req_builder = ctx
        .client
        .request(method, &request.url)
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS));

    // Add headers
    for (key, value) in &request.headers {
        req_builder = req_builder.header(key.as_str(), value.as_str());
    }

    // Add body if present
    if let Some(body) = request.body {
        req_builder = req_builder.body(body);
    }

    // Execute the request
    let response = req_builder.send().await.map_err(|e| {
        if e.is_timeout() {
            PluginError::ExecutionFailed(format!(
                "HTTP request timed out for plugin '{}' ({}s limit): {e}",
                ctx.plugin_id, REQUEST_TIMEOUT_SECS
            ))
        } else {
            PluginError::ExecutionFailed(format!(
                "HTTP request failed for plugin '{}': {e}",
                ctx.plugin_id
            ))
        }
    })?;

    let status = response.status().as_u16();

    // Collect response headers
    let headers: HashMap<String, String> = response
        .headers()
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect();

    debug!(
        plugin_id = %ctx.plugin_id,
        status = status,
        "HTTP request completed"
    );

    // Read response body with size limit
    let body_bytes = response.bytes().await.map_err(|e| {
        PluginError::ExecutionFailed(format!(
            "failed to read HTTP response body for plugin '{}': {e}",
            ctx.plugin_id
        ))
    })?;

    if body_bytes.len() > MAX_RESPONSE_BODY_SIZE {
        return Err(PluginError::ExecutionFailed(format!(
            "HTTP response body exceeds maximum size ({} bytes) for plugin '{}'",
            MAX_RESPONSE_BODY_SIZE, ctx.plugin_id
        )));
    }

    let body = String::from_utf8_lossy(&body_bytes).into_owned();

    let http_response = HttpResponse {
        status,
        headers,
        body,
    };

    // Serialize the response back to JSON bytes
    serde_json::to_vec(&http_response).map_err(|e| {
        PluginError::ExecutionFailed(format!(
            "failed to serialize HTTP response for plugin '{}': {e}",
            ctx.plugin_id
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashSet;

    // --- Helper functions ---

    fn make_capabilities(caps: &[Capability]) -> ApprovedCapabilities {
        let set: HashSet<Capability> = caps.iter().copied().collect();
        ApprovedCapabilities::new(set)
    }

    fn make_context(plugin_id: &str, caps: &[Capability]) -> HttpHostContext {
        HttpHostContext {
            plugin_id: plugin_id.to_string(),
            capabilities: make_capabilities(caps),
            client: Client::new(),
            allowed_domains: None,
        }
    }

    fn make_context_with_domains(
        plugin_id: &str,
        caps: &[Capability],
        domains: Vec<String>,
    ) -> HttpHostContext {
        HttpHostContext {
            plugin_id: plugin_id.to_string(),
            capabilities: make_capabilities(caps),
            client: Client::new(),
            allowed_domains: Some(domains),
        }
    }

    fn make_request_bytes(method: &str, url: &str) -> Vec<u8> {
        serde_json::to_vec(&HttpRequest {
            method: method.to_string(),
            url: url.to_string(),
            headers: HashMap::new(),
            body: None,
        })
        .unwrap()
    }

    fn make_request_bytes_with_headers(
        method: &str,
        url: &str,
        headers: HashMap<String, String>,
        body: Option<String>,
    ) -> Vec<u8> {
        serde_json::to_vec(&HttpRequest {
            method: method.to_string(),
            url: url.to_string(),
            headers,
            body,
        })
        .unwrap()
    }

    // --- Tests ---

    #[tokio::test]
    async fn request_without_http_outbound_returns_capability_error() {
        let ctx = make_context("test-plugin", &[Capability::StorageRead]);

        let input = make_request_bytes("GET", "https://example.com/api");
        let result = host_http_request(&ctx, &input).await;

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::RuntimeCapabilityViolation(_)));
        assert!(err.to_string().contains("http:outbound"));
        assert!(err.to_string().contains("test-plugin"));
    }

    #[tokio::test]
    async fn non_http_scheme_is_rejected() {
        let ctx = make_context("test-plugin", &[Capability::HttpOutbound]);

        let input = make_request_bytes("GET", "ftp://example.com/file");
        let result = host_http_request(&ctx, &input).await;

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ExecutionFailed(_)));
        assert!(err.to_string().contains("only http:// and https://"));
    }

    #[tokio::test]
    async fn file_scheme_is_rejected() {
        let ctx = make_context("test-plugin", &[Capability::HttpOutbound]);

        let input = make_request_bytes("GET", "file:///etc/passwd");
        let result = host_http_request(&ctx, &input).await;

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ExecutionFailed(_)));
        assert!(err.to_string().contains("only http:// and https://"));
    }

    #[tokio::test]
    async fn invalid_http_method_returns_error() {
        let ctx = make_context("test-plugin", &[Capability::HttpOutbound]);

        // Method with spaces is invalid per HTTP spec and reqwest rejects it
        let input = make_request_bytes("BAD METHOD", "https://example.com");
        let result = host_http_request(&ctx, &input).await;

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ExecutionFailed(_)));
        assert!(err.to_string().contains("invalid HTTP method"));
    }

    #[tokio::test]
    async fn invalid_json_input_returns_execution_error() {
        let ctx = make_context("test-plugin", &[Capability::HttpOutbound]);

        let result = host_http_request(&ctx, b"not valid json").await;

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ExecutionFailed(_)));
        assert!(err.to_string().contains("deserialize"));
    }

    #[tokio::test]
    async fn request_serialization_roundtrip() {
        let headers = HashMap::from([
            ("Content-Type".to_string(), "application/json".to_string()),
            ("Authorization".to_string(), "Bearer token123".to_string()),
        ]);
        let body = Some(r#"{"key":"value"}"#.to_string());

        let input = make_request_bytes_with_headers(
            "POST",
            "https://api.example.com/data",
            headers.clone(),
            body.clone(),
        );

        let request: HttpRequest = serde_json::from_slice(&input).unwrap();
        assert_eq!(request.method, "POST");
        assert_eq!(request.url, "https://api.example.com/data");
        assert_eq!(request.headers.get("Content-Type").unwrap(), "application/json");
        assert_eq!(request.body, body);
    }

    #[tokio::test]
    async fn response_serialization_roundtrip() {
        let response = HttpResponse {
            status: 200,
            headers: HashMap::from([
                ("content-type".to_string(), "application/json".to_string()),
            ]),
            body: r#"{"result":"ok"}"#.to_string(),
        };

        let bytes = serde_json::to_vec(&response).unwrap();
        let deserialized: HttpResponse = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(deserialized.status, 200);
        assert_eq!(
            deserialized.headers.get("content-type").unwrap(),
            "application/json"
        );
        assert_eq!(deserialized.body, r#"{"result":"ok"}"#);
    }

    #[tokio::test]
    async fn request_to_unreachable_host_returns_execution_error() {
        let ctx = make_context("test-plugin", &[Capability::HttpOutbound]);

        // Use a non-routable address to trigger a connection error
        let input = make_request_bytes("GET", "http://192.0.2.1:1/test");
        let result = host_http_request(&ctx, &input).await;

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ExecutionFailed(_)));
    }

    #[tokio::test]
    async fn undeclared_domain_is_rejected() {
        let ctx = make_context_with_domains(
            "test-plugin",
            &[Capability::HttpOutbound],
            vec!["api.example.com".to_string()],
        );

        let input = make_request_bytes("GET", "https://evil.com/data");
        let result = host_http_request(&ctx, &input).await;

        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::RuntimeCapabilityViolation(_)));
        assert!(err.to_string().contains("undeclared domain"));
        assert!(err.to_string().contains("evil.com"));
    }

    #[tokio::test]
    async fn declared_domain_is_allowed() {
        let ctx = make_context_with_domains(
            "test-plugin",
            &[Capability::HttpOutbound],
            vec!["192.0.2.1".to_string()],
        );

        // Domain is allowed but host is unreachable — we just check it gets past validation
        let input = make_request_bytes("GET", "http://192.0.2.1:1/test");
        let result = host_http_request(&ctx, &input).await;

        // Should fail with ExecutionFailed (network error), NOT CapabilityViolation
        let err = result.unwrap_err();
        assert!(
            matches!(err, PluginError::ExecutionFailed(_)),
            "expected ExecutionFailed, got: {err}"
        );
    }

    #[tokio::test]
    async fn no_domain_allowlist_permits_all_domains() {
        let ctx = make_context("test-plugin", &[Capability::HttpOutbound]);

        // Domain validation should not trigger when allowed_domains is None
        let input = make_request_bytes("GET", "http://192.0.2.1:1/test");
        let result = host_http_request(&ctx, &input).await;

        // Should fail with network error, not capability error
        let err = result.unwrap_err();
        assert!(matches!(err, PluginError::ExecutionFailed(_)));
    }
}
