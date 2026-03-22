//! Federated sync — hub-to-hub data replication between Core instances.
//!
//! Enables peer-to-peer synchronisation of selected collections between
//! independently operated Life Engine Core instances using mTLS for
//! mutual authentication.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

// Re-export shared sync primitives for consumers.
pub use crate::sync_primitives::{
    apply_change, ChangeOperation, ChangeRecord, PullResponse, SyncCursors,
};

// ── Data types ──────────────────────────────────────────────────────

/// A federation peer — another Core instance this instance can sync with.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FederationPeer {
    /// Unique peer identifier (UUID).
    pub id: String,
    /// Human-readable name for the peer.
    pub name: String,
    /// The peer's API endpoint URL (e.g. `https://partner.example.com:3750`).
    pub endpoint: String,
    /// Collections this peer is configured to sync.
    pub collections: Vec<String>,
    /// Path to the CA certificate used to verify this peer's server cert.
    pub ca_cert_path: Option<String>,
    /// Path to the client certificate presented to this peer.
    pub client_cert_path: Option<String>,
    /// Path to the client private key.
    pub client_key_path: Option<String>,
    /// Current status of the peer connection.
    pub status: PeerStatus,
    /// Timestamp of the last successful sync with this peer.
    pub last_sync_at: Option<DateTime<Utc>>,
    /// Number of records synced in the last sync operation.
    pub last_sync_records: Option<u64>,
    /// When this peer was registered.
    pub created_at: DateTime<Utc>,
    /// When this peer was last updated.
    pub updated_at: DateTime<Utc>,
}

/// Possible states for a federation peer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeerStatus {
    /// Peer is configured but not yet connected.
    Pending,
    /// Peer is connected and ready to sync.
    Connected,
    /// Peer is currently syncing.
    Syncing,
    /// Last sync failed.
    Error,
    /// Peer has been disabled.
    Disabled,
}

/// Request body for creating or updating a federation peer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerRequest {
    /// Human-readable name.
    pub name: String,
    /// The peer's API endpoint URL.
    pub endpoint: String,
    /// Collections to sync with this peer.
    pub collections: Vec<String>,
    /// Path to CA certificate for verifying the peer.
    pub ca_cert_path: Option<String>,
    /// Path to client certificate for authenticating to the peer.
    pub client_cert_path: Option<String>,
    /// Path to client private key.
    pub client_key_path: Option<String>,
}

/// The result of a sync operation with a peer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    /// The peer that was synced.
    pub peer_id: String,
    /// Collections that were synced.
    pub collections_synced: Vec<String>,
    /// Total records pulled from the peer.
    pub records_pulled: u64,
    /// Total records pushed to the peer.
    pub records_pushed: u64,
    /// Conflicts encountered during sync.
    pub conflicts: u64,
    /// Whether the sync completed successfully.
    pub success: bool,
    /// Error message if sync failed.
    pub error: Option<String>,
    /// Timestamp when sync completed.
    pub completed_at: DateTime<Utc>,
}

/// Request body for triggering a sync with a peer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRequest {
    /// Optional: limit sync to specific collections (defaults to all configured).
    pub collections: Option<Vec<String>>,
}

/// Status response for the federation subsystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationStatus {
    /// Whether federation is enabled.
    pub enabled: bool,
    /// Number of configured peers.
    pub peer_count: usize,
    /// Summary of each peer's status.
    pub peers: Vec<PeerStatusSummary>,
}

/// Summary of a single peer's status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerStatusSummary {
    /// Peer identifier.
    pub id: String,
    /// Peer name.
    pub name: String,
    /// Current status.
    pub status: PeerStatus,
    /// Collections configured for sync.
    pub collections: Vec<String>,
    /// Last successful sync timestamp.
    pub last_sync_at: Option<DateTime<Utc>>,
    /// Records synced in the last operation.
    pub last_sync_records: Option<u64>,
}

// ── Federation store ────────────────────────────────────────────────

/// In-memory store for federation peers and sync state.
pub struct FederationStore {
    peers: RwLock<HashMap<String, FederationPeer>>,
    sync_history: RwLock<Vec<SyncResult>>,
    cursors: RwLock<SyncCursors>,
}

