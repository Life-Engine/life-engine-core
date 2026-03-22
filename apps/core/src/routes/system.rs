//! System information, plugin listing, and config management routes.

use crate::plugin_loader::PluginStatus;
use crate::routes::health::AppState;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};

/// GET /api/system/info — Version, uptime, and system information.
pub async fn system_info(State(state): State<AppState>) -> Json<Value> {
    let uptime = state.start_time.elapsed().as_secs();
    let loader = state.plugin_loader.lock().await;

    Json(json!({
        "data": {
            "version": env!("CARGO_PKG_VERSION"),
            "plugins_loaded": loader.loaded_count(),
            "storage": "sqlite",
            "uptime_seconds": uptime,
        }
    }))
}

/// GET /api/system/config — Return the current configuration with secrets redacted.
pub async fn get_config(State(state): State<AppState>) -> Json<Value> {
    let config = state.config.read().await;
    Json(json!({ "data": config.to_redacted_json() }))
}

/// PUT /api/system/config — Merge partial config, validate, persist to YAML, and update in-memory state.
pub async fn put_config(
    State(state): State<AppState>,
    Json(partial): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let config_path = state.config_path.as_ref().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "no config file path configured" })),
        )
    })?;

    let mut config = state.config.write().await;

    // Merge partial into current config and validate.
    let merged = config.merge_partial(&partial).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
    })?;

    // Persist to YAML.
    let yaml = serde_yaml::to_string(&merged).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("failed to serialize config: {e}") })),
        )
    })?;

    // Ensure parent directory exists.
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("failed to create config directory: {e}") })),
            )
        })?;
    }

    std::fs::write(config_path, &yaml).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("failed to write config file: {e}") })),
        )
    })?;

    // Update in-memory config.
    *config = merged;

    // Apply hot-reloadable settings.
    reload_runtime_settings(&state, &config);

    Ok(Json(json!({ "data": config.to_redacted_json() })))
}

/// Apply hot-reloadable settings from the current config.
///
/// Updates the tracing log level and the general rate limiter without
/// requiring a server restart. CORS changes are applied automatically
/// by the dynamic CORS middleware which reads from the shared config.
fn reload_runtime_settings(state: &AppState, config: &crate::config::CoreConfig) {
    // Reload log level via the tracing EnvFilter handle.
    if let Some(ref handle) = state.log_reload_handle {
        match tracing_subscriber::EnvFilter::try_new(&config.core.log_level) {
            Ok(new_filter) => {
                if let Err(e) = handle.reload(new_filter) {
                    tracing::warn!(error = %e, "failed to reload log filter");
                } else {
                    tracing::info!(level = %config.core.log_level, "log level reloaded");
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, level = %config.core.log_level, "invalid log level, skipping reload");
            }
        }
    }

    // Reconfigure the general rate limiter.
    if let Some(ref limiter) = state.rate_limiter {
        limiter.reconfigure(config.network.rate_limit.requests_per_minute);
        tracing::info!(
            rpm = config.network.rate_limit.requests_per_minute,
            "rate limiter reconfigured"
        );
    }

    // CORS: no explicit reload needed — the dynamic CORS middleware reads
    // from the shared config on every request, so changes take effect
    // immediately once the in-memory config is updated.
}

