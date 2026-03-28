//! Error handling middleware (Requirement 11.3).
//!
//! Catches panics and ensures internal details are never exposed to clients.
//! All unhandled errors produce a generic 500 response with a safe error shape.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

/// Panic handler for use with `tower::ServiceBuilder::layer(CatchPanicLayer::custom(panic_handler))`.
///
/// Produces a safe 500 response that never leaks stack traces or internal state.
pub fn panic_handler(_err: Box<dyn std::any::Any + Send + 'static>) -> Response {
    tracing::error!("handler panicked — returning 500 to client");

    let body = serde_json::json!({
        "error": {
            "code": "INTERNAL_ERROR",
            "message": "An unexpected error occurred"
        }
    });

    (StatusCode::INTERNAL_SERVER_ERROR, axum::Json(body)).into_response()
}