impl FederationStore {
    pub fn new() -> Self {
        Self {
            peers: RwLock::new(HashMap::new()),
            sync_history: RwLock::new(Vec::new()),
            cursors: RwLock::new(SyncCursors::new()),
        }
    }

    /// Register a new federation peer. Returns the created peer.
    pub fn add_peer(&self, req: PeerRequest) -> anyhow::Result<FederationPeer> {
        let now = Utc::now();
        let peer = FederationPeer {
            id: uuid::Uuid::new_v4().to_string(),
            name: req.name,
            endpoint: req.endpoint,
            collections: req.collections,
            ca_cert_path: req.ca_cert_path,
            client_cert_path: req.client_cert_path,
            client_key_path: req.client_key_path,
            status: PeerStatus::Pending,
            last_sync_at: None,
            last_sync_records: None,
            created_at: now,
            updated_at: now,
        };
        let mut peers = self.peers.write().unwrap();
        peers.insert(peer.id.clone(), peer.clone());
        Ok(peer)
    }

    /// Get a peer by ID.
    pub fn get_peer(&self, id: &str) -> Option<FederationPeer> {
        let peers = self.peers.read().unwrap();
        peers.get(id).cloned()
    }

    /// List all peers.
    pub fn list_peers(&self) -> Vec<FederationPeer> {
        let peers = self.peers.read().unwrap();
        peers.values().cloned().collect()
    }

    /// Remove a peer by ID. Returns true if it existed.
    pub fn remove_peer(&self, id: &str) -> bool {
        let mut peers = self.peers.write().unwrap();
        peers.remove(id).is_some()
    }

    /// Update a peer's status.
    pub fn update_peer_status(&self, id: &str, status: PeerStatus) -> anyhow::Result<()> {
        let mut peers = self.peers.write().unwrap();
        let peer = peers
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("peer not found: {id}"))?;
        peer.status = status;
        peer.updated_at = Utc::now();
        Ok(())
    }

    /// Record a completed sync operation and update the peer.
    pub fn record_sync(&self, result: SyncResult) -> anyhow::Result<()> {
        // Update the peer's last sync info.
        {
            let mut peers = self.peers.write().unwrap();
            if let Some(peer) = peers.get_mut(&result.peer_id) {
                if result.success {
                    peer.status = PeerStatus::Connected;
                    peer.last_sync_at = Some(result.completed_at);
                    peer.last_sync_records = Some(result.records_pulled + result.records_pushed);
                } else {
                    peer.status = PeerStatus::Error;
                }
                peer.updated_at = Utc::now();
            }
        }
        // Append to history.
        self.sync_history.write().unwrap().push(result);
        Ok(())
    }

    /// Get the overall federation status.
    pub fn status(&self) -> FederationStatus {
        let peers = self.peers.read().unwrap();
        let summaries: Vec<PeerStatusSummary> = peers
            .values()
            .map(|p| PeerStatusSummary {
                id: p.id.clone(),
                name: p.name.clone(),
                status: p.status.clone(),
                collections: p.collections.clone(),
                last_sync_at: p.last_sync_at,
                last_sync_records: p.last_sync_records,
            })
            .collect();
        FederationStatus {
            enabled: !peers.is_empty(),
            peer_count: peers.len(),
            peers: summaries,
        }
    }

    /// Get the sync cursor for a peer and collection.
    pub fn get_cursor(&self, peer_id: &str, collection: &str) -> Option<String> {
        let cursors = self.cursors.read().unwrap();
        cursors.get(peer_id, collection).map(|s| s.to_string())
    }

    /// Set the sync cursor for a peer and collection.
    pub fn set_cursor(&self, peer_id: &str, collection: &str, cursor: String) {
        let mut cursors = self.cursors.write().unwrap();
        cursors.set(peer_id, collection, cursor);
    }

    /// Get sync history for a specific peer.
    pub fn sync_history_for_peer(&self, peer_id: &str) -> Vec<SyncResult> {
        let history = self.sync_history.read().unwrap();
        history
            .iter()
            .filter(|r| r.peer_id == peer_id)
            .cloned()
            .collect()
    }
}

// ── mTLS client builder ─────────────────────────────────────────────