/// GET /api/system/plugins — List all loaded plugins.
pub async fn system_plugins(State(state): State<AppState>) -> Json<Value> {
    let loader = state.plugin_loader.lock().await;

    let plugins: Vec<Value> = loader
        .plugin_info()
        .into_iter()
        .map(|info| {
            json!({
                "id": info.id,
                "name": info.display_name,
                "version": info.version,
                "status": match info.status {
                    PluginStatus::Registered => "registered",
                    PluginStatus::Loaded => "loaded",
                    PluginStatus::Failed(_) => "failed",
                    PluginStatus::Unloaded => "unloaded",
                },
            })
        })
        .collect();

    Json(json!({ "data": plugins }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::middleware::auth_middleware;
    use crate::config::CoreConfig;
    use crate::routes::health::health_check;
    use crate::test_helpers::{
        auth_request, body_json, create_auth_state, default_app_state, generate_test_token,
    };
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::{get, put};
    use axum::Router;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use tower::ServiceExt;

    async fn setup_test_app() -> (Router, String) {
        let (auth_state, provider) = create_auth_state();
        let state = default_app_state();

        let app = Router::new()
            .route("/api/system/health", get(health_check))
            .route("/api/system/info", get(system_info))
            .route("/api/system/plugins", get(system_plugins))
            .route(
                "/api/system/config",
                get(get_config).put(put_config),
            )
            .with_state(state)
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                auth_middleware,
            ));

        let token = generate_test_token(&provider).await;

        (app, token)
    }

    #[tokio::test]
    async fn system_info_returns_valid_data() {
        let (app, token) = setup_test_app().await;

        let req = auth_request("GET", "/api/system/info", &token, None);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["data"]["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(json["data"]["storage"], "sqlite");
        assert_eq!(json["data"]["plugins_loaded"], 0);
        assert!(json["data"]["uptime_seconds"].is_number());
    }

    #[tokio::test]
    async fn system_plugins_returns_empty_list() {
        let (app, token) = setup_test_app().await;

        let req = auth_request("GET", "/api/system/plugins", &token, None);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        let json = body_json(resp).await;
        assert!(json["data"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn system_info_requires_auth() {
        let (app, _token) = setup_test_app().await;

        let req = Request::builder()
            .uri("/api/system/info")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn system_plugins_requires_auth() {
        let (app, _token) = setup_test_app().await;

        let req = Request::builder()
            .uri("/api/system/plugins")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn health_still_skips_auth() {
        let (app, _token) = setup_test_app().await;

        let req = Request::builder()
            .uri("/api/system/health")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    }

    #[tokio::test]
    async fn get_config_returns_redacted_data() {
        let (auth_state, provider) = create_auth_state();
        let mut state = default_app_state();
        let mut config = CoreConfig::default();
        config.auth.oidc = Some(crate::config::OidcSettings {
            issuer_url: "https://idp.example.com".into(),
            client_id: "my-client".into(),
            client_secret: Some("super-secret".into()),
            jwks_uri: None,
            audience: None,
        });
        state.config = Arc::new(RwLock::new(config));

        let app = Router::new()
            .route("/api/system/config", get(get_config))
            .with_state(state)
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                auth_middleware,
            ));

        let token = generate_test_token(&provider).await;
        let req = auth_request("GET", "/api/system/config", &token, None);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["data"]["core"]["port"], 3750);
        assert_eq!(json["data"]["auth"]["oidc"]["client_secret"], "[REDACTED]");
        assert_eq!(json["data"]["auth"]["oidc"]["client_id"], "my-client");
    }

    #[tokio::test]
    async fn get_config_requires_auth() {
        let (auth_state, _provider) = create_auth_state();
        let state = default_app_state();

        let app = Router::new()
            .route("/api/system/config", get(get_config))
            .with_state(state)
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                auth_middleware,
            ));

        let req = Request::builder()
            .uri("/api/system/config")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn put_config_updates_and_persists() {
        let (auth_state, provider) = create_auth_state();
        let mut state = default_app_state();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        state.config_path = Some(tmp.path().to_path_buf());

        let app = Router::new()
            .route(
                "/api/system/config",
                get(get_config).put(put_config),
            )
            .with_state(state)
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                auth_middleware,
            ));

        let token = generate_test_token(&provider).await;
        let body = serde_json::json!({ "core": { "port": 9090 } }).to_string();
        let req = auth_request("PUT", "/api/system/config", &token, Some(body));
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["data"]["core"]["port"], 9090);

        // Verify file was written.
        let persisted = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(persisted.contains("9090"));
    }

    #[tokio::test]
    async fn put_config_rejects_invalid() {
        let (auth_state, provider) = create_auth_state();
        let mut state = default_app_state();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        state.config_path = Some(tmp.path().to_path_buf());

        let app = Router::new()
            .route("/api/system/config", put(put_config))
            .with_state(state)
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                auth_middleware,
            ));

        let token = generate_test_token(&provider).await;
        // Port 0 is invalid.
        let body = serde_json::json!({ "core": { "port": 0 } }).to_string();
        let req = auth_request("PUT", "/api/system/config", &token, Some(body));
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::BAD_REQUEST);

        let json = body_json(resp).await;
        assert!(json["error"].as_str().unwrap().contains("port"));
    }

    #[tokio::test]
    async fn put_config_requires_auth() {
        let (auth_state, _provider) = create_auth_state();
        let state = default_app_state();

        let app = Router::new()
            .route("/api/system/config", put(put_config))
            .with_state(state)
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                auth_middleware,
            ));

        let req = Request::builder()
            .method("PUT")
            .uri("/api/system/config")
            .header("Content-Type", "application/json")
            .body(Body::from("{}"))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn put_config_reloads_log_level() {
        let (auth_state, provider) = create_auth_state();
        let mut state = default_app_state();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        state.config_path = Some(tmp.path().to_path_buf());

        // Set up a real log reload handle so the reload path executes.
        let filter = tracing_subscriber::EnvFilter::new("info");
        let (_layer, handle) = tracing_subscriber::reload::Layer::<
            tracing_subscriber::EnvFilter,
            tracing_subscriber::Registry,
        >::new(filter);
        state.log_reload_handle = Some(handle);

        let app = Router::new()
            .route(
                "/api/system/config",
                get(get_config).put(put_config),
            )
            .with_state(state)
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                auth_middleware,
            ));

        let token = generate_test_token(&provider).await;
        let body = serde_json::json!({ "core": { "log_level": "debug" } }).to_string();
        let req = auth_request("PUT", "/api/system/config", &token, Some(body));
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["data"]["core"]["log_level"], "debug");
    }

    #[tokio::test]
    async fn put_config_reconfigures_rate_limiter() {
        let (auth_state, provider) = create_auth_state();
        let mut state = default_app_state();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        state.config_path = Some(tmp.path().to_path_buf());

        // Set up a real rate limiter so the reconfigure path executes.
        let limiter = crate::rate_limit::GeneralRateLimiter::new(60);
        state.rate_limiter = Some(limiter.clone());

        let shared_config = Arc::clone(&state.config);

        let app = Router::new()
            .route(
                "/api/system/config",
                get(get_config).put(put_config),
            )
            .with_state(state)
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                auth_middleware,
            ));

        let token = generate_test_token(&provider).await;
        let body =
            serde_json::json!({ "network": { "rate_limit": { "requests_per_minute": 120 } } })
                .to_string();
        let req = auth_request("PUT", "/api/system/config", &token, Some(body));
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(
            json["data"]["network"]["rate_limit"]["requests_per_minute"],
            120
        );

        // Verify in-memory config was updated.
        let config = shared_config.read().await;
        assert_eq!(config.network.rate_limit.requests_per_minute, 120);
    }

    #[tokio::test]
    async fn put_config_without_config_path_returns_error() {
        let (auth_state, provider) = create_auth_state();
        let state = default_app_state();
        // config_path is None by default.

        let app = Router::new()
            .route("/api/system/config", put(put_config))
            .with_state(state)
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                auth_middleware,
            ));

        let token = generate_test_token(&provider).await;
        let body = serde_json::json!({ "core": { "port": 9090 } }).to_string();
        let req = auth_request("PUT", "/api/system/config", &token, Some(body));
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );

        let json = body_json(resp).await;
        assert!(json["error"]
            .as_str()
            .unwrap()
            .contains("no config file path"));
    }
}
