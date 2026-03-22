//! Storage initialization endpoint.
//!
//! `POST /api/storage/init` accepts a passphrase and creates an encrypted
//! SQLCipher database. This endpoint does not require authentication and
//! can only be called once (returns 409 on subsequent calls).

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Mutex;

use crate::config::Argon2Settings;
use crate::sqlite_storage::SqliteStorage;

/// Maximum number of init attempts per IP within the rate-limit window.
const MAX_INIT_ATTEMPTS: usize = 5;
/// Rate-limit window in seconds.
const INIT_WINDOW_SECS: u64 = 300;

/// State for the storage init endpoint (separate from AppState, no auth).
#[derive(Clone)]
pub struct StorageInitState {
    /// Whether storage has already been initialised.
    pub initialized: Arc<AtomicBool>,
    /// Path to the database file.
    pub db_path: std::path::PathBuf,
    /// Argon2 settings for key derivation.
    pub argon2_settings: Argon2Settings,
    /// Per-IP rate limiter for init attempts.
    pub init_attempts: Arc<Mutex<HashMap<IpAddr, Vec<Instant>>>>,
}

/// Request body for `POST /api/storage/init`.
#[derive(Deserialize)]
pub struct InitStorageRequest {
    /// The master passphrase for encrypting the database.
    pub passphrase: String,
}

/// POST /api/storage/init
///
/// Creates an encrypted SQLCipher database using the provided passphrase.
/// Returns 201 on success, 409 if already initialized, 400 for validation
/// errors, or 500 for internal errors.
pub async fn init_storage(
    State(state): State<StorageInitState>,
    Json(body): Json<InitStorageRequest>,
) -> impl IntoResponse {
    // Rate-limit init attempts (use a fixed key since this is a one-shot endpoint).
    let client_ip = IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED);
    {
        let mut attempts = state.init_attempts.lock().await;
        let entries = attempts.entry(client_ip).or_default();
        let cutoff = Instant::now() - std::time::Duration::from_secs(INIT_WINDOW_SECS);
        entries.retain(|t| *t > cutoff);
        if entries.len() >= MAX_INIT_ATTEMPTS {
            tracing::warn!(ip = %client_ip, "storage init rate limited");
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({
                    "error": {
                        "code": "RATE_LIMITED",
                        "message": "too many init attempts, try again later"
                    }
                })),
            );
        }
        entries.push(Instant::now());
    }

    // Guard: only callable once.
    if state.initialized.swap(true, Ordering::SeqCst) {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "error": {
                    "code": "STORAGE_ALREADY_INITIALIZED",
                    "message": "storage has already been initialized"
                }
            })),
        );
    }

    // Validate passphrase (minimum 12 characters for master encryption key).
    if body.passphrase.len() < 12 {
        state.initialized.store(false, Ordering::SeqCst);
        tracing::warn!(ip = %client_ip, "storage init rejected: passphrase too short");
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "code": "INVALID_PASSPHRASE",
                    "message": "passphrase must be at least 12 characters"
                }
            })),
        );
    }

    // Create the encrypted database.
    match SqliteStorage::open_encrypted(&state.db_path, &body.passphrase, &state.argon2_settings) {
        Ok(_storage) => {
            tracing::info!(path = %state.db_path.display(), "encrypted storage initialized");
            (
                StatusCode::CREATED,
                Json(json!({
                    "status": "initialized",
                    "path": state.db_path.display().to_string()
                })),
            )
        }
        Err(e) => {
            state.initialized.store(false, Ordering::SeqCst);
            tracing::error!(error = %e, "failed to initialize encrypted storage");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "STORAGE_INIT_FAILED",
                        "message": format!("failed to create encrypted database: {e}")
                    }
                })),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn test_router(db_path: std::path::PathBuf) -> axum::Router {
        let state = StorageInitState {
            initialized: Arc::new(AtomicBool::new(false)),
            db_path,
            argon2_settings: Argon2Settings {
                memory_mb: 1,
                iterations: 1,
                parallelism: 1,
            },
            init_attempts: Arc::new(Mutex::new(HashMap::new())),
        };
        axum::Router::new()
            .route("/api/storage/init", axum::routing::post(init_storage))
            .with_state(state)
    }

    #[tokio::test]
    async fn init_storage_returns_201() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let app = test_router(tmp.path().to_path_buf());

        let req = Request::builder()
            .method("POST")
            .uri("/api/storage/init")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"passphrase":"my-secret-passphrase-12"}"#))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn init_storage_rejects_double_init() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let state = StorageInitState {
            initialized: Arc::new(AtomicBool::new(false)),
            db_path: tmp.path().to_path_buf(),
            argon2_settings: Argon2Settings {
                memory_mb: 1,
                iterations: 1,
                parallelism: 1,
            },
            init_attempts: Arc::new(Mutex::new(HashMap::new())),
        };
        let app = axum::Router::new()
            .route("/api/storage/init", axum::routing::post(init_storage))
            .with_state(state);

        let req1 = Request::builder()
            .method("POST")
            .uri("/api/storage/init")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"passphrase":"my-secret-passphrase-12"}"#))
            .unwrap();
        let resp1 = app.clone().oneshot(req1).await.unwrap();
        assert_eq!(resp1.status(), StatusCode::CREATED);

        let req2 = Request::builder()
            .method("POST")
            .uri("/api/storage/init")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"passphrase":"my-secret-passphrase-12"}"#))
            .unwrap();
        let resp2 = app.oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn init_storage_no_auth_required() {
        // This test verifies the endpoint works without any Authorization header.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let app = test_router(tmp.path().to_path_buf());

        let req = Request::builder()
            .method("POST")
            .uri("/api/storage/init")
            .header("content-type", "application/json")
            // No Authorization header
            .body(Body::from(r#"{"passphrase":"my-secret-passphrase-12"}"#))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn init_storage_rejects_short_passphrase() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let app = test_router(tmp.path().to_path_buf());

        let req = Request::builder()
            .method("POST")
            .uri("/api/storage/init")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"passphrase":"short"}"#))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
