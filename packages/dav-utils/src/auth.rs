//! HTTP Basic authentication header construction.
//!
//! Provides a standalone function for building `Authorization: Basic ...`
//! header values, shared by both CalDAV and CardDAV clients.

use base64::Engine;

/// Build an HTTP Basic Authorization header value.
///
/// Encodes `username:password` in Base64 and returns the full header
/// value string (e.g. `"Basic dXNlcjpwYXNz"`).
///
/// # Examples
///
/// ```
/// let header = dav_utils::auth::basic_auth_header("user", "pass");
/// assert!(header.starts_with("Basic "));
/// ```
pub fn basic_auth_header(username: &str, password: &str) -> String {
    let credentials = format!("{username}:{password}");
    let encoded = base64::engine::general_purpose::STANDARD.encode(credentials);
    format!("Basic {encoded}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_auth_starts_with_basic() {
        let header = basic_auth_header("user", "pass");
        assert!(header.starts_with("Basic "));
    }

    #[test]
    fn basic_auth_roundtrip() {
        let header = basic_auth_header("alice", "wonderland");
        let encoded = header.strip_prefix("Basic ").unwrap();
        let decoded_bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        let decoded = String::from_utf8(decoded_bytes).unwrap();
        assert_eq!(decoded, "alice:wonderland");
    }

    #[test]
    fn basic_auth_with_special_characters() {
        let header = basic_auth_header("user@example.com", "p@ss:w0rd!");
        let encoded = header.strip_prefix("Basic ").unwrap();
        let decoded_bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        let decoded = String::from_utf8(decoded_bytes).unwrap();
        assert_eq!(decoded, "user@example.com:p@ss:w0rd!");
    }

    #[test]
    fn basic_auth_with_empty_password() {
        let header = basic_auth_header("user", "");
        let encoded = header.strip_prefix("Basic ").unwrap();
        let decoded_bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        let decoded = String::from_utf8(decoded_bytes).unwrap();
        assert_eq!(decoded, "user:");
    }

    #[test]
    fn basic_auth_with_empty_username() {
        let header = basic_auth_header("", "pass");
        let encoded = header.strip_prefix("Basic ").unwrap();
        let decoded_bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        let decoded = String::from_utf8(decoded_bytes).unwrap();
        assert_eq!(decoded, ":pass");
    }

    #[test]
    fn basic_auth_with_unicode() {
        let header = basic_auth_header("rene", "muller");
        let encoded = header.strip_prefix("Basic ").unwrap();
        let decoded_bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        let decoded = String::from_utf8(decoded_bytes).unwrap();
        assert_eq!(decoded, "rene:muller");
    }
}
