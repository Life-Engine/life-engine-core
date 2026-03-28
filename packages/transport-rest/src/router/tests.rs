//! Unit tests for route merging, collision detection, path parameter
//! extraction, namespace validation, and handler-type dispatch.

use axum::Extension;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use life_engine_types::identity::Identity;

use crate::config::{
    HandlerConfig, ListenerConfig, PluginRoute, RouteConfig, default_listener_config,
};

use super::build::{build_merged_router, build_router, to_axum_path};
use super::merge::{MergedRoute, RouteSource, merge_routes};

// ── Route merging tests ─────────────────────────────────────────────

#[test]
fn merge_adds_plugin_routes_to_config_routes() {
    let config = default_listener_config();
    let plugin_routes = vec![PluginRoute {
        plugin_id: "com.test.plugin".into(),
        method: "POST".into(),
        path: "/api/v1/custom/action".into(),
        workflow: "custom.action".into(),
        public: false,
    }];

    let merged = merge_routes(&config, &plugin_routes).expect("merge should succeed");
    assert!(merged.iter().any(|m| m.route.workflow == "custom.action"));

    let custom = merged
        .iter()
        .find(|m| m.route.workflow == "custom.action")
        .unwrap();
    assert_eq!(custom.handler_type, "rest");
    assert_eq!(custom.source, RouteSource::Plugin("com.test.plugin".into()));
}

#[test]
fn merge_rejects_plugin_route_conflicting_with_config() {
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
    assert!(msg.contains("conflicts"), "should mention conflict: {msg}");
    assert!(
        msg.contains("com.test.conflicting"),
        "should identify the plugin: {msg}"
    );
}

#[test]
fn merge_rejects_duplicate_plugin_routes() {
    let config = ListenerConfig {
        binding: "test".into(),
        port: 3000,
        address: "127.0.0.1".into(),
        tls: None,
        auth: None,
        handlers: vec![],
    };

    let plugin_routes = vec![
        PluginRoute {
            plugin_id: "com.plugin.a".into(),
            method: "POST".into(),
            path: "/api/v1/do-thing".into(),
            workflow: "a.do_thing".into(),
            public: false,
        },
        PluginRoute {
            plugin_id: "com.plugin.b".into(),
            method: "POST".into(),
            path: "/api/v1/do-thing".into(),
            workflow: "b.do_thing".into(),
            public: false,
        },
    ];

    let err = merge_routes(&config, &plugin_routes).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("conflicts") || msg.contains("duplicate"),
        "should detect collision: {msg}"
    );
}

#[test]
fn merge_preserves_config_route_source() {
    let config = default_listener_config();
    let merged = merge_routes(&config, &[]).expect("merge should succeed");

    for m in &merged {
        assert_eq!(m.source, RouteSource::Config);
    }
}

// ── Namespace validation during merge ───────────────────────────────

#[test]
fn merge_rejects_plugin_route_outside_api_namespace() {
    let config = ListenerConfig {
        binding: "test".into(),
        port: 3000,
        address: "127.0.0.1".into(),
        tls: None,
        auth: None,
        handlers: vec![],
    };

    let plugin_routes = vec![PluginRoute {
        plugin_id: "com.bad.plugin".into(),
        method: "GET".into(),
        path: "/graphql/sneaky".into(),
        workflow: "bad.sneaky".into(),
        public: false,
    }];

    let err = merge_routes(&config, &plugin_routes).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("must start with /api/"),
        "should enforce REST namespace for plugins: {msg}"
    );
}

#[test]
fn merge_rejects_config_rest_route_outside_namespace() {
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
                path: "/not-api/bad".into(),
                workflow: "bad".into(),
                public: false,
            }],
        }],
    };

    let err = merge_routes(&config, &[]).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("must start with /api/"),
        "should enforce namespace: {msg}"
    );
}

#[test]
fn merge_rejects_graphql_route_outside_namespace() {
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
                path: "/api/v1/not-graphql".into(),
                workflow: "graphql.query".into(),
                public: false,
            }],
        }],
    };

    let err = merge_routes(&config, &[]).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("must start with /graphql"),
        "should enforce graphql namespace: {msg}"
    );
}

// ── Collision detection: collects all errors ────────────────────────

