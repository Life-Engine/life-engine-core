#![cfg(feature = "integration")]

//! Integration tests for the email connector against GreenMail Docker.
//!
//! Tests IMAP auth flows, full sync (seed via SMTP, fetch via IMAP,
//! normalize to CDM), incremental sync, attachment handling, and SMTP
//! sending with round-trip verification.
//!
//! GreenMail accepts any credentials and auto-creates user mailboxes.
//!
//! These tests require Docker Compose test services to be running:
//!
//!     docker compose -f docker-compose.test.yml up -d
//!
//! Run with: `cargo test -p connector-email --features integration`

use connector_email::imap::{ImapClient, ImapConfig};
use connector_email::normalizer::normalize_message;
use connector_email::smtp::{SmtpClient, SmtpConfig};
use futures::StreamExt;
use life_engine_test_utils::connectors::{
    greenmail_imap_config, greenmail_send_email, greenmail_smtp_config,
};
use life_engine_test_utils::skip_unless_docker;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build an `ImapConfig` and password from the shared test factory.
fn test_imap_config() -> (ImapConfig, String) {
    let cfg = greenmail_imap_config();
    let password = cfg.password.clone();
    (
        ImapConfig {
            host: cfg.host,
            port: cfg.port,
            username: cfg.username,
            credential_key: "imap_password".into(),
            use_tls: false, // GreenMail test port is plain IMAP
        },
        password,
    )
}

/// Build an `SmtpConfig` and password from the shared test factory.
fn test_smtp_config() -> (SmtpConfig, String) {
    let cfg = greenmail_smtp_config();
    let password = cfg.password.clone();
    (
        SmtpConfig {
            host: cfg.host,
            port: cfg.port,
            username: cfg.username,
            credential_key: "smtp_password".into(),
            use_tls: false, // GreenMail test port is plain SMTP
        },
        password,
    )
}

/// Send a plain-text test email via the GreenMail SMTP helper.
async fn send_test_email(subject: &str, body: &str) {
    greenmail_send_email(
        "test@life-engine.local",
        &["test@life-engine.local".to_string()],
        subject,
        body,
    )
    .await
    .expect("greenmail_send_email should succeed");
}

/// Connect to IMAP, select INBOX, and fetch all messages as `(uid, raw_bytes)`.
///
/// Returns the collected messages and the IMAP session for further use
/// (the caller must call `session.logout()` when finished).
///
/// NOTE: The `async_imap` crate is built on `async-std` networking types, so
/// `async_std::net::TcpStream` appears in the signature even though the tests
/// run under `#[tokio::test]`. This works because `async-std` streams implement
/// the standard `AsyncRead`/`AsyncWrite` traits and the tokio runtime can drive
/// them. This is intentional — not a runtime mismatch bug.
async fn fetch_all_messages(
    session: &mut async_imap::Session<async_std::net::TcpStream>,
) -> Vec<(u32, Vec<u8>)> {
    session
        .select("INBOX")
        .await
        .expect("should select INBOX");

    let fetches = session
        .uid_fetch("1:*", "RFC822")
        .await
        .expect("uid_fetch should succeed");

    let fetch_results: Vec<_> = fetches.collect::<Vec<_>>().await;
    let mut messages = Vec::new();
    for result in &fetch_results {
        if let Ok(fetch) = result {
            if let (Some(uid), Some(body)) = (fetch.uid, fetch.body()) {
                messages.push((uid, body.to_vec()));
            }
        }
    }
    messages
}

