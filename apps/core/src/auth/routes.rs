//! Token management route handlers.
//!
//! Provides endpoints for generating, revoking, and listing auth tokens,
//! and WebAuthn passkey ceremony routes.

use crate::auth::oidc::{OidcConfig, OidcLoginRequest, OidcRefreshRequest, OidcRegisterRequest};
use crate::auth::types::{AuthError, AuthIdentity, TokenRequest};
use crate::auth::webauthn_provider::WebAuthnProvider;
use crate::auth::AuthProvider;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use webauthn_rs::prelude::{
    CreationChallengeResponse, PublicKeyCredential, RegisterPublicKeyCredential,
    RequestChallengeResponse,
};

/// Shared state for auth route handlers.
#[derive(Clone)]
pub struct AuthRouteState {
    /// The active auth provider.
    pub auth_provider: Arc<dyn AuthProvider>,
    /// Optional OIDC configuration for OIDC-specific routes.
    pub oidc_config: Option<OidcConfig>,
    /// HTTP client for proxying OIDC requests.
    pub http_client: reqwest::Client,
    /// Optional WebAuthn provider for passkey ceremony routes.
    pub webauthn_provider: Option<Arc<WebAuthnProvider>>,
}

// ── WebAuthn request/response types ────────────────────────────────────

/// Request body for `POST /api/auth/webauthn/register/start`.
#[derive(Debug, Deserialize)]
struct RegisterStartRequest {
    /// A user-friendly label for this passkey (e.g. "MacBook Pro Touch ID").
    label: String,
}

/// Response body for `POST /api/auth/webauthn/register/start`.
#[derive(Debug, Serialize)]
struct RegisterStartResponse {
    /// Opaque challenge ID to pass back when finishing registration.
    challenge_id: String,
    /// The WebAuthn creation options to pass to `navigator.credentials.create()`.
    options: CreationChallengeResponse,
}

/// Request body for `POST /api/auth/webauthn/register/finish`.
#[derive(Debug, Deserialize)]
struct RegisterFinishRequest {
    /// The challenge ID returned by the start endpoint.
    challenge_id: String,
    /// The browser's `PublicKeyCredential` response from `navigator.credentials.create()`.
    response: RegisterPublicKeyCredential,
    /// A user-friendly label for this passkey (e.g. "MacBook Pro Touch ID").
    /// If omitted, defaults to "Passkey".
    label: Option<String>,
}

/// Request body for `POST /api/auth/webauthn/authenticate/start`.
#[derive(Debug, Deserialize)]
pub(crate) struct AuthenticateStartRequest {
    /// The user ID to authenticate.
    user_id: String,
}

/// Response body for `POST /api/auth/webauthn/authenticate/start`.
#[derive(Debug, Serialize)]
struct AuthenticateStartResponse {
    /// Opaque challenge ID to pass back when finishing authentication.
    challenge_id: String,
    /// The WebAuthn request options to pass to `navigator.credentials.get()`.
    options: RequestChallengeResponse,
}

/// Request body for `POST /api/auth/webauthn/authenticate/finish`.
#[derive(Debug, Deserialize)]
pub(crate) struct AuthenticateFinishRequest {
    /// The challenge ID returned by the start endpoint.
    challenge_id: String,
    /// The browser's `PublicKeyCredential` response from `navigator.credentials.get()`.
    response: PublicKeyCredential,
}

/// A passkey entry for display in the management UI.
#[derive(Debug, Serialize)]
struct PasskeyInfo {
    /// The stored passkey entry ID.
    id: String,
    /// User-friendly label.
    label: String,
    /// When this passkey was registered (ISO 8601).
    created_at: String,
    /// When this passkey was last used (ISO 8601), if ever.
    last_used_at: Option<String>,
}

/// POST /api/auth/token — Generate a new auth token.
///
/// Requires the master passphrase. On first call, sets the passphrase.
/// Returns the raw token exactly once.
pub async fn generate_token(
    State(state): State<AuthRouteState>,
    Json(body): Json<TokenRequest>,
) -> impl IntoResponse {
    match state.auth_provider.generate_token(&body).await {
        Ok(resp) => (StatusCode::CREATED, Json(json!(resp))).into_response(),
        Err(AuthError::InvalidCredentials) => {
            tracing::warn!("token generation failed: invalid passphrase");
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "AUTH_INVALID_CREDENTIALS"})),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "token generation error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "AUTH_INTERNAL_ERROR"})),
            )
                .into_response()
        }
    }
}

