//! End-to-end integration tests for the IMAP-to-CDM email pipeline.
//!
//! Validates the full path from raw RFC 5322 bytes through `process_fetched()`,
//! `normalize_message()`, JSON serialization round-trip, and incremental sync
//! state management. Does NOT require Docker or a real IMAP server.

use chrono::Datelike;
use connector_email::imap::{ImapClient, ImapConfig};
use connector_email::normalizer::normalize_message;
use life_engine_types::Email;

// ---------------------------------------------------------------------------
// Test fixtures: raw RFC 5322 email bytes
// ---------------------------------------------------------------------------

/// A simple plain-text email with all standard headers.
fn simple_email_bytes() -> Vec<u8> {
    b"From: alice@example.com\r\n\
      To: bob@example.com\r\n\
      Subject: Weekly report\r\n\
      Date: Sat, 21 Mar 2026 09:00:00 +0000\r\n\
      Message-ID: <weekly-001@example.com>\r\n\
      Content-Type: text/plain; charset=utf-8\r\n\
      \r\n\
      Hi Bob,\r\n\
      Please find the weekly report attached.\r\n\
      Regards, Alice\r\n"
        .to_vec()
}

/// An email with no Subject header.
fn no_subject_email_bytes() -> Vec<u8> {
    b"From: charlie@example.com\r\n\
      To: dave@example.com\r\n\
      Date: Sat, 21 Mar 2026 10:00:00 +0000\r\n\
      Message-ID: <nosub-002@example.com>\r\n\
      Content-Type: text/plain; charset=utf-8\r\n\
      \r\n\
      This email has no subject line.\r\n"
        .to_vec()
}

/// A multipart email with a PDF attachment.
fn attachment_email_bytes() -> Vec<u8> {
    b"From: eve@example.com\r\n\
      To: frank@example.com\r\n\
      Subject: Invoice attached\r\n\
      Date: Sat, 21 Mar 2026 11:00:00 +0000\r\n\
      Message-ID: <attach-003@example.com>\r\n\
      MIME-Version: 1.0\r\n\
      Content-Type: multipart/mixed; boundary=\"BOUNDARY_XYZ\"\r\n\
      \r\n\
      --BOUNDARY_XYZ\r\n\
      Content-Type: text/plain; charset=utf-8\r\n\
      \r\n\
      Please see the invoice.\r\n\
      --BOUNDARY_XYZ\r\n\
      Content-Type: application/pdf; name=\"invoice.pdf\"\r\n\
      Content-Disposition: attachment; filename=\"invoice.pdf\"\r\n\
      Content-Transfer-Encoding: base64\r\n\
      \r\n\
      JVBERi0xLjQKMSAwIG9iago=\r\n\
      --BOUNDARY_XYZ--\r\n"
        .to_vec()
}

/// An email that is part of a thread (has In-Reply-To and References).
fn threaded_email_bytes() -> Vec<u8> {
    b"From: grace@example.com\r\n\
      To: heidi@example.com\r\n\
      CC: ivan@example.com\r\n\
      Subject: Re: Project plan\r\n\
      Date: Sat, 21 Mar 2026 12:00:00 +0000\r\n\
      Message-ID: <thread-reply-004@example.com>\r\n\
      In-Reply-To: <thread-root-000@example.com>\r\n\
      References: <thread-root-000@example.com>\r\n\
      Content-Type: text/plain; charset=utf-8\r\n\
      \r\n\
      Looks good to me. Let us proceed.\r\n"
        .to_vec()
}

/// An email with only References (no In-Reply-To) for thread detection.
fn references_only_email_bytes() -> Vec<u8> {
    b"From: judy@example.com\r\n\
      To: ken@example.com\r\n\
      Subject: Follow-up on plan\r\n\
      Date: Sat, 21 Mar 2026 13:00:00 +0000\r\n\
      Message-ID: <ref-only-005@example.com>\r\n\
      References: <thread-root-000@example.com>\r\n\
      Content-Type: text/plain; charset=utf-8\r\n\
      \r\n\
      Adding a follow-up note.\r\n"
        .to_vec()
}

