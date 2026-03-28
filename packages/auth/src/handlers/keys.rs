//! API key management: create, list, validate, and revoke API keys.

use std::sync::Arc;

use async_trait::async_trait;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::Utc;
use life_engine_crypto::{generate_salt, hmac_sign, hmac_verify};
use life_engine_traits::StorageBackend;
use life_engine_types::{
    FilterOp, MessageMetadata, PipelineMessage, QueryFilter, SchemaValidated,
    StorageMutation, StorageQuery, TypedPayload,
};
use rand::TryRngCore;
use rand::rngs::OsRng;
use tracing::{info, warn};
use uuid::Uuid;

use crate::error::AuthError;
use crate::types::{ApiKeyMetadata, ApiKeyRecord, AuthIdentity};
use crate::AuthProvider;

/// Plugin ID used for API key storage operations.
const AUTH_PLUGIN_ID: &str = "core.auth";
/// Collection name for API key records.
const KEYS_COLLECTION: &str = "credentials";
/// Permissive JSON schema that accepts any value.
fn permissive_schema() -> serde_json::Value {
    serde_json::json!({})
}

/// API key authentication provider backed by persistent storage.
///
/// Validates API keys by looking up their salted HMAC-SHA256 hash in storage.
/// Provides CRUD operations for key lifecycle management.
pub struct ApiKeyProvider {
    storage: Arc<dyn StorageBackend>,
}

impl ApiKeyProvider {
    /// Create a new API key provider with the given storage backend.
    pub fn new(storage: Arc<dyn StorageBackend>) -> Self {
        Self { storage }
    }

    /// List all API key metadata (hashes and salts excluded).
    pub async fn list_keys(&self) -> Result<Vec<ApiKeyMetadata>, AuthError> {
        list_keys(self.storage.as_ref()).await
    }
}

/// Create a new API key, returning the raw key (shown once) and the stored record.
pub async fn create_key(
    storage: &dyn StorageBackend,
    name: String,
    scopes: Vec<String>,
    expires_at: Option<chrono::DateTime<Utc>>,
) -> Result<(String, ApiKeyRecord), AuthError> {
    // Generate a cryptographically random 32-byte key.
    let mut raw_bytes = [0u8; 32];
    OsRng.try_fill_bytes(&mut raw_bytes).expect("OS RNG should not fail");
    let raw_key = URL_SAFE_NO_PAD.encode(raw_bytes);

    // Generate a unique salt and hash the key with HMAC-SHA256.
    let salt = generate_salt();
    let salt_encoded = URL_SAFE_NO_PAD.encode(salt);
    let hash = hmac_sign(&salt, raw_key.as_bytes());
    let hash_encoded = URL_SAFE_NO_PAD.encode(&hash);

    let now = Utc::now();
    let id = Uuid::new_v4();

    let record = ApiKeyRecord {
        id,
        name: name.clone(),
        key_hash: hash_encoded,
        salt: salt_encoded,
        scopes: scopes.clone(),
        created_at: now,
        expires_at,
        revoked: false,
        last_used: None,
    };

    let data = record_to_pipeline_message(&record)?;

    storage
        .mutate(StorageMutation::Insert {
            plugin_id: AUTH_PLUGIN_ID.to_string(),
            collection: KEYS_COLLECTION.to_string(),
            data,
        })
        .await
        .map_err(|e| AuthError::ConfigInvalid(format!("storage error: {e}")))?;

    info!(key_id = %id, key_name = %name, "API key created");

    Ok((raw_key, record))
}

/// List all API key metadata from storage (excludes key_hash and salt).
pub async fn list_keys(
    storage: &dyn StorageBackend,
) -> Result<Vec<ApiKeyMetadata>, AuthError> {
    let keys = list_key_records(storage).await?;
    Ok(keys.into_iter().map(ApiKeyMetadata::from).collect())
}

/// List all full API key records from storage (internal use only).
async fn list_key_records(
    storage: &dyn StorageBackend,
) -> Result<Vec<ApiKeyRecord>, AuthError> {
    let query = StorageQuery {
        collection: KEYS_COLLECTION.to_string(),
        plugin_id: AUTH_PLUGIN_ID.to_string(),
        filters: vec![],
        sort: vec![],
        limit: None,
        offset: None,
    };

    let results = storage
        .execute(query)
        .await
        .map_err(|e| AuthError::ConfigInvalid(format!("storage error: {e}")))?;

    let mut keys = Vec::new();
    for msg in results {
        if let Ok(record) = extract_key_record(&msg) {
            keys.push(record);
        }
    }
    Ok(keys)
}

/// Revoke an API key by setting its `revoked` flag to `true`.
pub async fn revoke_key(
    storage: &dyn StorageBackend,
    key_id: Uuid,
) -> Result<(), AuthError> {
    let mut record = find_key_by_id(storage, key_id).await?;
    record.revoked = true;

    let data = record_to_pipeline_message(&record)?;

    storage
        .mutate(StorageMutation::Update {
            plugin_id: AUTH_PLUGIN_ID.to_string(),
            collection: KEYS_COLLECTION.to_string(),
            id: key_id,
            data,
            expected_version: 1,
        })
        .await
        .map_err(|e| AuthError::ConfigInvalid(format!("storage error: {e}")))?;

    info!(key_id = %key_id, "API key revoked");

    Ok(())
}

