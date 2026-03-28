//! Integration tests for the listener: startup, middleware stack, TLS, auth.

use std::sync::Arc;

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::Extension;
use axum::Router;
use life_engine_auth::{AuthError, AuthIdentity, AuthProvider};
use life_engine_types::identity::Identity;
use tower::ServiceExt;
use uuid::Uuid;

use crate::config::{HandlerConfig, ListenerConfig, RouteConfig};
use crate::listener::build_layered_router;

// ── Mock auth provider ──────────────────────────────────────────────

/// Always succeeds for any token.
struct AcceptAllProvider;

#[async_trait]
impl AuthProvider for AcceptAllProvider {
    async fn validate_token(&self, _token: &str) -> Result<AuthIdentity, AuthError> {
        Ok(AuthIdentity {
            user_id: "test-user".to_string(),
            provider: "test".to_string(),
            scopes: vec!["read".to_string()],
            authenticated_at: chrono::Utc::now(),
        })
    }

    async fn validate_key(&self, _key: &str) -> Result<AuthIdentity, AuthError> {
        Ok(AuthIdentity {
            user_id: "test-service".to_string(),
            provider: "api-key".to_string(),
            scopes: vec![],
            authenticated_at: chrono::Utc::now(),
        })
    }

    async fn revoke_key(&self, _key_id: Uuid) -> Result<(), AuthError> {
        Ok(())
    }
}

/// Always rejects tokens.
struct RejectAllProvider;

#[async_trait]
impl AuthProvider for RejectAllProvider {
    async fn validate_token(&self, _token: &str) -> Result<AuthIdentity, AuthError> {
        Err(AuthError::TokenInvalid("test rejection".to_string()))
    }

    async fn validate_key(&self, _key: &str) -> Result<AuthIdentity, AuthError> {
        Err(AuthError::KeyInvalid)
    }

    async fn revoke_key(&self, _key_id: Uuid) -> Result<(), AuthError> {
        Ok(())
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

fn test_listener_config() -> ListenerConfig {
    ListenerConfig {
        binding: "test".into(),
        port: 0,
        address: "127.0.0.1".into(),
        tls: None,
        auth: None,
        handlers: vec![
            HandlerConfig {
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
                        path: "/api/v1/data/:collection".into(),
                        workflow: "collection.list".into(),
                        public: false,
                    },
                ],
            },
            HandlerConfig {
                handler_type: "graphql".into(),
                routes: vec![RouteConfig {
                    method: "POST".into(),
                    path: "/graphql".into(),
                    workflow: "graphql.query".into(),
                    public: false,
                }],
            },
        ],
    }
}

/// Build a simple test router with a public health route and a protected data route.
fn test_router() -> Router {
    Router::new()
        .route(
            "/api/v1/health",
            get(|| async { axum::Json(serde_json::json!({"data": "healthy"})) }),
        )
        .route(
            "/api/v1/data/{collection}",
            get(|ext: Extension<Identity>| async move {
                axum::Json(serde_json::json!({"data": {"user": ext.0.subject}}))
            }),
        )
        .route(
            "/graphql",
            axum::routing::post(|ext: Extension<Identity>| async move {
                axum::Json(serde_json::json!({"data": {"user": ext.0.subject}}))
            }),
        )
}

// ── Layered router tests ────────────────────────────────────────────

#[tokio::test]
async fn layered_router_public_route_bypasses_auth() {
    let config = test_listener_config();
    let router = test_router();
    let app = build_layered_router(router, &config, Arc::new(RejectAllProvider));

    // Public route should succeed even with a reject-all auth provider.
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
    assert_eq!(json["data"], "healthy");
}

#[tokio::test]
async fn layered_router_authenticated_route_rejects_missing_token() {
    let config = test_listener_config();
    let router = test_router();
    let app = build_layered_router(router, &config, Arc::new(AcceptAllProvider));

    // Protected route without a token should return 401.
    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/data/tasks")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], "AUTH_001");
}