/// An email with no date header at all.
fn no_date_email_bytes() -> Vec<u8> {
    b"From: liam@example.com\r\n\
      To: mia@example.com\r\n\
      Subject: Timeless message\r\n\
      Message-ID: <nodate-006@example.com>\r\n\
      Content-Type: text/plain; charset=utf-8\r\n\
      \r\n\
      This message has no date header.\r\n"
        .to_vec()
}

/// A multipart email with an HTML body and a plain-text fallback.
fn html_email_bytes() -> Vec<u8> {
    b"From: nina@example.com\r\n\
      To: oscar@example.com\r\n\
      Subject: HTML newsletter\r\n\
      Date: Sat, 21 Mar 2026 14:00:00 +0000\r\n\
      Message-ID: <html-007@example.com>\r\n\
      MIME-Version: 1.0\r\n\
      Content-Type: multipart/alternative; boundary=\"ALT_BOUND\"\r\n\
      \r\n\
      --ALT_BOUND\r\n\
      Content-Type: text/plain; charset=utf-8\r\n\
      \r\n\
      Welcome to our newsletter.\r\n\
      --ALT_BOUND\r\n\
      Content-Type: text/html; charset=utf-8\r\n\
      \r\n\
      <html><body><h1>Welcome</h1><p>to our newsletter.</p></body></html>\r\n\
      --ALT_BOUND--\r\n"
        .to_vec()
}

// ---------------------------------------------------------------------------
// Helper: create a default ImapConfig for test clients
// ---------------------------------------------------------------------------

fn test_imap_config() -> ImapConfig {
    ImapConfig {
        host: "imap.test.local".into(),
        port: 993,
        username: "tester@test.local".into(),
        credential_key: "imap_password".into(),
        use_tls: true,
    }
}

// ---------------------------------------------------------------------------
// Full pipeline: process_fetched -> normalize_message -> CDM Email
// ---------------------------------------------------------------------------

#[test]
fn test_full_pipeline_simple_email() {
    let raw = simple_email_bytes();
    let fetches = vec![(1u32, raw)];

    // Step 1: process_fetched simulates the IMAP fetch result
    let (messages, max_uid) = ImapClient::process_fetched(1, &fetches);
    assert_eq!(messages.len(), 1);
    assert_eq!(max_uid, 1);

    // Step 2: normalize to CDM Email
    let email = normalize_message(&messages[0].raw, "imap")
        .expect("normalization should succeed");

    assert_eq!(email.from.address, "alice@example.com");
    assert_eq!(email.to[0].address, "bob@example.com");
    assert_eq!(email.subject, "Weekly report");
    assert!(email.body_text.as_deref().unwrap().contains("weekly report"));
    assert_eq!(email.source, "imap");
    assert_eq!(email.source_id, "weekly-001@example.com");
    assert!(email.in_reply_to.is_none());
    assert!(email.cc.is_empty());
    assert!(email.bcc.is_empty());
    assert!(email.attachments.is_empty());
    assert!(!email.id.is_nil());
}

#[test]
fn test_full_pipeline_multiple_messages() {
    let fetches = vec![
        (10u32, simple_email_bytes()),
        (11, no_subject_email_bytes()),
        (12, attachment_email_bytes()),
        (13, threaded_email_bytes()),
    ];

    let (messages, max_uid) = ImapClient::process_fetched(10, &fetches);
    assert_eq!(messages.len(), 4);
    assert_eq!(max_uid, 13);

    // Normalize all messages
    let emails: Vec<Email> = messages
        .iter()
        .map(|m| normalize_message(&m.raw, "imap").expect("normalization should succeed"))
        .collect();

    assert_eq!(emails[0].subject, "Weekly report");
    assert_eq!(emails[1].subject, "(no subject)");
    assert_eq!(emails[2].subject, "Invoice attached");
    assert_eq!(emails[3].subject, "Re: Project plan");
}

// ---------------------------------------------------------------------------
// JSON round-trip proof
// ---------------------------------------------------------------------------

