#![cfg(feature = "integration")]

//! Integration tests for S3 cloud storage connector against MinIO.
//!
//! These tests require a running MinIO instance started via Docker Compose:
//!
//!     docker compose -f docker-compose.test.yml up -d
//!
//! Run with: `cargo test -p connector-filesystem --features integration`
//!
//! Each test creates a unique bucket (`test-{uuid}`) for isolation and
//! cleans it up via a `Drop`-based guard, ensuring cleanup even on panic.

use chrono::Utc;
use connector_filesystem::s3::{CloudStorageConnector, S3Client, S3Config};
use life_engine_test_utils::connectors::minio_s3_config;
use uuid::Uuid;

/// Skip the test with `Ok(())` when Docker services are not available.
///
/// This variant of `skip_unless_docker` is compatible with tests that
/// return `anyhow::Result<()>`.
macro_rules! skip_unless_docker {
    () => {
        if !life_engine_test_utils::docker::is_service_available(
            life_engine_test_utils::docker::MINIO_HOST,
            life_engine_test_utils::docker::MINIO_API_PORT,
        ) {
            eprintln!(
                "SKIP: MinIO test service not available. \
                 Start with: docker compose -f docker-compose.test.yml up -d"
            );
            return Ok(());
        }
    };
}

// ---------------------------------------------------------------------------
// Test bucket guard (RAII cleanup)
// ---------------------------------------------------------------------------

/// RAII guard that creates a unique MinIO bucket on construction and
/// deletes all its objects + the bucket itself on drop.
///
/// Uses a `tokio::runtime::Handle` captured at creation time so the
/// synchronous `Drop` impl can execute async S3 calls.
struct TestBucket {
    /// The raw AWS SDK client used for bucket management.
    sdk_client: aws_sdk_s3::Client,
    /// The unique bucket name for this test.
    bucket: String,
    /// Handle to the tokio runtime for running async cleanup in `Drop`.
    rt_handle: tokio::runtime::Handle,
}

impl TestBucket {
    /// Create a new unique test bucket and return a guard.
    ///
    /// The bucket name is `test-{uuid}` to allow parallel test execution.
    async fn create() -> anyhow::Result<Self> {
        let cfg = minio_s3_config();
        let bucket = format!("test-{}", Uuid::new_v4());

        let creds = aws_sdk_s3::config::Credentials::new(
            &cfg.access_key,
            &cfg.secret_key,
            None,
            None,
            "integration-test",
        );
        let sdk_config = aws_sdk_s3::config::Builder::new()
            .endpoint_url(&cfg.endpoint)
            .region(aws_sdk_s3::config::Region::new(cfg.region))
            .credentials_provider(creds)
            .force_path_style(true)
            .behavior_version_latest()
            .build();
        let sdk_client = aws_sdk_s3::Client::from_conf(sdk_config);

        sdk_client.create_bucket().bucket(&bucket).send().await?;

        let rt_handle = tokio::runtime::Handle::current();
        Ok(Self {
            sdk_client,
            bucket,
            rt_handle,
        })
    }

    /// Build an `S3Config` pointing at this test bucket.
    fn s3_config(&self) -> S3Config {
        let cfg = minio_s3_config();
        S3Config {
            endpoint: cfg.endpoint,
            region: "us-east-1".into(),
            bucket: self.bucket.clone(),
            access_key_id: cfg.access_key,
            secret_access_key: cfg.secret_key,
            prefix: None,
        }
    }

    /// Build an `S3Client` pointing at this test bucket.
    fn s3_client(&self) -> S3Client {
        S3Client::new(self.s3_config())
    }

    /// Delete all objects in the bucket, then delete the bucket itself.
    async fn cleanup(&self) -> anyhow::Result<()> {
        // List and delete all objects (handles pagination for safety).
        let mut continuation_token: Option<String> = None;
        loop {
            let mut req = self
                .sdk_client
                .list_objects_v2()
                .bucket(&self.bucket);

            if let Some(token) = &continuation_token {
                req = req.continuation_token(token);
            }

            let resp = req.send().await?;

            for obj in resp.contents() {
                if let Some(key) = obj.key() {
                    self.sdk_client
                        .delete_object()
                        .bucket(&self.bucket)
                        .key(key)
                        .send()
                        .await?;
                }
            }

            if resp.is_truncated() == Some(true) {
                continuation_token = resp.next_continuation_token().map(String::from);
            } else {
                break;
            }
        }

        self.sdk_client
            .delete_bucket()
            .bucket(&self.bucket)
            .send()
            .await?;

        Ok(())
    }
}

