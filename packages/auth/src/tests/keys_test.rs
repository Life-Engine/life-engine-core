//! Tests for API key lifecycle: create, list, validate, revoke.

use std::sync::Mutex;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use life_engine_traits::{EngineError, StorageBackend};
use life_engine_types::{PipelineMessage, StorageMutation, StorageQuery};
use uuid::Uuid;

use crate::handlers::keys::{create_key, list_keys, revoke_key, validate_key};

// ---------------------------------------------------------------------------
// Mock storage backend
// ---------------------------------------------------------------------------

/// In-memory mock storage that persists `PipelineMessage`s across calls.
struct MockStorage {
    records: Mutex<Vec<PipelineMessage>>,
}

impl MockStorage {
    fn empty() -> Self {
        Self {
            records: Mutex::new(vec![]),
        }
    }
}

#[async_trait]
impl StorageBackend for MockStorage {
    async fn execute(
        &self,
        query: StorageQuery,
    ) -> Result<Vec<PipelineMessage>, Box<dyn EngineError>> {
        let records = self.records.lock().unwrap();
        let mut results: Vec<PipelineMessage> = records
            .iter()
            .filter(|msg| {
                // Apply ID filter if present.
                for f in &query.filters {
                    if f.field == "id" {
                        let target = f.value.as_str().unwrap_or("");
                        if msg.metadata.correlation_id.to_string() != target {
                            return false;
                        }
                    }
                }
                true
            })
            .cloned()
            .collect();

        if let Some(limit) = query.limit {
            results.truncate(limit as usize);
        }
        Ok(results)
    }

    async fn mutate(&self, op: StorageMutation) -> Result<(), Box<dyn EngineError>> {
        let mut records = self.records.lock().unwrap();
        match op {
            StorageMutation::Insert { data, .. } => {
                records.push(data);
            }
            StorageMutation::Update { id, data, .. } => {
                records.retain(|msg| msg.metadata.correlation_id != id);
                records.push(data);
            }
            StorageMutation::Delete { id, .. } => {
                records.retain(|msg| msg.metadata.correlation_id != id);
            }
        }
        Ok(())
    }

    async fn init(
        _config: toml::Value,
        _key: [u8; 32],
    ) -> Result<Self, Box<dyn EngineError>> {
        Ok(MockStorage::empty())
    }
}

// ---------------------------------------------------------------------------
// Create key tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_key_returns_raw_key_and_record() {
    let storage = MockStorage::empty();

    let (raw_key, record) =
        create_key(&storage, "test-key".into(), vec!["read".into()], None)
            .await
            .expect("create_key should succeed");

    assert!(!raw_key.is_empty(), "raw key should not be empty");
    assert_eq!(record.name, "test-key");
    assert_eq!(record.scopes, vec!["read"]);
    assert!(!record.revoked);
    assert!(record.last_used.is_none());
    assert!(record.expires_at.is_none());
}

#[tokio::test]
async fn create_key_stores_record_in_storage() {
    let storage = MockStorage::empty();

    let (_raw_key, record) =
        create_key(&storage, "stored-key".into(), vec![], None)
            .await
            .expect("create_key should succeed");

    let keys = list_keys(&storage).await.expect("list_keys should succeed");
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].id, record.id);
    assert_eq!(keys[0].name, "stored-key");
}

#[tokio::test]
async fn create_multiple_keys_are_all_listed() {
    let storage = MockStorage::empty();

    create_key(&storage, "key-a".into(), vec![], None)
        .await
        .unwrap();
    create_key(&storage, "key-b".into(), vec![], None)
        .await
        .unwrap();
    create_key(&storage, "key-c".into(), vec![], None)
        .await
        .unwrap();

    let keys = list_keys(&storage).await.unwrap();
    assert_eq!(keys.len(), 3);
}

// ---------------------------------------------------------------------------
// Validate key tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn validate_key_succeeds_with_correct_raw_key() {
    let storage = MockStorage::empty();

    let (raw_key, record) =
        create_key(&storage, "valid-key".into(), vec!["admin".into()], None)
            .await
            .unwrap();

    let identity = validate_key(&storage, &raw_key).await.unwrap();
    assert_eq!(identity.user_id, record.id.to_string());
    assert_eq!(identity.provider, "api-key");
    assert_eq!(identity.scopes, vec!["admin"]);
}

#[tokio::test]
async fn validate_key_fails_with_wrong_key() {
    let storage = MockStorage::empty();

    create_key(&storage, "real-key".into(), vec![], None)
        .await
        .unwrap();

    let result = validate_key(&storage, "totally-wrong-key").await;
    assert!(result.is_err(), "should reject invalid key");

    let err = result.unwrap_err();
    assert!(
        matches!(err, crate::error::AuthError::KeyInvalid),
        "expected KeyInvalid, got: {err:?}"
    );
}

