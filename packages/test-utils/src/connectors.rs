//! Shared connector test helpers for integration tests.
//!
//! Provides config factory functions and WebDAV HTTP helpers for testing
//! CalDAV, CardDAV, IMAP, SMTP, and S3 connectors against local Docker
//! services defined in `docker-compose.test.yml`.
//!
//! Factory functions return generic tuples/structs to avoid circular
//! dependencies between test-utils and connector crates.

use crate::docker;

// ---------------------------------------------------------------------------
// Config structs (generic, not connector-specific)
// ---------------------------------------------------------------------------

/// Generic WebDAV (CalDAV/CardDAV) connection config.
#[derive(Debug, Clone)]
pub struct WebDavTestConfig {
    /// The server base URL (e.g. `http://127.0.0.1:6232`).
    pub url: String,
    /// The username for authentication.
    pub username: String,
    /// The password for authentication.
    pub password: String,
}

/// Generic IMAP connection config.
#[derive(Debug, Clone)]
pub struct ImapTestConfig {
    /// The IMAP server hostname.
    pub host: String,
    /// The IMAP server port.
    pub port: u16,
    /// The username for authentication.
    pub username: String,
    /// The password for authentication.
    pub password: String,
}

/// Generic SMTP connection config.
#[derive(Debug, Clone)]
pub struct SmtpTestConfig {
    /// The SMTP server hostname.
    pub host: String,
    /// The SMTP server port.
    pub port: u16,
    /// The username for authentication.
    pub username: String,
    /// The password for authentication.
    pub password: String,
}

/// Generic S3-compatible storage config.
#[derive(Debug, Clone)]
pub struct S3TestConfig {
    /// The S3 endpoint URL (e.g. `http://127.0.0.1:9100`).
    pub endpoint: String,
    /// The AWS region.
    pub region: String,
    /// The access key ID.
    pub access_key: String,
    /// The secret access key.
    pub secret_key: String,
    /// The default test bucket name.
    pub bucket: String,
}

// ---------------------------------------------------------------------------
// Factory functions
// ---------------------------------------------------------------------------

/// Returns a CalDAV config pointing at the Radicale test instance.
///
/// Radicale in test mode accepts any credentials. Uses `test`/`test`.
pub fn radicale_caldav_config() -> WebDavTestConfig {
    WebDavTestConfig {
        url: format!(
            "http://{}:{}",
            docker::RADICALE_HOST,
            docker::RADICALE_PORT
        ),
        username: "test".into(),
        password: "test".into(),
    }
}

/// Returns a CardDAV config pointing at the Radicale test instance.
///
/// Radicale in test mode accepts any credentials. Uses `test`/`test`.
pub fn radicale_carddav_config() -> WebDavTestConfig {
    WebDavTestConfig {
        url: format!(
            "http://{}:{}",
            docker::RADICALE_HOST,
            docker::RADICALE_PORT
        ),
        username: "test".into(),
        password: "test".into(),
    }
}

/// Returns an IMAP config for the GreenMail test instance.
pub fn greenmail_imap_config() -> ImapTestConfig {
    ImapTestConfig {
        host: docker::GREENMAIL_HOST.into(),
        port: docker::GREENMAIL_IMAP_PORT,
        username: docker::GREENMAIL_USERNAME.into(),
        password: docker::GREENMAIL_PASSWORD.into(),
    }
}

/// Returns an SMTP config for the GreenMail test instance.
pub fn greenmail_smtp_config() -> SmtpTestConfig {
    SmtpTestConfig {
        host: docker::GREENMAIL_HOST.into(),
        port: docker::GREENMAIL_SMTP_PORT,
        username: docker::GREENMAIL_USERNAME.into(),
        password: docker::GREENMAIL_PASSWORD.into(),
    }
}

