//! Shared file metadata helper functions.
//!
//! Provides MIME type detection, SHA-256 checksum computation, and
//! `SystemTime` to `DateTime<Utc>` conversion. These utilities are
//! designed to be reused by all connectors that work with files.

use std::fs;
use std::path::Path;
use std::time::SystemTime;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

/// Detect the MIME type of a file from its extension.
///
/// Uses the `mime_guess` crate to determine the MIME type based on the
/// file extension. Falls back to `application/octet-stream` for unknown
/// or missing extensions.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use life_engine_types::file_helpers::detect_mime_type;
///
/// assert_eq!(detect_mime_type(Path::new("report.pdf")), "application/pdf");
/// assert_eq!(detect_mime_type(Path::new("noext")), "application/octet-stream");
/// ```
pub fn detect_mime_type(path: &Path) -> String {
    mime_guess::from_path(path)
        .first()
        .map(|m| m.to_string())
        .unwrap_or_else(|| "application/octet-stream".into())
}

/// Compute the SHA-256 checksum of a file's contents.
///
/// Reads the entire file into memory and returns the hash in the format
/// `sha256:{hex_digest}`. Returns an error if the file cannot be read.
///
/// # Errors
///
/// Returns `anyhow::Error` if the file at `path` cannot be read.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use life_engine_types::file_helpers::compute_sha256;
///
/// let checksum = compute_sha256(Path::new("myfile.txt")).unwrap();
/// assert!(checksum.starts_with("sha256:"));
/// ```
pub fn compute_sha256(path: &Path) -> Result<String> {
    let data = fs::read(path)
        .with_context(|| format!("failed to read file for checksum: {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let hash = hasher.finalize();
    Ok(format!("sha256:{}", hex::encode(hash)))
}

/// Convert a `SystemTime` to a `DateTime<Utc>`.
///
/// Attempts to convert the given system time to a UTC datetime. If the
/// conversion fails (e.g. the time is before the Unix epoch), falls back
/// to `Utc::now()`.
///
/// # Examples
///
/// ```
/// use std::time::{SystemTime, UNIX_EPOCH, Duration};
/// use life_engine_types::file_helpers::system_time_to_datetime;
///
/// let time = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
/// let dt = system_time_to_datetime(time);
/// assert_eq!(dt.timestamp(), 1_700_000_000);
/// ```
pub fn system_time_to_datetime(time: SystemTime) -> DateTime<Utc> {
    time.duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|d| DateTime::from_timestamp(d.as_secs() as i64, d.subsec_nanos()))
        .unwrap_or_else(Utc::now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};
    use tempfile::TempDir;

    // --- detect_mime_type tests ---

    #[test]
    fn detect_mime_type_pdf() {
        assert_eq!(detect_mime_type(Path::new("file.pdf")), "application/pdf");
    }

    #[test]
    fn detect_mime_type_jpg() {
        assert_eq!(detect_mime_type(Path::new("photo.jpg")), "image/jpeg");
    }

    #[test]
    fn detect_mime_type_txt() {
        assert_eq!(detect_mime_type(Path::new("notes.txt")), "text/plain");
    }

    #[test]
    fn detect_mime_type_rs() {
        assert_eq!(detect_mime_type(Path::new("main.rs")), "text/x-rust");
    }

    #[test]
    fn detect_mime_type_md() {
        assert_eq!(detect_mime_type(Path::new("README.md")), "text/markdown");
    }

    #[test]
    fn detect_mime_type_unknown() {
        assert_eq!(
            detect_mime_type(Path::new("noext")),
            "application/octet-stream"
        );
    }

    // --- compute_sha256 tests ---

    #[test]
    fn compute_sha256_known_content() {
        let dir = TempDir::new().expect("create temp dir");
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "hello world").expect("write file");

        let checksum = compute_sha256(&file_path).expect("compute checksum");
        assert!(checksum.starts_with("sha256:"));
        assert_eq!(
            checksum,
            "sha256:b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn compute_sha256_nonexistent_file() {
        let result = compute_sha256(Path::new("/nonexistent/file.txt"));
        assert!(result.is_err());
    }

    #[test]
    fn compute_sha256_empty_file() {
        let dir = TempDir::new().expect("create temp dir");
        let file_path = dir.path().join("empty.txt");
        fs::write(&file_path, "").expect("write empty file");

        let checksum = compute_sha256(&file_path).expect("compute checksum");
        assert!(checksum.starts_with("sha256:"));
        // SHA-256 of empty string
        assert_eq!(
            checksum,
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    // --- system_time_to_datetime tests ---

    #[test]
    fn system_time_to_datetime_known_timestamp() {
        let time = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let dt = system_time_to_datetime(time);
        assert_eq!(dt.timestamp(), 1_700_000_000);
    }

    #[test]
    fn system_time_to_datetime_epoch() {
        let dt = system_time_to_datetime(UNIX_EPOCH);
        assert_eq!(dt.timestamp(), 0);
    }

    #[test]
    fn system_time_to_datetime_with_subsec_nanos() {
        let time = UNIX_EPOCH + Duration::new(1_700_000_000, 500_000_000);
        let dt = system_time_to_datetime(time);
        assert_eq!(dt.timestamp(), 1_700_000_000);
        assert_eq!(dt.timestamp_subsec_nanos(), 500_000_000);
    }
}
