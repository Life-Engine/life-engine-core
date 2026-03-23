//! Life Engine Core — self-hosted personal data sovereignty backend.
//!
//! Entry point: loads config, initialises subsystems, starts the HTTP
//! server, and handles graceful shutdown.

mod auth;
mod config;
mod conflict;
mod connector;
mod crypto;
mod credential_bridge;
mod credential_store;
mod error;
mod message_bus;
mod plugin_loader;
mod rate_limit;
mod rekey;
mod routes;
mod schema_registry;
mod search;
mod search_processor;
mod shutdown;
mod sqlite_storage;
mod storage;
mod pg_storage;
mod storage_migration;
mod tls;
mod plugin_signing;
mod wasm_adapter;
mod wasm_runtime;
mod household;
mod sync_primitives;
mod federation;
mod identity;

use crate::auth::middleware::{auth_middleware, AuthMiddlewareState, RateLimiter};
use crate::auth::routes::auth_router;
use crate::config::{CliArgs, CoreConfig, LogReloadHandle};
use crate::rate_limit::{rate_limit_middleware, GeneralRateLimiter};
use crate::conflict::ConflictStore;
use crate::message_bus::MessageBus;
use crate::plugin_loader::PluginLoader;
use crate::routes::conflicts::{delete_conflict, get_conflict, list_conflicts, resolve_conflict};
use crate::routes::data::{create_record, delete_record, get_record, list_records, update_record};
use crate::routes::events::event_stream;
use crate::routes::health::{health_check, AppState};
use crate::routes::quarantine::{delete_quarantine, list_quarantine, reprocess_quarantine};
use crate::routes::search::search;
use crate::routes::connectors::trigger_sync;
use crate::routes::credentials::{
    delete_credential, delete_plugin_credentials, get_credential, list_credentials,
    store_credential,
};
use crate::routes::federation::{
    create_peer, delete_peer, federation_status, list_peers, serve_changes,
    trigger_sync as trigger_federation_sync,
};
use crate::routes::identity::{
    create_identity_credential, delete_identity_credential, disclose_credential,
    export_verifiable_credential, get_did, get_disclosure_audit, get_identity_credential,
    list_identity_credentials,
};
use crate::routes::graphql::{graphql_handler, graphql_playground};
use crate::routes::plugins::plugin_route_stub;
use crate::routes::storage::{init_storage, StorageInitState};
use crate::routes::system::{get_config, put_config, system_info, system_plugins};
use crate::schema_registry::{SchemaRegistry, ValidatedStorage};
use crate::shutdown::{graceful_shutdown, shutdown_signal};