impl Drop for TestBucket {
    fn drop(&mut self) {
        let sdk_client = self.sdk_client.clone();
        let bucket = self.bucket.clone();

        // Spawn cleanup on the runtime. We cannot `.await` in Drop, so
        // we use `spawn_blocking` + `block_on` to ensure it completes.
        let handle = self.rt_handle.clone();
        std::thread::spawn(move || {
            handle.block_on(async {
                // Best-effort cleanup: log but do not panic on failure.
                let guard = TestBucket {
                    sdk_client,
                    bucket,
                    rt_handle: handle.clone(),
                };
                if let Err(e) = guard.cleanup().await {
                    eprintln!("WARN: test bucket cleanup failed: {e}");
                }
                // Prevent recursive Drop by forgetting the guard.
                std::mem::forget(guard);
            });
        })
        .join()
        .ok();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Upload bytes, download them, and assert the content matches exactly.
#[tokio::test]
async fn put_and_get_object() -> anyhow::Result<()> {
    skip_unless_docker!();

    let tb = TestBucket::create().await?;
    let client = tb.s3_client();

    let payload = b"Hello, Life Engine S3 integration test!";
    client.put_object("greeting.txt", payload).await?;

    let downloaded = client.get_object("greeting.txt").await?;
    assert_eq!(
        downloaded, payload,
        "downloaded bytes must match uploaded bytes exactly"
    );

    Ok(())
}

/// Upload 3 files with different extensions, list with prefix, and verify
/// all 3 are returned with correct names, sizes, and MIME types.
#[tokio::test]
async fn list_objects_returns_uploaded_files() -> anyhow::Result<()> {
    skip_unless_docker!();

    let tb = TestBucket::create().await?;
    let client = tb.s3_client();

    let files: Vec<(&str, &[u8], &str, u64)> = vec![
        ("report.pdf", b"fake pdf content here", "application/pdf", 20),
        ("photo.jpg", b"fake jpg bytes", "image/jpeg", 14),
        ("data.json", b"{\"key\": \"value\"}", "application/json", 16),
    ];

    for (name, data, _, _) in &files {
        client.put_object(&format!("docs/{name}"), data).await?;
    }

    let listed = client.list_objects("docs/").await?;
    assert_eq!(listed.len(), 3, "expected 3 objects under docs/ prefix");

    for (name, _, expected_mime, expected_size) in &files {
        let found = listed
            .iter()
            .find(|f| f.name == *name)
            .unwrap_or_else(|| panic!("expected to find file '{name}' in listing"));

        assert_eq!(
            found.size, *expected_size,
            "size mismatch for {name}"
        );
        assert_eq!(
            found.mime_type, *expected_mime,
            "MIME type mismatch for {name}"
        );
        assert_eq!(found.source, "s3", "source should be 's3' for {name}");
        assert!(!found.id.is_nil(), "file id should not be nil for {name}");
    }

    Ok(())
}

/// Upload a file, delete it, assert `true` is returned, and verify a
/// subsequent `get_object` fails.
#[tokio::test]
async fn delete_object_existing() -> anyhow::Result<()> {
    skip_unless_docker!();

    let tb = TestBucket::create().await?;
    let client = tb.s3_client();

    client.put_object("to-delete.txt", b"delete me").await?;

    let existed = client.delete_object("to-delete.txt").await?;
    assert!(existed, "delete_object should return true for existing object");

    let get_result = client.get_object("to-delete.txt").await;
    assert!(
        get_result.is_err(),
        "get_object should fail after deletion"
    );

    Ok(())
}

/// Delete a key that was never uploaded and assert `false` is returned.
#[tokio::test]
async fn delete_object_nonexistent() -> anyhow::Result<()> {
    skip_unless_docker!();

    let tb = TestBucket::create().await?;
    let client = tb.s3_client();

    let key = format!("never-existed-{}.txt", Uuid::new_v4());
    let existed = client.delete_object(&key).await?;
    assert!(
        !existed,
        "delete_object should return false for non-existent object"
    );

    Ok(())
}

/// Upload files under `docs/` and `images/` prefixes, list only `docs/`,
/// and verify only `docs/` files are returned.
#[tokio::test]
async fn list_objects_with_prefix_filter() -> anyhow::Result<()> {
    skip_unless_docker!();

    let tb = TestBucket::create().await?;
    let client = tb.s3_client();

    // Upload files under two different prefixes
    client.put_object("docs/readme.md", b"# README").await?;
    client.put_object("docs/guide.md", b"# Guide").await?;
    client
        .put_object("images/logo.png", b"fake png bytes")
        .await?;
    client
        .put_object("images/banner.jpg", b"fake jpg bytes")
        .await?;

    // List only the docs/ prefix
    let docs = client.list_objects("docs/").await?;
    assert_eq!(docs.len(), 2, "expected exactly 2 objects under docs/");

    let doc_names: Vec<&str> = docs.iter().map(|f| f.name.as_str()).collect();
    assert!(
        doc_names.contains(&"readme.md"),
        "docs/ listing should contain readme.md"
    );
    assert!(
        doc_names.contains(&"guide.md"),
        "docs/ listing should contain guide.md"
    );

    // Verify none of the images/ files leaked into the docs/ listing
    for file in &docs {
        assert!(
            !file.name.contains("logo") && !file.name.contains("banner"),
            "docs/ listing should not contain images/ files"
        );
    }

    // Also verify the images/ prefix works independently
    let images = client.list_objects("images/").await?;
    assert_eq!(images.len(), 2, "expected exactly 2 objects under images/");

    Ok(())
}

/// Full round-trip lifecycle: put, list, get, delete on the same object,
/// verifying each step in sequence.
#[tokio::test]
async fn round_trip_lifecycle() -> anyhow::Result<()> {
    skip_unless_docker!();

    let tb = TestBucket::create().await?;
    let client = tb.s3_client();

    let prefix = format!("lifecycle-{}", Uuid::new_v4());
    let key = format!("{prefix}/roundtrip.txt");
    let payload = b"round-trip lifecycle test content";

    // Step 1: put_object
    client
        .put_object(&key, payload)
        .await
        .expect("put_object should succeed");

    // Step 2: list_objects — verify the object appears under our prefix
    let listed = client.list_objects(&format!("{prefix}/")).await?;
    assert_eq!(
        listed.len(),
        1,
        "list_objects should return exactly 1 object after put"
    );
    assert_eq!(
        listed[0].name, "roundtrip.txt",
        "listed object name should match"
    );
    assert_eq!(
        listed[0].size,
        payload.len() as u64,
        "listed object size should match uploaded payload"
    );

    // Step 3: get_object — verify byte-for-byte content match
    let downloaded = client.get_object(&key).await?;
    assert_eq!(
        downloaded, payload,
        "get_object bytes must match put_object bytes exactly"
    );

    // Step 4: delete_object — verify returns true for existing object
    let existed = client.delete_object(&key).await?;
    assert!(
        existed,
        "delete_object should return true for an existing object"
    );

    // Step 5: verify object is gone — list should be empty
    let listed_after = client.list_objects(&format!("{prefix}/")).await?;
    assert!(
        listed_after.is_empty(),
        "list_objects should return 0 objects after deletion"
    );

    // Step 6: verify get_object fails after deletion
    let get_after_delete = client.get_object(&key).await;
    assert!(
        get_after_delete.is_err(),
        "get_object should fail after the object has been deleted"
    );

    // Step 7: verify delete_object returns false for already-deleted object
    let second_delete = client.delete_object(&key).await?;
    assert!(
        !second_delete,
        "delete_object should return false for an already-deleted object"
    );

    Ok(())
}

/// Upload a file, call `track_object` to record it in sync state, verify
/// the sync state reflects the object, then call `mark_synced` and verify
/// `last_sync` is set.
#[tokio::test]
async fn sync_state_tracking() -> anyhow::Result<()> {
    skip_unless_docker!();

    let tb = TestBucket::create().await?;
    let mut client = tb.s3_client();

    // Upload a file so there is a real object to track
    let data = b"sync state tracking test data";
    client.put_object("tracked-file.txt", data).await?;

    // Verify initial sync state is empty
    assert!(
        client.sync_state().last_sync.is_none(),
        "last_sync should be None initially"
    );
    assert!(
        client.sync_state().known_objects.is_empty(),
        "known_objects should be empty initially"
    );

    // Track the object
    let now = Utc::now();
    client.track_object(
        "tracked-file.txt",
        data.len() as u64,
        now,
        Some("etag-abc123".into()),
    );

    // Verify sync state has the tracked object
    let known = client.sync_state().known_objects.get("tracked-file.txt");
    assert!(known.is_some(), "tracked object should exist in sync state");

    let obj_state = known.expect("checked above");
    assert_eq!(
        obj_state.size,
        data.len() as u64,
        "tracked object size should match"
    );
    assert_eq!(
        obj_state.etag,
        Some("etag-abc123".into()),
        "tracked object etag should match"
    );
    assert_eq!(
        obj_state.last_modified, now,
        "tracked object last_modified should match"
    );

    // Mark as synced
    client.mark_synced();

    let last_sync = client.sync_state().last_sync;
    assert!(
        last_sync.is_some(),
        "last_sync should be set after mark_synced"
    );

    // Verify the sync timestamp is recent (within last 5 seconds)
    let sync_time = last_sync.expect("checked above");
    let elapsed = Utc::now() - sync_time;
    assert!(
        elapsed.num_seconds() < 5,
        "last_sync should be within the last 5 seconds, but was {elapsed}"
    );

    Ok(())
}
