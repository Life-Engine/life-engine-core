//! Tests for REST transport config, validation, route merging, and router.

mod middleware_test;

use crate::config::{
    HandlerConfig, ListenerConfig, PluginRoute, RouteConfig, TlsConfig,
    default_listener_config, validate_listener, write_default_config,
};
use crate::config::merge_routes;
use crate::router::build_router;
use axum::Extension;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use life_engine_types::identity::Identity;
use tower::ServiceExt;

// ── Config validation tests ──────────────────────────────────────────

#[test]
fn test_default_config_is_valid() {
    let config = default_listener_config();
    validate_listener(&config).expect("default config should pass validation");
}

#[test]
fn test_port_zero_rejected() {
    let config = ListenerConfig {
        binding: "test".into(),
        port: 0,
        address: "127.0.0.1".into(),
        tls: None,
        auth: None,
        handlers: vec![],
    };
    let err = validate_listener(&config).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("port"), "error should mention port: {msg}");
}

#[test]
fn test_tls_empty_paths_rejected() {
    let config = ListenerConfig {
        binding: "test".into(),
        port: 8080,
        address: "127.0.0.1".into(),
        tls: Some(TlsConfig {
            cert: String::new(),
            key: "key.pem".into(),
        }),
        auth: None,
        handlers: vec![],
    };
    let err = validate_listener(&config).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("TLS"), "error should mention TLS: {msg}");
}

#[test]
fn test_duplicate_routes_rejected() {
    let config = ListenerConfig {
        binding: "test".into(),
        port: 3000,
        address: "127.0.0.1".into(),
        tls: None,
        auth: None,
        handlers: vec![HandlerConfig {
            handler_type: "rest".into(),
            routes: vec![
                RouteConfig {
                    method: "GET".into(),
                    path: "/api/v1/health".into(),
                    workflow: "health.check".into(),
                    public: true,
                },
                RouteConfig {
                    method: "GET".into(),
                    path: "/api/v1/health".into(),
                    workflow: "health.check2".into(),
                    public: true,
                },
            ],
        }],
    };
    let err = validate_listener(&config).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("duplicate route"),
        "error should mention duplicate: {msg}"
    );
}

#[test]
fn test_rest_route_must_start_with_api_prefix() {
    let config = ListenerConfig {
        binding: "test".into(),
        port: 3000,
        address: "127.0.0.1".into(),
        tls: None,
        auth: None,
        handlers: vec![HandlerConfig {
            handler_type: "rest".into(),
            routes: vec![RouteConfig {
                method: "GET".into(),
                path: "/graphql".into(),
                workflow: "oops".into(),
                public: false,
            }],
        }],
    };
    let err = validate_listener(&config).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("must start with /api/"),
        "error should enforce REST prefix: {msg}"
    );
}

#[test]
fn test_graphql_route_must_start_with_graphql_prefix() {
    let config = ListenerConfig {
        binding: "test".into(),
        port: 3000,
        address: "127.0.0.1".into(),
        tls: None,
        auth: None,
        handlers: vec![HandlerConfig {
            handler_type: "graphql".into(),
            routes: vec![RouteConfig {
                method: "POST".into(),
                path: "/api/v1/nope".into(),
                workflow: "graphql.query".into(),
                public: false,
            }],
        }],
    };
    let err = validate_listener(&config).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("must start with /graphql"),
        "error should enforce GraphQL prefix: {msg}"
    );
}

// ── Default config content tests ─────────────────────────────────────

#[test]
fn test_default_config_has_health_check() {
    let config = default_listener_config();
    let rest = config
        .handlers
        .iter()
        .find(|h| h.handler_type == "rest")
        .expect("should have a rest handler");
    let health = rest
        .routes
        .iter()
        .find(|r| r.path == "/api/v1/health")
        .expect("should have health route");
    assert!(health.public, "health route should be public");
    assert_eq!(health.method, "GET");
}

#[test]
fn test_default_config_has_crud_routes() {
    let config = default_listener_config();
    let rest = config
        .handlers
        .iter()
        .find(|h| h.handler_type == "rest")
        .expect("should have a rest handler");

    let workflows: Vec<&str> = rest.routes.iter().map(|r| r.workflow.as_str()).collect();
    assert!(workflows.contains(&"collection.list"));
    assert!(workflows.contains(&"collection.get"));
    assert!(workflows.contains(&"collection.create"));
    assert!(workflows.contains(&"collection.update"));
    assert!(workflows.contains(&"collection.delete"));
}

#[test]
fn test_default_config_has_graphql_endpoint() {
    let config = default_listener_config();
    let gql = config
        .handlers
        .iter()
        .find(|h| h.handler_type == "graphql")
        .expect("should have a graphql handler");
    assert_eq!(gql.routes.len(), 1);
    assert_eq!(gql.routes[0].path, "/graphql");
    assert_eq!(gql.routes[0].workflow, "graphql.query");
}

