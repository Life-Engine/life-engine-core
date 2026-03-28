//! Life Engine Core — self-hosted personal data sovereignty backend.
//!
//! Entry point: 10-step startup orchestrator that loads config, initialises
//! subsystems in dependency order, starts transports, and coordinates
//! graceful shutdown.

mod audit;
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
mod install_service;

use crate::auth::middleware::{auth_middleware, AuthMiddlewareState, RateLimiter};
use crate::auth::routes::auth_router;
use crate::config::{CliArgs, CliCommand, CoreConfig, LogReloadHandle};
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
use crate::shutdown::{graceful_shutdown, shutdown_signal, ShutdownConfig, ShutdownHandles};

use clap::Parser;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

/// Log a startup step completion with duration.
macro_rules! log_step {
    ($step:expr, $total:expr, $name:expr, $start:expr) => {
        tracing::info!(
            step = $step,
            total_steps = $total,
            duration_ms = $start.elapsed().as_millis() as u64,
            concat!("Step {}/{}: ", $name, "... done"),
            $step, $total
        );
    };
}

/// Log a startup step failure and exit.
macro_rules! fail_step {
    ($step:expr, $total:expr, $name:expr, $err:expr) => {{
        tracing::error!(
            step = $step,
            total_steps = $total,
            error = %$err,
            concat!("Step {}/{}: ", $name, "... FAILED"),
            $step, $total
        );
        std::process::exit(1);
    }};
}