#[tokio::test]
async fn validate_key_fails_when_no_keys_exist() {
    let storage = MockStorage::empty();

    let result = validate_key(&storage, "any-key").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn validate_key_identifies_correct_key_among_multiple() {
    let storage = MockStorage::empty();

    let (_key_a, _) = create_key(&storage, "key-a".into(), vec!["a".into()], None)
        .await
        .unwrap();
    let (key_b, record_b) =
        create_key(&storage, "key-b".into(), vec!["b".into()], None)
            .await
            .unwrap();
    let (_key_c, _) = create_key(&storage, "key-c".into(), vec!["c".into()], None)
        .await
        .unwrap();

    let identity = validate_key(&storage, &key_b).await.unwrap();
    assert_eq!(identity.user_id, record_b.id.to_string());
    assert_eq!(identity.scopes, vec!["b"]);
}

// ---------------------------------------------------------------------------
// Revoke key tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn revoked_key_cannot_be_validated() {
    let storage = MockStorage::empty();

    let (raw_key, record) =
        create_key(&storage, "revocable".into(), vec![], None)
            .await
            .unwrap();

    revoke_key(&storage, record.id).await.unwrap();

    let result = validate_key(&storage, &raw_key).await;
    assert!(result.is_err(), "revoked key should not validate");

    let err = result.unwrap_err();
    assert!(
        matches!(err, crate::error::AuthError::KeyRevoked),
        "expected KeyRevoked, got: {err:?}"
    );
}

#[tokio::test]
async fn revoke_nonexistent_key_returns_error() {
    let storage = MockStorage::empty();
    let fake_id = Uuid::new_v4();

    let result = revoke_key(&storage, fake_id).await;
    assert!(result.is_err(), "revoking nonexistent key should fail");
}

#[tokio::test]
async fn revoking_one_key_does_not_affect_others() {
    let storage = MockStorage::empty();

    let (key_a, record_a) =
        create_key(&storage, "key-a".into(), vec![], None)
            .await
            .unwrap();
    let (key_b, _record_b) =
        create_key(&storage, "key-b".into(), vec![], None)
            .await
            .unwrap();

    revoke_key(&storage, record_a.id).await.unwrap();

    // key_a should be revoked.
    assert!(validate_key(&storage, &key_a).await.is_err());

    // key_b should still work.
    assert!(validate_key(&storage, &key_b).await.is_ok());
}

// ---------------------------------------------------------------------------
// Expiration tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn expired_key_cannot_be_validated() {
    let storage = MockStorage::empty();

    let past = Utc::now() - Duration::hours(1);
    let (raw_key, _record) =
        create_key(&storage, "expired".into(), vec![], Some(past))
            .await
            .unwrap();

    let result = validate_key(&storage, &raw_key).await;
    assert!(result.is_err(), "expired key should not validate");
}

#[tokio::test]
async fn future_expiration_key_validates_successfully() {
    let storage = MockStorage::empty();

    let future = Utc::now() + Duration::hours(24);
    let (raw_key, _record) =
        create_key(&storage, "not-expired".into(), vec!["read".into()], Some(future))
            .await
            .unwrap();

    let identity = validate_key(&storage, &raw_key).await.unwrap();
    assert_eq!(identity.provider, "api-key");
}

// ---------------------------------------------------------------------------
// List keys tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_keys_returns_empty_when_no_keys() {
    let storage = MockStorage::empty();

    let keys = list_keys(&storage).await.unwrap();
    assert!(keys.is_empty());
}

// ---------------------------------------------------------------------------
// ApiKeyProvider trait integration
// ---------------------------------------------------------------------------

#[tokio::test]
async fn api_key_provider_validate_key_delegates_to_storage() {
    use std::sync::Arc;

    use crate::handlers::keys::ApiKeyProvider;
    use crate::AuthProvider;

    let storage = Arc::new(MockStorage::empty());

    let (raw_key, record) = create_key(
        storage.as_ref(),
        "provider-test".into(),
        vec!["write".into()],
        None,
    )
    .await
    .unwrap();

    let provider = ApiKeyProvider::new(storage.clone());
    let identity = provider.validate_key(&raw_key).await.unwrap();
    assert_eq!(identity.user_id, record.id.to_string());
    assert_eq!(identity.scopes, vec!["write"]);
}

#[tokio::test]
async fn api_key_provider_validate_token_returns_error() {
    use std::sync::Arc;

    use crate::handlers::keys::ApiKeyProvider;
    use crate::AuthProvider;

    let storage = Arc::new(MockStorage::empty());
    let provider = ApiKeyProvider::new(storage);

    let result = provider.validate_token("some-jwt").await;
    assert!(result.is_err(), "token validation should fail for API key provider");
}

#[tokio::test]
async fn api_key_provider_revoke_key_delegates_to_storage() {
    use std::sync::Arc;

    use crate::handlers::keys::ApiKeyProvider;
    use crate::AuthProvider;

    let storage = Arc::new(MockStorage::empty());

    let (raw_key, record) =
        create_key(storage.as_ref(), "revoke-test".into(), vec![], None)
            .await
            .unwrap();

    let provider = ApiKeyProvider::new(storage.clone());
    provider.revoke_key(record.id).await.unwrap();

    let result = provider.validate_key(&raw_key).await;
    assert!(result.is_err());
}