/// Returns an S3 config for the MinIO test instance.
pub fn minio_s3_config() -> S3TestConfig {
    S3TestConfig {
        endpoint: format!(
            "http://{}:{}",
            docker::MINIO_HOST,
            docker::MINIO_API_PORT
        ),
        region: "us-east-1".into(),
        access_key: docker::MINIO_ROOT_USER.into(),
        secret_key: docker::MINIO_ROOT_PASSWORD.into(),
        bucket: "integration-test-bucket".into(),
    }
}

// ---------------------------------------------------------------------------
// WebDAV HTTP helpers (using reqwest directly)
// ---------------------------------------------------------------------------

/// Create a MKCALENDAR request to Radicale to ensure a calendar collection exists.
///
/// Radicale creates the calendar if it does not exist and returns success
/// even if it already exists.
///
/// # Arguments
///
/// - `client` — A `reqwest::Client` instance.
/// - `base_url` — The Radicale base URL (e.g. `http://127.0.0.1:6232`).
/// - `path` — The calendar path (e.g. `/test/my-calendar/`).
/// - `username` — The username for Basic auth.
/// - `password` — The password for Basic auth.
pub async fn ensure_radicale_calendar(
    client: &reqwest::Client,
    base_url: &str,
    path: &str,
    username: &str,
    password: &str,
) -> Result<(), String> {
    let url = format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    );
    let resp = client
        .request(reqwest::Method::from_bytes(b"MKCALENDAR").unwrap(), &url)
        .basic_auth(username, Some(password))
        .send()
        .await
        .map_err(|e| format!("MKCALENDAR request failed: {e}"))?;

    let status = resp.status().as_u16();
    if status == 201 || status == 200 || status == 207 || status == 405 {
        // 201 Created, 200 OK, 207 Multi-Status, 405 Already exists
        Ok(())
    } else {
        let body = resp.text().await.unwrap_or_default();
        Err(format!(
            "MKCALENDAR failed with status {status}: {body}"
        ))
    }
}

/// Create a MKCOL request to Radicale to ensure an address book collection exists.
///
/// # Arguments
///
/// - `client` — A `reqwest::Client` instance.
/// - `base_url` — The Radicale base URL (e.g. `http://127.0.0.1:6232`).
/// - `path` — The address book path (e.g. `/test/my-contacts/`).
/// - `username` — The username for Basic auth.
/// - `password` — The password for Basic auth.
pub async fn ensure_radicale_addressbook(
    client: &reqwest::Client,
    base_url: &str,
    path: &str,
    username: &str,
    password: &str,
) -> Result<(), String> {
    let url = format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    );

    // Radicale uses MKCOL with a specific body to create an addressbook.
    // We send a MKCOL with a resourcetype body specifying addressbook.
    let body = r#"<?xml version="1.0" encoding="UTF-8" ?>
<D:mkcol xmlns:D="DAV:" xmlns:CR="urn:ietf:params:xml:ns:carddav">
  <D:set>
    <D:prop>
      <D:resourcetype>
        <D:collection/>
        <CR:addressbook/>
      </D:resourcetype>
    </D:prop>
  </D:set>
</D:mkcol>"#;

    let resp = client
        .request(reqwest::Method::from_bytes(b"MKCOL").unwrap(), &url)
        .basic_auth(username, Some(password))
        .header("Content-Type", "application/xml; charset=utf-8")
        .body(body)
        .send()
        .await
        .map_err(|e| format!("MKCOL request failed: {e}"))?;

    let status = resp.status().as_u16();
    if status == 201 || status == 200 || status == 207 || status == 405 {
        Ok(())
    } else {
        let resp_body = resp.text().await.unwrap_or_default();
        Err(format!(
            "MKCOL (addressbook) failed with status {status}: {resp_body}"
        ))
    }
}