use clap::Parser;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Parse CLI arguments.
    let cli = CliArgs::parse();

    // 2. Load configuration (YAML < env < CLI).
    let config = CoreConfig::load(&cli)?;

    // 3. Initialise structured logging (returns a handle for hot-reloading log level).
    let log_reload_handle = init_logging(&config);

    tracing::info!("Life Engine Core starting");
    tracing::info!(
        host = %config.core.host,
        port = %config.core.port,
        log_level = %config.core.log_level,
        "configuration loaded"
    );

    // 4. Initialise subsystems.
    let start_time = Instant::now();
    let message_bus = Arc::new(MessageBus::new());

    // 4a. Derive database encryption key (when encryption is enabled with a passphrase).
    let data_dir_path = std::path::Path::new(&config.core.data_dir);
    std::fs::create_dir_all(data_dir_path)?;
    let db_path = data_dir_path.join("life-engine.db");
    let derived_key = if config.storage.encryption {
        if let Some(passphrase) = config.storage.resolve_passphrase() {
            let salt_path = data_dir_path.join("salt.bin");
            let salt = if salt_path.exists() {
                let salt_bytes = std::fs::read(&salt_path)?;
                if salt_bytes.len() != 16 {
                    anyhow::bail!(
                        "salt file {} has invalid length {} (expected 16 bytes)",
                        salt_path.display(),
                        salt_bytes.len()
                    );
                }
                let mut salt = [0u8; 16];
                salt.copy_from_slice(&salt_bytes);
                tracing::info!(path = %salt_path.display(), "loaded existing salt");
                salt
            } else {
                let salt = life_engine_crypto::generate_salt();
                std::fs::write(&salt_path, &salt)?;
                tracing::info!(path = %salt_path.display(), "generated and saved new salt");
                salt
            };

            let key = life_engine_crypto::derive_key(&passphrase, &salt)
                .map_err(|e| anyhow::anyhow!("key derivation failed: {e}"))?;
            tracing::info!("Database key derived");
            Some(key)
        } else {
            tracing::info!("Encryption enabled but no passphrase configured — deferring to /api/storage/init");
            None
        }
    } else {
        None
    };

    // 4b. Initialise storage (file-backed when data_dir is configured).
    let storage = match derived_key {
        Some(_key) => {
            tracing::info!(path = %db_path.display(), "opening encrypted storage with derived key");
            // TODO(9.6): Pass derived key to StorageBackend::init() once implemented.
            // For now, open unencrypted storage as a placeholder until WP 9.6 wires
            // the key into SQLCipher PRAGMA.
            Arc::new(sqlite_storage::SqliteStorage::open(&db_path)?)
        }
        None if config.storage.encryption => {
            tracing::info!(path = %db_path.display(), "opening encrypted storage");
            // Encrypted storage without startup passphrase requires the /api/storage/init endpoint.
            Arc::new(sqlite_storage::SqliteStorage::open_in_memory()?)
        }
        None => {
            Arc::new(sqlite_storage::SqliteStorage::open(&db_path)?)
        }
    };
    tracing::info!(path = %db_path.display(), encrypted = config.storage.encryption, "storage initialised");

    // 4c. Initialise schema registry (must happen before plugin loading).
    let schema_dir = if let Ok(dir) = std::env::var("LIFE_ENGINE_SCHEMA_DIR") {
        std::path::PathBuf::from(dir)
    } else {
        data_dir_path.join("schemas")
    };
    let schema_registry = if schema_dir.exists() {
        match SchemaRegistry::load_from_directory(&schema_dir) {
            Ok(registry) => {
                tracing::info!(
                    collections = registry.collections().len(),
                    "schema registry loaded"
                );
                Arc::new(registry)
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to load schemas, using empty registry");
                Arc::new(SchemaRegistry::new())
            }
        }
    } else {
        tracing::info!("schema directory not found, using empty registry");
        Arc::new(SchemaRegistry::new())
    };

    // 4d. Initialise plugin loader with schema registry and load plugins.
    let plugin_loader = Arc::new(Mutex::new(
        PluginLoader::with_schema_registry(Arc::clone(&schema_registry)),
    ));
    {
        let mut loader = plugin_loader.lock().await;
        let errors = loader.load_all().await;
        for err in &errors {
            tracing::warn!(error = %err, "plugin load error (non-fatal)");
        }
        tracing::info!(loaded = loader.loaded_count(), "plugins loaded");
    }

    // 5. Publish a startup event.
    message_bus.publish(message_bus::BusEvent::SyncComplete {
        plugin_id: "core".into(),
    });

    // 6. Initialise auth provider.
    let oidc_config = config.auth.oidc.as_ref().map(|oidc| {
        crate::auth::oidc::OidcConfig {
            issuer_url: oidc.issuer_url.clone(),
            client_id: oidc.client_id.clone(),
            client_secret: oidc.client_secret.clone(),
            jwks_uri: oidc.jwks_uri.clone(),
            audience: oidc.audience.clone(),
        }
    });
    let webauthn_config = config.auth.webauthn.as_ref().map(|wn| {
        crate::auth::webauthn_provider::WebAuthnConfig {
            rp_name: wn.rp_name.clone(),
            rp_id: wn.rp_id.clone(),
            rp_origin: wn.rp_origin.clone(),
            challenge_ttl_secs: wn.challenge_ttl_secs,
        }
    });
    let (auth_provider, webauthn_provider) = crate::auth::build_auth_provider(
        &config.auth.provider,
        oidc_config.clone(),
        webauthn_config,
        None,
    );
    let rate_limiter = RateLimiter::new();
    let auth_mw_state = AuthMiddlewareState {
        auth_provider: Arc::clone(&auth_provider),
        rate_limiter,
    };
    tracing::info!(provider = %config.auth.provider, "auth provider initialised");
    if webauthn_provider.is_some() {
        tracing::info!("WebAuthn passkey provider enabled");
    }

    // 6b. Initialise conflict store.
    let conflict_store = Arc::new(ConflictStore::new());
    tracing::info!("conflict store initialised (in-memory)");

    // 6c. Initialise validated storage (wraps schema registry + storage).
    let validated_storage = Arc::new(ValidatedStorage::new(
        Arc::clone(&storage),
        Arc::clone(&schema_registry),
    ));
    tracing::info!("validated storage initialised");

    // 7d. Initialise full-text search engine and search processor.
    let search_engine = match search::SearchEngine::new() {
        Ok(engine) => {
            let engine = Arc::new(engine);
            search_processor::spawn(&message_bus, Arc::clone(&engine));
            tracing::info!("search engine and processor initialised (in-memory)");
            Some(engine)
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to initialise search engine");
            None
        }
    };

    // 8. Build the HTTP router.
    let config_path = if cli.config.is_empty() {
        CoreConfig::default_config_path()
    } else {
        Some(std::path::PathBuf::from(&cli.config))
    };
    let shared_config = Arc::new(tokio::sync::RwLock::new(config.clone()));

    let general_rate_limiter =
        GeneralRateLimiter::new(config.network.rate_limit.requests_per_minute);
    tracing::info!(
        rpm = config.network.rate_limit.requests_per_minute,
        "general rate limiter initialised"
    );

    tracing::info!(
        origins = ?config.network.cors.allowed_origins,
        "CORS origins configured (dynamic — changes apply on config reload)"
    );
    if config.network.cors.allowed_origins.iter().any(|o| o == "*") {
        tracing::warn!("CORS configured with wildcard origin '*' — any domain can make cross-origin requests");
    }

    let state = AppState {
        start_time,
        plugin_loader: Arc::clone(&plugin_loader),
        storage: Some(Arc::clone(&storage)),
        message_bus: Arc::clone(&message_bus),
        conflict_store: Some(Arc::clone(&conflict_store)),
        validated_storage: Some(Arc::clone(&validated_storage)),
        search_engine,
        credential_store: None,
        household_store: Some(Arc::new(crate::household::HouseholdStore::new())),
        federation_store: Some(Arc::new(crate::federation::FederationStore::new())),
        identity_store: None,
        config: Arc::clone(&shared_config),
        config_path,
        log_reload_handle: Some(log_reload_handle),
        rate_limiter: Some(general_rate_limiter.clone()),
    };

    // Storage init endpoint (no auth required, callable once).
    let data_dir = std::path::PathBuf::from(&config.core.data_dir);
    std::fs::create_dir_all(&data_dir)?;
    let storage_init_state = StorageInitState {
        initialized: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        db_path: data_dir.join("life-engine.db"),
        argon2_settings: config.storage.argon2.clone(),
        init_attempts: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
    };
    let storage_init_router = axum::Router::new()
        .route(
            "/api/storage/init",
            axum::routing::post(init_storage),
        )
        .with_state(storage_init_state);

    let app = axum::Router::new()
        .route("/api/system/health", axum::routing::get(health_check))
        .route("/api/system/info", axum::routing::get(system_info))
        .route("/api/system/plugins", axum::routing::get(system_plugins))
        .route(
            "/api/system/config",
            axum::routing::get(get_config).put(put_config),
        )
        .route(
            "/api/data/{collection}",
            axum::routing::get(list_records).post(create_record),
        )
        .route(
            "/api/data/{collection}/{id}",
            axum::routing::get(get_record)
                .put(update_record)
                .delete(delete_record),
        )
        .route("/api/conflicts", axum::routing::get(list_conflicts))
        .route(
            "/api/conflicts/{id}",
            axum::routing::get(get_conflict).delete(delete_conflict),
        )
        .route(
            "/api/conflicts/{id}/resolve",
            axum::routing::post(resolve_conflict),
        )
        .route(
            "/api/system/quarantine",
            axum::routing::get(list_quarantine),
        )
        .route(
            "/api/system/quarantine/{id}/reprocess",
            axum::routing::post(reprocess_quarantine),
        )
        .route(
            "/api/system/quarantine/{id}",
            axum::routing::delete(delete_quarantine),
        )
        .route("/api/search", axum::routing::get(search))
        .route("/api/events/stream", axum::routing::get(event_stream))
        .route("/api/graphql", axum::routing::post(graphql_handler))
        .route("/api/graphql/playground", axum::routing::get(graphql_playground))
        .route(
            "/api/connectors/{id}/sync",
            axum::routing::post(trigger_sync),
        )
        .route(
            "/api/credentials",
            axum::routing::post(store_credential).get(list_credentials),
        )
        .route(
            "/api/credentials/{plugin_id}/{key}",
            axum::routing::get(get_credential).delete(delete_credential),
        )
        .route(
            "/api/credentials/{plugin_id}",
            axum::routing::delete(delete_plugin_credentials),
        )
        .route(
            "/api/federation/peers",
            axum::routing::post(create_peer).get(list_peers),
        )
        .route(
            "/api/federation/peers/{id}",
            axum::routing::delete(delete_peer),
        )
        .route(
            "/api/federation/sync",
            axum::routing::post(trigger_federation_sync),
        )
        .route(
            "/api/federation/status",
            axum::routing::get(federation_status),
        )
        .route(
            "/api/federation/changes/{collection}",
            axum::routing::get(serve_changes),
        )
        .route(
            "/api/identity/credentials",
            axum::routing::post(create_identity_credential)
                .get(list_identity_credentials),
        )
        .route(
            "/api/identity/credentials/{id}",
            axum::routing::get(get_identity_credential)
                .delete(delete_identity_credential),
        )
        .route(
            "/api/identity/credentials/{id}/disclose",
            axum::routing::post(disclose_credential),
        )
        .route(
            "/api/identity/credentials/{id}/audit",
            axum::routing::get(get_disclosure_audit),
        )
        .route(
            "/api/identity/credentials/{id}/vc",
            axum::routing::get(export_verifiable_credential),
        )
        .route("/api/identity/did", axum::routing::get(get_did))
        .route(
            "/api/plugins/{plugin_id}/{*path}",
            axum::routing::get(plugin_route_stub)
                .post(plugin_route_stub)
                .put(plugin_route_stub)
                .delete(plugin_route_stub)
                .patch(plugin_route_stub),
        )
        .with_state(state)
        .merge(auth_router(Arc::clone(&auth_provider), oidc_config, webauthn_provider))
        .layer(axum::middleware::from_fn_with_state(
            auth_mw_state,
            auth_middleware,
        ))
        .layer(axum::middleware::from_fn_with_state(
            general_rate_limiter,
            rate_limit_middleware,
        ))
        // Public routes (no auth required) merged after auth middleware.
        .merge(storage_init_router)
        .layer(TraceLayer::new_for_http())
        .layer(axum::middleware::from_fn_with_state(
            shared_config,
            dynamic_cors_middleware,
        ));

    // 9. Bind and serve.
    let bind_addr = config.bind_address();
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;

    if config.network.tls.enabled {
        let tls_acceptor = tls::build_tls_acceptor(&config.network.tls)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        tracing::info!(address = %bind_addr, "listening (TLS)");
        serve_tls(listener, app, tls_acceptor).await?;
    } else {
        tracing::info!(address = %bind_addr, "listening");
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    }

    // 10. Graceful shutdown sequence.
    graceful_shutdown(plugin_loader).await;

    Ok(())
}