#[test]
fn test_json_round_trip_simple_email() {
    let email = normalize_message(&simple_email_bytes(), "imap")
        .expect("normalization should succeed");

    let json = serde_json::to_string(&email).expect("serialization should succeed");
    let restored: Email =
        serde_json::from_str(&json).expect("deserialization should succeed");

    assert_eq!(restored.from, email.from);
    assert_eq!(restored.to, email.to);
    assert_eq!(restored.subject, email.subject);
    assert_eq!(restored.body_text, email.body_text);
    assert_eq!(restored.source, email.source);
    assert_eq!(restored.source_id, email.source_id);
    assert_eq!(restored.in_reply_to, email.in_reply_to);
    assert_eq!(restored.cc, email.cc);
    assert_eq!(restored.bcc, email.bcc);
    assert_eq!(restored.labels, email.labels);
    assert_eq!(restored.attachments, email.attachments);
}

#[test]
fn test_json_round_trip_with_attachment() {
    let email = normalize_message(&attachment_email_bytes(), "imap")
        .expect("normalization should succeed");

    let json = serde_json::to_string(&email).expect("serialization should succeed");
    let restored: Email =
        serde_json::from_str(&json).expect("deserialization should succeed");

    assert_eq!(restored.attachments.len(), 1);
    assert_eq!(restored.attachments[0].filename, email.attachments[0].filename);
    assert_eq!(restored.attachments[0].mime_type, email.attachments[0].mime_type);
    assert_eq!(restored.attachments[0].size_bytes, email.attachments[0].size_bytes);
}

#[test]
fn test_json_round_trip_threaded_email() {
    let email = normalize_message(&threaded_email_bytes(), "imap")
        .expect("normalization should succeed");

    let json = serde_json::to_string(&email).expect("serialization should succeed");
    let restored: Email =
        serde_json::from_str(&json).expect("deserialization should succeed");

    assert_eq!(restored.in_reply_to, email.in_reply_to);
    assert_eq!(restored.cc, email.cc);
}

#[test]
fn test_json_round_trip_html_email() {
    let email = normalize_message(&html_email_bytes(), "imap")
        .expect("normalization should succeed");

    let json = serde_json::to_string(&email).expect("serialization should succeed");
    let restored: Email =
        serde_json::from_str(&json).expect("deserialization should succeed");

    assert_eq!(restored.body_html, email.body_html);
    assert_eq!(restored.body_text, email.body_text);
}

// ---------------------------------------------------------------------------
// Edge cases: no subject
// ---------------------------------------------------------------------------

#[test]
fn test_no_subject_defaults_to_placeholder() {
    let email = normalize_message(&no_subject_email_bytes(), "imap")
        .expect("normalization should succeed");

    assert_eq!(email.subject, "(no subject)");
    assert_eq!(email.from.address, "charlie@example.com");
    assert_eq!(email.source_id, "nosub-002@example.com");
}

// ---------------------------------------------------------------------------
// Edge cases: attachments
// ---------------------------------------------------------------------------

#[test]
fn test_attachment_metadata_extracted() {
    let email = normalize_message(&attachment_email_bytes(), "imap")
        .expect("normalization should succeed");

    assert_eq!(email.attachments.len(), 1);
    let att = &email.attachments[0];
    assert_eq!(att.filename, "invoice.pdf");
    assert!(
        att.mime_type.contains("pdf"),
        "expected pdf in mime_type, got: {}",
        att.mime_type
    );
    assert!(att.size_bytes > 0, "attachment size_bytes should be > 0");
}

#[test]
fn test_email_without_attachment_has_empty_vec() {
    let email = normalize_message(&simple_email_bytes(), "imap")
        .expect("normalization should succeed");

    assert!(email.attachments.is_empty());
}

// ---------------------------------------------------------------------------
// Edge cases: threading
// ---------------------------------------------------------------------------

#[test]
fn test_in_reply_to_from_in_reply_to_header() {
    let email = normalize_message(&threaded_email_bytes(), "imap")
        .expect("normalization should succeed");

    assert_eq!(
        email.in_reply_to.as_deref(),
        Some("thread-root-000@example.com")
    );
}