#[test]
fn merge_collects_all_errors_not_just_first() {
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
                path: "/api/v1/health".into(),
                workflow: "health.check".into(),
                public: true,
            }],
        }],
    };

    let plugin_routes = vec![
        PluginRoute {
            plugin_id: "com.bad.a".into(),
            method: "GET".into(),
            path: "/api/v1/health".into(),
            workflow: "bad.a".into(),
            public: false,
        },
        PluginRoute {
            plugin_id: "com.bad.b".into(),
            method: "GET".into(),
            path: "/not-api/oops".into(),
            workflow: "bad.b".into(),
            public: false,
        },
    ];

    let err = merge_routes(&config, &plugin_routes).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("conflicts"), "should have conflict error: {msg}");
    assert!(
        msg.contains("must start with /api/"),
        "should have namespace error: {msg}"
    );
}

// ── Path parameter conversion ───────────────────────────────────────

#[test]
fn to_axum_path_converts_colon_params() {
    assert_eq!(
        to_axum_path("/api/v1/data/:collection/:id"),
        "/api/v1/data/{collection}/{id}"
    );
}

#[test]
fn to_axum_path_leaves_static_paths_unchanged() {
    assert_eq!(to_axum_path("/api/v1/health"), "/api/v1/health");
}

#[test]
fn to_axum_path_handles_single_param() {
    assert_eq!(
        to_axum_path("/api/v1/plugins/:plugin_id"),
        "/api/v1/plugins/{plugin_id}"
    );
}

// ── Router build + path extraction ──────────────────────────────────

/// Helper: wrap a router with a guest Identity extension for testing.
fn with_identity(router: axum::Router) -> axum::Router {
    router.layer(Extension(Identity::guest()))
}

#[tokio::test]
async fn router_extracts_path_params() {
    let routes = vec![RouteConfig {
        method: "GET".into(),
        path: "/api/v1/data/:collection/:id".into(),
        workflow: "collection.get".into(),
        public: false,
    }];

    let app = with_identity(build_router(&routes));
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
async fn router_resolves_workflow_returns_ok() {
    let routes = vec![RouteConfig {
        method: "GET".into(),
        path: "/api/v1/health".into(),
        workflow: "health.check".into(),
        public: true,
    }];

    let app = with_identity(build_router(&routes));
    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/health")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn router_returns_404_for_unknown_route() {
    let routes = vec![RouteConfig {
        method: "GET".into(),
        path: "/api/v1/health".into(),
        workflow: "health.check".into(),
        public: true,
    }];

    let app = with_identity(build_router(&routes));
    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/nonexistent")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── Merged router (REST handler dispatch) ───────────────────────────

#[tokio::test]
async fn merged_router_dispatches_get_to_rest_handler() {
    let merged = vec![MergedRoute {
        route: RouteConfig {
            method: "GET".into(),
            path: "/api/v1/data/:collection".into(),
            workflow: "collection.list".into(),
            public: false,
        },
        handler_type: "rest".into(),
        source: RouteSource::Config,
    }];

    let app = with_identity(build_merged_router(&merged));
    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/data/notes")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json.get("data").is_some(), "REST handler should return data envelope");
}

#[tokio::test]
async fn merged_router_dispatches_post_with_body() {
    let merged = vec![MergedRoute {
        route: RouteConfig {
            method: "POST".into(),
            path: "/api/v1/data/:collection".into(),
            workflow: "collection.create".into(),
            public: false,
        },
        handler_type: "rest".into(),
        source: RouteSource::Config,
    }];

    let app = with_identity(build_merged_router(&merged));
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/data/tasks")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"title":"Buy milk"}"#))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json.get("data").is_some(), "POST handler should return data envelope");
}

#[tokio::test]
async fn merged_router_dispatches_graphql_post() {
    let merged = vec![MergedRoute {
        route: RouteConfig {
            method: "POST".into(),
            path: "/graphql".into(),
            workflow: "graphql.query".into(),
            public: false,
        },
        handler_type: "graphql".into(),
        source: RouteSource::Config,
    }];

    let app = with_identity(build_merged_router(&merged));
    let req = Request::builder()
        .method("POST")
        .uri("/graphql")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"query":"{ items { id } }"}"#))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json.get("data").is_some(), "GraphQL handler should return data envelope");
}

// ── End-to-end: merge then build ────────────────────────────────────

#[tokio::test]
async fn end_to_end_merge_and_build() {
    let config = default_listener_config();
    let plugin_routes = vec![PluginRoute {
        plugin_id: "com.test.search".into(),
        method: "GET".into(),
        path: "/api/v1/search".into(),
        workflow: "search.query".into(),
        public: false,
    }];

    let merged = merge_routes(&config, &plugin_routes).expect("merge should succeed");
    let app = with_identity(build_merged_router(&merged));

    // Config route still works.
    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/health")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Plugin route works.
    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/search")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json.get("data").is_some(), "handler should return data envelope");
}
