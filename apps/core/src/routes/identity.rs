//! Identity credential API routes.
//!
//! Exposes the identity credential store via REST endpoints at
//! `/api/identity/credentials`. Credential claims are NEVER included
//! in list responses — only metadata.

use crate::identity::{CredentialType, IdentityCredential};
use crate::routes::health::AppState;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

/// Request body for creating an identity credential.
#[derive(Debug, Deserialize)]
pub struct CreateCredentialRequest {
    pub credential_type: String,
    pub issuer: String,
    pub issued_date: String,
    pub expiry_date: Option<String>,
    /// Claims are accepted but NEVER logged.
    pub claims: serde_json::Value,
}

/// Request body for selective disclosure.
#[derive(Debug, Deserialize)]
pub struct DiscloseRequest {
    pub claim_names: Vec<String>,
    pub recipient: String,
    pub ttl_hours: Option<i64>,
}

/// POST /api/identity/credentials — Create an identity credential.
pub async fn create_identity_credential(
    State(state): State<AppState>,
    Json(body): Json<CreateCredentialRequest>,
) -> impl IntoResponse {
    let store = match &state.identity_store {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": {
                        "code": "SYSTEM_INTERNAL_ERROR",
                        "message": "identity store not available"
                    }
                })),
            )
                .into_response();
        }
    };

    let credential_type = match parse_credential_type(&body.credential_type) {
        Some(ct) => ct,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": {
                        "code": "DATA_VALIDATION_FAILED",
                        "message": "invalid credential_type"
                    }
                })),
            )
                .into_response();
        }
    };

    let now = Utc::now();
    let issued_date = match chrono::DateTime::parse_from_rfc3339(&body.issued_date) {
        Ok(d) => d.with_timezone(&Utc),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": {
                        "code": "DATA_VALIDATION_FAILED",
                        "message": "invalid issued_date format (expected RFC 3339)"
                    }
                })),
            )
                .into_response();
        }
    };

    let expiry_date = match body.expiry_date {
        Some(ref d) => match chrono::DateTime::parse_from_rfc3339(d) {
            Ok(d) => Some(d.with_timezone(&Utc)),
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": {
                            "code": "DATA_VALIDATION_FAILED",
                            "message": "invalid expiry_date format (expected RFC 3339)"
                        }
                    })),
                )
                    .into_response();
            }
        },
        None => None,
    };

    let credential = IdentityCredential {
        id: Uuid::new_v4().to_string(),
        credential_type,
        issuer: body.issuer.clone(),
        issued_date,
        expiry_date,
        claims: body.claims.clone(),
        created_at: now,
        updated_at: now,
    };

    match store.create(&credential).await {
        Ok(()) => (
            StatusCode::CREATED,
            Json(json!({
                "id": credential.id,
                "credential_type": body.credential_type,
                "issuer": body.issuer,
                "message": "identity credential created"
            })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to create identity credential");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "SYSTEM_INTERNAL_ERROR",
                        "message": "failed to create identity credential"
                    }
                })),
            )
                .into_response()
        }
    }
}

/// GET /api/identity/credentials — List all identity credentials (metadata only).
pub async fn list_identity_credentials(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let store = match &state.identity_store {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": {
                        "code": "SYSTEM_INTERNAL_ERROR",
                        "message": "identity store not available"
                    }
                })),
            )
                .into_response();
        }
    };

    match store.list().await {
        Ok(credentials) => {
            let total = credentials.len();
            (
                StatusCode::OK,
                Json(json!({
                    "credentials": credentials,
                    "total": total
                })),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to list identity credentials");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "SYSTEM_INTERNAL_ERROR",
                        "message": "failed to list identity credentials"
                    }
                })),
            )
                .into_response()
        }
    }
}

/// GET /api/identity/credentials/{id} — Get a credential (includes claims).
pub async fn get_identity_credential(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let store = match &state.identity_store {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": {
                        "code": "SYSTEM_INTERNAL_ERROR",
                        "message": "identity store not available"
                    }
                })),
            )
                .into_response();
        }
    };

    match store.get(&id).await {
        Ok(Some(credential)) => (StatusCode::OK, Json(json!(credential))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": {
                    "code": "CREDENTIAL_NOT_FOUND",
                    "message": format!("identity credential '{id}' not found")
                }
            })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to get identity credential");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "SYSTEM_INTERNAL_ERROR",
                        "message": "failed to get identity credential"
                    }
                })),
            )
                .into_response()
        }
    }
}