/// DELETE /api/auth/token/:id — Revoke a token by ID.
///
/// Requires a valid auth token (enforced by middleware).
pub async fn revoke_token(
    State(state): State<AuthRouteState>,
    Path(token_id): Path<String>,
    request: axum::extract::Request,
) -> impl IntoResponse {
    // Verify the caller is authenticated (identity in extensions).
    if request.extensions().get::<AuthIdentity>().is_none() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "AUTH_MISSING_TOKEN"})),
        )
            .into_response();
    }

    match state.auth_provider.revoke_token(&token_id).await {
        Ok(()) => (StatusCode::OK, Json(json!({"status": "revoked"}))).into_response(),
        Err(AuthError::TokenNotFound) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "AUTH_TOKEN_NOT_FOUND"})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "token revocation error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "AUTH_INTERNAL_ERROR"})),
            )
                .into_response()
        }
    }
}

/// GET /api/auth/tokens — List all active tokens.
///
/// Requires a valid auth token (enforced by middleware).
pub async fn list_tokens(
    State(state): State<AuthRouteState>,
    request: axum::extract::Request,
) -> impl IntoResponse {
    // Verify the caller is authenticated (identity in extensions).
    if request.extensions().get::<AuthIdentity>().is_none() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "AUTH_MISSING_TOKEN"})),
        )
            .into_response();
    }

    match state.auth_provider.list_tokens().await {
        Ok(tokens) => (StatusCode::OK, Json(json!({"tokens": tokens}))).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "token list error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "AUTH_INTERNAL_ERROR"})),
            )
                .into_response()
        }
    }
}

/// POST /api/auth/login — OIDC login via username/password.
///
/// Forwards credentials to the Pocket ID token endpoint using
/// the Resource Owner Password Credentials grant.
pub async fn oidc_login(
    State(state): State<AuthRouteState>,
    Json(body): Json<OidcLoginRequest>,
) -> axum::response::Response {
    let oidc = match &state.oidc_config {
        Some(c) => c,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "OIDC_NOT_CONFIGURED"})),
            )
                .into_response();
        }
    };

    let mut params: Vec<(&str, String)> = vec![
        ("grant_type", "password".to_string()),
        ("username", body.username),
        ("password", body.password),
        ("client_id", oidc.client_id.clone()),
    ];
    if let Some(ref secret) = oidc.client_secret {
        params.push(("client_secret", secret.clone()));
    }
    if let Some(ref aud) = oidc.audience {
        params.push(("scope", format!("openid profile email audience:{aud}")));
    } else {
        params.push(("scope", "openid profile email".into()));
    }

    let token_url = oidc.token_endpoint();
    let result = state.http_client.post(&token_url).form(&params).send().await;

    match result {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<serde_json::Value>().await {
                Ok(token_resp) => (StatusCode::OK, Json(token_resp)).into_response(),
                Err(e) => {
                    tracing::error!(error = %e, "failed to parse OIDC token response");
                    (
                        StatusCode::BAD_GATEWAY,
                        Json(json!({"error": "OIDC_TOKEN_PARSE_ERROR"})),
                    )
                        .into_response()
                }
            }
        }
        Ok(resp) => {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            tracing::warn!(status = %status, body = %body_text, "OIDC login failed");
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "OIDC_LOGIN_FAILED"})),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "OIDC token endpoint unreachable");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "OIDC_PROVIDER_UNREACHABLE"})),
            )
                .into_response()
        }
    }
}

