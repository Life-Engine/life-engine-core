//! File metadata normalizer: converts filesystem metadata to CDM `FileMetadata`.
//!
//! Delegates MIME type detection, SHA-256 checksum computation, and
//! `SystemTime` conversion to the shared helpers in `life_engine_types::file_helpers`.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use life_engine_types::file_helpers::{compute_sha256, system_time_to_datetime};
use life_engine_types::FileMetadata;
use uuid::Uuid;

/// Normalize a filesystem path into a Life Engine CDM `FileMetadata` value.
///
/// Reads the file's metadata (name, size, timestamps), detects the MIME type
/// from its extension, and optionally computes a SHA-256 checksum.
pub fn normalize_file(path: &Path, compute_checksum: bool) -> Result<FileMetadata> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("failed to read file metadata: {}", path.display()))?;

    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unnamed".into());

    let mime_type = detect_mime_type(path);
    let size_bytes = metadata.len();

    let checksum = if compute_checksum {
        compute_sha256(path)?
    } else {
        String::new()
    };

    let created_at = metadata
        .created()
        .map(system_time_to_datetime)
        .unwrap_or_else(|_| chrono::Utc::now());

    let modified_at = metadata
        .modified()
        .map(system_time_to_datetime)
        .unwrap_or_else(|_| chrono::Utc::now());

    Ok(FileMetadata {
        id: Uuid::new_v4(),
        filename,
        path: path.to_string_lossy().to_string(),
        mime_type,
        size_bytes,
        checksum,
        storage_backend: None,
        source: "local".into(),
        source_id: path.to_string_lossy().to_string(),
        extensions: None,
        created_at,
        updated_at: modified_at,
    })
}

// Re-export shared helpers so existing callers like s3.rs continue to work
// through `crate::normalizer::detect_mime_type`.
pub use life_engine_types::file_helpers::detect_mime_type;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn normalize_file_basic_metadata() {
        let dir = TempDir::new().expect("create temp dir");
        let file_path = dir.path().join("report.pdf");
        fs::write(&file_path, "fake pdf content").expect("write file");

        let meta = normalize_file(&file_path, true).expect("normalize");
        assert_eq!(meta.filename, "report.pdf");
        assert_eq!(meta.mime_type, "application/pdf");
        assert_eq!(meta.size_bytes, 16);
        assert_eq!(meta.source, "local");
        assert!(!meta.checksum.is_empty());
        assert!(!meta.id.is_nil());
    }

    #[test]
    fn normalize_file_with_checksum_disabled() {
        let dir = TempDir::new().expect("create temp dir");
        let file_path = dir.path().join("data.csv");
        fs::write(&file_path, "a,b,c").expect("write file");

        let meta = normalize_file(&file_path, false).expect("normalize");
        assert!(meta.checksum.is_empty());
        assert_eq!(meta.filename, "data.csv");
    }

    #[test]
    fn normalize_file_with_no_extension() {
        let dir = TempDir::new().expect("create temp dir");
        let file_path = dir.path().join("Makefile");
        fs::write(&file_path, "all: build").expect("write file");

        let meta = normalize_file(&file_path, false).expect("normalize");
        assert_eq!(meta.filename, "Makefile");
        assert_eq!(meta.mime_type, "application/octet-stream");
    }

    // NOTE: Unit tests for detect_mime_type and compute_sha256 have been moved
    // to life_engine_types::file_helpers::tests. The tests below verify that
    // normalize_file correctly delegates to the shared helpers.

    #[test]
    fn normalized_file_serializes_to_json() {
        let dir = TempDir::new().expect("create temp dir");
        let file_path = dir.path().join("doc.txt");
        fs::write(&file_path, "content").expect("write file");

        let meta = normalize_file(&file_path, true).expect("normalize");
        let json = serde_json::to_string(&meta).expect("should serialize");
        let restored: FileMetadata = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(restored.filename, meta.filename);
        assert_eq!(restored.mime_type, meta.mime_type);
        assert_eq!(restored.size_bytes, meta.size_bytes);
    }

    #[test]
    fn normalize_nonexistent_file_returns_error() {
        let result = normalize_file(Path::new("/nonexistent/file.txt"), true);
        assert!(result.is_err());
    }
}
