# Backup and Webhooks Plugin Review

Review date: 2026-03-28

## Summary

The backup plugin (`com.life-engine.backup`) and webhook plugins (`com.life-engine.webhook-sender`, `com.life-engine.webhook-receiver`) are structurally sound with good security fundamentals. Credentials are redacted in Debug/Serialize output, encryption uses AES-256-GCM with Argon2id key derivation, HMAC-SHA256 signature verification is implemented correctly, and path traversal is blocked in the local backend. However, several issues need attention: a fixed cryptographic salt weakens key derivation, the S3 backend lacks pagination for large bucket listings, the WebDAV PROPFIND parser discards `size` and `last_modified` metadata, retention policy only supports count-based cleanup (not age-based despite the config advertising `retention_days`), the webhook sender's `handle_event` does not actually dispatch HTTP requests, and the delivery log is purely in-memory with no persistence. The webhook receiver lacks a manifest.toml and does not implement the WASM `Plugin` trait.

---

## Plugin-by-Plugin Analysis

### Backup Plugin (`com.life-engine.backup`)

#### Cargo.toml

- `plugins/engine/backup/Cargo.toml`
- Correctly uses workspace versions and editions.
- S3 SDK dependencies (`aws-sdk-s3`, `aws-config`) are behind the `integration` feature gate, keeping the default build lean.
- `flate2` and `cron` pinned to major versions (`1` and `0.13` respectively) -- acceptable for early development.
- `quick-xml = "0.37"` is used only for WebDAV PROPFIND parsing -- appropriate.

#### manifest.toml

- `plugins/engine/backup/manifest.toml`
- Plugin ID, name, version all consistent with `lib.rs`.
- Six actions declared: `backup`, `backup_incremental`, `restore`, `restore_partial`, `list_backups`, `status`.
- Capabilities: `storage_read`, `storage_write`, `config_read`, `logging`. Does not declare `http_outbound` even though the WebDAV backend uses `reqwest` for HTTP. This is a capability gap.
- Config schema advertises `retention_days` (integer) but the actual retention implementation uses `max_count`. Misleading.
- Config schema does not include `passphrase` or `argon2` parameters, so the full `BackupConfig` type cannot be configured from the manifest schema alone.

#### src/lib.rs

- `plugins/engine/backup/src/lib.rs`
- Implements both the WASM `Plugin` trait and the `CorePlugin` async trait.
- The `Plugin::execute` method is a pass-through stub -- all actions simply return the input `PipelineMessage` unchanged. No actual backup logic is wired through the WASM execute path.
- `CorePlugin::on_load` does not read or deserialize configuration from the `PluginContext`, so `self.config` is always `None`. Configuration loading is unimplemented.
- `CorePlugin::handle_event` is a no-op. Scheduled backup triggers via event bus are not implemented.
- The `#[allow(dead_code)]` annotation on `config` confirms this field is unused.
- Tests are comprehensive for metadata, capabilities, routes, lifecycle, and both trait implementations.

#### src/engine.rs

- `plugins/engine/backup/src/engine.rs`
- Core backup/restore orchestration logic. Well-structured with clear separation of concerns.
- `create_full_backup`: Serializes all records into a `BackupArchive`, compresses with gzip, encrypts with AES-256-GCM, stores both the encrypted payload (`.enc`) and an unencrypted manifest (`.manifest.json`).
- The manifest is stored unencrypted. This leaks metadata (collection names, record counts, timestamps) to anyone with backend access. This is documented as intentional ("for listing") but should be explicitly noted as a security trade-off.
- `create_incremental_backup`: Uses `Vec::contains` for deduplication instead of a `HashSet` like the full backup path. Minor inconsistency but not a bug since collections lists are typically small.
- `restore_full`: Downloads encrypted blob, verifies SHA-256 checksum against the stored manifest, then decrypts. The checksum verification happens before decryption, which is correct (detect tamper before expending CPU on Argon2 + decrypt).
- `restore_partial`: Decrypts the entire backup and then filters by collection. This means even a single-collection partial restore must download, decrypt, and decompress the entire backup. For large backups this is wasteful but unavoidable with the current single-blob archive format.
- `list_backups`: Uses `futures::future::join_all` for concurrent manifest fetching. Silently skips manifests that fail to deserialize (`.flatten()` on Results). This means a corrupted manifest file is invisible rather than surfaced as an error.
- Tests are thorough: roundtrip, wrong passphrase, tampered data, empty records, incremental, partial restore, and listing.

