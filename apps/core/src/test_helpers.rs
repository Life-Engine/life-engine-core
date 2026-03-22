//! Shared test helper functions for the Core crate.
//!
//! Eliminates duplication of auth setup, request building, and response
//! parsing across route and middleware test modules.

use crate::auth::local_token::LocalTokenProvider;
use crate::auth::middleware::{AuthMiddlewareState, RateLimiter};
use crate::auth::types::TokenRequest;
use crate::auth::AuthProvider;
use crate::config::CoreConfig;
use crate::message_bus::MessageBus;
use crate::plugin_loader::PluginLoader;
use crate::routes::health::AppState;
use crate::storage::{
    Pagination, QueryFilters, QueryResult, Record, SortOptions, StorageAdapter, StorageError,
};
use async_trait::async_trait;
use axum::body::Body;
use axum::http::Request;
use chrono::Utc;
use http_body_util::BodyExt;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::Mutex as TokioMutex;
use tokio::sync::RwLock;

/// Create an `AuthMiddlewareState` and its backing `LocalTokenProvider`.
///
/// Returns both so callers can generate tokens from the provider.
pub fn create_auth_state() -> (AuthMiddlewareState, Arc<LocalTokenProvider>) {
    let provider = Arc::new(LocalTokenProvider::new());
    let rate_limiter = RateLimiter::new();
    let auth_state = AuthMiddlewareState {
        auth_provider: provider.clone(),
        rate_limiter,
    };
    (auth_state, provider)
}

/// Generate a bearer token string from the given provider.
pub async fn generate_test_token(provider: &Arc<LocalTokenProvider>) -> String {
    let req = TokenRequest {
        passphrase: "test".into(),
        expires_in_days: Some(30),
    };
    provider.generate_token(&req).await.unwrap().token
}

/// Build an HTTP request with `Authorization: Bearer` and `Content-Type: application/json` headers.
///
/// Pass `None` for `body` to send an empty body.
pub fn auth_request(method: &str, uri: &str, token: &str, body: Option<String>) -> Request<Body> {
    let builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json");

    match body {
        Some(b) => builder.body(Body::from(b)).unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    }
}

/// Consume an axum response and parse its body as JSON.
pub async fn body_json(response: axum::http::Response<Body>) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

/// Create a default `AppState` with all optional subsystems set to `None`.
pub fn default_app_state() -> AppState {
    AppState {
        start_time: Instant::now(),
        plugin_loader: Arc::new(TokioMutex::new(PluginLoader::new())),
        storage: None,
        message_bus: Arc::new(MessageBus::new()),
        conflict_store: None,
        validated_storage: None,
        search_engine: None,
        credential_store: None,
        household_store: None,
        federation_store: None,
        identity_store: None,
        config: Arc::new(RwLock::new(CoreConfig::default())),
        config_path: None,
        log_reload_handle: None,
        rate_limiter: None,
    }
}

/// In-memory mock storage for tests.
///
/// Shared across test modules to avoid duplicating this ~120-line impl
/// in every file that needs a `StorageAdapter`.
pub struct MockStorage {
    records: Mutex<HashMap<String, Record>>,
}

impl MockStorage {
    pub fn new() -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
        }
    }

    fn key(plugin_id: &str, collection: &str, id: &str) -> String {
        format!("{plugin_id}:{collection}:{id}")
    }
}

#[async_trait]
impl StorageAdapter for MockStorage {
    async fn get(
        &self,
        plugin_id: &str,
        collection: &str,
        id: &str,
    ) -> anyhow::Result<Option<Record>> {
        let key = Self::key(plugin_id, collection, id);
        Ok(self.records.lock().unwrap().get(&key).cloned())
    }

    async fn create(
        &self,
        plugin_id: &str,
        collection: &str,
        data: serde_json::Value,
    ) -> anyhow::Result<Record> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        let record = Record {
            id: id.clone(),
            plugin_id: plugin_id.into(),
            collection: collection.into(),
            data,
            version: 1,
            user_id: None,
            household_id: None,
            created_at: now,
            updated_at: now,
        };
        let key = Self::key(plugin_id, collection, &id);
        self.records.lock().unwrap().insert(key, record.clone());
        Ok(record)
    }

    async fn update(
        &self,
        plugin_id: &str,
        collection: &str,
        id: &str,
        data: serde_json::Value,
        version: i64,
    ) -> Result<Record, StorageError> {
        let key = Self::key(plugin_id, collection, id);
        let mut records = self.records.lock().unwrap();
        let record = records
            .get(&key)
            .ok_or(StorageError::NotFound)?;
        if record.version != version {
            return Err(StorageError::VersionMismatch);
        }
        let updated = Record {
            data,
            version: version + 1,
            updated_at: Utc::now(),
            ..record.clone()
        };
        records.insert(key, updated.clone());
        Ok(updated)
    }

    async fn query(
        &self,
        plugin_id: &str,
        collection: &str,
        _filters: QueryFilters,
        _sort: Option<SortOptions>,
        pagination: Pagination,
    ) -> anyhow::Result<QueryResult> {
        let records = self.records.lock().unwrap();
        let matching: Vec<Record> = records
            .values()
            .filter(|r| r.plugin_id == plugin_id && r.collection == collection)
            .cloned()
            .collect();
        let total = matching.len() as u64;
        let paged = matching
            .into_iter()
            .skip(pagination.offset as usize)
            .take(pagination.limit as usize)
            .collect();
        Ok(QueryResult {
            records: paged,
            total,
            limit: pagination.limit,
            offset: pagination.offset,
        })
    }

    async fn delete(
        &self,
        plugin_id: &str,
        collection: &str,
        id: &str,
    ) -> anyhow::Result<bool> {
        let key = Self::key(plugin_id, collection, id);
        Ok(self.records.lock().unwrap().remove(&key).is_some())
    }

    async fn list(
        &self,
        plugin_id: &str,
        collection: &str,
        sort: Option<SortOptions>,
        pagination: Pagination,
    ) -> anyhow::Result<QueryResult> {
        self.query(
            plugin_id,
            collection,
            QueryFilters::default(),
            sort,
            pagination,
        )
        .await
    }
}