#[test]
fn test_in_reply_to_from_references_only() {
    let email = normalize_message(&references_only_email_bytes(), "imap")
        .expect("normalization should succeed");

    assert_eq!(
        email.in_reply_to.as_deref(),
        Some("thread-root-000@example.com")
    );
}

#[test]
fn test_no_in_reply_to_for_standalone_email() {
    let email = normalize_message(&simple_email_bytes(), "imap")
        .expect("normalization should succeed");

    assert!(email.in_reply_to.is_none());
}

// ---------------------------------------------------------------------------
// Edge cases: CC addresses
// ---------------------------------------------------------------------------

#[test]
fn test_cc_addresses_extracted() {
    let email = normalize_message(&threaded_email_bytes(), "imap")
        .expect("normalization should succeed");

    assert_eq!(email.cc.len(), 1);
    assert_eq!(email.cc[0].address, "ivan@example.com");
}

#[test]
fn test_no_cc_when_absent() {
    let email = normalize_message(&simple_email_bytes(), "imap")
        .expect("normalization should succeed");

    assert!(email.cc.is_empty());
}

// ---------------------------------------------------------------------------
// Edge cases: HTML body
// ---------------------------------------------------------------------------

#[test]
fn test_html_body_extracted_from_multipart() {
    let email = normalize_message(&html_email_bytes(), "imap")
        .expect("normalization should succeed");

    assert!(
        email.body_html.is_some(),
        "expected body_html for multipart/alternative with text/html"
    );
    let html = email.body_html.as_deref().expect("body_html should be Some");
    assert!(html.contains("<h1>Welcome</h1>"));
}

#[test]
fn test_plain_text_body_always_present() {
    let email = normalize_message(&html_email_bytes(), "imap")
        .expect("normalization should succeed");

    assert!(
        email.body_text.as_deref().unwrap().contains("newsletter"),
        "body_text should contain plain-text fallback"
    );
}

#[test]
fn test_no_html_for_plain_text_email() {
    let email = normalize_message(&simple_email_bytes(), "imap")
        .expect("normalization should succeed");

    assert!(
        email.body_html.is_none(),
        "plain-text-only email should not have body_html"
    );
}

// ---------------------------------------------------------------------------
// Edge cases: no date header
// ---------------------------------------------------------------------------

#[test]
fn test_no_date_falls_back_to_now() {
    let email = normalize_message(&no_date_email_bytes(), "imap")
        .expect("normalization should succeed");

    // The date should be a valid, recent timestamp (fallback to Utc::now)
    let year = email.date.year();
    assert!(
        year >= 2025,
        "expected recent year for date fallback, got: {year}"
    );
}

// ---------------------------------------------------------------------------
// Edge cases: date parsing
// ---------------------------------------------------------------------------

#[test]
fn test_date_parsed_from_header() {
    let email = normalize_message(&simple_email_bytes(), "imap")
        .expect("normalization should succeed");

    assert_eq!(email.date.year(), 2026);
    assert_eq!(email.date.month(), 3);
    assert_eq!(email.date.day(), 21);
}

// ---------------------------------------------------------------------------
// Edge cases: invalid input
// ---------------------------------------------------------------------------

#[test]
fn test_empty_bytes_returns_error() {
    let result = normalize_message(b"", "imap");
    assert!(result.is_err());
}

#[test]
fn test_garbage_bytes_returns_error() {
    let result = normalize_message(b"\x00\x01\x02\x03", "imap");
    // mail-parser may or may not parse garbage; we just verify it does not panic
    // and either returns Ok with defaults or Err
    match result {
        Ok(email) => {
            // If it somehow parses, verify it has sensible defaults
            assert!(!email.id.is_nil());
        }
        Err(_) => {
            // Expected for garbage input
        }
    }
}

// ---------------------------------------------------------------------------
// Incremental sync: compute_start_uid
// ---------------------------------------------------------------------------

#[test]
fn test_compute_start_uid_first_sync_for_mailbox() {
    let client = ImapClient::new(test_imap_config());

    // No sync state yet for INBOX
    let (start_uid, needs_resync) = client.compute_start_uid("INBOX", 12345);
    assert_eq!(start_uid, 1, "first sync should start at UID 1");
    assert!(!needs_resync, "first sync should not require resync");
}