#### src/backend.rs

- `plugins/engine/backup/src/backend.rs`
- Clean `BackupBackend` trait with five methods: `put`, `get`, `delete`, `list`, `exists`.
- Object-safe (verified by test).
- `StoredBackup` uses `String` for `last_modified` rather than a typed timestamp. This makes cross-backend comparison harder.

#### src/backend/local.rs

- `plugins/engine/backup/src/backend/local.rs`
- Path traversal protection: rejects keys containing `..`, starting with `/` or `\\`, and verifies the resolved path is within `base_dir`. Good.
- `list` method: only reads flat files (not recursive). If backups were stored in subdirectories (which `put` supports via `create_dir_all`), they would not appear in listings.
- `exists` uses synchronous `path.exists()` inside an async method. This blocks the async runtime. Should use `tokio::fs::try_exists()` or `tokio::fs::metadata()`.
- Tests cover: put/get roundtrip, subdirectory creation, delete, list, exists, nonexistent get.

#### src/backend/s3.rs

- `plugins/engine/backup/src/backend/s3.rs`
- `S3BackupConfig.secret_access_key` is `#[serde(skip_serializing)]` -- good, prevents accidental leakage.
- Custom `Debug` implementation redacts `secret_access_key` -- good.
- Integration feature gate: real S3 implementation only compiles with `--features integration`. Non-integration stub panics with bail messages. Clean separation.
- `list` method does not handle pagination (`ContinuationToken`). S3 `list_objects_v2` returns a maximum of 1000 objects per request. Backups beyond 1000 will be silently truncated.
- `delete` method does a `head_object` check before `delete_object`. This is a TOCTOU race condition (object could be deleted between head and delete). The extra round-trip is also unnecessary since `delete_object` on a non-existent key is a no-op in S3.
- `build_sdk_client` creates a new S3 client on every operation. This is expensive due to HTTP connection setup. The client should be created once and cached.
- `force_path_style(true)` is set unconditionally. This is correct for MinIO and other S3-compatible services but may cause issues with AWS virtual-hosted-style buckets.
- `size` field cast: `obj.size().unwrap_or_default() as u64` -- `size()` returns `Option<i64>`. Negative values would produce incorrect results. In practice S3 never returns negative sizes, but the cast is technically unsound.

#### src/backend/webdav.rs

- `plugins/engine/backup/src/backend/webdav.rs`
- `WebDavConfig.password` is `#[serde(skip_serializing)]` with custom `Debug` redaction -- good.
- `full_url` uses `urlencoding::encode` on the key, which prevents path traversal via URL injection. Good.
- `list` uses PROPFIND with `Depth: 1`. The XML parser (`parse_propfind_response`) only extracts `<href>` elements. It does not extract `<getcontentlength>` or `<getlastmodified>` from the PROPFIND response, so `StoredBackup.size` is always 0 and `last_modified` is always empty. This means retention decisions based on file size or age are impossible from WebDAV listings.
- The PROPFIND parser silently ignores XML parse errors (`Err(_) => break`). A partial parse of a valid-but-large response would silently truncate results.
- `reqwest::Method::from_bytes(b"PROPFIND").unwrap()` -- this is fine; PROPFIND is a valid HTTP method. The unwrap is safe.
- The client is created once in `new()` and reused -- good (unlike S3 backend).
- No timeout configuration on HTTP requests. Long-running or hung connections will block indefinitely.

#### src/crypto.rs

