//! Email normalizer: converts raw RFC 5322 email bytes to CDM `Email` type.
//!
//! Handles edge cases: missing fields, malformed dates, encoded subjects,
//! multi-part bodies, and attachment extraction.

use chrono::{DateTime, Utc};
use life_engine_types::{Email, EmailAttachment};
use mail_parser::{MessageParser, MimeHeaders};
use uuid::Uuid;

/// Normalize a raw email message into the Life Engine CDM `Email` type.
///
/// `raw` is the full RFC 5322 message bytes.
/// `source` identifies the connector that produced this email (e.g. "imap").
pub fn normalize_message(raw: &[u8], source: &str) -> anyhow::Result<Email> {
    let parsed = MessageParser::default()
        .parse(raw)
        .ok_or_else(|| anyhow::anyhow!("failed to parse email message"))?;

    let from = extract_from(&parsed);
    let to = extract_to(&parsed);
    let cc = extract_cc(&parsed);
    let subject = parsed
        .subject()
        .unwrap_or("(no subject)")
        .to_string();
    let body_text = extract_body_text(&parsed);
    let body_html = extract_body_html(&parsed);
    let thread_id = extract_thread_id(&parsed);
    let attachments = extract_attachments(&parsed);
    let source_id = extract_message_id(&parsed);
    let created_at = extract_date(&parsed);

    Ok(Email {
        id: Uuid::new_v4(),
        from,
        to,
        cc,
        bcc: vec![],
        subject,
        body_text,
        body_html,
        thread_id,
        labels: vec![],
        attachments,
        source: source.into(),
        source_id,
        extensions: None,
        created_at,
        updated_at: Utc::now(),
    })
}

/// Extract the From address as a string.
fn extract_from(msg: &mail_parser::Message<'_>) -> String {
    msg.from()
        .and_then(|addrs| addrs.first())
        .map(|addr| {
            addr.address()
                .map(|a| a.to_string())
                .unwrap_or_default()
        })
        .unwrap_or_default()
}