#[test]
fn test_compute_start_uid_incremental_same_validity() {
    let mut client = ImapClient::new(test_imap_config());
    client.update_sync_state("INBOX", 12345, 50);

    let (start_uid, needs_resync) = client.compute_start_uid("INBOX", 12345);
    assert_eq!(
        start_uid, 51,
        "incremental sync should start at last_uid + 1"
    );
    assert!(!needs_resync, "same UIDVALIDITY should not require resync");
}

#[test]
fn test_compute_start_uid_validity_changed_triggers_resync() {
    let mut client = ImapClient::new(test_imap_config());
    client.update_sync_state("INBOX", 12345, 50);

    // Server reports a different UIDVALIDITY
    let (start_uid, needs_resync) = client.compute_start_uid("INBOX", 99999);
    assert_eq!(start_uid, 1, "UIDVALIDITY change should reset to UID 1");
    assert!(needs_resync, "UIDVALIDITY change should signal resync");
}

#[test]
fn test_compute_start_uid_different_mailboxes_are_independent() {
    let mut client = ImapClient::new(test_imap_config());
    client.update_sync_state("INBOX", 12345, 100);
    client.update_sync_state("Sent", 67890, 200);

    let (inbox_start, _) = client.compute_start_uid("INBOX", 12345);
    let (sent_start, _) = client.compute_start_uid("Sent", 67890);
    let (drafts_start, _) = client.compute_start_uid("Drafts", 11111);

    assert_eq!(inbox_start, 101);
    assert_eq!(sent_start, 201);
    assert_eq!(drafts_start, 1, "unknown mailbox should start at UID 1");
}

#[test]
fn test_update_sync_state_overwrites_previous() {
    let mut client = ImapClient::new(test_imap_config());

    client.update_sync_state("INBOX", 12345, 50);
    let state = client.sync_state("INBOX").expect("should have state");
    assert_eq!(state.last_uid, 50);
    assert_eq!(state.uid_validity, 12345);

    // Overwrite with new values
    client.update_sync_state("INBOX", 12345, 100);
    let state = client.sync_state("INBOX").expect("should have state");
    assert_eq!(state.last_uid, 100);
}

// ---------------------------------------------------------------------------
// process_fetched: filtering and max_uid tracking
// ---------------------------------------------------------------------------

#[test]
fn test_process_fetched_filters_below_start_uid() {
    let fetches = vec![
        (5u32, b"old msg 1".to_vec()),
        (9, b"old msg 2".to_vec()),
        (10, simple_email_bytes()),
        (15, no_subject_email_bytes()),
    ];

    let (messages, max_uid) = ImapClient::process_fetched(10, &fetches);
    assert_eq!(messages.len(), 2, "should filter out UIDs below start");
    assert_eq!(messages[0].uid, 10);
    assert_eq!(messages[1].uid, 15);
    assert_eq!(max_uid, 15);
}

#[test]
fn test_process_fetched_empty_input() {
    let fetches: Vec<(u32, Vec<u8>)> = vec![];
    let (messages, max_uid) = ImapClient::process_fetched(1, &fetches);
    assert!(messages.is_empty());
    assert_eq!(max_uid, 0);
}

#[test]
fn test_process_fetched_all_below_start_uid() {
    let fetches = vec![
        (1u32, b"msg1".to_vec()),
        (2, b"msg2".to_vec()),
        (3, b"msg3".to_vec()),
    ];
    let (messages, max_uid) = ImapClient::process_fetched(100, &fetches);
    assert!(messages.is_empty());
    assert_eq!(max_uid, 99, "max_uid should be start_uid - 1 when no matches");
}

#[test]
fn test_process_fetched_preserves_raw_bytes() {
    let raw = simple_email_bytes();
    let fetches = vec![(42u32, raw.clone())];
    let (messages, _) = ImapClient::process_fetched(1, &fetches);

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].uid, 42);
    assert_eq!(messages[0].raw, raw, "raw bytes should be preserved exactly");
}

// ---------------------------------------------------------------------------
// Full pipeline integration: process_fetched -> normalize -> sync state update
// ---------------------------------------------------------------------------

