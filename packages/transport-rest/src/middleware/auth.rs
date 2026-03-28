//! Authentication middleware (Requirement 13).
//!
//! Validates OIDC/API-key tokens at the transport boundary.
//! On success, inserts `Extension<Identity>` into the request.
//! On failure, returns 401 before reaching any handler.
//! Public routes bypass validation entirely.

use std::collections::HashSet;
use std::sync::Arc;

use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use life_engine_auth::{AuthError, AuthIdentity, AuthProvider, RateLimiter};
use serde::{Deserialize, Serialize};

/// Authenticated identity inserted as an Axum extension (Requirement 13.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    pub user_id: String,
    pub provider: String,
    pub scopes: Vec<String>,
}

impl From<AuthIdentity> for Identity {
    fn from(auth: AuthIdentity) -> Self {
        Self {
            user_id: auth.user_id,
            provider: auth.provider,
            scopes: auth.scopes,
        }
    }
}

/// Shared state for the auth middleware.
#[derive(Clone)]
pub struct AuthState {
    pub provider: Arc<dyn AuthProvider>,
    pub rate_limiter: Arc<RateLimiter>,
    pub public_routes: Arc<HashSet<String>>,
}

/// Auth middleware function for use with `axum::middleware::from_fn_with_state`.
///
/// Checks if the matched route is public. If not, validates the
/// `Authorization` header and inserts `Extension<Identity>` on success,
/// or returns 401 on failure.
pub async fn auth_middleware(
    axum::extract::State(state): axum::extract::State<AuthState>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();
    let method = request.method().to_string();
    let route_key = format!("{method} {path}");

    // Public route bypass (Requirement 13.4).
    if state.public_routes.contains(&route_key) {
        return next.run(request).await;
    }

    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let client_ip = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let result = life_engine_auth::validate_request(
        state.provider.as_ref(),
        auth_header.as_deref(),
        &state.rate_limiter,
        &client_ip,
    )
    .await;

    match result {
        Ok(auth_identity) => {
            let identity = Identity::from(auth_identity);
            let mut request = request;
            request.extensions_mut().insert(identity);
            next.run(request).await
        }
        Err(err) => auth_error_response(&err),
    }
}

/// Map an `AuthError` to an HTTP response. Never exposes internal details.
fn auth_error_response(err: &AuthError) -> Response {
    let (status, code, message) = match err {
        AuthError::TokenMissing => (
            StatusCode::UNAUTHORIZED,
            "AUTH_001",
            "Authorization header required",
        ),
        AuthError::TokenExpired => (
            StatusCode::UNAUTHORIZED,
            "AUTH_002",
            "Token has expired",
        ),
        AuthError::TokenInvalid(_) => (
            StatusCode::UNAUTHORIZED,
            "AUTH_003",
            "Invalid token",
        ),
        AuthError::ProviderUnreachable(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "AUTH_004",
            "Authentication service unavailable",
        ),
        AuthError::RateLimited { retry_after } => {
            let body = serde_json::json!({
                "error": {
                    "code": "AUTH_006",
                    "message": format!("Too many failed attempts, retry after {retry_after}s")
                }
            });
            return (StatusCode::TOO_MANY_REQUESTS, axum::Json(body)).into_response();
        }
        AuthError::KeyRevoked => (
            StatusCode::UNAUTHORIZED,
            "AUTH_007",
            "API key has been revoked",
        ),
        AuthError::KeyInvalid => (
            StatusCode::UNAUTHORIZED,
            "AUTH_008",
            "Invalid API key",
        ),
        AuthError::ConfigInvalid(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "AUTH_005",
            "Authentication configuration error",
        ),
    };

    let body = serde_json::json!({
        "error": {
            "code": code,
            "message": message
        }
    });

    (status, axum::Json(body)).into_response()
}
