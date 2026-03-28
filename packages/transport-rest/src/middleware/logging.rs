//! Structured JSON logging middleware (Requirement 11.2).
//!
//! Emits a structured log entry for every request with:
//! method, path, status code, and duration in milliseconds.

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;

/// Logging middleware that emits structured JSON log entries for every request.
pub async fn logging_middleware(request: Request, next: Next) -> Response {
    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    let start = std::time::Instant::now();

    let response = next.run(request).await;

    let duration_ms = start.elapsed().as_millis();
    let status = response.status().as_u16();

    tracing::info!(
        method = %method,
        path = %path,
        status = status,
        duration_ms = duration_ms as u64,
        "request completed"
    );

    response
}
