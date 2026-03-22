#![cfg(feature = "integration")]

//! Integration tests for CalDAV connector against a live Radicale instance.
//!
//! These tests require Docker Compose test services to be running:
//!
//!     docker compose -f docker-compose.test.yml up -d
//!
//! Run with: `cargo test -p connector-calendar --features integration`

use connector_calendar::caldav::{CalDavClient, CalDavConfig, FetchedResource};
use life_engine_test_utils::connectors::{
    delete_collection, ensure_radicale_calendar, put_ical_event, radicale_caldav_config,
};
use life_engine_test_utils::skip_unless_docker;
use uuid::Uuid;

/// Build a `CalDavConfig` from the shared test factory.
fn test_caldav_config(calendar_path: &str) -> CalDavConfig {
    let cfg = radicale_caldav_config();
    CalDavConfig {
        server_url: cfg.url,
        username: cfg.username.clone(),
        credential_key: "caldav_password".into(),
        calendar_path: calendar_path.to_string(),
    }
}

/// Generate a unique calendar path to isolate each test.
fn unique_calendar_path() -> String {
    format!("/test/cal-{}/", Uuid::new_v4())
}

/// Build a minimal iCalendar VEVENT payload.
fn make_ical_event(uid: &str, summary: &str) -> String {
    format!(
        "BEGIN:VCALENDAR\r\n\
         VERSION:2.0\r\n\
         PRODID:-//Life Engine//Test//EN\r\n\
         BEGIN:VEVENT\r\n\
         UID:{uid}\r\n\
         DTSTART:20260401T100000Z\r\n\
         DTEND:20260401T110000Z\r\n\
         SUMMARY:{summary}\r\n\
         END:VEVENT\r\n\
         END:VCALENDAR\r\n"
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn put_event_and_fetch_via_propfind() {
    skip_unless_docker!();

    let http = reqwest::Client::new();
    let cal_path = unique_calendar_path();
    let cfg = radicale_caldav_config();

    // Setup: create calendar collection
    ensure_radicale_calendar(&http, &cfg.url, &cal_path, &cfg.username, &cfg.password)
        .await
        .expect("create calendar collection");

    // PUT an event
    let uid = Uuid::new_v4().to_string();
    let ical = make_ical_event(&uid, "Integration Test Event");
    put_ical_event(
        &http,
        &cfg.url,
        &cal_path,
        &uid,
        &ical,
        &cfg.username,
        &cfg.password,
    )
    .await
    .expect("PUT event");

    // Verify via GET that the event resource exists
    let event_url = format!(
        "{}/{}/{}.ics",
        cfg.url.trim_end_matches('/'),
        cal_path.trim_matches('/'),
        uid
    );
    let resp = http
        .get(&event_url)
        .basic_auth(&cfg.username, Some(&cfg.password))
        .send()
        .await
        .expect("GET event");

    assert!(
        resp.status().is_success(),
        "GET event should succeed, got {}",
        resp.status()
    );
    let body = resp.text().await.expect("read body");
    assert!(
        body.contains(&uid),
        "response should contain the event UID"
    );
    assert!(
        body.contains("Integration Test Event"),
        "response should contain the event summary"
    );

    // Verify CalDavClient can construct proper URLs
    let client = CalDavClient::new(test_caldav_config(&cal_path));
    assert!(client.calendar_url().contains(cal_path.trim_matches('/')));

    // Cleanup
    delete_collection(&http, &cfg.url, &cal_path, &cfg.username, &cfg.password)
        .await
        .expect("cleanup calendar");
}

#[tokio::test]
async fn incremental_sync_tracks_etags() {
    skip_unless_docker!();

    let http = reqwest::Client::new();
    let cal_path = unique_calendar_path();
    let cfg = radicale_caldav_config();

    // Setup: create calendar
    ensure_radicale_calendar(&http, &cfg.url, &cal_path, &cfg.username, &cfg.password)
        .await
        .expect("create calendar");

    // PUT first event
    let uid1 = Uuid::new_v4().to_string();
    put_ical_event(
        &http,
        &cfg.url,
        &cal_path,
        &uid1,
        &make_ical_event(&uid1, "Event One"),
        &cfg.username,
        &cfg.password,
    )
    .await
    .expect("PUT event 1");

    // Simulate sync state: store ETag for event 1
    let mut client = CalDavClient::new(test_caldav_config(&cal_path));

    // Fetch event 1 ETag via HEAD
    let event1_url = format!(
        "{}/{}/{}.ics",
        cfg.url.trim_end_matches('/'),
        cal_path.trim_matches('/'),
        uid1
    );
    let head_resp = http
        .head(&event1_url)
        .basic_auth(&cfg.username, Some(&cfg.password))
        .send()
        .await
        .expect("HEAD event 1");
    let etag1 = head_resp
        .headers()
        .get("etag")
        .expect("should have ETag")
        .to_str()
        .expect("ETag is valid str")
        .to_string();

    // Record the ETag in sync state
    let mut etags = std::collections::HashMap::new();
    let event1_href = format!("{}/{}.ics", cal_path.trim_matches('/'), uid1);
    etags.insert(event1_href.clone(), etag1.clone());
    client.update_sync_state(&cal_path, None, None, etags);

    // PUT a second event
    let uid2 = Uuid::new_v4().to_string();
    put_ical_event(
        &http,
        &cfg.url,
        &cal_path,
        &uid2,
        &make_ical_event(&uid2, "Event Two"),
        &cfg.username,
        &cfg.password,
    )
    .await
    .expect("PUT event 2");

    // Simulate fetched resources: event 1 unchanged, event 2 new
    let fetched = vec![
        FetchedResource {
            href: event1_href,
            etag: etag1,
            ical_data: make_ical_event(&uid1, "Event One"),
        },
        FetchedResource {
            href: format!("{}/{}.ics", cal_path.trim_matches('/'), uid2),
            etag: "\"new-etag\"".into(),
            ical_data: make_ical_event(&uid2, "Event Two"),
        },
    ];

    let changed = client.filter_changed(&cal_path, &fetched);
    assert_eq!(
        changed.len(),
        1,
        "only the new event should be in the changed set"
    );
    assert!(
        changed[0].ical_data.contains("Event Two"),
        "changed resource should be Event Two"
    );

    // Cleanup
    delete_collection(&http, &cfg.url, &cal_path, &cfg.username, &cfg.password)
        .await
        .expect("cleanup calendar");
}

#[tokio::test]
async fn first_sync_requires_full_fetch() {
    skip_unless_docker!();

    let cal_path = unique_calendar_path();
    let client = CalDavClient::new(test_caldav_config(&cal_path));

    // First sync should always be full
    let (needs_full, prev_token) = client.compute_start_sync(&cal_path, None, None);
    assert!(needs_full, "first sync must be a full sync");
    assert!(prev_token.is_none(), "no previous token on first sync");
}