/// Extract all To addresses.
fn extract_to(msg: &mail_parser::Message<'_>) -> Vec<String> {
    msg.to()
        .map(|addrs| {
            addrs
                .iter()
                .filter_map(|addr| addr.address().map(|a| a.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

/// Extract all CC addresses.
fn extract_cc(msg: &mail_parser::Message<'_>) -> Vec<String> {
    msg.cc()
        .map(|addrs| {
            addrs
                .iter()
                .filter_map(|addr| addr.address().map(|a| a.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

/// Extract the plain text body from the message.
fn extract_body_text(msg: &mail_parser::Message<'_>) -> String {
    msg.body_text(0)
        .map(|t| t.to_string())
        .unwrap_or_default()
}

/// Extract the HTML body from the message, if present.
///
/// Only returns HTML when the message actually contains a `text/html`
/// MIME part — `mail-parser` can auto-generate HTML from plain text,
/// which we explicitly filter out.
fn extract_body_html(msg: &mail_parser::Message<'_>) -> Option<String> {
    // Check whether any part in the message has text/html content type
    let has_html_part = msg.parts.iter().any(|part| {
        part.content_type()
            .map(|ct| {
                ct.c_type.as_ref() == "text"
                    && ct.c_subtype.as_ref().map(|s| s.as_ref()) == Some("html")
            })
            .unwrap_or(false)
    });
    if !has_html_part {
        return None;
    }
    msg.body_html(0).map(|h| h.to_string())
}

/// Extract a thread ID from In-Reply-To or References headers.
///
/// Uses In-Reply-To first; falls back to the first entry in References.
fn extract_thread_id(msg: &mail_parser::Message<'_>) -> Option<String> {
    // Try In-Reply-To first
    if let Some(in_reply_to) = msg.in_reply_to().as_text() {
        return Some(in_reply_to.to_string());
    }

    // Fall back to References (first entry)
    if let Some(references) = msg.references().as_text() {
        return Some(references.to_string());
    }

    None
}

/// Extract attachment metadata from the message.
fn extract_attachments(msg: &mail_parser::Message<'_>) -> Vec<EmailAttachment> {
    msg.attachments()
        .map(|part| {
            let filename = part
                .attachment_name()
                .unwrap_or("unnamed")
                .to_string();
            let mime_type = part
                .content_type()
                .map(|ct| {
                    let main = ct.c_type.as_ref();
                    let sub = ct
                        .c_subtype
                        .as_ref()
                        .map(|s: &std::borrow::Cow<'_, str>| s.as_ref())
                        .unwrap_or("octet-stream");
                    format!("{main}/{sub}")
                })
                .unwrap_or_else(|| "application/octet-stream".into());
            let size = part.contents().len() as u64;

            EmailAttachment {
                file_id: Uuid::new_v4().to_string(),
                filename,
                mime_type,
                size,
            }
        })
        .collect()
}

/// Extract the Message-ID header as the source_id.
fn extract_message_id(msg: &mail_parser::Message<'_>) -> String {
    msg.message_id()
        .map(|id| id.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

/// Extract the Date header, falling back to current time.
fn extract_date(msg: &mail_parser::Message<'_>) -> DateTime<Utc> {
    msg.date()
        .and_then(|d| {
            DateTime::from_timestamp(d.to_timestamp(), 0)
        })
        .unwrap_or_else(Utc::now)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A simple plain-text email for testing.
    const SIMPLE_EMAIL: &[u8] = b"From: sender@example.com\r\n\
        To: recipient@example.com\r\n\
        Subject: Test Email\r\n\
        Date: Sat, 21 Mar 2026 10:00:00 +0000\r\n\
        Message-ID: <test-001@example.com>\r\n\
        Content-Type: text/plain; charset=utf-8\r\n\
        \r\n\
        This is a test email body.\r\n";

    /// An email with CC, In-Reply-To, and References headers.
    const THREADED_EMAIL: &[u8] = b"From: alice@example.com\r\n\
        To: bob@example.com\r\n\
        CC: carol@example.com, dave@example.com\r\n\
        Subject: Re: Project Update\r\n\
        Date: Sat, 21 Mar 2026 11:00:00 +0000\r\n\
        Message-ID: <reply-002@example.com>\r\n\
        In-Reply-To: <original-001@example.com>\r\n\
        References: <original-001@example.com>\r\n\
        Content-Type: text/plain; charset=utf-8\r\n\
        \r\n\
        Thanks for the update.\r\n";

    /// An email with no subject and no date.
    const MINIMAL_EMAIL: &[u8] = b"From: sender@example.com\r\n\
        To: recipient@example.com\r\n\
        Message-ID: <min-003@example.com>\r\n\
        Content-Type: text/plain; charset=utf-8\r\n\
        \r\n\
        Minimal message.\r\n";

    /// A multipart email with an attachment.
    const MULTIPART_EMAIL: &[u8] = b"From: sender@example.com\r\n\
        To: recipient@example.com\r\n\
        Subject: Email with attachment\r\n\
        Date: Sat, 21 Mar 2026 12:00:00 +0000\r\n\
        Message-ID: <attach-004@example.com>\r\n\
        MIME-Version: 1.0\r\n\
        Content-Type: multipart/mixed; boundary=\"boundary123\"\r\n\
        \r\n\
        --boundary123\r\n\
        Content-Type: text/plain; charset=utf-8\r\n\
        \r\n\
        Please see the attached file.\r\n\
        --boundary123\r\n\
        Content-Type: application/pdf; name=\"report.pdf\"\r\n\
        Content-Disposition: attachment; filename=\"report.pdf\"\r\n\
        Content-Transfer-Encoding: base64\r\n\
        \r\n\
        JVBERi0xLjQKMSAwIG9iago=\r\n\
        --boundary123--\r\n";

    /// An email with only References (no In-Reply-To) for thread detection.
    const REFERENCES_ONLY_EMAIL: &[u8] = b"From: sender@example.com\r\n\
        To: recipient@example.com\r\n\
        Subject: Follow-up\r\n\
        Date: Sat, 21 Mar 2026 13:00:00 +0000\r\n\
        Message-ID: <ref-005@example.com>\r\n\
        References: <thread-root@example.com>\r\n\
        Content-Type: text/plain; charset=utf-8\r\n\
        \r\n\
        Following up on our thread.\r\n";

    #[test]
    fn normalize_simple_email() {
        let email = normalize_message(SIMPLE_EMAIL, "imap").expect("should parse");
        assert_eq!(email.from, "sender@example.com");
        assert_eq!(email.to, vec!["recipient@example.com"]);
        assert_eq!(email.subject, "Test Email");
        assert!(email.body_text.contains("This is a test email body."));
        // mail-parser may generate HTML for plain text emails; just verify body_text is set
        assert_eq!(email.source, "imap");
        assert_eq!(email.source_id, "test-001@example.com");
        assert!(email.thread_id.is_none());
        assert!(email.cc.is_empty());
        assert!(email.bcc.is_empty());
        assert!(email.attachments.is_empty());
    }

    #[test]
    fn normalize_threaded_email() {
        let email = normalize_message(THREADED_EMAIL, "imap").expect("should parse");
        assert_eq!(email.from, "alice@example.com");
        assert_eq!(email.to, vec!["bob@example.com"]);
        assert_eq!(
            email.cc,
            vec!["carol@example.com", "dave@example.com"]
        );
        assert_eq!(email.subject, "Re: Project Update");
        assert_eq!(
            email.thread_id.as_deref(),
            Some("original-001@example.com")
        );
    }

    #[test]
    fn normalize_minimal_email_defaults() {
        let email = normalize_message(MINIMAL_EMAIL, "imap").expect("should parse");
        assert_eq!(email.from, "sender@example.com");
        assert_eq!(email.subject, "(no subject)");
        assert!(email.body_text.contains("Minimal message."));
        // No date header — should fall back to a valid DateTime
        assert!(email.created_at <= Utc::now());
    }

    #[test]
    fn normalize_multipart_with_attachment() {
        let email = normalize_message(MULTIPART_EMAIL, "imap").expect("should parse");
        assert_eq!(email.subject, "Email with attachment");
        assert!(email.body_text.contains("Please see the attached file."));
        assert_eq!(email.attachments.len(), 1);

        let attachment = &email.attachments[0];
        assert_eq!(attachment.filename, "report.pdf");
        assert!(attachment.mime_type.contains("pdf"));
        assert!(attachment.size > 0);
        assert!(!attachment.file_id.is_empty());
    }

    #[test]
    fn thread_id_from_references_only() {
        let email =
            normalize_message(REFERENCES_ONLY_EMAIL, "imap").expect("should parse");
        assert_eq!(
            email.thread_id.as_deref(),
            Some("thread-root@example.com")
        );
    }

    #[test]
    fn source_id_is_message_id() {
        let email = normalize_message(SIMPLE_EMAIL, "imap").expect("should parse");
        assert_eq!(email.source_id, "test-001@example.com");
    }

    #[test]
    fn invalid_input_returns_error() {
        let result = normalize_message(b"", "imap");
        assert!(result.is_err());
    }

    #[test]
    fn email_date_parsed_correctly() {
        let email = normalize_message(SIMPLE_EMAIL, "imap").expect("should parse");
        // The email is dated 2026-03-21 10:00:00 UTC
        assert_eq!(email.created_at.year(), 2026);
    }

    use chrono::Datelike;

    #[test]
    fn normalized_email_has_valid_uuid() {
        let email = normalize_message(SIMPLE_EMAIL, "imap").expect("should parse");
        assert!(!email.id.is_nil());
    }

    #[test]
    fn normalized_email_serializes_to_json() {
        let email = normalize_message(SIMPLE_EMAIL, "imap").expect("should parse");
        let json = serde_json::to_string(&email).expect("should serialize");
        let restored: Email = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(restored.from, email.from);
        assert_eq!(restored.subject, email.subject);
    }
}