/// POST /api/auth/refresh — Refresh an OIDC token.
///
/// Forwards the refresh token to the Pocket ID token endpoint.
pub async fn oidc_refresh(
    State(state): State<AuthRouteState>,
    Json(body): Json<OidcRefreshRequest>,
) -> axum::response::Response {
    let oidc = match &state.oidc_config {
        Some(c) => c,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "OIDC_NOT_CONFIGURED"})),
            )
                .into_response();
        }
    };

    let mut params: Vec<(&str, String)> = vec![
        ("grant_type", "refresh_token".to_string()),
        ("refresh_token", body.refresh_token),
        ("client_id", oidc.client_id.clone()),
    ];
    if let Some(ref secret) = oidc.client_secret {
        params.push(("client_secret", secret.clone()));
    }

    let token_url = oidc.token_endpoint();
    let result = state.http_client.post(&token_url).form(&params).send().await;

    match result {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<serde_json::Value>().await {
                Ok(token_resp) => (StatusCode::OK, Json(token_resp)).into_response(),
                Err(e) => {
                    tracing::error!(error = %e, "failed to parse OIDC refresh response");
                    (
                        StatusCode::BAD_GATEWAY,
                        Json(json!({"error": "OIDC_TOKEN_PARSE_ERROR"})),
                    )
                        .into_response()
                }
            }
        }
        Ok(resp) => {
            let status = resp.status();
            tracing::warn!(status = %status, "OIDC refresh failed");
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "OIDC_REFRESH_FAILED"})),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "OIDC token endpoint unreachable");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "OIDC_PROVIDER_UNREACHABLE"})),
            )
                .into_response()
        }
    }
}

/// GET /api/auth/userinfo — Proxy to the OIDC userinfo endpoint.
///
/// Forwards the caller's Bearer token to the identity provider's
/// userinfo endpoint and returns the response.
pub async fn oidc_userinfo(
    State(state): State<AuthRouteState>,
    request: axum::extract::Request,
) -> axum::response::Response {
    let oidc = match &state.oidc_config {
        Some(c) => c,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "OIDC_NOT_CONFIGURED"})),
            )
                .into_response();
        }
    };

    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let userinfo_url = oidc.userinfo_endpoint();
    let result = state
        .http_client
        .get(&userinfo_url)
        .header("Authorization", auth_header)
        .send()
        .await;

    match result {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<serde_json::Value>().await {
                Ok(info) => (StatusCode::OK, Json(info)).into_response(),
                Err(e) => {
                    tracing::error!(error = %e, "failed to parse userinfo response");
                    (
                        StatusCode::BAD_GATEWAY,
                        Json(json!({"error": "OIDC_USERINFO_PARSE_ERROR"})),
                    )
                        .into_response()
                }
            }
        }
        Ok(resp) => {
            let status = resp.status();
            tracing::warn!(status = %status, "OIDC userinfo failed");
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "OIDC_USERINFO_FAILED"})),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "OIDC userinfo endpoint unreachable");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "OIDC_PROVIDER_UNREACHABLE"})),
            )
                .into_response()
        }
    }
}

/// GET /api/auth/.well-known/openid-configuration — OIDC discovery document.
///
/// Proxies the identity provider's OIDC discovery document.
pub async fn oidc_discovery(
    State(state): State<AuthRouteState>,
) -> axum::response::Response {
    let oidc = match &state.oidc_config {
        Some(c) => c,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "OIDC_NOT_CONFIGURED"})),
            )
                .into_response();
        }
    };

    let discovery_url = oidc.discovery_endpoint();
    let result = state.http_client.get(&discovery_url).send().await;

    match result {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<serde_json::Value>().await {
                Ok(doc) => (StatusCode::OK, Json(doc)).into_response(),
                Err(e) => {
                    tracing::error!(error = %e, "failed to parse OIDC discovery document");
                    (
                        StatusCode::BAD_GATEWAY,
                        Json(json!({"error": "OIDC_DISCOVERY_PARSE_ERROR"})),
                    )
                        .into_response()
                }
            }
        }
        Ok(resp) => {
            let status = resp.status();
            tracing::warn!(status = %status, "OIDC discovery failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "OIDC_DISCOVERY_FAILED"})),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "OIDC discovery endpoint unreachable");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "OIDC_PROVIDER_UNREACHABLE"})),
            )
                .into_response()
        }
    }
}

