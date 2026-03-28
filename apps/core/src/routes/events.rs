//! Server-Sent Events (SSE) stream for real-time event delivery.

use crate::message_bus::BusEvent;
use crate::routes::health::AppState;

use axum::extract::{Query, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::Stream;
use serde::Deserialize;
use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

/// Query parameters for the SSE event stream.
#[derive(Debug, Deserialize)]
pub struct EventStreamParams {
    /// Filter events to a specific collection.
    pub collection: Option<String>,
    /// Filter events to a specific event type.
    pub event_type: Option<String>,
}

/// GET /api/events/stream — SSE event stream.
pub async fn event_stream(
    State(state): State<AppState>,
    Query(params): Query<EventStreamParams>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.message_bus.subscribe();
    let stream = BroadcastStream::new(rx);

    let filtered = stream.filter_map(move |result| {
        let event = match result {
            Ok(e) => e,
            Err(_) => return None,
        };

        // Determine the SSE event type name; skip internal-only events.
        let event_type = match &event {
            BusEvent::NewRecords { .. } => "new_records",
            BusEvent::SyncComplete { .. } => "sync_complete",
            BusEvent::PluginLoaded { .. } => "plugin_loaded",
            BusEvent::PluginError { .. } => "plugin_error",
            BusEvent::RecordChanged { .. }
            | BusEvent::RecordDeleted { .. }
            | BusEvent::BlobStored { .. }
            | BusEvent::BlobDeleted { .. }
            | BusEvent::AuthSuccess { .. }
            | BusEvent::AuthFailure { .. }
            | BusEvent::CredentialEvent { .. }
            | BusEvent::PluginUnloaded { .. }
            | BusEvent::ConnectorEvent { .. } => {
                return None;
            }
        };

        // Apply collection filter.
        if let Some(ref col) = params.collection
            && let BusEvent::NewRecords { collection, .. } = &event
            && collection != col
        {
            return None;
        }

        // Apply event_type filter.
        if let Some(ref et) = params.event_type
            && event_type != et.as_str()
        {
            return None;
        }

        let payload = match serde_json::to_string(&event) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "failed to serialize SSE event payload");
                return None;
            }
        };

        let sse_event = Event::default()
            .event(event_type)
            .data(payload);

        Some(Ok(sse_event))
    });

    Sse::new(filtered).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::middleware::auth_middleware;
    use crate::message_bus::MessageBus;
    use crate::test_helpers::{create_auth_state, default_app_state, generate_test_token};
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use axum::Router;
    use http_body_util::BodyExt;
    use std::sync::Arc;
    use tower::ServiceExt;

    async fn setup_sse_app() -> (Router, String, Arc<MessageBus>) {
        let message_bus = Arc::new(MessageBus::new());
        let (auth_state, provider) = create_auth_state();

        let mut state = default_app_state();
        state.message_bus = Arc::clone(&message_bus);

        let app = Router::new()
            .route("/api/events/stream", get(event_stream))
            .with_state(state)
            .layer(axum::middleware::from_fn_with_state(
                auth_state,
                auth_middleware,
            ));

        let token = generate_test_token(&provider).await;

        (app, token, message_bus)
    }

    #[tokio::test]
    async fn sse_stream_connects() {
        let (app, token, message_bus) = setup_sse_app().await;

        // Publish an event before connecting (will be missed, but tests connection).
        let req = Request::builder()
            .uri("/api/events/stream")
            .header("Authorization", format!("Bearer {token}"))
            .header("Accept", "text/event-stream")
            .body(Body::empty())
            .unwrap();

        // Use a timeout to prevent hanging.
        let resp = tokio::time::timeout(
            Duration::from_secs(2),
            app.oneshot(req),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        // Publish an event after connection.
        message_bus.publish(BusEvent::NewRecords {
            collection: "tasks".into(),
            count: 1,
        });

        // Read the first frame from the SSE stream.
        let mut body = resp.into_body();
        let frame = tokio::time::timeout(Duration::from_secs(2), body.frame())
            .await
            .unwrap()
            .unwrap()
            .unwrap();

        let data = frame.into_data().unwrap();
        let text = String::from_utf8(data.to_vec()).unwrap();
        assert!(text.contains("new_records"));
    }

    #[tokio::test]
    async fn sse_stream_requires_auth() {
        let (app, _token, _bus) = setup_sse_app().await;

        let req = Request::builder()
            .uri("/api/events/stream")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), axum::http::StatusCode::UNAUTHORIZED);
    }
}
