//! Auth error types.

use thiserror::Error;

/// Errors that can occur during authentication and authorization.
#[derive(Debug, Error)]
pub enum AuthError {
    /// Invalid credentials.
    #[error("invalid credentials")]
    InvalidCredentials,

    /// Token expired.
    #[error("token expired")]
    TokenExpired,

    /// Token validation failed.
    #[error("token validation failed: {0}")]
    TokenValidation(String),

    /// Rate limit exceeded.
    #[error("rate limit exceeded")]
    RateLimitExceeded,
}