#[tokio::test]
async fn layered_router_authenticated_route_rejects_invalid_token() {
    let config = test_listener_config();
    let router = test_router();
    let app = build_layered_router(router, &config, Arc::new(RejectAllProvider));

    // Protected route with an invalid token should return 401.
    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/data/tasks")
        .header("authorization", "Bearer bad-token")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], "AUTH_003");
}

#[tokio::test]
async fn layered_router_authenticated_route_accepts_valid_token() {
    let config = test_listener_config();
    let router = test_router();
    let app = build_layered_router(router, &config, Arc::new(AcceptAllProvider));

    // Protected route with a valid token should succeed and pass identity.
    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/data/tasks")
        .header("authorization", "Bearer good-token")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["data"]["user"], "test-user");
}

#[tokio::test]
async fn layered_router_rest_and_graphql_on_same_listener() {
    let config = test_listener_config();
    let router = test_router();
    let app = build_layered_router(router, &config, Arc::new(AcceptAllProvider));

    // REST request.
    let rest_req = Request::builder()
        .method("GET")
        .uri("/api/v1/data/tasks")
        .header("authorization", "Bearer token")
        .body(Body::empty())
        .unwrap();

    let rest_resp = app.clone().oneshot(rest_req).await.unwrap();
    assert_eq!(rest_resp.status(), StatusCode::OK);

    // GraphQL request on the same router.
    let gql_req = Request::builder()
        .method("POST")
        .uri("/graphql")
        .header("authorization", "Bearer token")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"query":"{ hello }"}"#))
        .unwrap();

    let gql_resp = app.oneshot(gql_req).await.unwrap();
    assert_eq!(gql_resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn layered_router_catches_panics() {
    let config = test_listener_config();

    // Build a router with a handler that panics.
    let router = Router::new().route(
        "/api/v1/health",
        get(|| async {
            panic!("test panic in handler");
            #[allow(unreachable_code)]
            "unreachable"
        }),
    );
    let app = build_layered_router(router, &config, Arc::new(AcceptAllProvider));

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/health")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], "INTERNAL_ERROR");
    let body_str = serde_json::to_string(&json).unwrap();
    assert!(
        !body_str.contains("panic"),
        "panic details should not leak to client"
    );
}

// ── TCP listener startup test ───────────────────────────────────────

#[tokio::test]
async fn listener_starts_and_accepts_http_requests() {
    let config = test_listener_config();
    let router = test_router();
    let app = build_layered_router(router, &config, Arc::new(AcceptAllProvider));

    // Bind to a random port.
    let tcp_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("should bind to random port");
    let addr = tcp_listener.local_addr().unwrap();

    // Spawn the server.
    tokio::spawn(async move {
        axum::serve(tcp_listener, app).await.ok();
    });

    let client = reqwest::Client::new();

    // Public health endpoint — no auth needed.
    let resp = client
        .get(format!("http://{addr}/api/v1/health"))
        .send()
        .await
        .expect("health request should succeed");
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["data"], "healthy");

    // Protected endpoint without auth — should get 401.
    let resp = client
        .get(format!("http://{addr}/api/v1/data/tasks"))
        .send()
        .await
        .expect("data request should succeed");
    assert_eq!(resp.status(), 401);

    // Protected endpoint with auth — should succeed.
    let resp = client
        .get(format!("http://{addr}/api/v1/data/tasks"))
        .header("authorization", "Bearer good-token")
        .send()
        .await
        .expect("authenticated data request should succeed");
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["data"]["user"], "test-user");
}

// ── TLS listener startup test ───────────────────────────────────────

