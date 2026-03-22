#![cfg(feature = "integration")]

//! Integration tests for CardDAV connector against a live Radicale instance.
//!
//! These tests require Docker Compose test services to be running:
//!
//!     docker compose -f docker-compose.test.yml up -d
//!
//! Run with: `cargo test -p connector-contacts --features integration`

use connector_contacts::carddav::{CardDavClient, CardDavConfig, FetchedVCard};
use life_engine_test_utils::connectors::{
    delete_collection, ensure_radicale_addressbook, put_vcard, radicale_carddav_config,
};
use life_engine_test_utils::skip_unless_docker;
use uuid::Uuid;

/// Build a `CardDavConfig` from the shared test factory.
fn test_carddav_config(addressbook_path: &str) -> CardDavConfig {
    let cfg = radicale_carddav_config();
    CardDavConfig {
        server_url: cfg.url,
        username: cfg.username.clone(),
        credential_key: "carddav_password".into(),
        addressbook_path: addressbook_path.to_string(),
    }
}

/// Generate a unique address book path to isolate each test.
fn unique_addressbook_path() -> String {
    format!("/test/contacts-{}/", Uuid::new_v4())
}

/// Build a minimal vCard 3.0 payload.
fn make_vcard(uid: &str, full_name: &str, email: &str) -> String {
    format!(
        "BEGIN:VCARD\r\n\
         VERSION:3.0\r\n\
         UID:{uid}\r\n\
         FN:{full_name}\r\n\
         N:{full_name};;;;\r\n\
         EMAIL:{email}\r\n\
         END:VCARD\r\n"
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn put_vcard_and_verify_round_trip() {
    skip_unless_docker!();

    let http = reqwest::Client::new();
    let ab_path = unique_addressbook_path();
    let cfg = radicale_carddav_config();

    // Setup: create address book collection
    ensure_radicale_addressbook(&http, &cfg.url, &ab_path, &cfg.username, &cfg.password)
        .await
        .expect("create addressbook");

    // PUT a vCard
    let uid = Uuid::new_v4().to_string();
    let vcard = make_vcard(&uid, "Alice Integration", "alice@integration.test");
    put_vcard(
        &http,
        &cfg.url,
        &ab_path,
        &uid,
        &vcard,
        &cfg.username,
        &cfg.password,
    )
    .await
    .expect("PUT vCard");

    // Verify via GET that the vCard resource exists
    let vcard_url = format!(
        "{}/{}/{}.vcf",
        cfg.url.trim_end_matches('/'),
        ab_path.trim_matches('/'),
        uid
    );
    let resp = http
        .get(&vcard_url)
        .basic_auth(&cfg.username, Some(&cfg.password))
        .send()
        .await
        .expect("GET vCard");

    assert!(
        resp.status().is_success(),
        "GET vCard should succeed, got {}",
        resp.status()
    );
    let body = resp.text().await.expect("read body");
    assert!(body.contains(&uid), "response should contain the vCard UID");
    assert!(
        body.contains("Alice Integration"),
        "response should contain the full name"
    );
    assert!(
        body.contains("alice@integration.test"),
        "response should contain the email"
    );

    // Verify CardDavClient constructs the correct URL
    let client = CardDavClient::new(test_carddav_config(&ab_path));
    assert!(client
        .addressbook_url()
        .contains(ab_path.trim_matches('/')));

    // Cleanup
    delete_collection(&http, &cfg.url, &ab_path, &cfg.username, &cfg.password)
        .await
        .expect("cleanup addressbook");
}

#[tokio::test]
async fn filter_changed_detects_new_vcards() {
    skip_unless_docker!();

    let http = reqwest::Client::new();
    let ab_path = unique_addressbook_path();
    let cfg = radicale_carddav_config();

    // Setup
    ensure_radicale_addressbook(&http, &cfg.url, &ab_path, &cfg.username, &cfg.password)
        .await
        .expect("create addressbook");

    // PUT two vCards
    let uid1 = Uuid::new_v4().to_string();
    let uid2 = Uuid::new_v4().to_string();
    put_vcard(
        &http,
        &cfg.url,
        &ab_path,
        &uid1,
        &make_vcard(&uid1, "Bob Known", "bob@test.com"),
        &cfg.username,
        &cfg.password,
    )
    .await
    .expect("PUT vCard 1");

    put_vcard(
        &http,
        &cfg.url,
        &ab_path,
        &uid2,
        &make_vcard(&uid2, "Carol New", "carol@test.com"),
        &cfg.username,
        &cfg.password,
    )
    .await
    .expect("PUT vCard 2");

    // Simulate client that already knows about vCard 1
    let mut client = CardDavClient::new(test_carddav_config(&ab_path));

    // Fetch vCard 1 ETag
    let vcard1_url = format!(
        "{}/{}/{}.vcf",
        cfg.url.trim_end_matches('/'),
        ab_path.trim_matches('/'),
        uid1
    );
    let head_resp = http
        .head(&vcard1_url)
        .basic_auth(&cfg.username, Some(&cfg.password))
        .send()
        .await
        .expect("HEAD vCard 1");
    let etag1 = head_resp
        .headers()
        .get("etag")
        .expect("should have ETag")
        .to_str()
        .expect("ETag is valid str")
        .to_string();

    client.update_etag(
        &format!("{}/{}.vcf", ab_path.trim_matches('/'), uid1),
        etag1.clone(),
    );

    // Simulate fetched list with both vCards
    let fetched = vec![
        FetchedVCard {
            href: format!("{}/{}.vcf", ab_path.trim_matches('/'), uid1),
            etag: etag1,
            data: make_vcard(&uid1, "Bob Known", "bob@test.com"),
        },
        FetchedVCard {
            href: format!("{}/{}.vcf", ab_path.trim_matches('/'), uid2),
            etag: "\"new-etag\"".into(),
            data: make_vcard(&uid2, "Carol New", "carol@test.com"),
        },
    ];

    let changed = client.filter_changed(&fetched);
    assert_eq!(
        changed.len(),
        1,
        "only the new vCard should be in the changed set"
    );
    assert!(
        changed[0].data.contains("Carol New"),
        "changed resource should be Carol"
    );

    // Cleanup
    delete_collection(&http, &cfg.url, &ab_path, &cfg.username, &cfg.password)
        .await
        .expect("cleanup addressbook");
}

#[tokio::test]
async fn reset_sync_state_clears_etags() {
    skip_unless_docker!();

    let ab_path = unique_addressbook_path();
    let mut client = CardDavClient::new(test_carddav_config(&ab_path));

    client.update_sync_token("token-1".into());
    client.update_ctag("ctag-1".into());
    client.update_etag("/contact/1.vcf", "etag-1".into());

    client.reset_sync_state();

    assert!(client.sync_state().sync_token.is_none());
    assert!(client.sync_state().ctag.is_none());
    assert!(client.sync_state().etags.is_empty());
}