const TOTAL_STEPS: u32 = 10;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let total_start = Instant::now();

    // ── Step 1/10: Load configuration ────────────────────────────────
    let step_start = Instant::now();
    let cli = CliArgs::parse();

    // Handle subcommands before starting the server.
    if let Some(CliCommand::InstallService) = cli.command {
        install_service::run();
        return Ok(());
    }

    let config = CoreConfig::load(&cli).unwrap_or_else(|e| {
        eprintln!("Step 1/{TOTAL_STEPS}: Load configuration... FAILED: {e}");
        std::process::exit(1);
    });
    // Also validate via the new TOML-based config system (Phase 9.2-9.3).
    let toml_config_path = if cli.config.is_empty() { None } else { Some(cli.config.as_str()) };
    match crate::config::startup::load_config(toml_config_path) {
        Ok(startup_cfg) => {
            if let Err(e) = crate::config::startup::validate_config(&startup_cfg) {
                tracing::warn!(error = %e, "new-architecture config validation warning");
            }
        }
        Err(e) => {
            // Non-fatal during transition — the legacy config system is still primary.
            tracing::debug!(error = %e, "new-architecture config not loaded (expected during transition)");
        }
    }
    let step1_duration = step_start.elapsed();

    // ── Step 2/10: Initialize logging ────────────────────────────────
    let step_start = Instant::now();
    let log_reload_handle = init_logging(&config);
    log_step!(2, TOTAL_STEPS, "Initialize logging", step_start);
    // Log step 1 retroactively now that the logger is available.
    tracing::info!(
        step = 1,
        total_steps = TOTAL_STEPS,
        duration_ms = step1_duration.as_millis() as u64,
        "Step 1/{TOTAL_STEPS}: Load configuration... done"
    );
    tracing::info!("Life Engine Core starting");

    // Log deployment mode and active configuration summary.
    let deployment_mode = detect_deployment_mode();
    let tls_status = if config.network.tls.enabled { "enabled" } else { "disabled" };
    let plugins_dir = if config.plugins.paths.is_empty() {
        "plugins".to_string()
    } else {
        config.plugins.paths.join(", ")
    };
    let workflows_dir = "workflows";
    tracing::info!(
        deployment_mode = %deployment_mode,
        bind = %format!("{}:{}", config.core.host, config.core.port),
        tls = %tls_status,
        behind_proxy = %config.network.behind_proxy,
        auth_provider = %config.auth.provider,
        database_path = %config.core.data_dir,
        plugins_dir = %plugins_dir,
        workflows_dir = %workflows_dir,
        log_level = %config.core.log_level,
        "startup configuration summary"
    );

    if config.network.behind_proxy {
        tracing::info!(
            "behind-proxy mode active: trusting X-Forwarded-For and X-Forwarded-Proto headers"
        );
    }

    // Log security posture so operators can verify the instance configuration at a glance.
    let is_localhost = config.core.host == "127.0.0.1"
        || config.core.host == "localhost"
        || config.core.host == "::1";
    let rate_limit_status = if !is_localhost || config.network.rate_limit.requests_per_minute > 0 {
        "enabled"
    } else {
        "disabled"
    };
    tracing::info!(
        tls = %tls_status,
        auth = %config.auth.provider,
        rate_limit = %rate_limit_status,
        bind_address = %format!("{}:{}", config.core.host, config.core.port),
        "Security: TLS={}, Auth={}, RateLimit={}, BindAddress={}:{}",
        tls_status,
        config.auth.provider,
        rate_limit_status,
        config.core.host,
        config.core.port,
    );

    // ── Step 3/10: Derive database key ───────────────────────────────
    let step_start = Instant::now();
    let start_time = Instant::now();
    let data_dir_path = std::path::Path::new(&config.core.data_dir);
    std::fs::create_dir_all(data_dir_path).unwrap_or_else(|e| {
        fail_step!(3, TOTAL_STEPS, "Derive database key", e);
    });
    let db_path = data_dir_path.join("life-engine.db");
    let derived_key = if config.storage.encryption {
        if let Some(passphrase) = config.storage.resolve_passphrase() {
            let salt_path = data_dir_path.join("salt.bin");
            let salt = if salt_path.exists() {
                let salt_bytes = std::fs::read(&salt_path).unwrap_or_else(|e| {
                    fail_step!(3, TOTAL_STEPS, "Derive database key", e);
                });
                if salt_bytes.len() != 16 {
                    tracing::error!(
                        path = %salt_path.display(),
                        actual_len = salt_bytes.len(),
                        "salt file has invalid length (expected 16 bytes)"
                    );
                    std::process::exit(1);
                }
                let mut salt = [0u8; 16];
                salt.copy_from_slice(&salt_bytes);
                tracing::info!(path = %salt_path.display(), "loaded existing salt");
                salt
            } else {
                let salt = life_engine_crypto::generate_salt();
                std::fs::write(&salt_path, salt).unwrap_or_else(|e| {
                    fail_step!(3, TOTAL_STEPS, "Derive database key", e);
                });
                tracing::info!(path = %salt_path.display(), "generated and saved new salt");
                salt
            };

            let key = life_engine_crypto::derive_key(&passphrase, &salt).unwrap_or_else(|e| {
                fail_step!(3, TOTAL_STEPS, "Derive database key", e);
            });
            tracing::info!("Database key derived");
            Some(key)
        } else {
            tracing::info!("Encryption enabled but no passphrase configured — deferring to /api/storage/init");
            None
        }
    } else {
        None
    };
    log_step!(3, TOTAL_STEPS, "Derive database key", step_start);

    // ── Step 4/10: Initialize storage ────────────────────────────────
    let step_start = Instant::now();
    let message_bus = Arc::new(MessageBus::new());
    let storage = match derived_key {
        Some(ref key) => {
            tracing::info!(path = %db_path.display(), "opening encrypted storage with derived key");
            match sqlite_storage::SqliteStorage::open_with_key(&db_path, key) {
                Ok(s) => Arc::new(s),
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("unable to decrypt") {
                        tracing::error!(
                            path = %db_path.display(),
                            "storage initialization failed — check passphrase"
                        );
                        fail_step!(4, TOTAL_STEPS, "Initialize storage",
                            format_args!(
                                "Cannot open encrypted database at {}: {}. \
                                 Verify LIFE_ENGINE_STORAGE_PASSPHRASE or storage.passphrase is correct.",
                                db_path.display(), msg
                            )
                        );
                    } else if msg.contains("permission denied") || msg.contains("readonly") {
                        fail_step!(4, TOTAL_STEPS, "Initialize storage",
                            format_args!(
                                "Cannot open database at {}: {}. Check file permissions.",
                                db_path.display(), msg
                            )
                        );
                    } else {
                        fail_step!(4, TOTAL_STEPS, "Initialize storage", e);
                    }
                }
            }
        }
        None if config.storage.encryption => {
            tracing::info!(path = %db_path.display(), "encryption enabled but no passphrase — deferring to /api/storage/init");
            Arc::new(sqlite_storage::SqliteStorage::open_in_memory().unwrap_or_else(|e| {
                fail_step!(4, TOTAL_STEPS, "Initialize storage", e);
            }))
        }
        None => {
            tracing::info!(path = %db_path.display(), "opening unencrypted storage");
            Arc::new(sqlite_storage::SqliteStorage::open(&db_path).unwrap_or_else(|e| {
                fail_step!(4, TOTAL_STEPS, "Initialize storage", e);
            }))
        }
    };
    tracing::info!(path = %db_path.display(), encrypted = config.storage.encryption, "storage initialized");

    // Schema registry (part of storage initialization).
    let schema_dir = if let Ok(dir) = std::env::var("LIFE_ENGINE_SCHEMA_DIR") {
        std::path::PathBuf::from(dir)
    } else {
        data_dir_path.join("schemas")
    };
    let schema_registry = if schema_dir.exists() {
        match SchemaRegistry::load_from_directory(&schema_dir) {
            Ok(registry) => {
                tracing::info!(collections = registry.collections().len(), "schema registry loaded");
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
    log_step!(4, TOTAL_STEPS, "Initialize storage", step_start);

    // ── Step 4b: Run canonical schema migrations ─────────────────────
    // Compare each canonical collection's stored schema version against
    // the declared version in the types crate. If behind, run WASM
    // migration transforms through the same migration engine used for
    // plugin migrations.
    {
        use life_engine_types::migrations::{CANONICAL_COLLECTIONS, CANONICAL_PLUGIN_ID};

        let migrations_base = data_dir_path.join("migrations");

        // Phase 1: Check versions and collect migrations that need to run.
        // Hold the connection lock only briefly for version checks.
        struct PendingMigration {
            collection_name: &'static str,
            stored_version: i64,
            declared_version: i64,
            wasm_path: std::path::PathBuf,
            entries: Vec<life_engine_workflow_engine::migration::MigrationEntry>,
        }

        let mut pending_migrations = Vec::new();

        {
            let conn = storage.connection().await;

            for collection in CANONICAL_COLLECTIONS {
                let stored_version = conn
                    .query_row(
                        "SELECT version FROM schema_versions WHERE plugin_id = ?1 AND collection = ?2",
                        rusqlite::params![CANONICAL_PLUGIN_ID, collection.name],
                        |row| row.get::<_, i64>(0),
                    )
                    .unwrap_or(0);

                if stored_version == 0 {
                    // First run — stamp the current version, no migration needed.
                    let now = chrono::Utc::now().to_rfc3339();
                    let _ = conn.execute(
                        "INSERT OR REPLACE INTO schema_versions (plugin_id, collection, version, updated_at) \
                         VALUES (?1, ?2, ?3, ?4)",
                        rusqlite::params![CANONICAL_PLUGIN_ID, collection.name, collection.version, now],
                    );
                    tracing::debug!(
                        collection = collection.name,
                        version = collection.version,
                        "canonical collection version stamped (first run)"
                    );
                    continue;
                }

                if stored_version >= collection.version {
                    tracing::debug!(
                        collection = collection.name,
                        stored = stored_version,
                        declared = collection.version,
                        "canonical collection schema is current"
                    );
                    continue;
                }

                // Schema version is behind — look for WASM migration transforms.
                let migration_dir = migrations_base.join(collection.migration_dir);
                if !migration_dir.exists() {
                    tracing::warn!(
                        collection = collection.name,
                        stored = stored_version,
                        declared = collection.version,
                        dir = %migration_dir.display(),
                        "canonical migration directory not found, skipping"
                    );
                    continue;
                }

                let manifest_path = migration_dir.join("manifest.toml");
                if !manifest_path.exists() {
                    tracing::warn!(
                        collection = collection.name,
                        "canonical migration manifest.toml not found, skipping"
                    );
                    continue;
                }

                let wasm_path = migration_dir.join("transform.wasm");
                if !wasm_path.exists() {
                    tracing::warn!(
                        collection = collection.name,
                        "canonical transform.wasm not found, skipping"
                    );
                    continue;
                }

                match life_engine_workflow_engine::migration::parse_migration_entries(&manifest_path) {
                    Ok(entries) => {
                        let relevant: Vec<_> = entries
                            .into_iter()
                            .filter(|e| {
                                let from_major = e.from.split('.').next()
                                    .and_then(|s| s.parse::<i64>().ok())
                                    .unwrap_or(0);
                                from_major >= stored_version && (e.to.major as i64) <= collection.version
                            })
                            .collect();

                        if !relevant.is_empty() {
                            pending_migrations.push(PendingMigration {
                                collection_name: collection.name,
                                stored_version,
                                declared_version: collection.version,
                                wasm_path,
                                entries: relevant,
                            });
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            collection = collection.name,
                            error = %e,
                            "failed to parse canonical migration manifest"
                        );
                    }
                }
            }
        } // Connection lock released here.

        // Phase 2: Run any pending migrations (without holding the connection lock).
        for migration in &pending_migrations {
            tracing::info!(
                collection = migration.collection_name,
                from = migration.stored_version,
                to = migration.declared_version,
                entries = migration.entries.len(),
                "running canonical schema migration"
            );

            match life_engine_workflow_engine::migration::run_migrations(
                &migration.wasm_path,
                &migration.entries,
                CANONICAL_PLUGIN_ID,
                &db_path,
                data_dir_path,
            )
            .await
            {
                Ok(result) => {
                    tracing::info!(
                        collection = migration.collection_name,
                        migrated = result.migrated,
                        quarantined = result.quarantined,
                        duration_ms = result.duration_ms,
                        "canonical migration complete"
                    );

                    // Stamp the new version.
                    let conn = storage.connection().await;
                    let now = chrono::Utc::now().to_rfc3339();
                    let _ = conn.execute(
                        "INSERT OR REPLACE INTO schema_versions (plugin_id, collection, version, updated_at) \
                         VALUES (?1, ?2, ?3, ?4)",
                        rusqlite::params![CANONICAL_PLUGIN_ID, migration.collection_name, migration.declared_version, now],
                    );
                }
                Err(e) => {
                    tracing::error!(
                        collection = migration.collection_name,
                        error = %e,
                        "canonical migration failed (non-fatal)"
                    );
                }
            }
        }
    }

    // ── Step 5/10: Initialize auth ───────────────────────────────────
    let step_start = Instant::now();
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
        message_bus: Some(Arc::clone(&message_bus)),
    };
    tracing::info!(provider = %config.auth.provider, "auth provider initialized");
    if webauthn_provider.is_some() {
        tracing::info!("WebAuthn passkey provider enabled");
    }
    log_step!(5, TOTAL_STEPS, "Initialize auth", step_start);

    // ── Step 6/10: Create workflow engine ─────────────────────────────
    let step_start = Instant::now();
    let _workflow_executor = life_engine_workflow_engine::PipelineExecutor::new(
        Arc::new(NoOpPluginExecutor),
    );
    tracing::info!("workflow engine created");
    log_step!(6, TOTAL_STEPS, "Create workflow engine", step_start);

    // ── Step 7/10: Load workflows ────────────────────────────────────
    let step_start = Instant::now();
    let workflows_path = std::path::Path::new("workflows");
    if workflows_path.is_dir() {
        let wf_config = life_engine_workflow_engine::WorkflowConfig {
            path: "workflows".to_string(),
        };
        match life_engine_workflow_engine::load_workflows(&wf_config) {
            Ok(workflows) => {
                let count = workflows.len();
                match life_engine_workflow_engine::TriggerRegistry::build(workflows) {
                    Ok(_trigger_registry) => {
                        tracing::info!(count, "workflows loaded and trigger registry built");
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to build trigger registry");
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to load workflows (non-fatal)");
            }
        }
    } else {
        tracing::info!("no workflows directory found, skipping workflow loading");
    }
    log_step!(7, TOTAL_STEPS, "Load workflows", step_start);

    // ── Step 8/10: Discover and load plugins ─────────────────────────
    let step_start = Instant::now();
    let plugin_loader = Arc::new(Mutex::new({
        let mut loader = PluginLoader::with_schema_registry(Arc::clone(&schema_registry));
        loader.set_message_bus(Arc::clone(&message_bus));
        loader
    }));
    {
        let mut loader = plugin_loader.lock().await;
        let errors = loader.load_all().await;
        for err in &errors {
            tracing::warn!(error = %err, "plugin load error (non-fatal)");
        }
        tracing::info!(loaded = loader.loaded_count(), "plugins loaded");
    }
    // Conflict store, validated storage, and search engine (supporting subsystems).
    let conflict_store = Arc::new(ConflictStore::new());
    let validated_storage = Arc::new(ValidatedStorage::new(
        Arc::clone(&storage),
        Arc::clone(&schema_registry),
    ));
    let search_index_dir = data_dir_path.join("search_index");
    let search_engine = match search::SearchEngine::open_in_dir(&search_index_dir, 50) {
        Ok(engine) => {
            let engine = Arc::new(engine);
            search_processor::spawn(&message_bus, Arc::clone(&engine));
            tracing::info!(path = %search_index_dir.display(), "search engine initialized (disk-backed)");
            Some(engine)
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to initialize search engine, falling back to in-memory");
            match search::SearchEngine::new() {
                Ok(engine) => {
                    let engine = Arc::new(engine);
                    search_processor::spawn(&message_bus, Arc::clone(&engine));
                    tracing::info!("search engine initialized (in-memory fallback)");
                    Some(engine)
                }
                Err(e2) => {
                    tracing::warn!(error = %e2, "failed to initialize in-memory search engine");
                    None
                }
            }
        }
    };
    // Publish startup event.
    message_bus.publish(message_bus::BusEvent::SyncComplete {
        plugin_id: "core".into(),
    });
    log_step!(8, TOTAL_STEPS, "Discover and load plugins", step_start);

    // ── Step 9/10: Start active transports ───────────────────────────
    let step_start = Instant::now();
    let config_path = if cli.config.is_empty() {
        CoreConfig::default_config_path()
    } else {
        Some(std::path::PathBuf::from(&cli.config))
    };
    let shared_config = Arc::new(tokio::sync::RwLock::new(config.clone()));

    let general_rate_limiter =
        GeneralRateLimiter::new(config.network.rate_limit.requests_per_minute);

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

    let data_dir = std::path::PathBuf::from(&config.core.data_dir);
    std::fs::create_dir_all(&data_dir)?;
    let storage_init_state = StorageInitState {
        initialized: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        db_path: data_dir.join("life-engine.db"),
        argon2_settings: config.storage.argon2.clone(),
        init_attempts: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
    };
    let storage_init_router = axum::Router::new()
        .route("/api/storage/init", axum::routing::post(init_storage))
        .with_state(storage_init_state);

    let app = build_router(state)
        .merge(auth_router(Arc::clone(&auth_provider), oidc_config, webauthn_provider))
        .layer(axum::middleware::from_fn_with_state(auth_mw_state, auth_middleware))
        .layer(axum::middleware::from_fn_with_state(general_rate_limiter, rate_limit_middleware))
        .merge(storage_init_router)
        .layer(axum::extract::DefaultBodyLimit::max(2 * 1024 * 1024)) // 2 MB
        .layer(TraceLayer::new_for_http())
        .layer(axum::middleware::from_fn_with_state(shared_config, dynamic_cors_middleware));

    let bind_addr = config.bind_address();
    let listener = tokio::net::TcpListener::bind(&bind_addr).await.unwrap_or_else(|e| {
        fail_step!(9, TOTAL_STEPS, "Start active transports", e);
    });

    // Instantiate configured transports from the new-architecture TOML config.
    // The built-in REST transport (axum router above) is always active via the
    // legacy config's host/port. Additional transports configured in the
    // [transports.*] TOML sections are instantiated here using the Transport trait.
    let transport_handles = instantiate_transports(&cli).await;

    log_step!(9, TOTAL_STEPS, "Start active transports", step_start);

    // ── Step 10/10: Wait for shutdown signal ─────────────────────────
    tracing::info!(
        total_startup_ms = total_start.elapsed().as_millis() as u64,
        "Step 10/{TOTAL_STEPS}: Wait for shutdown signal — all systems ready"
    );

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

    // Graceful shutdown sequence — reverse startup order with per-step timeouts.
    let handles = ShutdownHandles {
        transports: transport_handles,
        plugin_loader,
        storage: Some(Arc::clone(&storage)),
    };
    graceful_shutdown(handles, ShutdownConfig::default()).await;

    Ok(())
}

/// Instantiate configured transports from the new-architecture TOML config.
///
/// Reads the `[transports.*]` sections from `config.toml` and creates a
/// `Transport` trait object for each configured transport. If a transport
/// fails to start, the error is logged but other transports continue
/// starting — one failed transport should not prevent others from starting.
async fn instantiate_transports(cli: &CliArgs) -> Vec<Box<dyn life_engine_traits::Transport>> {
    let toml_config_path = if cli.config.is_empty() {
        None
    } else {
        Some(cli.config.as_str())
    };

    let transports_map = match crate::config::startup::load_config(toml_config_path) {
        Ok(cfg) => cfg.transports,
        Err(_) => {
            tracing::debug!("no new-architecture config available, skipping transport instantiation");
            return Vec::new();
        }
    };

    if transports_map.is_empty() {
        tracing::debug!("no transports configured in [transports] section");
        return Vec::new();
    }

    let mut handles: Vec<Box<dyn life_engine_traits::Transport>> = Vec::new();

    for (name, config_value) in &transports_map {
        let result: Result<Box<dyn life_engine_traits::Transport>, _> = match name.as_str() {
            "rest" => life_engine_transport_rest::RestTransport::from_config(config_value)
                .map(|t| Box::new(t) as Box<dyn life_engine_traits::Transport>),
            "graphql" => life_engine_transport_graphql::GraphqlTransport::from_config(config_value)
                .map(|t| Box::new(t) as Box<dyn life_engine_traits::Transport>),
            "caldav" => life_engine_transport_caldav::CaldavTransport::from_config(config_value)
                .map(|t| Box::new(t) as Box<dyn life_engine_traits::Transport>),
            "carddav" => life_engine_transport_carddav::CarddavTransport::from_config(config_value)
                .map(|t| Box::new(t) as Box<dyn life_engine_traits::Transport>),
            "webhook" => life_engine_transport_webhook::WebhookTransport::from_config(config_value)
                .map(|t| Box::new(t) as Box<dyn life_engine_traits::Transport>),
            unknown => {
                tracing::warn!(transport = unknown, "unknown transport configured, skipping");
                continue;
            }
        };

        match result {
            Ok(transport) => {
                // Call start() — log errors but continue with other transports.
                let transport_name = transport.name().to_string();
                let config_clone = config_value.clone();
                match transport.start(config_clone).await {
                    Ok(()) => {
                        handles.push(transport);
                    }
                    Err(e) => {
                        tracing::error!(
                            transport = %transport_name,
                            error = %e,
                            "transport failed to start, continuing with remaining transports"
                        );
                    }
                }
            }
            Err(e) => {
                tracing::error!(
                    transport = %name,
                    error = %e,
                    "failed to create transport from config, continuing with remaining transports"
                );
            }
        }
    }

    tracing::info!(
        count = handles.len(),
        configured = transports_map.len(),
        "transport instantiation complete"
    );

    handles
}

/// No-op plugin executor used during startup before plugins are loaded.
///
/// The workflow engine requires a `PluginExecutor` at construction time.
/// This placeholder is replaced when transport-level routing dispatches
/// to real plugin instances.
struct NoOpPluginExecutor;

#[async_trait::async_trait]
impl life_engine_workflow_engine::PluginExecutor for NoOpPluginExecutor {
    async fn execute(
        &self,
        plugin_id: &str,
        action: &str,
        _input: life_engine_types::PipelineMessage,
    ) -> Result<life_engine_types::PipelineMessage, Box<dyn life_engine_traits::EngineError>> {
        Err(Box::new(life_engine_workflow_engine::WorkflowError::PluginExecutionError {
            plugin: plugin_id.to_string(),
            cause: format!("plugin executor not yet initialized (action: {action})"),
        }))
    }
}

/// Build the main HTTP router with all API routes.
fn build_router(state: AppState) -> axum::Router {
    axum::Router::new()
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
/// Configures:
/// - JSON or pretty output format
/// - Global log level with per-module overrides (e.g. `log_modules: { storage: "debug" }`)
/// - Common fields on every log entry: `version` (from Cargo.toml) and `pid`
///
/// Returns a [`LogReloadHandle`] that can be used to hot-reload the
/// EnvFilter (log level) at runtime without restarting the server.
/// Detect the deployment mode from the runtime environment.
///
/// Returns one of:
/// - `"docker"` — running inside a container (`/.dockerenv` exists or `container` env var set)
/// - `"standalone"` — default for direct binary execution
fn detect_deployment_mode() -> &'static str {
    // Docker: check for /.dockerenv or container runtime env vars.
    if std::path::Path::new("/.dockerenv").exists()
        || std::env::var("container").is_ok()
        || std::env::var("DOCKER_CONTAINER").is_ok()
    {
        return "docker";
    }

    "standalone"
}

fn init_logging(config: &CoreConfig) -> LogReloadHandle {
    use tracing_subscriber::prelude::*;

    // Build the filter directive: start with the global level, then append
    // per-module overrides (e.g. "info,life_engine_storage_sqlite=debug").
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        let mut directives = config.core.log_level.clone();
        for (module, level) in &config.core.log_modules {
            // Convert user-friendly module names (e.g. "storage") to crate
            // names by prefixing with `life_engine_` and replacing `-` with `_`.
            let crate_name = if module.starts_with("life_engine_") {
                module.clone()
            } else {
                format!("life_engine_{}", module.replace('-', "_"))
            };
            directives.push_str(&format!(",{crate_name}={level}"));
        }
        EnvFilter::new(directives)
    });

    let (filter_layer, reload_handle) = tracing_subscriber::reload::Layer::new(env_filter);

    let version = env!("CARGO_PKG_VERSION");
    let pid = std::process::id();

    match config.core.log_format.as_str() {
        "pretty" => {
            tracing_subscriber::registry()
                .with(filter_layer)
                .with(
                    tracing_subscriber::fmt::layer()
                        .pretty()
                        .with_file(false)
                        .with_line_number(false),
                )
                .init();
        }
        _ => {
            tracing_subscriber::registry()
                .with(filter_layer)
                .with(
                    tracing_subscriber::fmt::layer()
                        .json()
                        .with_current_span(true),
                )
                .init();
        }
    }

    // Enter a root span that injects version and pid into every log entry.
    // This span lives for the process lifetime via `leaked_enter`.
    let root_span = tracing::info_span!("core", version = version, pid = pid);
    // Intentionally leak the guard so the span stays active for the process lifetime.
    let guard = root_span.enter();
    std::mem::forget(guard);

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
