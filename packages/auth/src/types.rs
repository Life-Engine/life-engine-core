//! Authentication types: identities, tokens, and API key records.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Authenticated identity extracted from a validated token or API key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthIdentity {
    /// Subject claim from JWT or API key owner.
    pub user_id: String,
    /// Authentication provider: "pocket-id" or "api-key".
    pub provider: String,
    /// Authorized scopes.
    pub scopes: Vec<String>,
    /// When the identity was authenticated.
    pub authenticated_at: DateTime<Utc>,
}

/// Parsed authorization token from request headers.
#[derive(Debug, Clone)]
pub enum AuthToken {
    /// JWT Bearer token.
    Bearer(String),
    /// API key.
    ApiKey(String),
}

/// Stored API key record. The raw key is never persisted — only the salted hash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyRecord {
    /// Unique key identifier.
    pub id: Uuid,
    /// Human-readable label.
    pub name: String,
    /// Salted SHA-256 hash of the key.
    pub key_hash: String,
    /// Unique salt for this key.
    pub salt: String,
    /// Authorized scopes.
    pub scopes: Vec<String>,
    /// When the key was created.
    pub created_at: DateTime<Utc>,
    /// Optional expiration.
    pub expires_at: Option<DateTime<Utc>>,
    /// Whether the key has been revoked.
    pub revoked: bool,
    /// Last time the key was used successfully.
    pub last_used: Option<DateTime<Utc>>,
}
