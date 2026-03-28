//! TLS termination using rustls.
//!
//! Loads PEM certificate chains and private keys from disk, builds a
//! `rustls::ServerConfig` suitable for use with `tokio-rustls`.
#![allow(dead_code)]

use crate::config::TlsSettings;
use crate::error::CoreError;

use rustls::pki_types::PrivateKeyDer;
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use tokio_rustls::TlsAcceptor;

/// Load a rustls `ServerConfig` from the paths specified in `TlsSettings`.
///
/// Reads PEM-encoded certificate chain and private key files, then
/// constructs a `ServerConfig` with no client authentication.
pub fn load_tls_config(settings: &TlsSettings) -> Result<rustls::ServerConfig, CoreError> {
    // Read certificate chain.
    let cert_file = File::open(&settings.cert_path).map_err(|e| {
        CoreError::Tls(format!(
            "failed to open certificate file '{}': {e}",
            settings.cert_path
        ))
    })?;
    let mut cert_reader = BufReader::new(cert_file);
    let certs: Vec<_> = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            CoreError::Tls(format!(
                "failed to parse certificates from '{}': {e}",
                settings.cert_path
            ))
        })?;

    if certs.is_empty() {
        return Err(CoreError::Tls(format!(
            "no certificates found in '{}'",
            settings.cert_path
        )));
    }

    // Read private key.
    let key_file = File::open(&settings.key_path).map_err(|e| {
        CoreError::Tls(format!(
            "failed to open key file '{}': {e}",
            settings.key_path
        ))
    })?;
    let mut key_reader = BufReader::new(key_file);

    let key: PrivateKeyDer = read_private_key(&mut key_reader, &settings.key_path)?;

    // Build ServerConfig.
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| CoreError::Tls(format!("failed to build TLS config: {e}")))?;

    Ok(config)
}

/// Build a `TlsAcceptor` from the given `TlsSettings`.
pub fn build_tls_acceptor(settings: &TlsSettings) -> Result<TlsAcceptor, CoreError> {
    let config = load_tls_config(settings)?;
    Ok(TlsAcceptor::from(Arc::new(config)))
}

/// Read a single private key from a PEM reader (public alias for cross-module use).
pub fn read_private_key_from_reader(
    reader: &mut BufReader<File>,
    path: &str,
) -> Result<PrivateKeyDer<'static>, CoreError> {
    read_private_key(reader, path)
}

