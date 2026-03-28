//! Tests for CardDAV transport.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use std::collections::HashMap;
use std::sync::Arc;
use tower::ServiceExt;

use crate::handlers::CarddavState;
use crate::types::{AddressbookCollection, ContactResource};
use crate::CarddavTransport;

fn test_state() -> Arc<CarddavState> {
    let mut resources = HashMap::new();
    resources.insert(
        "contacts".to_string(),
        vec![
            ContactResource {
                uid: "alice.vcf".into(),
                data: "BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Alice Smith\r\nEND:VCARD".into(),
                etag: "\"etag-a\"".into(),
            },
            ContactResource {
                uid: "bob.vcf".into(),
                data: "BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Bob Jones\r\nEND:VCARD".into(),
                etag: "\"etag-b\"".into(),
            },
        ],
    );

    Arc::new(CarddavState {
        base_path: "/carddav".into(),
        principal: "/principals/testuser".into(),
        addressbooks: vec![AddressbookCollection {
            display_name: "My Contacts".into(),
            path: "contacts".into(),
            description: Some("Personal contacts".into()),
        }],
        resources,
    })
}

fn test_router() -> axum::Router {
    let config_toml = toml::Value::try_from(toml::toml! {
        host = "127.0.0.1"
        port = 5233
        base_path = "/carddav"
    })
    .unwrap();
    let transport = CarddavTransport::from_config(&config_toml).unwrap();
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
                .uri("/carddav/")
                .header("Depth", "0")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::MULTI_STATUS);
    let body_bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let xml = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert!(xml.contains("Addressbook Home"));
    assert!(!xml.contains("My Contacts"));
}

#[tokio::test]
async fn propfind_root_depth_1_lists_addressbooks() {
    let app = test_router();

    let resp = app
        .oneshot(
            Request::builder()
                .method("PROPFIND")
                .uri("/carddav/")
                .header("Depth", "1")
                .body(Body::from(""))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::MULTI_STATUS);
    let body_bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let xml = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert!(xml.contains("My Contacts"));
    assert!(xml.contains("/carddav/contacts/"));
}

#[tokio::test]
async fn propfind_addressbook_depth_1_lists_contacts() {
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
                .uri("/carddav/contacts/")
                .header("Depth", "1")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::MULTI_STATUS);
    let body_bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let xml = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert!(xml.contains("etag-a"));
    assert!(xml.contains("etag-b"));
    assert!(xml.contains("alice.vcf"));
}

#[tokio::test]
async fn propfind_nonexistent_addressbook_returns_404() {
    let app = test_router();

    let resp = app
        .oneshot(
            Request::builder()
                .method("PROPFIND")
                .uri("/carddav/nonexistent/")
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
async fn report_addressbook_query() {
    let app = test_router();
    let body = r#"<?xml version="1.0" encoding="UTF-8"?>
        <CR:addressbook-query xmlns:D="DAV:" xmlns:CR="urn:ietf:params:xml:ns:carddav">
            <D:prop>
                <D:getetag/>
                <CR:address-data/>
            </D:prop>
        </CR:addressbook-query>"#;

    let resp = app
        .oneshot(
            Request::builder()
                .method("REPORT")
                .uri("/carddav/contacts")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::MULTI_STATUS);
    let body_bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let xml = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert!(xml.contains("etag-a"));
    assert!(xml.contains("BEGIN:VCARD"));
}

#[tokio::test]
async fn report_addressbook_multiget() {
    let app = test_router();
    let body = r#"<?xml version="1.0" encoding="UTF-8"?>
        <CR:addressbook-multiget xmlns:D="DAV:" xmlns:CR="urn:ietf:params:xml:ns:carddav">
            <D:prop>
                <D:getetag/>
                <CR:address-data/>
            </D:prop>
            <D:href>/carddav/contacts/alice.vcf</D:href>
        </CR:addressbook-multiget>"#;

    let resp = app
        .oneshot(
            Request::builder()
                .method("REPORT")
                .uri("/carddav/contacts")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::MULTI_STATUS);
    let body_bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let xml = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert!(xml.contains("alice.vcf"));
    assert!(xml.contains("etag-a"));
}

// --- GET tests ---

#[tokio::test]
async fn get_contact_returns_vcard() {
    let app = test_router();

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/carddav/contacts/alice.vcf")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "text/vcard; charset=utf-8"
    );
    assert_eq!(resp.headers().get("etag").unwrap(), "\"etag-a\"");

    let body_bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let text = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert!(text.contains("BEGIN:VCARD"));
    assert!(text.contains("Alice Smith"));
}

#[tokio::test]
async fn get_nonexistent_contact_returns_404() {
    let app = test_router();

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/carddav/contacts/nope.vcf")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// --- PUT tests ---

#[tokio::test]
async fn put_new_contact_returns_created() {
    let app = test_router();
    let vcard = "BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Charlie\r\nEND:VCARD";

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/carddav/contacts/charlie.vcf")
                .body(Body::from(vcard))
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
                .uri("/carddav/contacts/alice.vcf")
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
async fn delete_existing_contact() {
    let app = test_router();

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/carddav/contacts/alice.vcf")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn delete_nonexistent_contact_returns_404() {
    let app = test_router();

    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/carddav/contacts/nope.vcf")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// --- MKCOL tests ---

#[tokio::test]
async fn mkcol_new_addressbook() {
    let app = test_router();

    let resp = app
        .oneshot(
            Request::builder()
                .method("MKCOL")
                .uri("/carddav/newbook/mkcol")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn mkcol_existing_addressbook_returns_conflict() {
    let app = test_router();

    let resp = app
        .oneshot(
            Request::builder()
                .method("MKCOL")
                .uri("/carddav/contacts/mkcol")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CONFLICT);
}