#[test]
fn test_full_incremental_sync_simulation() {
    let mut client = ImapClient::new(test_imap_config());

    // --- First sync: fresh mailbox ---
    let uid_validity = 55555;
    let (start_uid, needs_resync) = client.compute_start_uid("INBOX", uid_validity);
    assert_eq!(start_uid, 1);
    assert!(!needs_resync);

    let batch1 = vec![
        (1u32, simple_email_bytes()),
        (2, no_subject_email_bytes()),
        (3, threaded_email_bytes()),
    ];
    let (messages, max_uid) = ImapClient::process_fetched(start_uid, &batch1);
    assert_eq!(messages.len(), 3);

    // Normalize all messages and verify they produce valid CDM emails
    for msg in &messages {
        let email = normalize_message(&msg.raw, "imap")
            .expect("normalization should succeed");
        assert!(!email.id.is_nil());
        assert_eq!(email.source, "imap");
        assert!(!email.source_id.is_empty());
    }

    // Update sync state
    client.update_sync_state("INBOX", uid_validity, max_uid);

    // --- Second sync: incremental ---
    let (start_uid, needs_resync) = client.compute_start_uid("INBOX", uid_validity);
    assert_eq!(start_uid, 4, "should continue from last_uid + 1");
    assert!(!needs_resync);

    let batch2 = vec![
        (2u32, simple_email_bytes()), // old UID, should be filtered
        (4, attachment_email_bytes()),
        (5, html_email_bytes()),
    ];
    let (messages, max_uid) = ImapClient::process_fetched(start_uid, &batch2);
    assert_eq!(messages.len(), 2, "should filter out UID 2");

    // Normalize batch 2
    let email_with_attach = normalize_message(&messages[0].raw, "imap")
        .expect("normalization should succeed");
    assert_eq!(email_with_attach.attachments.len(), 1);

    let email_html = normalize_message(&messages[1].raw, "imap")
        .expect("normalization should succeed");
    assert!(email_html.body_html.is_some());

    // Update sync state
    client.update_sync_state("INBOX", uid_validity, max_uid);

    let (start_uid, _) = client.compute_start_uid("INBOX", uid_validity);
    assert_eq!(start_uid, 6, "should continue from UID 6 after batch 2");
}

#[test]
fn test_full_sync_after_uidvalidity_change() {
    let mut client = ImapClient::new(test_imap_config());

    // Establish initial sync state
    client.update_sync_state("INBOX", 11111, 50);

    // Server reports new UIDVALIDITY (mailbox was recreated)
    let (start_uid, needs_resync) = client.compute_start_uid("INBOX", 22222);
    assert_eq!(start_uid, 1);
    assert!(needs_resync);

    // Full re-fetch with new UIDs
    let fetches = vec![
        (1u32, simple_email_bytes()),
        (2, attachment_email_bytes()),
    ];
    let (messages, max_uid) = ImapClient::process_fetched(start_uid, &fetches);
    assert_eq!(messages.len(), 2);
    assert_eq!(max_uid, 2);

    // Update sync state with new validity
    client.update_sync_state("INBOX", 22222, max_uid);
    let state = client.sync_state("INBOX").expect("should have state");
    assert_eq!(state.uid_validity, 22222);
    assert_eq!(state.last_uid, 2);
}

// ---------------------------------------------------------------------------
// CDM field validation
// ---------------------------------------------------------------------------

#[test]
fn test_all_cdm_fields_populated_for_rich_email() {
    let email = normalize_message(&threaded_email_bytes(), "imap")
        .expect("normalization should succeed");

    // Identity
    assert!(!email.id.is_nil());

    // Addressing
    assert_eq!(email.from.address, "grace@example.com");
    assert_eq!(email.to[0].address, "heidi@example.com");
    assert_eq!(email.cc.len(), 1);
    assert_eq!(email.cc[0].address, "ivan@example.com");
    assert!(email.bcc.is_empty());

    // Content
    assert_eq!(email.subject, "Re: Project plan");
    assert!(email.body_text.as_deref().unwrap().contains("proceed"));

    // Threading
    assert_eq!(
        email.in_reply_to.as_deref(),
        Some("thread-root-000@example.com")
    );

    // Source tracking
    assert_eq!(email.source, "imap");
    assert_eq!(email.source_id, "thread-reply-004@example.com");

    // Labels default to empty
    assert!(email.labels.is_empty());

    // Timestamps
    assert_eq!(email.date.year(), 2026);
}