// ---------------------------------------------------------------------------
// 1. IMAP auth flow
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_imap_auth_flow() -> anyhow::Result<()> {
    skip_unless_docker!();

    let (imap_config, imap_pass) = test_imap_config();
    let client = ImapClient::new(imap_config);
    let mut session = client.connect_plain(&imap_pass).await?;

    // List all mailboxes and verify INBOX is present.
    let mailboxes_stream = session.list(Some(""), Some("*")).await?;
    let mailbox_results: Vec<_> = mailboxes_stream.collect::<Vec<_>>().await;

    let mailbox_names: Vec<String> = mailbox_results
        .iter()
        .filter_map(|r| r.as_ref().ok())
        .map(|mb| mb.name().to_string())
        .collect();

    assert!(
        mailbox_names.iter().any(|n| n == "INBOX"),
        "INBOX should exist in mailbox list, got: {mailbox_names:?}"
    );

    session.logout().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. Full sync
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_full_sync() -> anyhow::Result<()> {
    skip_unless_docker!();

    let unique_id = Uuid::new_v4();
    let subjects: Vec<String> = (0..3)
        .map(|i| format!("FullSync-{i}-{unique_id}"))
        .collect();

    // Seed 3 emails with unique subjects.
    for subject in &subjects {
        send_test_email(subject, &format!("Body for {subject}")).await;
    }

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Connect and fetch all messages.
    let (imap_config, imap_pass) = test_imap_config();
    let client = ImapClient::new(imap_config);
    let mut session = client.connect_plain(&imap_pass).await?;
    let raw_messages = fetch_all_messages(&mut session).await;

    assert!(
        raw_messages.len() >= 3,
        "should have at least 3 messages, got {}",
        raw_messages.len()
    );

    // Normalize each message through the CDM pipeline.
    let mut found_subjects: Vec<String> = Vec::new();
    for (_uid, raw) in &raw_messages {
        let email = normalize_message(raw, "imap")?;

        // Validate CDM fields on every message.
        assert!(!email.id.is_nil(), "email should have a valid UUID");
        assert_eq!(email.source, "imap");
        assert!(!email.source_id.is_empty());
        assert!(!email.from.is_empty());

        if subjects.contains(&email.subject) {
            found_subjects.push(email.subject.clone());
            assert!(
                email.body_text.contains("Body for"),
                "body_text should contain seeded content"
            );
            assert!(
                email.to.contains(&"test@life-engine.local".to_string()),
                "to should contain test@life-engine.local"
            );
        }
    }

    // Verify all 3 unique subjects were found.
    for subject in &subjects {
        assert!(
            found_subjects.contains(subject),
            "should have found subject '{subject}' among fetched emails"
        );
    }

    session.logout().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 3. Incremental sync
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_incremental_sync() -> anyhow::Result<()> {
    skip_unless_docker!();

    let unique_id = Uuid::new_v4();
    let (smtp_config, smtp_pass) = test_smtp_config();
    let smtp = SmtpClient::new(smtp_config);

    // Send first batch (2 emails).
    for i in 0..2 {
        smtp.send(
            "test@life-engine.local",
            &["test@life-engine.local".to_string()],
            &format!("Batch1-{i}-{unique_id}"),
            &format!("First batch email {i}."),
            &smtp_pass,
        )
        .await?;
    }

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // First sync: fetch all, establish sync state via update_sync_state().
    let (imap_config, imap_pass) = test_imap_config();
    let mut imap_client = ImapClient::new(imap_config);
    let mut session = imap_client.connect_plain(&imap_pass).await?;

    let mailbox = session.select("INBOX").await?;
    let uid_validity = mailbox.uid_validity.unwrap_or(1);

    let fetches = session
        .uid_fetch("1:*", "RFC822")
        .await?;
    let fetch_results: Vec<_> = fetches.collect::<Vec<_>>().await;
    let mut raw_messages: Vec<(u32, Vec<u8>)> = Vec::new();
    for result in &fetch_results {
        if let Ok(fetch) = result {
            if let (Some(uid), Some(body)) = (fetch.uid, fetch.body()) {
                raw_messages.push((uid, body.to_vec()));
            }
        }
    }
    drop(fetch_results);

    let (_first_batch, max_uid) = ImapClient::process_fetched(1, &raw_messages);
    imap_client.update_sync_state("INBOX", uid_validity, max_uid);

    session.logout().await?;

    // Send second batch (2 emails).
    for i in 0..2 {
        smtp.send(
            "test@life-engine.local",
            &["test@life-engine.local".to_string()],
            &format!("Batch2-{i}-{unique_id}"),
            &format!("Second batch email {i}."),
            &smtp_pass,
        )
        .await?;
    }

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Incremental sync: use compute_start_uid() to find where to resume.
    let (start_uid, _resync) = imap_client.compute_start_uid("INBOX", uid_validity);
    assert!(
        start_uid > 1,
        "incremental sync should start after first batch, start_uid={start_uid}"
    );

    let mut session = imap_client.connect_plain(&imap_pass).await?;
    session.select("INBOX").await?;

    let fetch_range = format!("{start_uid}:*");
    let fetches = session.uid_fetch(&fetch_range, "RFC822").await?;
    let fetch_results: Vec<_> = fetches.collect::<Vec<_>>().await;
    let mut new_raw: Vec<(u32, Vec<u8>)> = Vec::new();
    for result in &fetch_results {
        if let Ok(fetch) = result {
            if let (Some(uid), Some(body)) = (fetch.uid, fetch.body()) {
                new_raw.push((uid, body.to_vec()));
            }
        }
    }
    drop(fetch_results);

    // Use process_fetched() to filter only UIDs >= start_uid.
    let (new_messages, _new_max) = ImapClient::process_fetched(start_uid, &new_raw);

    assert_eq!(
        new_messages.len(),
        2,
        "incremental sync should return exactly 2 new messages, got {}",
        new_messages.len()
    );

    // Verify all new messages normalize successfully and belong to batch 2.
    for msg in &new_messages {
        let email = normalize_message(&msg.raw, "imap")?;
        assert!(!email.id.is_nil());
        assert_eq!(email.source, "imap");
        assert!(
            email.subject.contains(&unique_id.to_string()),
            "new message should have our unique ID in the subject"
        );
    }

    session.logout().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 4. Attachment handling
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_attachment_handling() -> anyhow::Result<()> {
    skip_unless_docker!();

    let unique_id = Uuid::new_v4();
    let unique_subject = format!("Attachment-{unique_id}");

    // Build a multipart message with a PDF attachment using lettre.
    use lettre::message::{
        header::ContentType, Attachment, Body, Message, MultiPart, SinglePart,
    };
    use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
    use lettre::transport::smtp::authentication::Credentials;

    // Fake PDF content (the base64-decoded bytes of "JVBERi0xLjQKMSAwIG9iago=")
    let pdf_bytes: Vec<u8> = vec![
        0x25, 0x50, 0x44, 0x46, 0x2D, 0x31, 0x2E, 0x34,
        0x0A, 0x31, 0x20, 0x30, 0x20, 0x6F, 0x62, 0x6A,
        0x0A,
    ];

    let attachment = Attachment::new("test-report.pdf".to_string())
        .body(Body::new(pdf_bytes), ContentType::parse("application/pdf")?);

    let message = Message::builder()
        .from("test@life-engine.local".parse()?)
        .to("test@life-engine.local".parse()?)
        .subject(&unique_subject)
        .multipart(
            MultiPart::mixed()
                .singlepart(SinglePart::plain(
                    "Please see the attached PDF document.".to_string(),
                ))
                .singlepart(attachment),
        )?;

    // Send via lettre's async transport to GreenMail.
    let cfg = greenmail_smtp_config();
    let creds = Credentials::new(cfg.username.clone(), cfg.password.clone());
    let transport = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&cfg.host)
        .port(cfg.port)
        .credentials(creds)
        .build();

    transport.send(message).await.map_err(|e| anyhow::anyhow!("SMTP send failed: {e}"))?;

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Fetch via IMAP and normalize.
    let (imap_config, imap_pass) = test_imap_config();
    let client = ImapClient::new(imap_config);
    let mut session = client.connect_plain(&imap_pass).await?;
    let raw_messages = fetch_all_messages(&mut session).await;

    let mut found = false;
    for (_uid, raw) in &raw_messages {
        let email = normalize_message(raw, "imap")?;
        if email.subject == unique_subject {
            found = true;

            assert_eq!(
                email.attachments.len(),
                1,
                "should have exactly one attachment"
            );

            let att = &email.attachments[0];
            assert_eq!(att.filename, "test-report.pdf");
            assert!(
                att.mime_type.contains("pdf"),
                "mime_type should contain 'pdf', got: {}",
                att.mime_type
            );
            assert!(att.size > 0, "attachment size should be > 0");
            assert!(
                !att.file_id.is_empty(),
                "file_id should not be empty"
            );
            assert!(
                email.body_text.contains("attached PDF document"),
                "body_text should contain the plain text part"
            );
            break;
        }
    }

    assert!(
        found,
        "should have found email with subject '{unique_subject}'"
    );

    session.logout().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 5. Send via SMTP
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_send_via_smtp() -> anyhow::Result<()> {
    skip_unless_docker!();

    let unique_id = Uuid::new_v4();
    let unique_subject = format!("SmtpSend-{unique_id}");

    // Send via SmtpClient.
    let (smtp_config, smtp_pass) = test_smtp_config();
    let smtp = SmtpClient::new(smtp_config);
    smtp.send(
        "test@life-engine.local",
        &["test@life-engine.local".to_string()],
        &unique_subject,
        "This email was sent via SmtpClient::send().",
        &smtp_pass,
    )
    .await?;

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Verify arrival via IMAP.
    let (imap_config, imap_pass) = test_imap_config();
    let client = ImapClient::new(imap_config);
    let mut session = client.connect_plain(&imap_pass).await?;
    let raw_messages = fetch_all_messages(&mut session).await;

    let mut found = false;
    for (_uid, raw) in &raw_messages {
        let email = normalize_message(raw, "imap")?;
        if email.subject == unique_subject {
            found = true;
            assert_eq!(email.from, "test@life-engine.local");
            assert!(email.to.contains(&"test@life-engine.local".to_string()));
            assert!(
                email.body_text.contains("SmtpClient::send()"),
                "body_text should contain the sent body content"
            );
            break;
        }
    }

    assert!(
        found,
        "should have found the email sent via SMTP with subject '{unique_subject}'"
    );

    session.logout().await?;
    Ok(())
}