/// POST /api/auth/register — Register a new user via OIDC.
///
/// Proxies the registration request to the Pocket ID identity provider.
/// This endpoint is unauthenticated (no Bearer token required).
pub async fn oidc_register(
    State(state): State<AuthRouteState>,
    Json(body): Json<OidcRegisterRequest>,
) -> axum::response::Response {
    let oidc = match &state.oidc_config {
        Some(c) => c,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "OIDC_NOT_CONFIGURED"})),
            )
                .into_response();
        }
    };

    let mut params: Vec<(&str, String)> = vec![
        ("username", body.username),
        ("password", body.password),
    ];
    if let Some(display_name) = body.display_name {
        params.push(("display_name", display_name));
    }

    let register_url = oidc.registration_endpoint();
    let result = state.http_client.post(&register_url).form(&params).send().await;

    match result {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<serde_json::Value>().await {
                Ok(register_resp) => (StatusCode::CREATED, Json(register_resp)).into_response(),
                Err(e) => {
                    tracing::error!(error = %e, "failed to parse OIDC registration response");
                    (
                        StatusCode::BAD_GATEWAY,
                        Json(json!({"error": "OIDC_REGISTER_PARSE_ERROR"})),
                    )
                        .into_response()
                }
            }
        }
        Ok(resp) => {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            tracing::warn!(status = %status, body = %body_text, "OIDC registration failed");
            (
                StatusCode::CONFLICT,
                Json(json!({"error": "OIDC_REGISTRATION_FAILED"})),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "OIDC registration endpoint unreachable");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "OIDC_PROVIDER_UNREACHABLE"})),
            )
                .into_response()
        }
    }
}

// ── WebAuthn route handlers ────────────────────────────────────────────

/// Helper to extract the WebAuthn provider from state, returning a 404
/// JSON error if it is not configured.
#[allow(clippy::result_large_err)]
fn require_webauthn(
    state: &AuthRouteState,
) -> Result<&Arc<WebAuthnProvider>, axum::response::Response> {
    state.webauthn_provider.as_ref().ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "WEBAUTHN_NOT_CONFIGURED"})),
        )
            .into_response()
    })
}

/// POST /api/auth/webauthn/register/start — Begin passkey registration.
///
/// Requires a valid auth token. The authenticated user's identity is used
/// as the WebAuthn user ID.
pub async fn webauthn_register_start(
    State(state): State<AuthRouteState>,
    request: axum::extract::Request,
) -> axum::response::Response {
    let identity = match request.extensions().get::<AuthIdentity>() {
        Some(id) => id.clone(),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "AUTH_MISSING_TOKEN"})),
            )
                .into_response();
        }
    };

    let wn = match require_webauthn(&state) {
        Ok(wn) => wn,
        Err(resp) => return resp,
    };

    // Parse the label from the body.
    let body_bytes = match axum::body::to_bytes(request.into_body(), 1024 * 16).await {
        Ok(b) => b,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "INVALID_REQUEST_BODY"})),
            )
                .into_response();
        }
    };
    let req: RegisterStartRequest = match serde_json::from_slice(&body_bytes) {
        Ok(r) => r,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "INVALID_REQUEST_BODY"})),
            )
                .into_response();
        }
    };

    // Use the stable OIDC user_id (sub claim) so passkeys work across sessions.
    // Fall back to token_id only if no OIDC user_id is available.
    let user_id = identity
        .user_id
        .as_deref()
        .unwrap_or(&identity.token_id);
    let user_name = if req.label.is_empty() {
        user_id
    } else {
        &req.label
    };

    match wn.start_registration(user_id, user_name).await {
        Ok((challenge_id, options)) => {
            let resp = RegisterStartResponse {
                challenge_id,
                options,
            };
            (StatusCode::OK, Json(json!(resp))).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "webauthn register start failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "WEBAUTHN_INTERNAL_ERROR"})),
            )
                .into_response()
        }
    }
}