/// Read a single private key from a PEM reader.
///
/// Tries PKCS#8, RSA, and EC key formats in order.
fn read_private_key(
    reader: &mut BufReader<File>,
    path: &str,
) -> Result<PrivateKeyDer<'static>, CoreError> {
    // Collect all items from the PEM file.
    let mut keys: Vec<PrivateKeyDer<'static>> = Vec::new();

    for item in rustls_pemfile::read_all(reader) {
        match item {
            Ok(rustls_pemfile::Item::Pkcs1Key(key)) => {
                keys.push(PrivateKeyDer::Pkcs1(key));
            }
            Ok(rustls_pemfile::Item::Pkcs8Key(key)) => {
                keys.push(PrivateKeyDer::Pkcs8(key));
            }
            Ok(rustls_pemfile::Item::Sec1Key(key)) => {
                keys.push(PrivateKeyDer::Sec1(key));
            }
            Ok(_) => {
                // Skip certificates and other items.
            }
            Err(e) => {
                return Err(CoreError::Tls(format!(
                    "failed to parse key from '{path}': {e}"
                )));
            }
        }
    }

    keys.into_iter().next().ok_or_else(|| {
        CoreError::Tls(format!("no private key found in '{path}'"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn fails_for_nonexistent_cert_file() {
        let settings = TlsSettings {
            enabled: true,
            cert_path: "/nonexistent/cert.pem".into(),
            key_path: "/nonexistent/key.pem".into(),
        };
        let err = load_tls_config(&settings).unwrap_err();
        assert!(err.to_string().contains("failed to open certificate file"));
    }

    #[test]
    fn fails_for_nonexistent_key_file() {
        // Create a valid-looking cert file but point to a missing key.
        let mut cert_file = NamedTempFile::new().unwrap();
        // Write a minimal self-signed cert (we just need a parseable PEM).
        // Use an actual PEM structure so rustls_pemfile can parse it.
        write!(
            cert_file,
            "-----BEGIN CERTIFICATE-----\n\
             MIIBkTCB+wIUY5mGRqXqBE9G1AAAAAAAAAAAAAAWMA0GCSqGSIb3DQEBCwUAMBEx\n\
             DzANBgNVBAMMBnRlc3RDQTAEFW0yNDA1MDEwMDAwMDBaFw0yNTA1MDEwMDAwMDBa\n\
             MBExDzANBgNVBAMMBnRlc3RDQTBcMA0GCSqGSIb3DQEBAQUAAw==\n\
             -----END CERTIFICATE-----\n"
        )
        .unwrap();

        let settings = TlsSettings {
            enabled: true,
            cert_path: cert_file.path().to_string_lossy().to_string(),
            key_path: "/nonexistent/key.pem".into(),
        };
        let err = load_tls_config(&settings).unwrap_err();
        assert!(err.to_string().contains("failed to open key file"));
    }

    #[test]
    fn fails_for_invalid_cert_pem() {
        let mut cert_file = NamedTempFile::new().unwrap();
        cert_file.write_all(b"not a valid PEM file").unwrap();

        let mut key_file = NamedTempFile::new().unwrap();
        key_file.write_all(b"not a valid key").unwrap();

        let settings = TlsSettings {
            enabled: true,
            cert_path: cert_file.path().to_string_lossy().to_string(),
            key_path: key_file.path().to_string_lossy().to_string(),
        };
        let err = load_tls_config(&settings).unwrap_err();
        // Should fail because no certs were found.
        assert!(err.to_string().contains("no certificates found"));
    }

    #[test]
    fn fails_for_invalid_key_pem() {
        // Write a certificate PEM that rustls_pemfile will parse.
        let mut cert_file = NamedTempFile::new().unwrap();
        write!(
            cert_file,
            "-----BEGIN CERTIFICATE-----\n\
             MIIBkTCB+wIUY5mGRqXqBE9G1AAAAAAAAAAAAAAWMA0GCSqGSIb3DQEBCwUAMBEx\n\
             DzANBgNVBAMMBnRlc3RDQTAEFW0yNDA1MDEwMDAwMDBaFw0yNTA1MDEwMDAwMDBa\n\
             MBExDzANBgNVBAMMBnRlc3RDQTBcMA0GCSqGSIb3DQEBAQUAAw==\n\
             -----END CERTIFICATE-----\n"
        )
        .unwrap();

        let mut key_file = NamedTempFile::new().unwrap();
        key_file.write_all(b"this is not a valid PEM key").unwrap();

        let settings = TlsSettings {
            enabled: true,
            cert_path: cert_file.path().to_string_lossy().to_string(),
            key_path: key_file.path().to_string_lossy().to_string(),
        };
        let err = load_tls_config(&settings).unwrap_err();
        // Should fail because no private key found.
        assert!(err.to_string().contains("no private key found"));
    }

    #[test]
    fn build_tls_acceptor_fails_for_bad_config() {
        let settings = TlsSettings {
            enabled: true,
            cert_path: "/nonexistent/cert.pem".into(),
            key_path: "/nonexistent/key.pem".into(),
        };
        let result = build_tls_acceptor(&settings);
        assert!(result.is_err());
    }

    /// Install the default rustls `CryptoProvider` for tests that build
    /// a full `ServerConfig`. This is safe to call multiple times (the
    /// second call is a no-op if the provider is already installed).
    fn ensure_crypto_provider() {
        let _ = rustls::crypto::CryptoProvider::install_default(
            rustls::crypto::aws_lc_rs::default_provider(),
        );
    }

    /// Generate a self-signed certificate and key using rcgen, write them
    /// to temp files, and verify that `load_tls_config` succeeds.
    #[test]
    fn load_tls_config_succeeds_with_rcgen_self_signed() {
        ensure_crypto_provider();

        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let cert_pem = cert.cert.pem();
        let key_pem = cert.key_pair.serialize_pem();

        let mut cert_file = NamedTempFile::new().unwrap();
        cert_file.write_all(cert_pem.as_bytes()).unwrap();

        let mut key_file = NamedTempFile::new().unwrap();
        key_file.write_all(key_pem.as_bytes()).unwrap();

        let settings = TlsSettings {
            enabled: true,
            cert_path: cert_file.path().to_string_lossy().to_string(),
            key_path: key_file.path().to_string_lossy().to_string(),
        };

        let config = load_tls_config(&settings);
        assert!(config.is_ok(), "expected TLS config to load: {config:?}");
    }

    /// Verify `build_tls_acceptor` returns a valid acceptor with real certs.
    #[test]
    fn build_tls_acceptor_succeeds_with_rcgen_self_signed() {
        ensure_crypto_provider();

        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let cert_pem = cert.cert.pem();
        let key_pem = cert.key_pair.serialize_pem();

        let mut cert_file = NamedTempFile::new().unwrap();
        cert_file.write_all(cert_pem.as_bytes()).unwrap();

        let mut key_file = NamedTempFile::new().unwrap();
        key_file.write_all(key_pem.as_bytes()).unwrap();

        let settings = TlsSettings {
            enabled: true,
            cert_path: cert_file.path().to_string_lossy().to_string(),
            key_path: key_file.path().to_string_lossy().to_string(),
        };

        let acceptor = build_tls_acceptor(&settings);
        assert!(
            acceptor.is_ok(),
            "expected TLS acceptor to build: {}",
            acceptor.err().map_or_else(String::new, |e| e.to_string())
        );
    }
}