/// DELETE /api/identity/credentials/{id} — Delete a credential.
pub async fn delete_identity_credential(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let store = match &state.identity_store {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": {
                        "code": "SYSTEM_INTERNAL_ERROR",
                        "message": "identity store not available"
                    }
                })),
            )
                .into_response();
        }
    };

    match store.delete(&id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": {
                    "code": "CREDENTIAL_NOT_FOUND",
                    "message": format!("identity credential '{id}' not found")
                }
            })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to delete identity credential");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "SYSTEM_INTERNAL_ERROR",
                        "message": "failed to delete identity credential"
                    }
                })),
            )
                .into_response()
        }
    }
}

/// POST /api/identity/credentials/{id}/disclose — Create a selective disclosure token.
pub async fn disclose_credential(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<DiscloseRequest>,
) -> impl IntoResponse {
    let store = match &state.identity_store {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": {
                        "code": "SYSTEM_INTERNAL_ERROR",
                        "message": "identity store not available"
                    }
                })),
            )
                .into_response();
        }
    };

    let ttl = Duration::hours(body.ttl_hours.unwrap_or(24));

    match store
        .disclose(&id, &body.claim_names, &body.recipient, ttl)
        .await
    {
        Ok(token) => (StatusCode::CREATED, Json(json!(token))).into_response(),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("not found") {
                (
                    StatusCode::NOT_FOUND,
                    Json(json!({
                        "error": {
                            "code": "CREDENTIAL_NOT_FOUND",
                            "message": format!("identity credential '{id}' not found")
                        }
                    })),
                )
                    .into_response()
            } else {
                tracing::error!(error = %e, "failed to create disclosure token");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": {
                            "code": "SYSTEM_INTERNAL_ERROR",
                            "message": "failed to create disclosure token"
                        }
                    })),
                )
                    .into_response()
            }
        }
    }
}

/// GET /api/identity/credentials/{id}/audit — Get disclosure audit log.
pub async fn get_disclosure_audit(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let store = match &state.identity_store {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": {
                        "code": "SYSTEM_INTERNAL_ERROR",
                        "message": "identity store not available"
                    }
                })),
            )
                .into_response();
        }
    };

    match store.get_audit_log(&id).await {
        Ok(entries) => {
            let total = entries.len();
            (
                StatusCode::OK,
                Json(json!({
                    "audit_log": entries,
                    "total": total
                })),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to get disclosure audit log");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "SYSTEM_INTERNAL_ERROR",
                        "message": "failed to get disclosure audit log"
                    }
                })),
            )
                .into_response()
        }
    }
}

/// GET /api/identity/did — Get the local DID.
pub async fn get_did(State(state): State<AppState>) -> impl IntoResponse {
    let store = match &state.identity_store {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": {
                        "code": "SYSTEM_INTERNAL_ERROR",
                        "message": "identity store not available"
                    }
                })),
            )
                .into_response();
        }
    };

    let did = store.generate_did();
    (StatusCode::OK, Json(json!({ "did": did }))).into_response()
}

/// GET /api/identity/credentials/{id}/vc — Export as W3C Verifiable Credential.
pub async fn export_verifiable_credential(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let store = match &state.identity_store {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": {
                        "code": "SYSTEM_INTERNAL_ERROR",
                        "message": "identity store not available"
                    }
                })),
            )
                .into_response();
        }
    };

    match store.get(&id).await {
        Ok(Some(credential)) => {
            let did = store.generate_did();
            let vc = store.to_verifiable_credential(&credential, &did);
            (StatusCode::OK, Json(json!(vc))).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": {
                    "code": "CREDENTIAL_NOT_FOUND",
                    "message": format!("identity credential '{id}' not found")
                }
            })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to export verifiable credential");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "SYSTEM_INTERNAL_ERROR",
                        "message": "failed to export verifiable credential"
                    }
                })),
            )
                .into_response()
        }
    }
}