/// POST /api/auth/webauthn/register/finish — Complete passkey registration.
///
/// Requires a valid auth token.
pub async fn webauthn_register_finish(
    State(state): State<AuthRouteState>,
    request: axum::extract::Request,
) -> axum::response::Response {
    let identity = match request.extensions().get::<AuthIdentity>() {
        Some(id) => id.clone(),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "AUTH_MISSING_TOKEN"})),
            )
                .into_response();
        }
    };

    let wn = match require_webauthn(&state) {
        Ok(wn) => wn,
        Err(resp) => return resp,
    };

    let body_bytes = match axum::body::to_bytes(request.into_body(), 1024 * 64).await {
        Ok(b) => b,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "INVALID_REQUEST_BODY"})),
            )
                .into_response();
        }
    };
    let req: RegisterFinishRequest = match serde_json::from_slice(&body_bytes) {
        Ok(r) => r,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "INVALID_REQUEST_BODY"})),
            )
                .into_response();
        }
    };

    // Use the stable OIDC user_id (sub claim) so passkeys work across sessions.
    let user_id = identity
        .user_id
        .as_deref()
        .unwrap_or(&identity.token_id);
    // Preserve the user-provided label; default to "Passkey" if not supplied.
    let label = req
        .label
        .as_deref()
        .filter(|l| !l.is_empty())
        .unwrap_or("Passkey");

    match wn
        .finish_registration(&req.challenge_id, user_id, label, &req.response)
        .await
    {
        Ok(stored) => {
            let last_used = stored
                .last_used_at
                .map(|t| t.to_rfc3339());
            let info = PasskeyInfo {
                id: stored.id.to_string(),
                label: stored.label,
                created_at: stored.created_at.to_rfc3339(),
                last_used_at: last_used,
            };
            (StatusCode::CREATED, Json(json!(info))).into_response()
        }
        Err(e) => {
            let msg: String = e.to_string();
            tracing::error!(error = %msg, "webauthn register finish failed");

            if msg.contains("expired") {
                (
                    StatusCode::GONE,
                    Json(json!({"error": "WEBAUTHN_CHALLENGE_EXPIRED"})),
                )
                    .into_response()
            } else if msg.contains("already registered") {
                (
                    StatusCode::CONFLICT,
                    Json(json!({"error": "WEBAUTHN_CREDENTIAL_EXISTS"})),
                )
                    .into_response()
            } else {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "WEBAUTHN_INVALID_RESPONSE"})),
                )
                    .into_response()
            }
        }
    }
}

/// POST /api/auth/webauthn/authenticate/start — Begin passkey authentication.
///
/// This is a public endpoint (no auth required).
pub async fn webauthn_authenticate_start(
    State(state): State<AuthRouteState>,
    Json(body): Json<AuthenticateStartRequest>,
) -> axum::response::Response {
    let wn = match require_webauthn(&state) {
        Ok(wn) => wn,
        Err(resp) => return resp,
    };

    match wn.start_authentication(&body.user_id).await {
        Ok((challenge_id, options)) => {
            let resp = AuthenticateStartResponse {
                challenge_id,
                options,
            };
            (StatusCode::OK, Json(json!(resp))).into_response()
        }
        Err(e) => {
            let msg: String = e.to_string();
            tracing::error!(error = %msg, "webauthn authenticate start failed");

            if msg.contains("no passkeys registered") {
                (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "WEBAUTHN_NO_CREDENTIALS"})),
                )
                    .into_response()
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "WEBAUTHN_INTERNAL_ERROR"})),
                )
                    .into_response()
            }
        }
    }
}

/// POST /api/auth/webauthn/authenticate/finish — Complete passkey authentication.
///
/// This is a public endpoint (no auth required). Returns a session token on success.
pub async fn webauthn_authenticate_finish(
    State(state): State<AuthRouteState>,
    Json(body): Json<AuthenticateFinishRequest>,
) -> axum::response::Response {
    let wn = match require_webauthn(&state) {
        Ok(wn) => wn,
        Err(resp) => return resp,
    };

    match wn.finish_authentication(&body.challenge_id, &body.response).await {
        Ok(token_resp) => (StatusCode::OK, Json(json!(token_resp))).into_response(),
        Err(e) => {
            let msg: String = e.to_string();
            tracing::error!(error = %msg, "webauthn authenticate finish failed");

            if msg.contains("expired") {
                (
                    StatusCode::GONE,
                    Json(json!({"error": "WEBAUTHN_CHALLENGE_EXPIRED"})),
                )
                    .into_response()
            } else {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"error": "WEBAUTHN_INVALID_RESPONSE"})),
                )
                    .into_response()
            }
        }
    }
}

/// GET /api/auth/webauthn/passkeys — List passkeys for the authenticated user.
///
/// Requires a valid auth token.
pub async fn webauthn_list_passkeys(
    State(state): State<AuthRouteState>,
    request: axum::extract::Request,
) -> axum::response::Response {
    let identity = match request.extensions().get::<AuthIdentity>() {
        Some(id) => id.clone(),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "AUTH_MISSING_TOKEN"})),
            )
                .into_response();
        }
    };

    let wn = match require_webauthn(&state) {
        Ok(wn) => wn,
        Err(resp) => return resp,
    };

    // Use the stable OIDC user_id so passkeys are consistent across sessions.
    let user_id = identity
        .user_id
        .as_deref()
        .unwrap_or(&identity.token_id);

    match wn.list_passkeys(user_id).await {
        Ok(passkeys) => {
            let infos: Vec<PasskeyInfo> = passkeys
                .into_iter()
                .map(|pk| {
                    let last_used = pk.last_used_at.map(|t| t.to_rfc3339());
                    PasskeyInfo {
                        id: pk.id.to_string(),
                        label: pk.label,
                        created_at: pk.created_at.to_rfc3339(),
                        last_used_at: last_used,
                    }
                })
                .collect();
            (StatusCode::OK, Json(json!({"passkeys": infos}))).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "webauthn list passkeys failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "WEBAUTHN_INTERNAL_ERROR"})),
            )
                .into_response()
        }
    }
}

