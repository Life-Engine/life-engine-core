//! Docker service constants and helpers for integration tests.
//!
//! Provides host/port constants matching `docker-compose.test.yml`
//! and helper functions for checking service availability before
//! running integration tests.

use std::net::TcpStream;
use std::time::Duration;

// ---------------------------------------------------------------------------
// GreenMail (test email server)
// ---------------------------------------------------------------------------

/// GreenMail SMTP port as mapped in `docker-compose.test.yml`.
pub const GREENMAIL_SMTP_PORT: u16 = 4025;

/// GreenMail IMAP port as mapped in `docker-compose.test.yml`.
pub const GREENMAIL_IMAP_PORT: u16 = 4143;

/// GreenMail test username.
pub const GREENMAIL_USERNAME: &str = "test";

/// GreenMail test password.
pub const GREENMAIL_PASSWORD: &str = "test";

/// GreenMail host (localhost for local Docker).
pub const GREENMAIL_HOST: &str = "127.0.0.1";

/// GreenMail test email address.
pub const GREENMAIL_EMAIL: &str = "test@life-engine.local";

// ---------------------------------------------------------------------------
// Radicale (CalDAV / CardDAV server)
// ---------------------------------------------------------------------------

/// Radicale CalDAV/CardDAV port as mapped in `docker-compose.test.yml`.
pub const RADICALE_PORT: u16 = 6232;

/// Radicale host (localhost for local Docker).
pub const RADICALE_HOST: &str = "127.0.0.1";

// ---------------------------------------------------------------------------
// MinIO (S3-compatible storage)
// ---------------------------------------------------------------------------

/// MinIO S3 API port as mapped in `docker-compose.test.yml`.
pub const MINIO_API_PORT: u16 = 9100;

/// MinIO web console port as mapped in `docker-compose.test.yml`.
pub const MINIO_CONSOLE_PORT: u16 = 9101;

/// MinIO root user (default credentials).
pub const MINIO_ROOT_USER: &str = "minioadmin";

/// MinIO root password (default credentials).
pub const MINIO_ROOT_PASSWORD: &str = "minioadmin";

/// MinIO host (localhost for local Docker).
pub const MINIO_HOST: &str = "127.0.0.1";

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Check whether a TCP service is accepting connections.
///
/// Attempts a TCP connect to `host:port` with a 1-second timeout.
/// Returns `true` if the connection succeeds, `false` otherwise.
///
/// # Example
///
/// ```rust,ignore
/// use life_engine_test_utils::docker::is_service_available;
///
/// if is_service_available("127.0.0.1", 4025) {
///     // GreenMail is running, proceed with integration test
/// }
/// ```
pub fn is_service_available(host: &str, port: u16) -> bool {
    let addr = format!("{host}:{port}");
    TcpStream::connect_timeout(
        &addr.parse().unwrap_or_else(|_| {
            // Fall back to resolving the address
            use std::net::ToSocketAddrs;
            addr.to_socket_addrs()
                .expect("invalid address")
                .next()
                .expect("no addresses resolved")
        }),
        Duration::from_secs(1),
    )
    .is_ok()
}

/// Require a Docker service to be available, panicking with a helpful
/// message if it is not.
///
/// Intended for use at the top of integration tests to provide a clear
/// skip/fail message when Docker services are not running.
///
/// # Panics
///
/// Panics if the service at `host:port` is not accepting TCP connections.
/// The panic message includes the service name and instructions to start
/// Docker Compose.
///
/// # Example
///
/// ```rust,ignore
/// use life_engine_test_utils::docker::require_docker_service;
///
/// #[test]
/// fn test_imap_sync() {
///     require_docker_service("GreenMail", "127.0.0.1", 4025);
///     // ... rest of integration test
/// }
/// ```
pub fn require_docker_service(name: &str, host: &str, port: u16) {
    if !is_service_available(host, port) {
        panic!(
            "{name} service is not available at {host}:{port}. \
             Start it with: docker compose -f docker-compose.test.yml up -d"
        );
    }
}

