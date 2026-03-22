//! Federation API route handlers.
//!
//! - `POST /api/federation/peers` — Register a new federation peer.
//! - `GET  /api/federation/peers` — List all federation peers.
//! - `DELETE /api/federation/peers/{id}` — Remove a federation peer.
//! - `POST /api/federation/sync` — Trigger sync with a peer.
//! - `GET  /api/federation/status` — Get federation status.
//! - `GET  /api/federation/changes/{collection}` — Serve changes to a pulling peer.

use crate::federation::{
    FederationStatus, FederationStore, PeerRequest, SyncRequest, SyncResult,
};
use crate::routes::health::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ── Peer management ─────────────────────────────────────────────────

/// POST /api/federation/peers — Register a new federation peer.
pub async fn create_peer(
    State(state): State<AppState>,
    Json(req): Json<PeerRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let store = get_federation_store(&state)?;

    match store.add_peer(req).await {
        Ok(peer) => Ok((
            StatusCode::CREATED,
            Json(serde_json::to_value(&peer).unwrap()),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )),
    }
}

/// GET /api/federation/peers — List all federation peers.
pub async fn list_peers(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let store = get_federation_store(&state)?;
    let peers = store.list_peers().await;
    Ok(Json(serde_json::to_value(&peers).unwrap()))
}

/// DELETE /api/federation/peers/{id} — Remove a federation peer.
pub async fn delete_peer(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    let store = get_federation_store(&state)?;

    if store.remove_peer(&id).await {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "peer not found"})),
        ))
    }
}

// ── Sync ────────────────────────────────────────────────────────────

/// POST /api/federation/sync — Trigger sync with a specific peer.
pub async fn trigger_sync(
    State(state): State<AppState>,
    Json(body): Json<TriggerSyncBody>,
) -> Result<Json<SyncResult>, (StatusCode, Json<serde_json::Value>)> {
    let store = get_federation_store(&state)?;

    let peer = store.get_peer(&body.peer_id).await.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "peer not found"})),
        )
    })?;

    let storage = state.storage.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "storage not available"})),
        )
    })?;

    let collections_filter = body.collections.as_deref();

    // Update peer status to syncing.
    let _ = store.update_peer_status(
        &peer.id,
        crate::federation::PeerStatus::Syncing,
    ).await;

    let result = crate::federation::sync_with_peer(
        &peer,
        &store,
        storage.as_ref(),
        collections_filter,
    )
    .await;

    // Record the sync result.
    let _ = store.record_sync(result.clone()).await;

    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
pub struct TriggerSyncBody {
    pub peer_id: String,
    pub collections: Option<Vec<String>>,
}

// ── Status ──────────────────────────────────────────────────────────

/// GET /api/federation/status — Get the federation subsystem status.
pub async fn federation_status(
    State(state): State<AppState>,
) -> Result<Json<FederationStatus>, (StatusCode, Json<serde_json::Value>)> {
    let store = get_federation_store(&state)?;
    Ok(Json(store.status().await))
}

// ── Changes endpoint (served to pulling peers) ──────────────────────

#[derive(Debug, Deserialize)]
pub struct ChangesQuery {
    /// ISO 8601 timestamp; return changes newer than this.
    #[serde(default)]
    pub since: String,
}

/// GET /api/federation/changes/{collection} — Serve changes to a pulling peer.
///
/// Returns records in the given collection that were updated after the
/// `since` cursor. This endpoint is called by remote peers during sync.
pub async fn serve_changes(
    State(state): State<AppState>,
    Path(collection): Path<String>,
    Query(query): Query<ChangesQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let storage = state.storage.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "storage not available"})),
        )
    })?;

    // Query all records in the collection, then filter by updated_at > since.
    use crate::storage::StorageAdapter;
    let result = StorageAdapter::list(
        storage.as_ref(),
        "core",
        &collection,
        None,
        crate::storage::Pagination { limit: 1000, offset: 0 },
    )
    .await
    .map_err(|e: anyhow::Error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;

    let since_dt = if query.since.is_empty() {
        None
    } else {
        match chrono::DateTime::parse_from_rfc3339(&query.since) {
            Ok(dt) => Some(dt.with_timezone(&chrono::Utc)),
            Err(_) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "invalid 'since' timestamp, expected RFC 3339 format"})),
                ));
            }
        }
    };

    let changes: Vec<crate::federation::ChangeRecord> = result
        .records
        .into_iter()
        .filter(|r| {
            since_dt
                .map(|since| r.updated_at > since)
                .unwrap_or(true)
        })
        .map(|r| crate::federation::ChangeRecord {
            id: r.id,
            collection: r.collection,
            operation: crate::federation::ChangeOperation::Update,
            data: Some(r.data),
            version: r.version,
            timestamp: r.updated_at,
        })
        .collect();

    let cursor = chrono::Utc::now().to_rfc3339();

    Ok(Json(serde_json::json!({
        "changes": changes,
        "cursor": cursor,
    })))
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Extract the federation store from app state.
fn get_federation_store(
    state: &AppState,
) -> Result<Arc<FederationStore>, (StatusCode, Json<serde_json::Value>)> {
    state.federation_store.as_ref().cloned().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "federation store not available"})),
        )
    })
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::default_app_state;

    #[tokio::test]
    async fn federation_status_returns_empty_initially() {
        let state = default_app_state();
        // The OnceLock store is shared across tests in the same process,
        // so we test the store directly for isolation.
        let store = FederationStore::new();
        let status = store.status().await;
        assert!(!status.enabled);
        assert_eq!(status.peer_count, 0);
    }

    #[tokio::test]
    async fn create_and_list_peers_via_store() {
        let store = FederationStore::new();
        let peer = store
            .add_peer(PeerRequest {
                name: "Test Peer".into(),
                endpoint: "https://test:3750".into(),
                collections: vec!["events".into()],
                ca_cert_path: None,
                client_cert_path: None,
                client_key_path: None,
            })
            .await
            .unwrap();

        let peers = store.list_peers().await;
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].id, peer.id);
    }

    #[tokio::test]
    async fn delete_peer_via_store() {
        let store = FederationStore::new();
        let peer = store
            .add_peer(PeerRequest {
                name: "Delete me".into(),
                endpoint: "https://delete:3750".into(),
                collections: vec![],
                ca_cert_path: None,
                client_cert_path: None,
                client_key_path: None,
            })
            .await
            .unwrap();

        assert!(store.remove_peer(&peer.id).await);
        assert!(store.get_peer(&peer.id).await.is_none());
    }
}
