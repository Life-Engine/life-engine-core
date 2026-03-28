//! SMTP client for sending emails via `lettre`.
//!
//! Provides a simple interface to build and send email messages
//! through an SMTP server with optional TLS.

use anyhow::{Context, Result};
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use serde::{Deserialize, Serialize};
use tracing;

/// Configuration for an SMTP connection.
///
/// The password is not stored in this config struct. Instead, the
/// `credential_key` names the key under which the password is stored
/// in the credential store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpConfig {
    /// The SMTP server hostname.
    pub host: String,
    /// The SMTP server port (typically 587 for STARTTLS, 465 for TLS).
    pub port: u16,
    /// The username for authentication.
    pub username: String,
    /// The key used to look up the password in the credential store.
    #[serde(default = "default_smtp_credential_key")]
    pub credential_key: String,
    /// Whether to use TLS for the connection.
    pub use_tls: bool,
}

/// Default credential key for SMTP passwords.
fn default_smtp_credential_key() -> String {
    "smtp_password".to_string()
}

/// SMTP client for sending email messages.
pub struct SmtpClient {
    /// Connection configuration.
    config: SmtpConfig,
}

impl SmtpClient {
    /// Create a new SMTP client with the given configuration.
    pub fn new(config: SmtpConfig) -> Self {
        Self { config }
    }

    /// Send an email message.
    ///
    /// Builds an RFC 5322 message from the provided parameters and
    /// sends it through the configured SMTP server. The `password`
    /// parameter should be retrieved from the credential store using
    /// the config's `credential_key`.
    pub async fn send(
        &self,
        from: &str,
        to: &[String],
        subject: &str,
        body: &str,
        password: &str,
    ) -> Result<()> {
        if to.is_empty() {
            return Err(anyhow::anyhow!("at least one recipient is required"));
        }

        let mut message_builder = Message::builder()
            .from(from.parse().context("invalid from address")?)
            .subject(subject)
            .header(ContentType::TEXT_PLAIN);

        for recipient in to {
            message_builder =
                message_builder.to(recipient.parse().context("invalid recipient address")?);
        }

        let message = message_builder
            .body(body.to_string())
            .context("failed to build email message")?;

        let creds = Credentials::new(self.config.username.clone(), password.to_string());

        let smtp_timeout = std::time::Duration::from_secs(30);

        let transport = if self.config.use_tls {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&self.config.host)
                .context("failed to create SMTP relay")?
                .port(self.config.port)
                .credentials(creds)
                .timeout(Some(smtp_timeout))
                .build()
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&self.config.host)
                .port(self.config.port)
                .credentials(creds)
                .timeout(Some(smtp_timeout))
                .build()
        };

        transport
            .send(message)
            .await
            .context("failed to send email")?;

        tracing::info!(
            from = from,
            to_count = to.len(),
            subject = subject,
            "email sent"
        );

        Ok(())
    }

    /// Returns the SMTP configuration.
    pub fn config(&self) -> &SmtpConfig {
        &self.config
    }
}

/// Build a valid `lettre::Message` from the given parameters.
///
/// Exposed for unit testing message construction without sending.
pub fn build_message(from: &str, to: &[String], subject: &str, body: &str) -> Result<Message> {
    if to.is_empty() {
        return Err(anyhow::anyhow!("at least one recipient is required"));
    }

    let mut builder = Message::builder()
        .from(from.parse().context("invalid from address")?)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN);

    for recipient in to {
        builder = builder.to(recipient.parse().context("invalid recipient address")?);
    }

    builder
        .body(body.to_string())
        .context("failed to build email message")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smtp_config_serialization() {
        let config = SmtpConfig {
            host: "smtp.example.com".into(),
            port: 587,
            username: "user@example.com".into(),
            credential_key: "smtp_password".into(),
            use_tls: true,
        };
        let json = serde_json::to_string(&config).expect("serialize");
        let restored: SmtpConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.host, "smtp.example.com");
        assert_eq!(restored.port, 587);
        assert!(restored.use_tls);
    }

    #[test]
    fn smtp_client_construction() {
        let config = SmtpConfig {
            host: "smtp.example.com".into(),
            port: 587,
            username: "user@example.com".into(),
            credential_key: "smtp_password".into(),
            use_tls: true,
        };
        let client = SmtpClient::new(config);
        assert_eq!(client.config().host, "smtp.example.com");
    }

    #[test]
    fn build_message_simple() {
        let msg = build_message(
            "sender@example.com",
            &["recipient@example.com".into()],
            "Test Subject",
            "Hello, World!",
        );
        assert!(msg.is_ok());
    }

    #[test]
    fn build_message_multiple_recipients() {
        let msg = build_message(
            "sender@example.com",
            &[
                "alice@example.com".into(),
                "bob@example.com".into(),
            ],
            "Group message",
            "Hello everyone!",
        );
        assert!(msg.is_ok());
    }

    #[test]
    fn build_message_no_recipients_fails() {
        let msg = build_message("sender@example.com", &[], "No recipients", "Body");
        assert!(msg.is_err());
    }

    #[test]
    fn build_message_invalid_from_fails() {
        let msg = build_message(
            "not-an-email",
            &["recipient@example.com".into()],
            "Test",
            "Body",
        );
        assert!(msg.is_err());
    }
}