/// Validate a raw API key against stored records.
///
/// Hashes the provided key with each stored key's salt, compares using
/// constant-time comparison, checks revocation and expiration status,
/// and updates `last_used` on success.
pub async fn validate_key(
    storage: &dyn StorageBackend,
    raw_key: &str,
) -> Result<AuthIdentity, AuthError> {
    let all_keys = list_key_records(storage).await?;

    for record in &all_keys {
        let salt = URL_SAFE_NO_PAD
            .decode(&record.salt)
            .map_err(|_| AuthError::KeyInvalid)?;

        let stored_hash = URL_SAFE_NO_PAD
            .decode(&record.key_hash)
            .map_err(|_| AuthError::KeyInvalid)?;

        // Constant-time comparison via hmac_verify.
        if !hmac_verify(&salt, raw_key.as_bytes(), &stored_hash) {
            continue;
        }

        // Key matched — check revocation.
        if record.revoked {
            warn!(key_id = %record.id, "attempt to use revoked API key");
            return Err(AuthError::KeyRevoked);
        }

        // Check expiration.
        if let Some(expires_at) = record.expires_at
            && Utc::now() > expires_at
        {
            warn!(key_id = %record.id, "attempt to use expired API key");
            return Err(AuthError::KeyRevoked);
        }

        // Update last_used timestamp (best-effort, don't fail auth).
        let mut updated = record.clone();
        updated.last_used = Some(Utc::now());
        if let Ok(data) = record_to_pipeline_message(&updated) {
            let _ = storage
                .mutate(StorageMutation::Update {
                    plugin_id: AUTH_PLUGIN_ID.to_string(),
                    collection: KEYS_COLLECTION.to_string(),
                    id: record.id,
                    data,
                    expected_version: 1,
                })
                .await;
        }

        return Ok(AuthIdentity {
            user_id: record.id.to_string(),
            provider: "api-key".to_string(),
            scopes: record.scopes.clone(),
            authenticated_at: Utc::now(),
        });
    }

    Err(AuthError::KeyInvalid)
}

/// Find a key record by its UUID.
async fn find_key_by_id(
    storage: &dyn StorageBackend,
    key_id: Uuid,
) -> Result<ApiKeyRecord, AuthError> {
    let query = StorageQuery {
        collection: KEYS_COLLECTION.to_string(),
        plugin_id: AUTH_PLUGIN_ID.to_string(),
        filters: vec![QueryFilter {
            field: "id".to_string(),
            operator: FilterOp::Eq,
            value: serde_json::Value::String(key_id.to_string()),
        }],
        sort: vec![],
        limit: Some(1),
        offset: None,
    };

    let results = storage
        .execute(query)
        .await
        .map_err(|e| AuthError::ConfigInvalid(format!("storage error: {e}")))?;

    let msg = results.into_iter().next().ok_or(AuthError::KeyInvalid)?;
    extract_key_record(&msg)
}

/// Extract an `ApiKeyRecord` from a `PipelineMessage` payload.
fn extract_key_record(msg: &PipelineMessage) -> Result<ApiKeyRecord, AuthError> {
    let json = match &msg.payload {
        TypedPayload::Custom(validated) => serde_json::to_value(&**validated)
            .map_err(|e| AuthError::ConfigInvalid(format!("serialization error: {e}")))?,
        TypedPayload::Cdm(cdm) => serde_json::to_value(&**cdm)
            .map_err(|e| AuthError::ConfigInvalid(format!("serialization error: {e}")))?,
    };

    serde_json::from_value(json).map_err(|e| {
        AuthError::ConfigInvalid(format!("failed to deserialize API key record: {e}"))
    })
}

/// Serialize an `ApiKeyRecord` into a `PipelineMessage` for storage.
fn record_to_pipeline_message(record: &ApiKeyRecord) -> Result<PipelineMessage, AuthError> {
    let value = serde_json::to_value(record)
        .map_err(|e| AuthError::ConfigInvalid(format!("serialization error: {e}")))?;

    let schema = permissive_schema();
    let validated = SchemaValidated::new(value, &schema)
        .map_err(|e| AuthError::ConfigInvalid(format!("schema validation error: {e}")))?;

    Ok(PipelineMessage {
        metadata: MessageMetadata {
            correlation_id: record.id,
            source: "core.auth.keys".to_string(),
            timestamp: Utc::now(),
            auth_context: None,
            warnings: vec![],
        },
        payload: TypedPayload::Custom(validated),
    })
}

#[async_trait]
impl AuthProvider for ApiKeyProvider {
    async fn validate_token(&self, _token: &str) -> Result<AuthIdentity, AuthError> {
        Err(AuthError::TokenInvalid(
            "api-key provider does not support bearer tokens".to_string(),
        ))
    }

    async fn validate_key(&self, key: &str) -> Result<AuthIdentity, AuthError> {
        validate_key(self.storage.as_ref(), key).await
    }

    async fn revoke_key(&self, key_id: Uuid) -> Result<(), AuthError> {
        revoke_key(self.storage.as_ref(), key_id).await
    }
}
