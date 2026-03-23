//! Auth error types implementing the EngineError trait.

use life_engine_traits::{EngineError, Severity};
use thiserror::Error;

/// Errors that can occur during authentication and authorization.
#[derive(Debug, Error)]
pub enum AuthError {
    /// No authorization header present.
    #[error("no authorization header present")]
    TokenMissing,

    /// JWT past its expiration claim.
    #[error("token expired")]
    TokenExpired,

    /// Signature verification failed or malformed JWT.
    #[error("token validation failed: {0}")]
    TokenInvalid(String),

    /// Cannot reach OIDC issuer for key refresh.
    #[error("authentication provider unreachable: {0}")]
    ProviderUnreachable(String),

    /// Invalid auth configuration.
    #[error("invalid auth configuration: {0}")]
    ConfigInvalid(String),

    /// Too many failed attempts from this IP.
    #[error("rate limit exceeded, retry after {retry_after}s")]
    RateLimited {
        /// Seconds until retry is allowed.
        retry_after: u64,
    },

    /// API key has been revoked.
    #[error("API key has been revoked")]
    KeyRevoked,

    /// API key not found or wrong hash.
    #[error("invalid API key")]
    KeyInvalid,
}

impl EngineError for AuthError {
    fn code(&self) -> &str {
        match self {
            AuthError::TokenMissing => "AUTH_001",
            AuthError::TokenExpired => "AUTH_002",
            AuthError::TokenInvalid(_) => "AUTH_003",
            AuthError::ProviderUnreachable(_) => "AUTH_004",
            AuthError::ConfigInvalid(_) => "AUTH_005",
            AuthError::RateLimited { .. } => "AUTH_006",
            AuthError::KeyRevoked => "AUTH_007",
            AuthError::KeyInvalid => "AUTH_008",
        }
    }

    fn severity(&self) -> Severity {
        match self {
            AuthError::ProviderUnreachable(_) => Severity::Retryable,
            _ => Severity::Fatal,
        }
    }

    fn source_module(&self) -> &str {
        "auth"
    }
}