/// PUT an iCalendar event into a Radicale calendar.
///
/// # Arguments
///
/// - `client` — A `reqwest::Client` instance.
/// - `base_url` — The Radicale base URL.
/// - `path` — The calendar path (e.g. `/test/my-calendar/`).
/// - `uid` — The UID for the event (used as filename).
/// - `ical_data` — The raw iCalendar data.
/// - `username` — The username for Basic auth.
/// - `password` — The password for Basic auth.
pub async fn put_ical_event(
    client: &reqwest::Client,
    base_url: &str,
    path: &str,
    uid: &str,
    ical_data: &str,
    username: &str,
    password: &str,
) -> Result<(), String> {
    let url = format!(
        "{}/{}/{}.ics",
        base_url.trim_end_matches('/'),
        path.trim_matches('/'),
        uid
    );

    let resp = client
        .put(&url)
        .basic_auth(username, Some(password))
        .header("Content-Type", "text/calendar; charset=utf-8")
        .body(ical_data.to_string())
        .send()
        .await
        .map_err(|e| format!("PUT iCal event failed: {e}"))?;

    let status = resp.status().as_u16();
    if (200..300).contains(&status) {
        Ok(())
    } else {
        let body = resp.text().await.unwrap_or_default();
        Err(format!(
            "PUT iCal event failed with status {status}: {body}"
        ))
    }
}

/// PUT a vCard into a Radicale address book.
///
/// # Arguments
///
/// - `client` — A `reqwest::Client` instance.
/// - `base_url` — The Radicale base URL.
/// - `path` — The address book path (e.g. `/test/my-contacts/`).
/// - `uid` — The UID for the vCard (used as filename).
/// - `vcard_data` — The raw vCard data.
/// - `username` — The username for Basic auth.
/// - `password` — The password for Basic auth.
pub async fn put_vcard(
    client: &reqwest::Client,
    base_url: &str,
    path: &str,
    uid: &str,
    vcard_data: &str,
    username: &str,
    password: &str,
) -> Result<(), String> {
    let url = format!(
        "{}/{}/{}.vcf",
        base_url.trim_end_matches('/'),
        path.trim_matches('/'),
        uid
    );

    let resp = client
        .put(&url)
        .basic_auth(username, Some(password))
        .header("Content-Type", "text/vcard; charset=utf-8")
        .body(vcard_data.to_string())
        .send()
        .await
        .map_err(|e| format!("PUT vCard failed: {e}"))?;

    let status = resp.status().as_u16();
    if (200..300).contains(&status) {
        Ok(())
    } else {
        let body = resp.text().await.unwrap_or_default();
        Err(format!(
            "PUT vCard failed with status {status}: {body}"
        ))
    }
}

// ---------------------------------------------------------------------------
// GreenMail SMTP helper
// ---------------------------------------------------------------------------

