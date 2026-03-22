//! Credential canonical data model.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The type of credential stored.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialType {
    OauthToken,
    ApiKey,
    IdentityDocument,
    Passkey,
}

/// A credential in the Life Engine canonical data model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Credential {
    pub id: Uuid,
    #[serde(rename = "type")]
    pub credential_type: CredentialType,
    pub issuer: String,
    pub issued_date: NaiveDate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_date: Option<NaiveDate>,
    pub claims: serde_json::Value,
    pub source: String,
    pub source_id: String,
    /// Plugin-specific extension data, namespaced by plugin ID (reverse-domain format).
    /// Each key is a plugin's manifest `id` (e.g. `com.life-engine.credentials`) and each value
    /// is an opaque JSON object owned by that plugin. See ADR-014.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