- `plugins/engine/backup/src/crypto.rs`
- Uses AES-256-GCM for authenticated encryption with a random 96-bit nonce per encryption call. Correct.
- Uses Argon2id for key derivation (V0x13). Correct algorithm choice.
- **Fixed salt** (`b"life-engine-salt"`): This is a significant weakness. The same passphrase always produces the same encryption key across all installations and all backups. An attacker who compromises one user's key (via passphrase brute-force) can use rainbow tables against all other users with the same passphrase. The salt should be randomly generated per backup and stored alongside the encrypted data.
- The comment says "Matches the Core SQLCipher key derivation salt" -- so this is intentionally synchronized with the database encryption. This creates a dependency: if the backup passphrase equals the database passphrase, the backup encryption key is the same as the database encryption key. Compromising one compromises both.
- Nonce is generated randomly per `encrypt()` call using `OsRng` -- correct. No nonce reuse risk.
- `decrypt` validates minimum ciphertext length (nonce + tag = 28 bytes) before attempting decryption -- good.
- `compress`/`decompress` use gzip with default compression level. No decompression bomb protection (unbounded `read_to_end`). A maliciously crafted backup could expand to consume all available memory.
- SHA-256 checksum is computed over the encrypted (not plaintext) data. This is correct -- it verifies integrity of what is stored on the backend.
- Tests are comprehensive: roundtrip, large data, wrong key, tampered data, too-short data, full pipeline.

#### src/retention.rs

- `plugins/engine/backup/src/retention.rs`
- Implements count-based retention only (`RetentionPolicy.max_count`).
- The manifest config schema in `manifest.toml` advertises `retention_days` but there is no age-based retention implementation. Users who configure `retention_days` will not get the behavior they expect.
- `enforce_retention` sorts manifests by `created_at` descending and deletes the oldest beyond `max_count`. Correct.
- Does not handle partial deletion failures. If deleting the `.enc` file succeeds but the `.manifest.json` delete fails, the system is left in an inconsistent state (manifest without data, or data without manifest).
- Deletion is sequential per manifest. For backends with high latency (S3, WebDAV), this could be slow with many expired backups. Could use concurrent deletion.
- Tests are thorough: delete old, under limit, keeps newest, empty list, max_count zero.

#### src/schedule.rs

- `plugins/engine/backup/src/schedule.rs`
- Clean conversion from `BackupSchedule` enum to cron expression strings.
- Validates hour (0-23) and day-of-week (0-6) ranges. Good.
- `BackupSchedule::Cron` passes the expression through without validation. The validation happens lazily in `next_run` when parsing.
- `is_due` computes whether the next scheduled run (after the last run time) is in the past. Correct.
- No jitter or randomization in scheduling. Multiple Life Engine instances on the same schedule would all back up simultaneously.

#### src/config.rs

- `plugins/engine/backup/src/config.rs`
- `BackupPluginConfig` duplicates some fields from `BackupConfig` in types.rs (`backend`, `schedule`, `retention_days`, `encryption_enabled`).
- `encryption_enabled` defaults to `true` but there is no code path that respects this flag. Encryption is always applied in `engine.rs`. If a user sets `encryption_enabled: false`, backups are still encrypted.
- `retention_days` is a `u32` but the retention system uses `max_count`. These are disconnected.

#### src/types.rs

- `plugins/engine/backup/src/types.rs`
- `BackupConfig.passphrase` is `#[serde(skip_serializing)]` with custom `Debug` redaction -- good.
- `BackupTarget` enum variants for S3 and WebDav include credentials directly. The `secret_access_key` and `password` fields are in the enum but the custom Debug redacts them. However, the `Serialize` derive does not skip them (unlike the flat config structs in the backend modules). This means `serde_json::to_string(&BackupTarget::S3{...})` will include the secret access key in the output.
- `Argon2Params` defaults: 64 MB memory, 3 iterations, 1 parallelism. These are reasonable for a personal/household device but on the low end for security-critical applications. OWASP recommends 19 MiB / 2 iterations / 1 parallelism as minimum; these defaults exceed that.
- `BackupManifest.compressed_size` is pre-encryption compressed size. Could be useful for estimating storage but the more operationally useful metric (encrypted size) is not recorded.
- Tests cover serialization of all types, schedule variants, and defaults.