/// Poll a TCP port asynchronously until it accepts a connection or the
/// timeout elapses.
///
/// Returns `Ok(())` if a connection is established within `timeout`,
/// or `Err` with a descriptive message if the timeout expires.
pub async fn wait_for_port(host: &str, port: u16, timeout: Duration) -> Result<(), String> {
    let addr = format!("{host}:{port}");
    let start = std::time::Instant::now();
    let poll_interval = Duration::from_millis(200);

    loop {
        match tokio::net::TcpStream::connect(&addr).await {
            Ok(_) => return Ok(()),
            Err(_) if start.elapsed() >= timeout => {
                return Err(format!(
                    "timed out waiting for {addr} after {timeout:?}"
                ));
            }
            Err(_) => {
                tokio::time::sleep(poll_interval).await;
            }
        }
    }
}

/// Require a Docker service to be available (async version with retry).
///
/// Attempts to connect to `host:port` with a 5-second timeout, polling
/// every 200ms. Panics with a clear skip message if the service is not
/// available.
///
/// # Panics
///
/// Panics if the service at `host:port` is not accepting TCP connections
/// within the timeout period.
pub async fn require_docker_service_async(name: &str, host: &str, port: u16) {
    let timeout = Duration::from_secs(5);
    if let Err(e) = wait_for_port(host, port, timeout).await {
        panic!(
            "{name} service is not available at {host}:{port} ({e}). \
             Start it with: docker compose -f docker-compose.test.yml up -d"
        );
    }
}

/// Skip the current test unless Docker test services are reachable.
///
/// Place at the top of each integration test function. Uses a synchronous
/// TCP probe. If the services are not running, prints a skip message to
/// stderr and returns early from the test.
///
/// # Example
///
/// ```rust,ignore
/// use life_engine_test_utils::skip_unless_docker;
///
/// #[tokio::test]
/// async fn test_caldav_sync() {
///     skip_unless_docker!();
///     // ... rest of test
/// }
/// ```
#[macro_export]
macro_rules! skip_unless_docker {
    () => {
        if !$crate::docker::is_service_available(
            $crate::docker::GREENMAIL_HOST,
            $crate::docker::GREENMAIL_SMTP_PORT,
        ) {
            eprintln!(
                "SKIP: Docker test services not available. \
                 Start with: docker compose -f docker-compose.test.yml up -d"
            );
            return;
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greenmail_constants_match_compose() {
        assert_eq!(GREENMAIL_SMTP_PORT, 4025);
        assert_eq!(GREENMAIL_IMAP_PORT, 4143);
        assert_eq!(GREENMAIL_USERNAME, "test");
        assert_eq!(GREENMAIL_PASSWORD, "test");
        assert_eq!(GREENMAIL_HOST, "127.0.0.1");
    }

    #[test]
    fn radicale_constants_match_compose() {
        assert_eq!(RADICALE_PORT, 6232);
        assert_eq!(RADICALE_HOST, "127.0.0.1");
    }

    #[test]
    fn minio_constants_match_compose() {
        assert_eq!(MINIO_API_PORT, 9100);
        assert_eq!(MINIO_CONSOLE_PORT, 9101);
        assert_eq!(MINIO_ROOT_USER, "minioadmin");
        assert_eq!(MINIO_ROOT_PASSWORD, "minioadmin");
        assert_eq!(MINIO_HOST, "127.0.0.1");
    }

    #[test]
    fn is_service_available_returns_false_for_unused_port() {
        // Port 1 is almost certainly not listening on localhost
        assert!(!is_service_available("127.0.0.1", 1));
    }

    #[test]
    #[should_panic(expected = "service is not available")]
    fn require_docker_service_panics_when_unavailable() {
        require_docker_service("FakeService", "127.0.0.1", 1);
    }

    #[tokio::test]
    async fn wait_for_port_fails_on_unused_port() {
        let result = wait_for_port(
            "127.0.0.1",
            1, // almost certainly not listening
            Duration::from_millis(500),
        )
        .await;
        assert!(result.is_err());
    }
}