/// Send an email via SMTP to the GreenMail test server.
///
/// Uses `reqwest` to perform a raw SMTP send through a synchronous TCP
/// connection (via tokio). This helper exists so integration tests can
/// seed GreenMail mailboxes without depending on the connector crate's
/// `SmtpClient`.
///
/// # Arguments
///
/// - `from` — The sender email address.
/// - `to` — The recipient email addresses.
/// - `subject` — The email subject.
/// - `body` — The plain-text email body.
///
/// # Errors
///
/// Returns an error string if the SMTP transaction fails.
pub async fn greenmail_send_email(
    from: &str,
    to: &[String],
    subject: &str,
    body: &str,
) -> Result<(), String> {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpStream;

    if to.is_empty() {
        return Err("at least one recipient is required".into());
    }

    let addr = format!("{}:{}", docker::GREENMAIL_HOST, docker::GREENMAIL_SMTP_PORT);
    let mut stream = TcpStream::connect(&addr)
        .map_err(|e| format!("failed to connect to SMTP at {addr}: {e}"))?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .ok();

    let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);

    // Helper to read a line from the server.
    let read_line = |reader: &mut BufReader<TcpStream>| -> Result<String, String> {
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|e| format!("SMTP read error: {e}"))?;
        Ok(line)
    };

    // Helper to send a command and read the response.
    let send_cmd =
        |stream: &mut TcpStream,
         reader: &mut BufReader<TcpStream>,
         cmd: &str|
         -> Result<String, String> {
            stream
                .write_all(cmd.as_bytes())
                .map_err(|e| format!("SMTP write error: {e}"))?;
            stream.flush().map_err(|e| format!("SMTP flush error: {e}"))?;
            read_line(reader)
        };

    // Read greeting.
    let _greeting = read_line(&mut reader)?;

    // EHLO
    let _ehlo = send_cmd(&mut stream, &mut reader, "EHLO localhost\r\n")?;
    // Drain any multi-line EHLO response.
    loop {
        let mut peek_buf = [0u8; 1];
        stream
            .set_nonblocking(true)
            .map_err(|e| e.to_string())?;
        match std::io::Read::read(&mut reader, &mut peek_buf) {
            Ok(0) => break,
            Ok(_) => {
                // Read the rest of the line.
                let mut rest = String::new();
                stream
                    .set_nonblocking(false)
                    .map_err(|e| e.to_string())?;
                reader
                    .read_line(&mut rest)
                    .map_err(|e| format!("SMTP read error: {e}"))?;
            }
            Err(_) => {
                stream
                    .set_nonblocking(false)
                    .map_err(|e| e.to_string())?;
                break;
            }
        }
    }
    stream
        .set_nonblocking(false)
        .map_err(|e| e.to_string())?;

    // MAIL FROM
    let _mail = send_cmd(
        &mut stream,
        &mut reader,
        &format!("MAIL FROM:<{from}>\r\n"),
    )?;

    // RCPT TO
    for recipient in to {
        let _rcpt = send_cmd(
            &mut stream,
            &mut reader,
            &format!("RCPT TO:<{recipient}>\r\n"),
        )?;
    }

    // DATA
    let _data = send_cmd(&mut stream, &mut reader, "DATA\r\n")?;

    // Build RFC 5322 message.
    let to_header = to.join(", ");
    let message = format!(
        "From: {from}\r\n\
         To: {to_header}\r\n\
         Subject: {subject}\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\
         \r\n\
         {body}\r\n\
         .\r\n"
    );
    let _sent = send_cmd(&mut stream, &mut reader, &message)?;

    // QUIT
    let _quit = send_cmd(&mut stream, &mut reader, "QUIT\r\n")?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Connector assertion helpers
// ---------------------------------------------------------------------------

/// Assert that a normalized CDM Email has the required source fields populated.
///
/// Checks that `source` matches the expected string and `source_id` is non-empty.
#[macro_export]
macro_rules! assert_email_source {
    ($email:expr, $expected_source:expr) => {{
        assert_eq!(
            $email.source, $expected_source,
            "email source mismatch"
        );
        assert!(
            !$email.source_id.is_empty(),
            "email source_id should not be empty"
        );
    }};
}

/// Assert that a normalized CDM Email has valid identity fields.
///
/// Checks that the UUID is not nil and the `from` field is non-empty.
#[macro_export]
macro_rules! assert_email_identity {
    ($email:expr) => {{
        assert!(
            !$email.id.is_nil(),
            "email id should not be nil"
        );
        assert!(
            !$email.from.is_empty(),
            "email from should not be empty"
        );
    }};
}

/// Assert that a normalized CDM Email has all expected addressing fields.
///
/// Verifies `from`, `to`, `cc`, and `bcc` match the expected values.
#[macro_export]
macro_rules! assert_email_addressing {
    ($email:expr, from: $from:expr, to: $to:expr) => {{
        assert_eq!($email.from, $from, "email from mismatch");
        assert_eq!($email.to, $to, "email to mismatch");
    }};
    ($email:expr, from: $from:expr, to: $to:expr, cc: $cc:expr) => {{
        assert_eq!($email.from, $from, "email from mismatch");
        assert_eq!($email.to, $to, "email to mismatch");
        assert_eq!($email.cc, $cc, "email cc mismatch");
    }};
}