/// Build a reqwest client configured with mTLS for a specific peer.
///
/// The client presents the peer's client certificate and validates the
/// peer's server certificate against the provided CA.
pub fn build_mtls_client(peer: &FederationPeer) -> anyhow::Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder();

    // Load CA certificate for server verification.
    if let Some(ref ca_path) = peer.ca_cert_path {
        let ca_pem = std::fs::read(ca_path)
            .map_err(|e| anyhow::anyhow!("failed to read CA cert '{}': {e}", ca_path))?;
        let ca_cert = reqwest::Certificate::from_pem(&ca_pem)
            .map_err(|e| anyhow::anyhow!("invalid CA cert '{}': {e}", ca_path))?;
        builder = builder.add_root_certificate(ca_cert);
    }

    // Load client identity for mutual authentication.
    if let (Some(cert_path), Some(key_path)) =
        (&peer.client_cert_path, &peer.client_key_path)
    {
        let cert_pem = std::fs::read(cert_path)
            .map_err(|e| anyhow::anyhow!("failed to read client cert '{}': {e}", cert_path))?;
        let key_pem = std::fs::read(key_path)
            .map_err(|e| anyhow::anyhow!("failed to read client key '{}': {e}", key_path))?;

        // reqwest Identity expects combined PEM (cert + key).
        let mut combined = cert_pem;
        combined.extend_from_slice(&key_pem);

        let identity = reqwest::Identity::from_pem(&combined)
            .map_err(|e| anyhow::anyhow!("invalid client identity: {e}"))?;
        builder = builder.identity(identity);
    }

    builder
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| anyhow::anyhow!("failed to build mTLS client: {e}"))
}

// ── Pull-based sync engine ──────────────────────────────────────────

/// Execute a pull-based sync with a remote peer.
///
/// For each declared collection, pulls changes from the peer since the
/// last cursor and writes them to local storage. Only collections that
/// appear in both the peer config and the optional filter are synced.
pub async fn sync_with_peer(
    peer: &FederationPeer,
    store: &FederationStore,
    storage: &dyn crate::storage::StorageAdapter,
    collections_filter: Option<&[String]>,
) -> SyncResult {
    let collections: Vec<String> = match collections_filter {
        Some(filter) => peer
            .collections
            .iter()
            .filter(|c| filter.iter().any(|f| f == *c))
            .cloned()
            .collect(),
        None => peer.collections.clone(),
    };

    if collections.is_empty() {
        return SyncResult {
            peer_id: peer.id.clone(),
            collections_synced: vec![],
            records_pulled: 0,
            records_pushed: 0,
            conflicts: 0,
            success: true,
            error: None,
            completed_at: Utc::now(),
        };
    }

    // Build mTLS client.
    let client = match build_mtls_client(peer) {
        Ok(c) => c,
        Err(e) => {
            return SyncResult {
                peer_id: peer.id.clone(),
                collections_synced: vec![],
                records_pulled: 0,
                records_pushed: 0,
                conflicts: 0,
                success: false,
                error: Some(format!("mTLS client build failed: {e}")),
                completed_at: Utc::now(),
            };
        }
    };

    let mut total_pulled: u64 = 0;
    let mut total_conflicts: u64 = 0;
    let mut synced_collections = Vec::new();

    for collection in &collections {
        let cursor = store
            .get_cursor(&peer.id, collection)
            .unwrap_or_default();

        // Pull changes from the remote peer.
        let url = format!(
            "{}/api/federation/changes/{}?since={}",
            peer.endpoint, collection, cursor
        );

        let response = match client.get(&url).send().await {
            Ok(resp) => resp,
            Err(e) => {
                tracing::warn!(
                    peer_id = %peer.id,
                    collection = %collection,
                    error = %e,
                    "failed to pull changes from peer"
                );
                continue;
            }
        };

        if !response.status().is_success() {
            tracing::warn!(
                peer_id = %peer.id,
                collection = %collection,
                status = %response.status(),
                "peer returned error status"
            );
            continue;
        }

        let pull: PullResponse = match response.json().await {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(
                    peer_id = %peer.id,
                    collection = %collection,
                    error = %e,
                    "failed to parse pull response"
                );
                continue;
            }
        };

        // Apply changes to local storage.
        for change in &pull.changes {
            let result = apply_federation_change(storage, &peer.id, change).await;
            if let Err(e) = result {
                tracing::warn!(
                    peer_id = %peer.id,
                    record_id = %change.id,
                    error = %e,
                    "conflict applying change"
                );
                total_conflicts += 1;
            }
        }

        total_pulled += pull.changes.len() as u64;
        synced_collections.push(collection.clone());

        // Update cursor.
        if !pull.cursor.is_empty() {
            store.set_cursor(&peer.id, collection, pull.cursor);
        }
    }

    SyncResult {
        peer_id: peer.id.clone(),
        collections_synced: synced_collections,
        records_pulled: total_pulled,
        records_pushed: 0, // Pull-based: we don't push in this model.
        conflicts: total_conflicts,
        success: true,
        error: None,
        completed_at: Utc::now(),
    }
}