fn parse_credential_type(s: &str) -> Option<CredentialType> {
    match s {
        "passport" => Some(CredentialType::Passport),
        "drivers_licence" => Some(CredentialType::DriversLicence),
        "certificate" => Some(CredentialType::Certificate),
        "identity_card" => Some(CredentialType::IdentityCard),
        other if !other.is_empty() => Some(CredentialType::Custom(other.into())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::IdentityStore;
    use crate::test_helpers::{auth_request, body_json, create_auth_state, generate_test_token};
    use axum::routing::{delete, get, post};
    use axum::Router;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tower::ServiceExt;

    async fn setup_state() -> (AppState, String, axum::middleware::FromFnLayer<
        fn(
            axum::extract::State<crate::auth::middleware::AuthMiddlewareState>,
            axum::http::Request<axum::body::Body>,
            axum::middleware::Next,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = axum::http::Response<axum::body::Body>> + Send>>,
        crate::auth::middleware::AuthMiddlewareState,
        axum::body::Body,
    >) {
        // This is a simplified helper — see individual tests.
        unreachable!("use per-test setup instead");
    }

    async fn make_test_app() -> (Router, String) {
        let (auth_state, provider) = create_auth_state();
        let token = generate_test_token(&provider).await;

        let conn =
            rusqlite::Connection::open_in_memory().expect("in-memory SQLite should open");
        let conn = Arc::new(Mutex::new(conn));
        let identity_store = Arc::new(
            IdentityStore::new(conn, "test-identity-secret")
                .expect("store should create"),
        );
        identity_store.init().await.expect("init should succeed");

        let mut state = crate::test_helpers::default_app_state();
        state.identity_store = Some(identity_store);

        let app = Router::new()
            .route(
                "/api/identity/credentials",
                post(create_identity_credential).get(list_identity_credentials),
            )
            .route(
                "/api/identity/credentials/{id}",
                get(get_identity_credential).delete(delete_identity_credential),
            )
            .route(
                "/api/identity/credentials/{id}/disclose",
                post(disclose_credential),
            )
            .route(
                "/api/identity/credentials/{id}/audit",
                get(get_disclosure_audit),
            )
            .route(
                "/api/identity/credentials/{id}/vc",
                get(export_verifiable_credential),
            )
            .route("/api/identity/did", get(get_did))
            .with_state(state)
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                crate::auth::middleware::auth_middleware,
            ));

        (app, token)
    }

    #[tokio::test]
    async fn create_and_get_identity_credential_via_api() {
        let (app, token) = make_test_app().await;

        let req = auth_request(
            "POST",
            "/api/identity/credentials",
            &token,
            Some(
                serde_json::json!({
                    "credential_type": "passport",
                    "issuer": "au.gov",
                    "issued_date": "2024-01-15T00:00:00Z",
                    "claims": {
                        "full_name": "Jane Doe",
                        "passport_number": "PA123"
                    }
                })
                .to_string(),
            ),
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let json = body_json(resp).await;
        let id = json["id"].as_str().unwrap().to_string();

        // Get the credential.
        let req = auth_request(
            "GET",
            &format!("/api/identity/credentials/{id}"),
            &token,
            None,
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["claims"]["full_name"], "Jane Doe");
    }

    #[tokio::test]
    async fn list_credentials_excludes_claims() {
        let (app, token) = make_test_app().await;

        let req = auth_request(
            "POST",
            "/api/identity/credentials",
            &token,
            Some(
                serde_json::json!({
                    "credential_type": "passport",
                    "issuer": "au.gov",
                    "issued_date": "2024-01-15T00:00:00Z",
                    "claims": { "secret": "hidden-value" }
                })
                .to_string(),
            ),
        );
        app.clone().oneshot(req).await.unwrap();

        let req = auth_request("GET", "/api/identity/credentials", &token, None);
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["total"], 1);
        // Claims must not appear in list response.
        let cred_json = json["credentials"][0].to_string();
        assert!(
            !cred_json.contains("hidden-value"),
            "claims must not appear in list response"
        );
    }

    #[tokio::test]
    async fn delete_identity_credential_via_api() {
        let (app, token) = make_test_app().await;

        let req = auth_request(
            "POST",
            "/api/identity/credentials",
            &token,
            Some(
                serde_json::json!({
                    "credential_type": "certificate",
                    "issuer": "test-ca",
                    "issued_date": "2024-06-01T00:00:00Z",
                    "claims": {}
                })
                .to_string(),
            ),
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        let json = body_json(resp).await;
        let id = json["id"].as_str().unwrap().to_string();

        let req = auth_request(
            "DELETE",
            &format!("/api/identity/credentials/{id}"),
            &token,
            None,
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        // Verify it's gone.
        let req = auth_request(
            "GET",
            &format!("/api/identity/credentials/{id}"),
            &token,
            None,
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn disclose_via_api() {
        let (app, token) = make_test_app().await;

        let req = auth_request(
            "POST",
            "/api/identity/credentials",
            &token,
            Some(
                serde_json::json!({
                    "credential_type": "passport",
                    "issuer": "au.gov",
                    "issued_date": "2024-01-15T00:00:00Z",
                    "claims": { "nationality": "Australian", "age": 34 }
                })
                .to_string(),
            ),
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        let json = body_json(resp).await;
        let id = json["id"].as_str().unwrap().to_string();

        let req = auth_request(
            "POST",
            &format!("/api/identity/credentials/{id}/disclose"),
            &token,
            Some(
                serde_json::json!({
                    "claim_names": ["nationality"],
                    "recipient": "border-control",
                    "ttl_hours": 1
                })
                .to_string(),
            ),
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let json = body_json(resp).await;
        assert_eq!(json["disclosed_claims"]["nationality"], "Australian");
        assert!(json["disclosed_claims"].get("age").is_none());
        assert!(!json["signature"].as_str().unwrap().is_empty());
    }

    #[tokio::test]
    async fn audit_log_via_api() {
        let (app, token) = make_test_app().await;

        let req = auth_request(
            "POST",
            "/api/identity/credentials",
            &token,
            Some(
                serde_json::json!({
                    "credential_type": "passport",
                    "issuer": "au.gov",
                    "issued_date": "2024-01-15T00:00:00Z",
                    "claims": { "nationality": "Australian" }
                })
                .to_string(),
            ),
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        let json = body_json(resp).await;
        let id = json["id"].as_str().unwrap().to_string();

        // Disclose to create an audit entry.
        let req = auth_request(
            "POST",
            &format!("/api/identity/credentials/{id}/disclose"),
            &token,
            Some(
                serde_json::json!({
                    "claim_names": ["nationality"],
                    "recipient": "customs"
                })
                .to_string(),
            ),
        );
        app.clone().oneshot(req).await.unwrap();

        let req = auth_request(
            "GET",
            &format!("/api/identity/credentials/{id}/audit"),
            &token,
            None,
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["total"], 1);
        assert_eq!(json["audit_log"][0]["recipient"], "customs");
    }

    #[tokio::test]
    async fn get_did_via_api() {
        let (app, token) = make_test_app().await;

        let req = auth_request("GET", "/api/identity/did", &token, None);
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        let did = json["did"].as_str().unwrap();
        assert!(did.starts_with("did:key:z"));
    }

    #[tokio::test]
    async fn export_vc_via_api() {
        let (app, token) = make_test_app().await;

        let req = auth_request(
            "POST",
            "/api/identity/credentials",
            &token,
            Some(
                serde_json::json!({
                    "credential_type": "passport",
                    "issuer": "au.gov",
                    "issued_date": "2024-01-15T00:00:00Z",
                    "expiry_date": "2034-01-15T00:00:00Z",
                    "claims": { "full_name": "Jane Doe" }
                })
                .to_string(),
            ),
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        let json = body_json(resp).await;
        let id = json["id"].as_str().unwrap().to_string();

        let req = auth_request(
            "GET",
            &format!("/api/identity/credentials/{id}/vc"),
            &token,
            None,
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        assert!(json["@context"].is_array());
        assert!(json["type"].as_array().unwrap().contains(&json!("VerifiableCredential")));
        assert!(json["type"].as_array().unwrap().contains(&json!("PassportCredential")));
        assert!(json["id"].as_str().unwrap().starts_with("urn:uuid:"));
        assert_eq!(json["issuer"], "au.gov");
    }
}