#### src/error.rs

- `plugins/engine/backup/src/error.rs`
- Clean error enum with unique codes (BACKUP_001 through BACKUP_005).
- `BackupFailed` is `Severity::Retryable`; all others are `Fatal`. Reasonable classification.

#### src/steps/mod.rs and src/transform/mod.rs

- Both are empty stubs (doc comments only). Pipeline step handlers and message transformation are not implemented. These will need to be fleshed out to wire actions through the plugin pipeline.

#### src/tests/mod.rs

- Empty stub. No additional integration tests beyond the inline module tests.

---

### Webhook Sender Plugin (`com.life-engine.webhook-sender`)

#### Cargo.toml

- `plugins/engine/webhook-sender/Cargo.toml`
- Depends on `reqwest` for HTTP outbound but does not actually use it in any source file. The HTTP dispatch is not implemented.
- No `hmac` or `sha2` dependency. HMAC signature generation for outgoing payloads (mentioned in models as "HMAC-SHA256 signing") is not implemented in this plugin. The `WebhookSubscription.secret` field exists but nothing uses it to sign outbound requests.

#### manifest.toml

- `plugins/engine/webhook-sender/manifest.toml`
- Four actions: `subscribe`, `unsubscribe`, `subscriptions`, `deliveries`.
- Capabilities include `http_outbound` and `events_subscribe` -- correct for outbound webhook delivery.
- No config schema defined in the manifest.

#### src/lib.rs

- `plugins/engine/webhook-sender/src/lib.rs`
- Implements both `Plugin` (WASM) and `CorePlugin` (async) traits.
- `Plugin::execute` is a pass-through stub, same pattern as backup.
- `handle_event` finds matching subscriptions and logs but does not actually dispatch HTTP requests. The entire delivery mechanism is a no-op. This is the most significant gap in this plugin.
- Subscription management (`subscribe`, `unsubscribe`, `find_subscription`, `matching_subscriptions`) works correctly in-memory.
- `record_delivery_success` and `record_delivery_failure` properly track delivery attempts and manage retry state. The retry integration with `RetryState` from the SDK is clean.
- `on_unload` clears subscriptions and retry states -- correct cleanup.
- Tests are extensive: metadata, lifecycle, subscription CRUD, event matching (including inactive exclusion), delivery tracking, retry exhaustion, retry reset on success, status code tracking, WASM trait parity.

#### src/delivery.rs

- `plugins/engine/webhook-sender/src/delivery.rs`
- In-memory delivery log with bounded capacity (default 10,000). Evicts oldest entries when full. Good memory safety.
- `delivery_status` is a static method that takes `max_retries` as a parameter. This creates a risk of inconsistency if different callers pass different `max_retries` values.
- No persistence. All delivery history is lost on plugin unload or process restart. The doc comment acknowledges this ("will be replaced with storage-backed persistence").
- `recent()` returns entries in reverse order (newest first). Good for display.
- Tests are thorough: empty log, record/retrieve, status codes, subscription filtering, recent ordering, mixed counts.

#### src/models.rs

- `plugins/engine/webhook-sender/src/models.rs`
- `WebhookSubscription.secret` is `#[serde(skip_serializing)]` -- good.
- Custom `Debug` redacts secret -- good.
- `DeliveryRecord` stores the full `payload` (as `serde_json::Value`) for every delivery attempt including retries. For high-volume webhooks with large payloads, this could consume significant memory. The delivery log should consider storing a payload hash or reference instead of the full payload.
- `DeliveryStatus` enum has three states: `Success`, `Failed`, `Exhausted`. Clean.
- No URL validation on `WebhookSubscription.url`. Malformed URLs will cause runtime errors during delivery (when implemented).
- No event type validation. Empty `event_types` vec means the subscription matches nothing, which could confuse users.

#### src/config.rs

