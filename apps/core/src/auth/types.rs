//! Authentication types for the Life Engine Core.
//!
//! Defines the identity, request/response, and error types used
//! by all `AuthProvider` implementations and the auth middleware.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The authenticated identity extracted from a valid token.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AuthIdentity {
    /// Unique identifier for the token.
    pub token_id: String,
    /// The authenticated user's unique identifier (OIDC `sub` claim).
    pub user_id: Option<String>,
    /// The household this user belongs to (if any).
    pub household_id: Option<String>,
    /// The user's role within their household.
    pub role: Option<HouseholdRole>,
    /// When the token was created.
    pub created_at: DateTime<Utc>,
    /// When the token expires.
    pub expires_at: DateTime<Utc>,
}

/// Role within a household, defining access permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HouseholdRole {
    /// Full control: user management, data access, settings.
    Admin,
    /// Own data plus shared collections.
    Member,
    /// Read-only access to specific shared collections.
    Guest,
}

/// Request to generate a new token.
#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    /// The master passphrase to authenticate the request.
    pub passphrase: String,
    /// Number of days until the token expires (default 30).
    pub expires_in_days: Option<u32>,
}

/// Response from token generation.
#[derive(Debug, Serialize)]
pub struct TokenResponse {
    /// Unique identifier for the token.
    pub token_id: String,
    /// The raw token value, shown once only.
    pub token: String,
    /// When the token expires (ISO 8601).
    pub expires_at: String,
}

/// Token metadata (no raw token value).
#[derive(Debug, Serialize)]
pub struct TokenInfo {
    /// Unique identifier for the token.
    pub id: String,
    /// When the token was created (ISO 8601).
    pub created_at: String,
    /// When the token expires (ISO 8601).
    pub expires_at: String,
    /// Whether the token has expired.
    pub is_expired: bool,
}

/// Auth-specific errors.
#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum AuthError {
    /// The provided credentials (passphrase) are invalid.
    #[error("invalid credentials")]
    InvalidCredentials,
    /// The token has expired.
    #[error("token expired")]
    TokenExpired,
    /// The token was not found.
    #[error("token not found")]
    TokenNotFound,
    /// Too many failed attempts; the client is rate limited.
    #[error("rate limited")]
    RateLimited,
    /// An internal error occurred.
    #[error("internal error: {0}")]
    Internal(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_error_display_invalid_credentials() {
        let err = AuthError::InvalidCredentials;
        assert_eq!(err.to_string(), "invalid credentials");
    }

    #[test]
    fn auth_error_display_token_expired() {
        let err = AuthError::TokenExpired;
        assert_eq!(err.to_string(), "token expired");
    }

    #[test]
    fn auth_error_display_token_not_found() {
        let err = AuthError::TokenNotFound;
        assert_eq!(err.to_string(), "token not found");
    }

    #[test]
    fn auth_error_display_rate_limited() {
        let err = AuthError::RateLimited;
        assert_eq!(err.to_string(), "rate limited");
    }

    #[test]
    fn auth_error_display_internal() {
        let err = AuthError::Internal("database connection failed".into());
        assert_eq!(
            err.to_string(),
            "internal error: database connection failed"
        );
    }

    #[test]
    fn token_request_deserializes() {
        let json = r#"{"passphrase": "secret", "expires_in_days": 7}"#;
        let req: TokenRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.passphrase, "secret");
        assert_eq!(req.expires_in_days, Some(7));
    }

    #[test]
    fn token_request_deserializes_without_optional() {
        let json = r#"{"passphrase": "secret"}"#;
        let req: TokenRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.passphrase, "secret");
        assert!(req.expires_in_days.is_none());
    }

    #[test]
    fn token_response_serializes() {
        let resp = TokenResponse {
            token_id: "tid-1".into(),
            token: "abc123".into(),
            expires_at: "2026-04-20T00:00:00Z".into(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["token_id"], "tid-1");
        assert_eq!(json["token"], "abc123");
        assert_eq!(json["expires_at"], "2026-04-20T00:00:00Z");
    }

    #[test]
    fn token_info_serializes() {
        let info = TokenInfo {
            id: "tid-2".into(),
            created_at: "2026-03-20T00:00:00Z".into(),
            expires_at: "2026-04-19T00:00:00Z".into(),
            is_expired: false,
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["id"], "tid-2");
        assert!(!json["is_expired"].as_bool().unwrap());
    }

    #[test]
    fn auth_identity_clone() {
        let identity = AuthIdentity {
            token_id: "tid-3".into(),
            user_id: None,
            household_id: None,
            role: None,
            created_at: Utc::now(),
            expires_at: Utc::now(),
        };
        let cloned = identity.clone();
        assert_eq!(cloned.token_id, "tid-3");
    }
}