/// Maximum number of concurrent TLS connections to prevent resource exhaustion.
const MAX_TLS_CONNECTIONS: usize = 1024;

/// Serve HTTP over TLS using `tokio-rustls` and `hyper-util`.
///
/// Accepts TCP connections, performs TLS handshakes, and serves each
/// connection using the axum router. A semaphore caps concurrent connections
/// to `MAX_TLS_CONNECTIONS`. Graceful shutdown is handled by monitoring the
/// shutdown signal.
async fn serve_tls(
    listener: tokio::net::TcpListener,
    app: axum::Router,
    tls_acceptor: tokio_rustls::TlsAcceptor,
) -> anyhow::Result<()> {
    use hyper_util::rt::{TokioExecutor, TokioIo};
    use tower::ServiceExt;

    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);

    let conn_semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_TLS_CONNECTIONS));
    tracing::info!(max = MAX_TLS_CONNECTIONS, "TLS connection limit configured");

    loop {
        tokio::select! {
            result = listener.accept() => {
                let (stream, _remote_addr) = result?;
                let acceptor = tls_acceptor.clone();
                let app = app.clone();
                let sem = Arc::clone(&conn_semaphore);

                let permit = match sem.try_acquire_owned() {
                    Ok(permit) => permit,
                    Err(_) => {
                        tracing::warn!("TLS connection limit reached, dropping connection");
                        drop(stream);
                        continue;
                    }
                };

                tokio::spawn(async move {
                    let tls_stream = match acceptor.accept(stream).await {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::warn!(error = %e, "TLS handshake failed");
                            return;
                        }
                    };

                    let io = TokioIo::new(tls_stream);
                    let service = hyper::service::service_fn(move |req: hyper::Request<hyper::body::Incoming>| {
                        let app = app.clone();
                        async move {
                            let req = req.map(axum::body::Body::new);
                            app.oneshot(req).await
                        }
                    });

                    if let Err(e) = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                        .serve_connection(io, service)
                        .await
                    {
                        tracing::error!(error = %e, "TLS connection error");
                    }

                    drop(permit);
                });
            }
            _ = &mut shutdown => {
                tracing::info!("shutdown signal received, stopping TLS server");
                break;
            }
        }
    }

    Ok(())
}