- `plugins/engine/webhook-sender/src/config.rs`
- `WebhookSenderConfig` has `max_retries` (default 5) and `max_delivery_log_size` (default 10,000).
- This config struct is not used anywhere in the plugin code. The delivery log hardcodes `DEFAULT_MAX_CAPACITY = 10_000` and the retry state uses the SDK's default of 5.

#### src/types.rs

- `plugins/engine/webhook-sender/src/types.rs`
- `WebhookSenderStatus` struct for status reporting. Not used anywhere in the plugin code.

#### src/error.rs

- `plugins/engine/webhook-sender/src/error.rs`
- Clean error enum: `SubscriptionNotFound` (WEBHOOK_001), `DeliveryFailed` (WEBHOOK_002), `RetriesExhausted` (WEBHOOK_003), `UnknownAction` (WEBHOOK_004).
- `DeliveryFailed` is `Severity::Retryable` -- correct.

#### src/steps/mod.rs, src/transform/mod.rs, src/tests/mod.rs

- All empty stubs. Pipeline steps and message transformation not implemented.

---

### Webhook Receiver Plugin (`com.life-engine.webhook-receiver`)

#### Cargo.toml

- `plugins/engine/webhook-receiver/Cargo.toml`
- Depends on `hmac = "0.12"` and `sha2` for HMAC signature verification. Correct.
- Does not define `crate-type = ["cdylib", "rlib"]` like the other plugins. Only builds as `rlib`. This means it cannot be loaded as a WASM plugin.
- No `thiserror` dependency; error handling uses `anyhow` only. Less structured than the other plugins.

#### No manifest.toml

- The webhook receiver has no `manifest.toml` file. This means it cannot be discovered or configured through the plugin manifest system. It relies entirely on programmatic registration.

#### src/lib.rs

- `plugins/engine/webhook-receiver/src/lib.rs`
- Implements `CorePlugin` but does not implement the WASM `Plugin` trait. Cannot be loaded in the WASM sandbox. Inconsistent with the other two plugins which implement both traits.
- `process_webhook` is the core method. It looks up the endpoint, verifies the HMAC signature if a secret is configured, applies payload mappings, and returns a `WebhookReceivedEvent`. Well-structured.
- When no secret is configured, signature verification is skipped entirely. No warning is logged. This is correct behavior but could be a security footgun if users forget to configure secrets.
- `process_webhook` takes both `raw_body: &[u8]` and `body: serde_json::Value` as separate parameters. The caller must ensure these are consistent (the `body` is the parsed version of `raw_body`). If they diverge, the signature is verified against `raw_body` but the mapping is applied to `body`, which could lead to accepting a payload that doesn't match the signed content.
- `handle_event` is a no-op. The receiver does not emit events after processing a webhook. The capability `EventsEmit` is declared but unused.
- `on_unload` clears endpoints -- correct.
- Tests are comprehensive: metadata, lifecycle, payload processing (with and without secret), signature rejection, mapping, unknown endpoint.

#### src/signature.rs

- `plugins/engine/webhook-receiver/src/signature.rs`
- `verify_hmac_sha256`: Uses constant-time comparison via `mac.verify_slice()`. Correct, prevents timing attacks.
- Supports the standard `sha256=<hex>` format used by GitHub, Stripe, etc. Good compatibility.
- `compute_hmac_sha256`: Helper for generating signatures. Uses `expect("HMAC key should be valid")` -- this is technically safe since HMAC-SHA256 accepts any key length, but `expect` in library code is questionable.
- Tests are thorough: roundtrip, wrong secret, tampered body, missing prefix, invalid hex, truncated signature, empty body.

#### src/mapping.rs

- `plugins/engine/webhook-receiver/src/mapping.rs`
- `apply_mappings` extracts fields via dot-separated paths and builds a flat output object. Clean implementation.
- `resolve_path` does not support array indexing (e.g., `items.0.name`). Only object navigation is supported. This limits mapping expressiveness for webhooks with array payloads.
- Missing fields are silently skipped. This is documented but could make debugging mapping errors difficult.
- Tests cover: single mapping, nested mapping, multiple mappings, missing path, empty mappings, type preservation, deeply nested paths.