#[tokio::test]
async fn listener_starts_with_tls_and_accepts_https_requests() {
    // Install rustls crypto provider for tests.
    let _ = rustls::crypto::CryptoProvider::install_default(
        rustls::crypto::aws_lc_rs::default_provider(),
    );

    // Generate a self-signed certificate for testing.
    let subject_alt_names = vec!["localhost".to_string(), "127.0.0.1".to_string()];
    let cert_params = rcgen::CertificateParams::new(subject_alt_names).unwrap();
    let key_pair = rcgen::KeyPair::generate().unwrap();
    let cert = cert_params.self_signed(&key_pair).unwrap();

    let cert_pem = cert.pem();
    let key_pem = key_pair.serialize_pem();

    // Write cert and key to temp files.
    let tmp_dir = std::env::temp_dir().join(format!("le-tls-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmp_dir).unwrap();
    let cert_path = tmp_dir.join("cert.pem");
    let key_path = tmp_dir.join("key.pem");
    std::fs::write(&cert_path, &cert_pem).unwrap();
    std::fs::write(&key_path, &key_pem).unwrap();

    let config = ListenerConfig {
        binding: "test".into(),
        port: 0,
        address: "127.0.0.1".into(),
        tls: Some(crate::config::TlsConfig {
            cert: cert_path.to_string_lossy().into_owned(),
            key: key_path.to_string_lossy().into_owned(),
        }),
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

    let router = Router::new().route(
        "/api/v1/health",
        get(|| async { axum::Json(serde_json::json!({"data": "healthy"})) }),
    );
    let app = build_layered_router(router, &config, Arc::new(AcceptAllProvider));

    // Build the TLS acceptor.
    let tls_config = config.tls.as_ref().unwrap();
    let cert_pem_bytes = std::fs::read(&tls_config.cert).unwrap();
    let key_pem_bytes = std::fs::read(&tls_config.key).unwrap();
    let certs = rustls_pemfile::certs(&mut &cert_pem_bytes[..])
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let key = rustls_pemfile::private_key(&mut &key_pem_bytes[..])
        .unwrap()
        .unwrap();
    let server_tls_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs.clone(), key)
        .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_tls_config));

    // Bind to a random port.
    let tcp_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("should bind");
    let addr = tcp_listener.local_addr().unwrap();

    // Spawn TLS server.
    tokio::spawn(async move {
        loop {
            let (tcp_stream, _) = match tcp_listener.accept().await {
                Ok(s) => s,
                Err(_) => break,
            };
            let acceptor = acceptor.clone();
            let svc = app.clone();
            tokio::spawn(async move {
                if let Ok(tls_stream) = acceptor.accept(tcp_stream).await {
                    let io = hyper_util::rt::TokioIo::new(tls_stream);
                    let hyper_service = hyper::service::service_fn(
                        move |req: hyper::Request<hyper::body::Incoming>| {
                            let mut svc = svc.clone();
                            async move {
                                use tower::Service;
                                let req = req.map(axum::body::Body::new);
                                let _ = std::future::poll_fn(|cx| {
                                    <Router as Service<
                                        axum::http::Request<axum::body::Body>,
                                    >>::poll_ready(
                                        &mut svc, cx
                                    )
                                })
                                .await;
                                let resp: axum::response::Response =
                                    svc.call(req).await.unwrap_or_else(|err| match err {});
                                Ok::<_, std::convert::Infallible>(resp)
                            }
                        },
                    );
                    let _ = hyper_util::server::conn::auto::Builder::new(
                        hyper_util::rt::TokioExecutor::new(),
                    )
                    .serve_connection(io, hyper_service)
                    .await;
                }
            });
        }
    });

    // Build a reqwest client that trusts our self-signed cert.
    let mut root_store = rustls::RootCertStore::empty();
    for cert in &certs {
        root_store.add(cert.clone()).unwrap();
    }
    let client_tls_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let client = reqwest::Client::builder()
        .use_preconfigured_tls(client_tls_config)
        .build()
        .unwrap();

    let resp = client
        .get(format!("https://localhost:{}/api/v1/health", addr.port()))
        .send()
        .await
        .expect("HTTPS request should succeed");
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["data"], "healthy");

    // Clean up temp files.
    std::fs::remove_dir_all(&tmp_dir).ok();
}