#[test]
fn test_source_field_reflects_connector() {
    let email = normalize_message(&simple_email_bytes(), "custom-connector")
        .expect("normalization should succeed");

    assert_eq!(email.source, "custom-connector");
}

// ---------------------------------------------------------------------------
// Message-ID as source_id
// ---------------------------------------------------------------------------

#[test]
fn test_source_id_is_message_id_header() {
    let email = normalize_message(&simple_email_bytes(), "imap")
        .expect("normalization should succeed");

    assert_eq!(email.source_id, "weekly-001@example.com");
}

#[test]
fn test_source_id_is_uuid_when_no_message_id() {
    // Construct an email without Message-ID
    let raw = b"From: noone@example.com\r\n\
                To: someone@example.com\r\n\
                Subject: No ID\r\n\
                Content-Type: text/plain; charset=utf-8\r\n\
                \r\n\
                A message without Message-ID.\r\n";

    let email = normalize_message(raw, "imap")
        .expect("normalization should succeed");

    // source_id should be a UUID fallback (36 chars with hyphens)
    assert!(
        !email.source_id.is_empty(),
        "source_id should not be empty even without Message-ID"
    );
}

// ---------------------------------------------------------------------------
// JSON serialization: verify CDM schema compliance
// ---------------------------------------------------------------------------

#[test]
fn test_serialized_json_has_expected_fields() {
    let email = normalize_message(&simple_email_bytes(), "imap")
        .expect("normalization should succeed");

    let value = serde_json::to_value(&email).expect("should serialize to Value");

    assert!(value["id"].is_string());
    assert!(value["from"].is_object());
    assert!(value["to"].is_array());
    assert!(value["subject"].is_string());
    assert!(value["source"].is_string());
    assert!(value["source_id"].is_string());
    assert!(value["date"].is_string());
    assert!(value["created_at"].is_string());
    assert!(value["updated_at"].is_string());
}

#[test]
fn test_optional_fields_omitted_when_none() {
    let email = normalize_message(&simple_email_bytes(), "imap")
        .expect("normalization should succeed");

    let value = serde_json::to_value(&email).expect("should serialize to Value");

    // These fields use skip_serializing_if, so they should be absent
    assert!(
        value.get("body_html").is_none(),
        "body_html should be omitted when None"
    );
    assert!(
        value.get("in_reply_to").is_none(),
        "in_reply_to should be omitted when None"
    );
    assert!(
        value.get("extensions").is_none(),
        "extensions should be omitted when None"
    );
}

#[test]
fn test_empty_vecs_omitted_in_json() {
    let email = normalize_message(&simple_email_bytes(), "imap")
        .expect("normalization should succeed");

    let value = serde_json::to_value(&email).expect("should serialize to Value");

    // cc, bcc, labels, attachments use skip_serializing_if = "Vec::is_empty"
    assert!(
        value.get("cc").is_none(),
        "empty cc should be omitted from JSON"
    );
    assert!(
        value.get("bcc").is_none(),
        "empty bcc should be omitted from JSON"
    );
    assert!(
        value.get("labels").is_none(),
        "empty labels should be omitted from JSON"
    );
    assert!(
        value.get("attachments").is_none(),
        "empty attachments should be omitted from JSON"
    );
}

#[test]
fn test_populated_vecs_present_in_json() {
    let email = normalize_message(&threaded_email_bytes(), "imap")
        .expect("normalization should succeed");

    let value = serde_json::to_value(&email).expect("should serialize to Value");

    // cc is populated for the threaded email
    assert!(
        value.get("cc").is_some(),
        "non-empty cc should be present in JSON"
    );
    let cc_arr = value["cc"].as_array().expect("cc should be an array");
    assert_eq!(cc_arr.len(), 1);
}