#### src/models.rs

- `plugins/engine/webhook-receiver/src/models.rs`
- `WebhookEndpoint.secret` is `#[serde(skip_serializing)]` with custom Debug redaction -- good.
- `PayloadMapping` is a simple `source_path` -> `target_field` pair. No transformation functions or type coercion.
- `WebhookReceivedEvent` has all necessary fields for downstream processing.
- No `id` field on `WebhookReceivedEvent`. Each received event is not uniquely identified, making deduplication impossible.

---

## Problems Found

### Critical

1. **Fixed Argon2 salt in backup encryption** (`backup/src/crypto.rs:17`). The hardcoded salt `b"life-engine-salt"` means the same passphrase always produces the same encryption key across all installations. This weakens the key derivation by enabling pre-computed dictionary attacks. A random salt should be generated per backup and stored in the encrypted blob header.

2. **Webhook sender does not actually send webhooks** (`webhook-sender/src/lib.rs:261-271`). The `handle_event` method matches subscriptions but only logs -- it never dispatches HTTP POST requests. The entire delivery mechanism is unimplemented. The `reqwest` dependency is unused.

3. **Unencrypted manifests leak metadata** (`backup/src/engine.rs:83-85`). Backup manifests are stored unencrypted, revealing collection names, record counts, timestamps, and backup IDs to anyone with backend access (S3 bucket, WebDAV server, or local filesystem).

### Major

4. **S3 list does not paginate** (`backup/src/backend/s3.rs:121-148`). `list_objects_v2` returns max 1000 objects. Users with more than 1000 backups will have silently truncated listings, causing retention enforcement to miss old backups.

5. **S3 client recreated per operation** (`backup/src/backend/s3.rs:166-183`). `build_sdk_client()` is called on every `put`, `get`, `delete`, `list`, and `exists`. HTTP connection pooling and TLS handshakes are wasted.

6. **Retention policy mismatch**: The manifest config schema advertises `retention_days` (`manifest.toml:50`), `BackupPluginConfig` has a `retention_days` field (`config.rs:11`), but the actual `RetentionPolicy` only supports `max_count` (`types.rs:149-152`). Age-based retention is not implemented.

7. **Backup configuration never loaded** (`backup/src/lib.rs:106-109`). `CorePlugin::on_load` does not read configuration from the `PluginContext`. The `config` field is always `None`.

8. **Webhook receiver missing manifest.toml**. Cannot be registered through the plugin manifest system. Not discoverable via standard plugin loading.

9. **Webhook receiver missing WASM Plugin trait**. Unlike the backup and webhook-sender plugins, the receiver does not implement the `Plugin` trait, so it cannot be loaded as a WASM sandboxed plugin.

10. **Delivery log payload duplication** (`webhook-sender/src/models.rs:47`). Every delivery attempt (including retries) stores the full `serde_json::Value` payload. For webhooks with large payloads retried 5 times, this stores 5 copies of the same data.

11. **WebDAV PROPFIND parser discards metadata** (`backup/src/backend/webdav.rs:167-207`). The parser only extracts `<href>` elements, ignoring `<getcontentlength>` and `<getlastmodified>`. All `StoredBackup` entries from WebDAV have `size: 0` and empty `last_modified`.

12. **No HTTP request timeouts on WebDAV backend** (`backup/src/backend/webdav.rs:39-41`). `reqwest::Client::new()` uses no timeout configuration. Hung connections block indefinitely.

13. **Webhook sender HMAC signing not implemented**. `WebhookSubscription.secret` field exists and is documented for "HMAC-SHA256 signing of outgoing payloads" but no code generates signatures on outbound requests. The `hmac`/`sha2` crates are not even in the webhook-sender's dependencies.

### Minor

14. **Incremental backup dedup uses linear search** (`backup/src/engine.rs:106`). `collections.contains(&col)` is O(n) per record. Uses `HashSet` in full backup but `Vec::contains` in incremental. Inconsistent.