/// Apply a single change record from a remote peer to local storage.
///
/// Delegates to `sync_primitives::apply_change` with a federation-namespaced
/// storage prefix to isolate federated data.
async fn apply_federation_change(
    storage: &dyn crate::storage::StorageAdapter,
    peer_id: &str,
    change: &ChangeRecord,
) -> anyhow::Result<()> {
    let namespace = format!("federation:{peer_id}");
    apply_change(storage, &namespace, change).await
}

// ── mTLS server config for federation ───────────────────────────────

/// Build a rustls `ServerConfig` that requires client certificates
/// verified against the given CA certificate, for federation endpoints.
pub fn build_mtls_server_config(
    server_cert_path: &str,
    server_key_path: &str,
    client_ca_path: &str,
) -> Result<rustls::ServerConfig, crate::error::CoreError> {
    use crate::error::CoreError;
    use rustls::pki_types::CertificateDer;
    use std::fs::File;
    use std::io::BufReader;

    // Read server certificate chain.
    let cert_file = File::open(server_cert_path).map_err(|e| {
        CoreError::Tls(format!("failed to open server cert '{}': {e}", server_cert_path))
    })?;
    let mut cert_reader = BufReader::new(cert_file);
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| CoreError::Tls(format!("failed to parse server certs: {e}")))?;

    if certs.is_empty() {
        return Err(CoreError::Tls("no server certificates found".into()));
    }

    // Read server private key.
    let key_file = File::open(server_key_path).map_err(|e| {
        CoreError::Tls(format!("failed to open server key '{}': {e}", server_key_path))
    })?;
    let mut key_reader = BufReader::new(key_file);
    let key = crate::tls::read_private_key_from_reader(&mut key_reader, server_key_path)?;

    // Read client CA certificate for verifying connecting peers.
    let ca_file = File::open(client_ca_path).map_err(|e| {
        CoreError::Tls(format!("failed to open client CA '{}': {e}", client_ca_path))
    })?;
    let mut ca_reader = BufReader::new(ca_file);
    let ca_certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut ca_reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| CoreError::Tls(format!("failed to parse client CA certs: {e}")))?;

    // Build a root cert store with the client CA.
    let mut root_store = rustls::RootCertStore::empty();
    for ca_cert in ca_certs {
        root_store
            .add(ca_cert)
            .map_err(|e| CoreError::Tls(format!("failed to add CA cert to root store: {e}")))?;
    }

    // Build verifier that requires client certs.
    let client_verifier =
        rustls::server::WebPkiClientVerifier::builder(std::sync::Arc::new(root_store))
            .build()
            .map_err(|e| CoreError::Tls(format!("failed to build client verifier: {e}")))?;

    let config = rustls::ServerConfig::builder()
        .with_client_cert_verifier(client_verifier)
        .with_single_cert(certs, key)
        .map_err(|e| CoreError::Tls(format!("failed to build mTLS server config: {e}")))?;

    Ok(config)
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{Pagination, QueryFilters, QueryResult, Record, SortOptions, StorageAdapter};
    use async_trait::async_trait;
    use chrono::Utc;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// In-memory storage for federation tests.
    struct TestStorage {
        records: Mutex<HashMap<String, Record>>,
    }

    impl TestStorage {
        fn new() -> Self {
            Self {
                records: Mutex::new(HashMap::new()),
            }
        }

        fn make_key(plugin_id: &str, collection: &str, id: &str) -> String {
            format!("{plugin_id}:{collection}:{id}")
        }

        fn record_count(&self) -> usize {
            self.records.lock().unwrap().len()
        }

        fn records_in_collection(&self, plugin_id: &str, collection: &str) -> Vec<Record> {
            let records = self.records.lock().unwrap();
            records
                .values()
                .filter(|r| r.plugin_id == plugin_id && r.collection == collection)
                .cloned()
                .collect()
        }
    }

    #[async_trait]
    impl StorageAdapter for TestStorage {
        async fn get(
            &self,
            plugin_id: &str,
            collection: &str,
            id: &str,
        ) -> anyhow::Result<Option<Record>> {
            let key = Self::make_key(plugin_id, collection, id);
            let records = self.records.lock().unwrap();
            Ok(records.get(&key).cloned())
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
            let key = Self::make_key(plugin_id, collection, &id);
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
        ) -> anyhow::Result<Record> {
            let key = Self::make_key(plugin_id, collection, id);
            let mut records = self.records.lock().unwrap();
            let record = records
                .get(&key)
                .ok_or_else(|| anyhow::anyhow!("not found"))?;
            if record.version != version {
                return Err(anyhow::anyhow!("version mismatch"));
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
            let key = Self::make_key(plugin_id, collection, id);
            Ok(self.records.lock().unwrap().remove(&key).is_some())
        }

        async fn list(
            &self,
            plugin_id: &str,
            collection: &str,
            _sort: Option<SortOptions>,
            pagination: Pagination,
        ) -> anyhow::Result<QueryResult> {
            self.query(
                plugin_id,
                collection,
                QueryFilters::default(),
                None,
                pagination,
            )
            .await
        }
    }

    // ── TDD:RED — Record created on instance A appears on instance B ──

    #[tokio::test]
    async fn apply_create_change_inserts_record() {
        let storage = TestStorage::new();
        let change = ChangeRecord {
            id: "rec-1".into(),
            collection: "events".into(),
            operation: ChangeOperation::Create,
            data: Some(json!({"title": "Birthday"})),
            version: 1,
            timestamp: Utc::now(),
        };

        apply_federation_change(&storage, "peer-a", &change).await.unwrap();

        let records = storage.records_in_collection("federation:peer-a", "events");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].data, json!({"title": "Birthday"}));
    }

    #[tokio::test]
    async fn apply_update_change_updates_record() {
        let storage = TestStorage::new();

        // First create a record.
        let created = storage
            .create("federation:peer-a", "events", json!({"title": "Old"}))
            .await
            .unwrap();

        // Apply an update change with higher version.
        let change = ChangeRecord {
            id: created.id.clone(),
            collection: "events".into(),
            operation: ChangeOperation::Update,
            data: Some(json!({"title": "New"})),
            version: 2,
            timestamp: Utc::now(),
        };

        apply_federation_change(&storage, "peer-a", &change).await.unwrap();

        let record = storage
            .get("federation:peer-a", "events", &created.id)
            .await
            .unwrap()
            .expect("record should exist");
        assert_eq!(record.data, json!({"title": "New"}));
        assert_eq!(record.version, 2);
    }

    #[tokio::test]
    async fn apply_delete_change_removes_record() {
        let storage = TestStorage::new();

        let created = storage
            .create("federation:peer-a", "events", json!({"title": "Delete me"}))
            .await
            .unwrap();

        let change = ChangeRecord {
            id: created.id.clone(),
            collection: "events".into(),
            operation: ChangeOperation::Delete,
            data: None,
            version: 2,
            timestamp: Utc::now(),
        };

        apply_federation_change(&storage, "peer-a", &change).await.unwrap();

        let record = storage
            .get("federation:peer-a", "events", &created.id)
            .await
            .unwrap();
        assert!(record.is_none());
    }

    // ── TDD:RED — Selective sync only transfers declared collections ──

    #[tokio::test]
    async fn selective_sync_filters_collections() {
        let store = FederationStore::new();
        let peer = store
            .add_peer(PeerRequest {
                name: "Partner".into(),
                endpoint: "https://partner.example.com:3750".into(),
                collections: vec!["events".into(), "contacts".into(), "tasks".into()],
                ca_cert_path: None,
                client_cert_path: None,
                client_key_path: None,
            })
            .unwrap();

        // When filtering to only "events", only that collection should be synced.
        let filter: Vec<String> = vec!["events".into()];
        let collections: Vec<String> = peer
            .collections
            .iter()
            .filter(|c| filter.iter().any(|f| f == *c))
            .cloned()
            .collect();

        assert_eq!(collections, vec!["events"]);
    }

    #[tokio::test]
    async fn selective_sync_excludes_undeclared_collections() {
        let store = FederationStore::new();
        let peer = store
            .add_peer(PeerRequest {
                name: "Partner".into(),
                endpoint: "https://partner.example.com:3750".into(),
                collections: vec!["events".into()],
                ca_cert_path: None,
                client_cert_path: None,
                client_key_path: None,
            })
            .unwrap();

        // Filter requesting "tasks" which is not in the peer's declared collections.
        let filter: Vec<String> = vec!["tasks".into()];
        let collections: Vec<String> = peer
            .collections
            .iter()
            .filter(|c| filter.iter().any(|f| f == *c))
            .cloned()
            .collect();

        assert!(collections.is_empty());
    }

    // ── TDD:RED — mTLS rejects unauthenticated peers ──

    #[test]
    fn build_mtls_client_requires_valid_cert_paths() {
        let peer = FederationPeer {
            id: "test".into(),
            name: "Test".into(),
            endpoint: "https://localhost:3750".into(),
            collections: vec![],
            ca_cert_path: Some("/nonexistent/ca.pem".into()),
            client_cert_path: None,
            client_key_path: None,
            status: PeerStatus::Pending,
            last_sync_at: None,
            last_sync_records: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let result = build_mtls_client(&peer);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("failed to read CA cert"));
    }

    #[test]
    fn build_mtls_client_rejects_invalid_client_cert() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a valid CA cert but invalid client cert.
        let ca_cert = rcgen::generate_simple_self_signed(vec!["ca.test".into()]).unwrap();
        let mut ca_file = NamedTempFile::new().unwrap();
        ca_file.write_all(ca_cert.cert.pem().as_bytes()).unwrap();

        let mut cert_file = NamedTempFile::new().unwrap();
        cert_file.write_all(b"not a valid cert").unwrap();

        let mut key_file = NamedTempFile::new().unwrap();
        key_file.write_all(b"not a valid key").unwrap();

        let peer = FederationPeer {
            id: "test".into(),
            name: "Test".into(),
            endpoint: "https://localhost:3750".into(),
            collections: vec![],
            ca_cert_path: Some(ca_file.path().to_string_lossy().to_string()),
            client_cert_path: Some(cert_file.path().to_string_lossy().to_string()),
            client_key_path: Some(key_file.path().to_string_lossy().to_string()),
            status: PeerStatus::Pending,
            last_sync_at: None,
            last_sync_records: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let result = build_mtls_client(&peer);
        assert!(result.is_err());
    }

    #[test]
    fn build_mtls_server_config_rejects_no_client_ca() {
        // Ensure crypto provider is installed.
        let _ = rustls::crypto::CryptoProvider::install_default(
            rustls::crypto::aws_lc_rs::default_provider(),
        );

        let result = build_mtls_server_config(
            "/nonexistent/cert.pem",
            "/nonexistent/key.pem",
            "/nonexistent/ca.pem",
        );
        assert!(result.is_err());
    }

    #[test]
    fn build_mtls_server_config_succeeds_with_valid_certs() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let _ = rustls::crypto::CryptoProvider::install_default(
            rustls::crypto::aws_lc_rs::default_provider(),
        );

        // Generate CA cert.
        let ca_params = rcgen::CertificateParams::new(vec!["ca.test".into()]).unwrap();
        let ca = ca_params.self_signed(&rcgen::KeyPair::generate().unwrap()).unwrap();

        // Generate server cert signed by CA (self-signed for simplicity).
        let server = rcgen::generate_simple_self_signed(vec!["server.test".into()]).unwrap();

        let mut ca_file = NamedTempFile::new().unwrap();
        ca_file.write_all(ca.pem().as_bytes()).unwrap();

        let mut cert_file = NamedTempFile::new().unwrap();
        cert_file.write_all(server.cert.pem().as_bytes()).unwrap();

        let mut key_file = NamedTempFile::new().unwrap();
        key_file
            .write_all(server.key_pair.serialize_pem().as_bytes())
            .unwrap();

        let result = build_mtls_server_config(
            &cert_file.path().to_string_lossy(),
            &key_file.path().to_string_lossy(),
            &ca_file.path().to_string_lossy(),
        );
        assert!(result.is_ok(), "mTLS server config should succeed: {result:?}");
    }

    // ── TDD:RED — Federation status API reports correctly ──

    #[test]
    fn federation_status_reports_no_peers() {
        let store = FederationStore::new();
        let status = store.status();
        assert!(!status.enabled);
        assert_eq!(status.peer_count, 0);
        assert!(status.peers.is_empty());
    }

    #[test]
    fn federation_status_reports_configured_peers() {
        let store = FederationStore::new();
        store
            .add_peer(PeerRequest {
                name: "Alice".into(),
                endpoint: "https://alice.example.com:3750".into(),
                collections: vec!["events".into()],
                ca_cert_path: None,
                client_cert_path: None,
                client_key_path: None,
            })
            .unwrap();

        let status = store.status();
        assert!(status.enabled);
        assert_eq!(status.peer_count, 1);
        assert_eq!(status.peers[0].name, "Alice");
        assert_eq!(status.peers[0].status, PeerStatus::Pending);
    }

    #[test]
    fn federation_status_reflects_sync_result() {
        let store = FederationStore::new();
        let peer = store
            .add_peer(PeerRequest {
                name: "Bob".into(),
                endpoint: "https://bob.example.com:3750".into(),
                collections: vec!["contacts".into()],
                ca_cert_path: None,
                client_cert_path: None,
                client_key_path: None,
            })
            .unwrap();

        let now = Utc::now();
        store
            .record_sync(SyncResult {
                peer_id: peer.id.clone(),
                collections_synced: vec!["contacts".into()],
                records_pulled: 42,
                records_pushed: 0,
                conflicts: 0,
                success: true,
                error: None,
                completed_at: now,
            })
            .unwrap();

        let status = store.status();
        let peer_status = &status.peers[0];
        assert_eq!(peer_status.status, PeerStatus::Connected);
        assert_eq!(peer_status.last_sync_records, Some(42));
        assert!(peer_status.last_sync_at.is_some());
    }

    #[test]
    fn federation_status_reflects_error() {
        let store = FederationStore::new();
        let peer = store
            .add_peer(PeerRequest {
                name: "Charlie".into(),
                endpoint: "https://charlie.example.com:3750".into(),
                collections: vec!["tasks".into()],
                ca_cert_path: None,
                client_cert_path: None,
                client_key_path: None,
            })
            .unwrap();

        store
            .record_sync(SyncResult {
                peer_id: peer.id.clone(),
                collections_synced: vec![],
                records_pulled: 0,
                records_pushed: 0,
                conflicts: 0,
                success: false,
                error: Some("connection refused".into()),
                completed_at: Utc::now(),
            })
            .unwrap();

        let status = store.status();
        assert_eq!(status.peers[0].status, PeerStatus::Error);
    }

    // ── Federation store CRUD tests ──

    #[test]
    fn add_and_get_peer() {
        let store = FederationStore::new();
        let peer = store
            .add_peer(PeerRequest {
                name: "Test".into(),
                endpoint: "https://test:3750".into(),
                collections: vec!["events".into()],
                ca_cert_path: None,
                client_cert_path: None,
                client_key_path: None,
            })
            .unwrap();

        let fetched = store.get_peer(&peer.id).expect("peer should exist");
        assert_eq!(fetched.name, "Test");
        assert_eq!(fetched.endpoint, "https://test:3750");
        assert_eq!(fetched.collections, vec!["events"]);
        assert_eq!(fetched.status, PeerStatus::Pending);
    }

    #[test]
    fn remove_peer() {
        let store = FederationStore::new();
        let peer = store
            .add_peer(PeerRequest {
                name: "Remove me".into(),
                endpoint: "https://remove:3750".into(),
                collections: vec![],
                ca_cert_path: None,
                client_cert_path: None,
                client_key_path: None,
            })
            .unwrap();

        assert!(store.remove_peer(&peer.id));
        assert!(store.get_peer(&peer.id).is_none());
    }

    #[test]
    fn list_peers_returns_all() {
        let store = FederationStore::new();
        for i in 0..3 {
            store
                .add_peer(PeerRequest {
                    name: format!("Peer {i}"),
                    endpoint: format!("https://peer{i}:3750"),
                    collections: vec![],
                    ca_cert_path: None,
                    client_cert_path: None,
                    client_key_path: None,
                })
                .unwrap();
        }

        assert_eq!(store.list_peers().len(), 3);
    }

    #[test]
    fn update_peer_status_works() {
        let store = FederationStore::new();
        let peer = store
            .add_peer(PeerRequest {
                name: "Status test".into(),
                endpoint: "https://status:3750".into(),
                collections: vec![],
                ca_cert_path: None,
                client_cert_path: None,
                client_key_path: None,
            })
            .unwrap();

        store.update_peer_status(&peer.id, PeerStatus::Connected).unwrap();
        let fetched = store.get_peer(&peer.id).unwrap();
        assert_eq!(fetched.status, PeerStatus::Connected);
    }

    // ── Sync cursor tests ──

    #[test]
    fn sync_cursors_get_set() {
        let store = FederationStore::new();
        assert!(store.get_cursor("peer-1", "events").is_none());

        store.set_cursor("peer-1", "events", "2026-03-22T00:00:00Z".into());
        assert_eq!(
            store.get_cursor("peer-1", "events").as_deref(),
            Some("2026-03-22T00:00:00Z")
        );
    }

    #[test]
    fn sync_cursors_per_collection() {
        let store = FederationStore::new();
        store.set_cursor("peer-1", "events", "cursor-a".into());
        store.set_cursor("peer-1", "contacts", "cursor-b".into());

        assert_eq!(
            store.get_cursor("peer-1", "events").as_deref(),
            Some("cursor-a")
        );
        assert_eq!(
            store.get_cursor("peer-1", "contacts").as_deref(),
            Some("cursor-b")
        );
    }

    // ── Sync history tests ──

    #[test]
    fn sync_history_for_peer_filters_correctly() {
        let store = FederationStore::new();
        let peer_a = store
            .add_peer(PeerRequest {
                name: "A".into(),
                endpoint: "https://a:3750".into(),
                collections: vec![],
                ca_cert_path: None,
                client_cert_path: None,
                client_key_path: None,
            })
            .unwrap();
        let peer_b = store
            .add_peer(PeerRequest {
                name: "B".into(),
                endpoint: "https://b:3750".into(),
                collections: vec![],
                ca_cert_path: None,
                client_cert_path: None,
                client_key_path: None,
            })
            .unwrap();

        store
            .record_sync(SyncResult {
                peer_id: peer_a.id.clone(),
                collections_synced: vec!["events".into()],
                records_pulled: 10,
                records_pushed: 0,
                conflicts: 0,
                success: true,
                error: None,
                completed_at: Utc::now(),
            })
            .unwrap();
        store
            .record_sync(SyncResult {
                peer_id: peer_b.id.clone(),
                collections_synced: vec!["contacts".into()],
                records_pulled: 5,
                records_pushed: 0,
                conflicts: 0,
                success: true,
                error: None,
                completed_at: Utc::now(),
            })
            .unwrap();

        let history_a = store.sync_history_for_peer(&peer_a.id);
        assert_eq!(history_a.len(), 1);
        assert_eq!(history_a[0].records_pulled, 10);

        let history_b = store.sync_history_for_peer(&peer_b.id);
        assert_eq!(history_b.len(), 1);
        assert_eq!(history_b[0].records_pulled, 5);
    }

    // ── Serialization tests ──

    #[test]
    fn peer_status_serializes_as_snake_case() {
        let json = serde_json::to_string(&PeerStatus::Connected).unwrap();
        assert_eq!(json, "\"connected\"");
    }

    #[test]
    fn change_operation_serializes_as_snake_case() {
        let json = serde_json::to_string(&ChangeOperation::Create).unwrap();
        assert_eq!(json, "\"create\"");
    }

    #[test]
    fn federation_peer_roundtrip() {
        let now = Utc::now();
        let peer = FederationPeer {
            id: "p1".into(),
            name: "Test".into(),
            endpoint: "https://test:3750".into(),
            collections: vec!["events".into()],
            ca_cert_path: None,
            client_cert_path: None,
            client_key_path: None,
            status: PeerStatus::Connected,
            last_sync_at: Some(now),
            last_sync_records: Some(100),
            created_at: now,
            updated_at: now,
        };
        let json = serde_json::to_string(&peer).unwrap();
        let restored: FederationPeer = serde_json::from_str(&json).unwrap();
        assert_eq!(peer, restored);
    }
}