/// DELETE /api/auth/webauthn/passkeys/:id — Remove a passkey.
///
/// Requires a valid auth token.
pub async fn webauthn_delete_passkey(
    State(state): State<AuthRouteState>,
    Path(passkey_id): Path<String>,
    request: axum::extract::Request,
) -> axum::response::Response {
    let identity = match request.extensions().get::<AuthIdentity>() {
        Some(id) => id.clone(),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "AUTH_MISSING_TOKEN"})),
            )
                .into_response();
        }
    };

    let wn = match require_webauthn(&state) {
        Ok(wn) => wn,
        Err(resp) => return resp,
    };

    let pk_uuid = match uuid::Uuid::parse_str(&passkey_id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "INVALID_PASSKEY_ID"})),
            )
                .into_response();
        }
    };

    // Verify the passkey belongs to the authenticated user (prevents IDOR).
    let user_id = identity
        .user_id
        .as_deref()
        .unwrap_or(&identity.token_id);
    match wn.list_passkeys(user_id).await {
        Ok(passkeys) => {
            if !passkeys.iter().any(|pk| pk.id == pk_uuid) {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "WEBAUTHN_NO_CREDENTIALS"})),
                )
                    .into_response();
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "webauthn list passkeys for ownership check failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "WEBAUTHN_INTERNAL_ERROR"})),
            )
                .into_response();
        }
    }

    match wn.remove_passkey(&pk_uuid).await {
        Ok(()) => (StatusCode::OK, Json(json!({"status": "deleted"}))).into_response(),
        Err(e) => {
            let msg: String = e.to_string();
            if msg.contains("not found") {
                (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "WEBAUTHN_NO_CREDENTIALS"})),
                )
                    .into_response()
            } else {
                tracing::error!(error = %msg, "webauthn delete passkey failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "WEBAUTHN_INTERNAL_ERROR"})),
                )
                    .into_response()
            }
        }
    }
}