15. **`encryption_enabled` config flag is dead** (`backup/src/config.rs:12`). No code path reads this flag; encryption is always applied.

16. **`WebhookSenderConfig` and `WebhookSenderStatus` are unused** (`webhook-sender/src/config.rs`, `webhook-sender/src/types.rs`). Defined but not referenced anywhere in the plugin.

17. **Local backend `exists` blocks async runtime** (`backup/src/backend/local.rs:120`). Uses synchronous `path.exists()` inside an async method.

18. **`list_backups` silently drops corrupted manifests** (`backup/src/engine.rs:257`). `.flatten()` on Results discards errors. A corrupted manifest file is invisible.

19. **No decompression bomb protection** (`backup/src/crypto.rs:61-64`). `decompress` calls `read_to_end` with no size limit. A maliciously crafted gzip stream could expand to consume all memory.

20. **S3 delete has TOCTOU race** (`backup/src/backend/s3.rs:93-115`). `head_object` followed by `delete_object` is unnecessary; `delete_object` on non-existent keys is already a no-op in S3.

21. **Webhook receiver does not emit events** (`webhook-receiver/src/lib.rs:119-125`). Declares `Capability::EventsEmit` but `handle_event` is a no-op and `process_webhook` does not emit events. The processed webhook data is returned but not pushed to the event bus.

22. **No URL validation on webhook subscriptions** (`webhook-sender/src/models.rs:12`). `WebhookSubscription.url` is an unvalidated `String`.

23. **Backup `BackupTarget::S3` leaks credentials in Serialize** (`backup/src/types.rs:79-92`). Unlike the standalone `S3BackupConfig` which skips the secret, the enum variant's `Serialize` derive includes `secret_access_key`.

24. **No array index support in webhook mapping** (`webhook-receiver/src/mapping.rs:32-37`). `resolve_path` only supports object key navigation, not array indexing.

25. **`WebhookReceivedEvent` has no unique ID** (`webhook-receiver/src/models.rs:50-62`). Makes deduplication of received webhooks impossible.

26. **Empty step/transform stubs** across all three plugins. Pipeline integration is not wired.

27. **`process_webhook` takes both raw bytes and parsed JSON** (`webhook-receiver/src/lib.rs:64-69`). The caller must ensure consistency between `raw_body` and `body`. A mismatch would verify the signature against one payload but process a different one.

---

## Recommendations

1. **Generate a random salt per backup** and store it in the encrypted blob header (first 16 bytes before the nonce). Update `derive_key` to accept a salt parameter.

2. **Implement actual HTTP webhook delivery** in the webhook sender's `handle_event` method. Use `reqwest` to POST payloads, generate HMAC-SHA256 signatures when a subscription secret is configured, respect the retry state with exponential backoff.

3. **Add S3 pagination** using `ContinuationToken` in a loop until `is_truncated` is false.

4. **Cache the S3 client** in the `S3Backend` struct (create once in `new()`).

5. **Implement age-based retention** (`retention_days`) alongside count-based. Align the manifest config schema with actual capabilities.

6. **Wire plugin configuration loading** in `on_load` for all three plugins.

7. **Add a manifest.toml for the webhook receiver** and implement the WASM `Plugin` trait for consistency.

8. **Add request timeouts** to the WebDAV backend (e.g., `reqwest::Client::builder().timeout(Duration::from_secs(30)).build()`).

9. **Extract WebDAV PROPFIND metadata** (`<getcontentlength>`, `<getlastmodified>`) from the XML response to populate `StoredBackup` fields.

10. **Add decompression size limits** to prevent zip bombs (e.g., check decompressed size against a configurable maximum during `read_to_end`).

11. **Store payload references instead of full payloads** in delivery records to reduce memory usage, or deduplicate payloads by correlation ID.

12. **Add a unique event ID** to `WebhookReceivedEvent` for deduplication.

13. **Fix `BackupTarget::S3` serialization** to skip `secret_access_key` (add `#[serde(skip_serializing)]` to the field, or implement custom `Serialize`).