// ── Route merging tests ──────────────────────────────────────────────

#[test]
fn test_merge_routes_adds_plugin_routes() {
    let config = default_listener_config();
    let plugin_routes = vec![PluginRoute {
        plugin_id: "com.test.plugin".into(),
        method: "POST".into(),
        path: "/api/v1/custom/action".into(),
        workflow: "custom.action".into(),
        public: false,
    }];
    let merged = merge_routes(&config, &plugin_routes).expect("merge should succeed");
    assert!(merged.iter().any(|r| r.workflow == "custom.action"));
}

#[test]
fn test_merge_routes_detects_conflict() {
    let config = default_listener_config();
    let plugin_routes = vec![PluginRoute {
        plugin_id: "com.test.conflicting".into(),
        method: "GET".into(),
        path: "/api/v1/health".into(),
        workflow: "plugin.health".into(),
        public: true,
    }];
    let err = merge_routes(&config, &plugin_routes).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("conflicts"),
        "error should mention conflict: {msg}"
    );
}

// ── Multi-error validation tests ─────────────────────────────────────

#[test]
fn test_validation_collects_all_errors() {
    let config = ListenerConfig {
        binding: "test".into(),
        port: 0,
        address: "127.0.0.1".into(),
        tls: Some(TlsConfig {
            cert: String::new(),
            key: String::new(),
        }),
        auth: None,
        handlers: vec![HandlerConfig {
            handler_type: "rest".into(),
            routes: vec![RouteConfig {
                method: "GET".into(),
                path: "/graphql".into(),
                workflow: "bad".into(),
                public: false,
            }],
        }],
    };
    let err = validate_listener(&config).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("port"), "should report port error: {msg}");
    assert!(msg.contains("TLS"), "should report TLS error: {msg}");
    assert!(
        msg.contains("must start with /api/"),
        "should report namespace error: {msg}"
    );
}

// ── YAML parsing tests ──────────────────────────────────────────────

#[test]
fn test_config_parses_from_yaml() {
    let yaml = r#"
binding: test
port: 8080
address: "0.0.0.0"
handlers:
  - type: rest
    routes:
      - method: GET
        path: /api/v1/health
        workflow: health.check
        public: true
"#;
    let config: ListenerConfig = serde_yaml::from_str(yaml).expect("should parse YAML");
    assert_eq!(config.port, 8080);
    assert_eq!(config.address, "0.0.0.0");
    assert_eq!(config.handlers.len(), 1);
    assert_eq!(config.handlers[0].routes[0].workflow, "health.check");
    assert!(config.handlers[0].routes[0].public);
}

#[test]
fn test_config_yaml_defaults_applied() {
    let yaml = r#"
port: 3000
handlers: []
"#;
    let config: ListenerConfig = serde_yaml::from_str(yaml).expect("should parse YAML");
    assert_eq!(config.binding, "default");
    assert_eq!(config.address, "127.0.0.1");
}

// ── Default config file generation tests ─────────────────────────────

#[test]
fn test_write_default_config_produces_parseable_yaml() {
    let dir = std::env::temp_dir().join("le-config-test");
    std::fs::create_dir_all(&dir).unwrap();

    let path = write_default_config(&dir).expect("should write config");
    assert!(path.exists(), "file should exist at {}", path.display());

    let contents = std::fs::read_to_string(&path).unwrap();
    let parsed: ListenerConfig =
        serde_yaml::from_str(&contents).expect("written YAML should parse back");
    validate_listener(&parsed).expect("written config should be valid");

    std::fs::remove_dir_all(&dir).ok();
}

// ── Router construction + path parameter extraction tests ────────────

#[tokio::test]
async fn test_router_extracts_path_params() {
    let routes = vec![RouteConfig {
        method: "GET".into(),
        path: "/api/v1/data/:collection/:id".into(),
        workflow: "collection.get".into(),
        public: false,
    }];

    let app = build_router(&routes).layer(Extension(Identity::guest()));

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/data/tasks/abc-123")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    // Real handler returns { "data": ... } envelope.
    assert!(json.get("data").is_some(), "response should have data envelope");
}

#[tokio::test]
async fn test_router_resolves_workflow_by_name() {
    let routes = vec![RouteConfig {
        method: "GET".into(),
        path: "/api/v1/health".into(),
        workflow: "health.check".into(),
        public: true,
    }];

    let app = build_router(&routes).layer(Extension(Identity::guest()));

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/health")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json.get("data").is_some(), "response should have data envelope");
}

#[tokio::test]
async fn test_router_returns_404_for_unknown_route() {
    let routes = vec![RouteConfig {
        method: "GET".into(),
        path: "/api/v1/health".into(),
        workflow: "health.check".into(),
        public: true,
    }];

    let app = build_router(&routes).layer(Extension(Identity::guest()));

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/nonexistent")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