/// Build an axum Router with all auth routes (local + OIDC + WebAuthn).
pub fn auth_router(
    auth_provider: Arc<dyn AuthProvider>,
    oidc_config: Option<OidcConfig>,
    webauthn_provider: Option<Arc<WebAuthnProvider>>,
) -> axum::Router {
    let state = AuthRouteState {
        auth_provider,
        oidc_config,
        http_client: reqwest::Client::new(),
        webauthn_provider,
    };

    axum::Router::new()
        .route("/api/auth/token", axum::routing::post(generate_token))
        .route(
            "/api/auth/token/{id}",
            axum::routing::delete(revoke_token),
        )
        .route("/api/auth/tokens", axum::routing::get(list_tokens))
        .route("/api/auth/login", axum::routing::post(oidc_login))
        .route("/api/auth/register", axum::routing::post(oidc_register))
        .route("/api/auth/refresh", axum::routing::post(oidc_refresh))
        .route("/api/auth/userinfo", axum::routing::get(oidc_userinfo))
        .route(
            "/api/auth/.well-known/openid-configuration",
            axum::routing::get(oidc_discovery),
        )
        // WebAuthn ceremony routes.
        .route(
            "/api/auth/webauthn/register/start",
            axum::routing::post(webauthn_register_start),
        )
        .route(
            "/api/auth/webauthn/register/finish",
            axum::routing::post(webauthn_register_finish),
        )
        .route(
            "/api/auth/webauthn/authenticate/start",
            axum::routing::post(webauthn_authenticate_start),
        )
        .route(
            "/api/auth/webauthn/authenticate/finish",
            axum::routing::post(webauthn_authenticate_finish),
        )
        .route(
            "/api/auth/webauthn/passkeys",
            axum::routing::get(webauthn_list_passkeys),
        )
        .route(
            "/api/auth/webauthn/passkeys/{id}",
            axum::routing::delete(webauthn_delete_passkey),
        )
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::local_token::LocalTokenProvider;
    use crate::auth::middleware::auth_middleware;
    use crate::auth::AuthProvider;
    use crate::test_helpers::create_auth_state;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn setup_auth_router() -> (axum::Router, Arc<LocalTokenProvider>) {
        let (auth_mw_state, provider) = create_auth_state();

        let router = auth_router(provider.clone(), None, None).layer(
            axum::middleware::from_fn_with_state(auth_mw_state, auth_middleware),
        );

        (router, provider)
    }

    #[tokio::test]
    async fn generate_token_endpoint() {
        let (app, _provider) = setup_auth_router();
        let body = serde_json::to_string(&json!({
            "passphrase": "test-secret",
            "expires_in_days": 7
        }))
        .unwrap();

        let request = Request::builder()
            .method("POST")
            .uri("/api/auth/token")
            .header("Content-Type", "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.get("token_id").is_some());
        assert!(json.get("token").is_some());
        assert!(json.get("expires_at").is_some());
    }

    #[tokio::test]
    async fn generate_token_with_wrong_passphrase() {
        let (app, provider) = setup_auth_router();

        // First call sets the passphrase.
        let req = crate::auth::types::TokenRequest {
            passphrase: "correct".into(),
            expires_in_days: None,
        };
        provider.generate_token(&req).await.unwrap();

        // Second call with wrong passphrase.
        let body = serde_json::to_string(&json!({
            "passphrase": "wrong"
        }))
        .unwrap();

        let request = Request::builder()
            .method("POST")
            .uri("/api/auth/token")
            .header("Content-Type", "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn list_tokens_requires_auth() {
        let (app, _provider) = setup_auth_router();

        let request = Request::builder()
            .uri("/api/auth/tokens")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn list_tokens_with_valid_auth() {
        let (app, provider) = setup_auth_router();

        // Generate a token.
        let req = crate::auth::types::TokenRequest {
            passphrase: "test".into(),
            expires_in_days: Some(30),
        };
        let resp = provider.generate_token(&req).await.unwrap();

        let request = Request::builder()
            .uri("/api/auth/tokens")
            .header("Authorization", format!("Bearer {}", resp.token))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let tokens = json["tokens"].as_array().unwrap();
        assert_eq!(tokens.len(), 1);
    }

    #[tokio::test]
    async fn revoke_token_with_valid_auth() {
        let (app, provider) = setup_auth_router();

        // Generate two tokens.
        let req = crate::auth::types::TokenRequest {
            passphrase: "test".into(),
            expires_in_days: Some(30),
        };
        let resp1 = provider.generate_token(&req).await.unwrap();
        let req2 = crate::auth::types::TokenRequest {
            passphrase: "test".into(),
            expires_in_days: Some(30),
        };
        let resp2 = provider.generate_token(&req2).await.unwrap();

        // Revoke the second token using the first token for auth.
        let request = Request::builder()
            .method("DELETE")
            .uri(format!("/api/auth/token/{}", resp2.token_id))
            .header("Authorization", format!("Bearer {}", resp1.token))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Verify it was revoked.
        let err = provider.validate_token(&resp2.token).await.unwrap_err();
        assert!(matches!(err, AuthError::TokenNotFound));
    }

    #[tokio::test]
    async fn revoke_nonexistent_token() {
        let (app, provider) = setup_auth_router();

        let req = crate::auth::types::TokenRequest {
            passphrase: "test".into(),
            expires_in_days: Some(30),
        };
        let resp = provider.generate_token(&req).await.unwrap();

        let request = Request::builder()
            .method("DELETE")
            .uri("/api/auth/token/nonexistent-id")
            .header("Authorization", format!("Bearer {}", resp.token))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn revoke_token_requires_auth() {
        let (app, _provider) = setup_auth_router();

        let request = Request::builder()
            .method("DELETE")
            .uri("/api/auth/token/some-id")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn register_returns_not_found_without_oidc() {
        let (app, _provider) = setup_auth_router();

        let body = serde_json::to_string(&json!({
            "username": "newuser",
            "password": "secret123"
        }))
        .unwrap();

        let request = Request::builder()
            .method("POST")
            .uri("/api/auth/register")
            .header("Content-Type", "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "OIDC_NOT_CONFIGURED");
    }
}