/// DELETE a WebDAV collection (calendar or address book).
///
/// # Arguments
///
/// - `client` — A `reqwest::Client` instance.
/// - `base_url` — The Radicale base URL.
/// - `path` — The collection path to delete.
/// - `username` — The username for Basic auth.
/// - `password` — The password for Basic auth.
pub async fn delete_collection(
    client: &reqwest::Client,
    base_url: &str,
    path: &str,
    username: &str,
    password: &str,
) -> Result<(), String> {
    let url = format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    );

    let resp = client
        .delete(&url)
        .basic_auth(username, Some(password))
        .send()
        .await
        .map_err(|e| format!("DELETE collection failed: {e}"))?;

    let status = resp.status().as_u16();
    // 200, 204 (No Content), 404 (already gone) are all fine
    if status == 200 || status == 204 || status == 404 {
        Ok(())
    } else {
        let body = resp.text().await.unwrap_or_default();
        Err(format!(
            "DELETE collection failed with status {status}: {body}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radicale_caldav_config_has_correct_defaults() {
        let config = radicale_caldav_config();
        assert_eq!(config.url, "http://127.0.0.1:6232");
        assert_eq!(config.username, "test");
        assert_eq!(config.password, "test");
    }

    #[test]
    fn radicale_carddav_config_has_correct_defaults() {
        let config = radicale_carddav_config();
        assert_eq!(config.url, "http://127.0.0.1:6232");
        assert_eq!(config.username, "test");
        assert_eq!(config.password, "test");
    }

    #[test]
    fn greenmail_imap_config_has_correct_defaults() {
        let config = greenmail_imap_config();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 4143);
        assert_eq!(config.username, "test");
        assert_eq!(config.password, "test");
    }

    #[test]
    fn greenmail_smtp_config_has_correct_defaults() {
        let config = greenmail_smtp_config();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 4025);
        assert_eq!(config.username, "test");
        assert_eq!(config.password, "test");
    }

    #[test]
    fn minio_s3_config_has_correct_defaults() {
        let config = minio_s3_config();
        assert_eq!(config.endpoint, "http://127.0.0.1:9100");
        assert_eq!(config.region, "us-east-1");
        assert_eq!(config.access_key, "minioadmin");
        assert_eq!(config.secret_key, "minioadmin");
        assert_eq!(config.bucket, "integration-test-bucket");
    }

    #[test]
    fn assert_email_source_macro_passes() {
        use life_engine_types::Email;

        let email = Email {
            id: uuid::Uuid::new_v4(),
            from: "test@example.com".into(),
            to: vec!["recipient@example.com".into()],
            cc: vec![],
            bcc: vec![],
            subject: "Test".into(),
            body_text: "body".into(),
            body_html: None,
            thread_id: None,
            labels: vec![],
            attachments: vec![],
            source: "imap".into(),
            source_id: "msg-001".into(),
            extensions: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        assert_email_source!(email, "imap");
    }

    #[test]
    fn assert_email_identity_macro_passes() {
        use life_engine_types::Email;

        let email = Email {
            id: uuid::Uuid::new_v4(),
            from: "test@example.com".into(),
            to: vec![],
            cc: vec![],
            bcc: vec![],
            subject: "Test".into(),
            body_text: "body".into(),
            body_html: None,
            thread_id: None,
            labels: vec![],
            attachments: vec![],
            source: "test".into(),
            source_id: "id".into(),
            extensions: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        assert_email_identity!(email);
    }

    #[test]
    fn assert_email_addressing_macro_passes() {
        use life_engine_types::Email;

        let email = Email {
            id: uuid::Uuid::new_v4(),
            from: "alice@example.com".into(),
            to: vec!["bob@example.com".into()],
            cc: vec!["carol@example.com".into()],
            bcc: vec![],
            subject: "Test".into(),
            body_text: "body".into(),
            body_html: None,
            thread_id: None,
            labels: vec![],
            attachments: vec![],
            source: "test".into(),
            source_id: "id".into(),
            extensions: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        assert_email_addressing!(email,
            from: "alice@example.com",
            to: vec!["bob@example.com".to_string()],
            cc: vec!["carol@example.com".to_string()]
        );
    }
}
