//! CORS middleware configuration (Requirement 12).
//!
//! Auto-configures based on bind address:
//! - `127.0.0.1` -> permissive (any origin, any method, any header)
//! - `0.0.0.0`   -> strict (no origins allowed unless explicitly configured)

use axum::http::{HeaderValue, Method};
use tower_http::cors::{AllowOrigin, CorsLayer};

/// Build a CORS layer based on the bind address and optional explicit origins.
///
/// When `explicit_origins` is non-empty, those origins are used regardless
/// of the bind address (Requirement 12.3).
pub fn cors_layer(bind_address: &str, explicit_origins: &[String]) -> CorsLayer {
    if !explicit_origins.is_empty() {
        let origins: Vec<HeaderValue> = explicit_origins
            .iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        return CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers(tower_http::cors::Any);
    }

    if is_localhost(bind_address) {
        CorsLayer::permissive()
    } else {
        CorsLayer::new()
            .allow_origin(AllowOrigin::exact(HeaderValue::from_static(
                "https://localhost",
            )))
            .allow_methods([Method::GET, Method::OPTIONS])
            .allow_headers(tower_http::cors::Any)
    }
}

/// Returns `true` if the address is a localhost variant.
fn is_localhost(address: &str) -> bool {
    // Strip port suffix for IPv4/hostname (e.g. "127.0.0.1:3000"), but
    // handle IPv6 literals like "::1" and "[::1]:3000" correctly.
    let host = if address.starts_with('[') {
        // Bracketed IPv6: "[::1]:3000"
        address.split(']').next().unwrap_or(address).trim_start_matches('[')
    } else if address.contains("::") {
        // Bare IPv6 like "::1"
        address
    } else {
        // IPv4 or hostname, possibly with :port
        address.rsplit_once(':').map_or(address, |(host, _)| host)
    };
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_localhost_recognises_loopback() {
        assert!(is_localhost("127.0.0.1"));
        assert!(is_localhost("localhost"));
        assert!(is_localhost("::1"));
        assert!(is_localhost("127.0.0.1:3000"));
    }

    #[test]
    fn is_localhost_rejects_wildcard() {
        assert!(!is_localhost("0.0.0.0"));
        assert!(!is_localhost("0.0.0.0:3000"));
        assert!(!is_localhost("192.168.1.1"));
    }
}
