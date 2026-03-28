//! Unit tests for route merging, collision detection, path parameter
//! extraction, namespace validation, and handler-type dispatch.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

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

#[tokio::test]
async fn router_extracts_path_params() {
    let routes = vec![RouteConfig {
        method: "GET".into(),
        path: "/api/v1/data/:collection/:id".into(),
        workflow: "collection.get".into(),
        public: false,
    }];

    let app = build_router(&routes);
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
    assert_eq!(json["workflow"], "collection.get");
    assert_eq!(json["params"]["collection"], "tasks");
    assert_eq!(json["params"]["id"], "abc-123");
}

#[tokio::test]
async fn router_resolves_workflow_by_name() {
    let routes = vec![RouteConfig {
        method: "GET".into(),
        path: "/api/v1/health".into(),
        workflow: "health.check".into(),
        public: true,
    }];

    let app = build_router(&routes);
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
    assert_eq!(json["workflow"], "health.check");
    assert_eq!(json["public"], true);
}

#[tokio::test]
async fn router_returns_404_for_unknown_route() {
    let routes = vec![RouteConfig {
        method: "GET".into(),
        path: "/api/v1/health".into(),
        workflow: "health.check".into(),
        public: true,
    }];

    let app = build_router(&routes);
    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/nonexistent")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── Merged router (handler-type dispatch) ───────────────────────────

#[tokio::test]
async fn merged_router_includes_handler_type() {
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

    let app = build_merged_router(&merged);
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
    assert_eq!(json["workflow"], "collection.list");
    assert_eq!(json["handler_type"], "rest");
    assert_eq!(json["params"]["collection"], "notes");
}

#[tokio::test]
async fn merged_router_dispatches_graphql_route() {
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

    let app = build_merged_router(&merged);
    let req = Request::builder()
        .method("POST")
        .uri("/graphql")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["workflow"], "graphql.query");
    assert_eq!(json["handler_type"], "graphql");
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
    let app = build_merged_router(&merged);

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
    assert_eq!(json["workflow"], "search.query");
    assert_eq!(json["handler_type"], "rest");
}
