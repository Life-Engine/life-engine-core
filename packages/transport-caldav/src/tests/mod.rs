//! Tests for CalDAV transport.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use std::collections::HashMap;
use std::sync::Arc;
use tower::ServiceExt;

use crate::handlers::CaldavState;
use crate::types::{CalendarCollection, CalendarResource};
use crate::CaldavTransport;

fn test_state() -> Arc<CaldavState> {
    let mut resources = HashMap::new();
    resources.insert(
        "default".to_string(),
        vec![
            CalendarResource {
                uid: "event1.ics".into(),
                data: "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Test\r\nEND:VEVENT\r\nEND:VCALENDAR".into(),
                etag: "\"etag-1\"".into(),
            },
            CalendarResource {
                uid: "event2.ics".into(),
                data: "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Meeting\r\nEND:VEVENT\r\nEND:VCALENDAR".into(),
                etag: "\"etag-2\"".into(),
            },
        ],
    );

    Arc::new(CaldavState {
        base_path: "/caldav".into(),
        principal: "/principals/testuser".into(),
        calendars: vec![CalendarCollection {
            display_name: "Default Calendar".into(),
            path: "default".into(),
            description: Some("My calendar".into()),
            color: None,
        }],
        resources,
    })
}

fn test_router() -> axum::Router {
    let config_toml = toml::Value::try_from(toml::toml! {
        host = "127.0.0.1"
        port = 5232
        base_path = "/caldav"
    })
    .unwrap();
    let transport = CaldavTransport::from_config(&config_toml).unwrap();
    transport.build_router(test_state())
}

// --- PROPFIND tests ---

#[tokio::test]
async fn propfind_root_depth_0() {
    let app = test_router();
    let body = r#"<?xml version="1.0" encoding="UTF-8"?>
        <D:propfind xmlns:D="DAV:">
            <D:allprop/>
        </D:propfind>"#;

    let resp = app
        .oneshot(
            Request::builder()
                .method("PROPFIND")
                .uri("/caldav/")
                .header("Depth", "0")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::MULTI_STATUS);
    let body_bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let xml = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert!(xml.contains("<D:multistatus"));
    assert!(xml.contains("Calendar Home"));
    // Depth 0 should NOT include child collections
    assert!(!xml.contains("Default Calendar"));
}

#[tokio::test]
async fn propfind_root_depth_1_lists_calendars() {
    let app = test_router();

    let resp = app
        .oneshot(
            Request::builder()
                .method("PROPFIND")
                .uri("/caldav/")
                .header("Depth", "1")
                .body(Body::from(""))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::MULTI_STATUS);
    let body_bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let xml = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert!(xml.contains("Default Calendar"));
    assert!(xml.contains("/caldav/default/"));
}

#[tokio::test]
async fn propfind_collection_depth_1_lists_resources() {
    let app = test_router();
    let body = r#"<?xml version="1.0" encoding="UTF-8"?>
        <D:propfind xmlns:D="DAV:">
            <D:prop>
                <D:getetag/>
            </D:prop>
        </D:propfind>"#;

    let resp = app
        .oneshot(
            Request::builder()
                .method("PROPFIND")
                .uri("/caldav/default/")
                .header("Depth", "1")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::MULTI_STATUS);
    let body_bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let xml = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert!(xml.contains("etag-1"));
    assert!(xml.contains("etag-2"));
    assert!(xml.contains("event1.ics"));
}

#[tokio::test]
async fn propfind_nonexistent_collection_returns_404() {
    let app = test_router();

    let resp = app
        .oneshot(
            Request::builder()
                .method("PROPFIND")
                .uri("/caldav/nonexistent/")
                .header("Depth", "0")
                .body(Body::from(""))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// --- REPORT tests ---

#[tokio::test]
async fn report_calendar_query() {
    let app = test_router();
    let body = r#"<?xml version="1.0" encoding="UTF-8"?>
        <C:calendar-query xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
            <D:prop>
                <D:getetag/>
                <C:calendar-data/>
            </D:prop>
        </C:calendar-query>"#;

    let resp = app
        .oneshot(
            Request::builder()
                .method("REPORT")
                .uri("/caldav/default")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::MULTI_STATUS);
    let body_bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let xml = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert!(xml.contains("etag-1"));
    assert!(xml.contains("BEGIN:VCALENDAR"));
}

#[tokio::test]
async fn report_calendar_multiget() {
    let app = test_router();
    let body = r#"<?xml version="1.0" encoding="UTF-8"?>
        <C:calendar-multiget xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
            <D:prop>
                <D:getetag/>
                <C:calendar-data/>
            </D:prop>
            <D:href>/caldav/default/event1.ics</D:href>
        </C:calendar-multiget>"#;

    let resp = app
        .oneshot(
            Request::builder()
                .method("REPORT")
                .uri("/caldav/default")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::MULTI_STATUS);
    let body_bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let xml = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert!(xml.contains("etag-1"));
    // Should only contain event1, not event2
    assert!(xml.contains("event1.ics"));
}

// --- GET tests ---

#[tokio::test]
async fn get_resource_returns_ical() {
    let app = test_router();

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/caldav/default/event1.ics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "text/calendar; charset=utf-8"
    );
    assert_eq!(resp.headers().get("etag").unwrap(), "\"etag-1\"");

    let body_bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let text = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert!(text.contains("BEGIN:VCALENDAR"));
}

#[tokio::test]
async fn get_nonexistent_resource_returns_404() {
    let app = test_router();

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/caldav/default/nope.ics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// --- PUT tests ---

#[tokio::test]
async fn put_new_resource_returns_created() {
    let app = test_router();
    let ical = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:New\r\nEND:VEVENT\r\nEND:VCALENDAR";

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/caldav/default/new-event.ics")
                .body(Body::from(ical))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    assert!(resp.headers().get("etag").is_some());
}

#[tokio::test]
async fn put_with_if_match_mismatch_returns_precondition_failed() {
    let app = test_router();

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/caldav/default/event1.ics")
                .header("If-Match", "\"wrong-etag\"")
                .body(Body::from("data"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::PRECONDITION_FAILED);
}

// --- DELETE tests ---

#[tokio::test]
async fn delete_existing_resource() {
    let app = test_router();

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/caldav/default/event1.ics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn delete_nonexistent_resource_returns_404() {
    let app = test_router();

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/caldav/default/nope.ics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// --- MKCALENDAR tests ---

#[tokio::test]
async fn mkcalendar_new_collection() {
    let app = test_router();

    let resp = app
        .oneshot(
            Request::builder()
                .method("MKCALENDAR")
                .uri("/caldav/newcal/mkcalendar")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn mkcalendar_existing_collection_returns_conflict() {
    let app = test_router();

    let resp = app
        .oneshot(
            Request::builder()
                .method("MKCALENDAR")
                .uri("/caldav/default/mkcalendar")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CONFLICT);
}
