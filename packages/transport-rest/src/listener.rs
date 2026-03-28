//! Listener: bind address+port, optional TLS, startup warnings (Requirements 15, 16).

use std::sync::Arc;

use axum::Router;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

use crate::config::{ListenerConfig, TlsConfig};
use crate::error::RestError;

/// Start serving the given router on the configured address and port.
///
/// If TLS is configured, uses `tokio-rustls` for termination (Requirement 15).
/// Logs a warning when bound to `0.0.0.0` (Requirement 16).
pub async fn serve(config: &ListenerConfig, router: Router) -> Result<(), RestError> {
    let addr = format!("{}:{}", config.address, config.port);

    // Startup warning for non-localhost binding (Requirement 16).
    if config.address == "0.0.0.0" {
        tracing::warn!(
            address = %addr,
            "listener is bound to 0.0.0.0 — accessible from the network"
        );
    }

    let tcp_listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| RestError::BindFailed(format!("{addr}: {e}")))?;

    tracing::info!(address = %addr, "REST listener started");

    match &config.tls {
        Some(tls_config) => serve_tls(tcp_listener, tls_config, router).await,
        None => serve_plain(tcp_listener, router).await,
    }
}

/// Serve plaintext HTTP.
async fn serve_plain(listener: TcpListener, router: Router) -> Result<(), RestError> {
    axum::serve(listener, router)
        .await
        .map_err(|e| RestError::BindFailed(format!("serve failed: {e}")))
}

/// Serve HTTPS with TLS termination via tokio-rustls (Requirement 15).
async fn serve_tls(
    listener: TcpListener,
    tls_config: &TlsConfig,
    router: Router,
) -> Result<(), RestError> {
    let acceptor = build_tls_acceptor(tls_config)?;

    loop {
        let (tcp_stream, _remote_addr) = listener
            .accept()
            .await
            .map_err(|e| RestError::BindFailed(format!("accept failed: {e}")))?;

        let acceptor = acceptor.clone();
        let svc = router.clone();

        tokio::spawn(async move {
            match acceptor.accept(tcp_stream).await {
                Ok(tls_stream) => {
                    let io = hyper_util::rt::TokioIo::new(tls_stream);
                    let hyper_service =
                        hyper::service::service_fn(move |req: hyper::Request<hyper::body::Incoming>| {
                            let mut svc = svc.clone();
                            async move {
                                use tower::Service;
                                let req = req.map(axum::body::Body::new);
                                let _ = std::future::poll_fn(|cx| {
                                    <Router as Service<axum::http::Request<axum::body::Body>>>::poll_ready(&mut svc, cx)
                                }).await;
                                let resp: axum::response::Response = svc
                                    .call(req)
                                    .await
                                    .unwrap_or_else(|err| match err {});
                                Ok::<_, std::convert::Infallible>(resp)
                            }
                        });
                    if let Err(e) = hyper_util::server::conn::auto::Builder::new(
                        hyper_util::rt::TokioExecutor::new(),
                    )
                    .serve_connection(io, hyper_service)
                    .await
                    {
                        tracing::debug!(error = %e, "TLS connection error");
                    }
                }
                Err(e) => {
                    tracing::debug!(error = %e, "TLS handshake failed");
                }
            }
        });
    }
}

/// Build a `TlsAcceptor` from cert and key PEM files.
fn build_tls_acceptor(tls_config: &TlsConfig) -> Result<TlsAcceptor, RestError> {
    let cert_pem = std::fs::read(&tls_config.cert).map_err(|e| {
        RestError::InvalidConfig(format!("failed to read TLS cert '{}': {e}", tls_config.cert))
    })?;
    let key_pem = std::fs::read(&tls_config.key).map_err(|e| {
        RestError::InvalidConfig(format!("failed to read TLS key '{}': {e}", tls_config.key))
    })?;

    let certs = rustls_pemfile::certs(&mut &cert_pem[..])
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| RestError::InvalidConfig(format!("invalid TLS cert: {e}")))?;

    let key = rustls_pemfile::private_key(&mut &key_pem[..])
        .map_err(|e| RestError::InvalidConfig(format!("invalid TLS key: {e}")))?
        .ok_or_else(|| RestError::InvalidConfig("no private key found in PEM file".to_string()))?;

    let server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| RestError::InvalidConfig(format!("TLS config error: {e}")))?;

    Ok(TlsAcceptor::from(Arc::new(server_config)))
}
