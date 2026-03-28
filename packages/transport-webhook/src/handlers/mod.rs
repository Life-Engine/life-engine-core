//! Webhook request handlers.
//!
//! Implements inbound webhook receiving with HMAC signature verification,
//! timestamp-based replay protection, idempotency key deduplication,
//! and content-type validation.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::{
    CONTENT_TYPE_JSON, HEADER_IDEMPOTENCY_KEY, HEADER_SIGNATURE, HEADER_TIMESTAMP,
    MAX_TIMESTAMP_AGE_SECS,
};

type HmacSha256 = Hmac<Sha256>;

/// Shared state for the webhook transport.
#[derive(Debug)]
pub struct WebhookState {
    /// Shared secret for HMAC-SHA256 signature verification.
    /// If `None`, signature verification is skipped.
    pub secret: Option<String>,
    /// Set of seen idempotency keys for deduplication.
    pub seen_keys: Mutex<HashSet<String>>,
    /// Maximum number of idempotency keys to retain before eviction.
    pub max_seen_keys: usize,
}

impl WebhookState {
    /// Check if an idempotency key has been seen, and mark it as seen.
    /// Returns `true` if the key was already present (duplicate).
    fn check_and_record_key(&self, key: &str) -> bool {
        let mut seen = self.seen_keys.lock().unwrap_or_else(|e| e.into_inner());
        if seen.contains(key) {
            return true;
        }
        // Simple eviction: clear all when max is reached
        if seen.len() >= self.max_seen_keys {
            seen.clear();
        }
        seen.insert(key.to_string());
        false
    }
}

/// Inbound webhook handler.
///
/// Validates the request in this order:
/// 1. Content-Type must be `application/json`
/// 2. HMAC-SHA256 signature verification (if secret is configured)
/// 3. Timestamp replay protection
/// 4. Idempotency key deduplication
pub async fn handle_webhook(
    State(state): State<Arc<WebhookState>>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    // 1. Content-Type validation
    if let Err(resp) = validate_content_type(&headers) {
        return resp;
    }

    // 2. HMAC signature verification
    if let Some(secret) = &state.secret {
        if let Err(resp) = verify_signature(&headers, &body, secret) {
            return resp;
        }
    }

    // 3. Timestamp replay protection
    if let Err(resp) = validate_timestamp(&headers) {
        return resp;
    }

    // 4. Idempotency key deduplication
    if let Some(key) = headers
        .get(HEADER_IDEMPOTENCY_KEY)
        .and_then(|v| v.to_str().ok())
    {
        if state.check_and_record_key(key) {
            return (StatusCode::OK, "Duplicate delivery, already processed").into_response();
        }
    }

    tracing::info!(body_len = body.len(), "Webhook received and validated");
    (StatusCode::OK, "Webhook accepted").into_response()
}

/// Validate that the Content-Type header is application/json.
fn validate_content_type(headers: &HeaderMap) -> Result<(), axum::response::Response> {
    match headers.get("content-type").and_then(|v| v.to_str().ok()) {
        Some(ct) if ct.starts_with(CONTENT_TYPE_JSON) => Ok(()),
        Some(ct) => Err((
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            format!("Expected Content-Type: {CONTENT_TYPE_JSON}, got: {ct}"),
        )
            .into_response()),
        None => Err((
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "Missing Content-Type header",
        )
            .into_response()),
    }
}

/// Verify the HMAC-SHA256 signature of the request body.
fn verify_signature(
    headers: &HeaderMap,
    body: &str,
    secret: &str,
) -> Result<(), axum::response::Response> {
    let signature = headers
        .get(HEADER_SIGNATURE)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                format!("Missing {HEADER_SIGNATURE} header"),
            )
                .into_response()
        })?;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC can accept keys of any size");
    mac.update(body.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());

    // Strip optional "sha256=" prefix
    let provided = signature.strip_prefix("sha256=").unwrap_or(signature);

    if !constant_time_eq(provided.as_bytes(), expected.as_bytes()) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid webhook signature").into_response());
    }

    Ok(())
}

/// Validate the webhook timestamp for replay protection.
fn validate_timestamp(headers: &HeaderMap) -> Result<(), axum::response::Response> {
    let ts_str = match headers.get(HEADER_TIMESTAMP).and_then(|v| v.to_str().ok()) {
        Some(ts) => ts,
        None => return Ok(()), // No timestamp header = no replay protection
    };

    let ts: i64 = ts_str.parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid {HEADER_TIMESTAMP} value"),
        )
            .into_response()
    })?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let age = (now - ts).abs();
    if age > MAX_TIMESTAMP_AGE_SECS {
        return Err((
            StatusCode::FORBIDDEN,
            format!("Webhook timestamp too old: {age}s exceeds {MAX_TIMESTAMP_AGE_SECS}s limit"),
        )
            .into_response());
    }

    Ok(())
}

/// Constant-time byte comparison to prevent timing attacks on HMAC verification.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