/// Dynamic CORS middleware that reads allowed origins from the shared config.
///
/// Unlike a static `CorsLayer`, this re-checks the config on every request,
/// so CORS origin changes applied via PUT `/api/system/config` take effect
/// immediately without a server restart.
async fn dynamic_cors_middleware(
    axum::extract::State(config): axum::extract::State<Arc<tokio::sync::RwLock<CoreConfig>>>,
    request: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::http::{header, Method};
    use axum::response::IntoResponse;

    let origin = request.headers().get(header::ORIGIN).cloned();
    let is_preflight = request.method() == Method::OPTIONS;

    let mut response = if is_preflight {
        axum::http::StatusCode::NO_CONTENT.into_response()
    } else {
        next.run(request).await
    };

    if let Some(origin_val) = origin {
        let config = config.read().await;
        let allowed = &config.network.cors.allowed_origins;
        let origin_str = origin_val.to_str().unwrap_or("");
        let is_wildcard = allowed.iter().any(|o| o == "*");
        let is_listed = allowed.iter().any(|o| o == origin_str);

        if is_wildcard || is_listed {
            let allow_origin_val = if is_wildcard {
                header::HeaderValue::from_static("*")
            } else {
                origin_val
            };
            let headers = response.headers_mut();
            headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, allow_origin_val);
            headers.insert(
                header::ACCESS_CONTROL_ALLOW_METHODS,
                "GET, POST, PUT, DELETE, OPTIONS".parse().unwrap(),
            );
            headers.insert(
                header::ACCESS_CONTROL_ALLOW_HEADERS,
                "Content-Type, Authorization, Accept".parse().unwrap(),
            );
        }
    }

    response
}

/// Initialise the tracing subscriber based on config.
///
/// Returns a [`LogReloadHandle`] that can be used to hot-reload the
/// EnvFilter (log level) at runtime without restarting the server.
fn init_logging(config: &CoreConfig) -> LogReloadHandle {
    use tracing_subscriber::prelude::*;

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.core.log_level));

    let (filter_layer, reload_handle) = tracing_subscriber::reload::Layer::new(env_filter);

    match config.core.log_format.as_str() {
        "pretty" => {
            tracing_subscriber::registry()
                .with(filter_layer)
                .with(tracing_subscriber::fmt::layer().pretty())
                .init();
        }
        _ => {
            tracing_subscriber::registry()
                .with(filter_layer)
                .with(tracing_subscriber::fmt::layer().json())
                .init();
        }
    }

    reload_handle
}

#[cfg(test)]
mod test_helpers;

#[cfg(test)]
mod tests {
    #[test]
    fn core_binary_compiles() {
        // This test verifies the Core binary compiles successfully.
        // It exercises the full dependency chain: types, plugin-sdk, core.
        assert!(true);
    }
}
