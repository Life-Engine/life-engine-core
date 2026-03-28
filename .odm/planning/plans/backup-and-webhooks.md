<!--
project: backup-and-webhooks
source: .odm/qa/reports/phase-3/backup-and-webhooks.md
updated: 2026-03-28
-->

# Backup and Webhooks — QA Remediation Plan

## Plan Overview

This plan addresses the 27 issues identified in the phase-3 QA review of the backup plugin (`com.life-engine.backup`), webhook sender (`com.life-engine.webhook-sender`), and webhook receiver (`com.life-engine.webhook-receiver`). Work packages are sequenced by priority: critical security and functionality gaps first, then major correctness and completeness issues, then minor improvements.

**Source:** .odm/qa/reports/phase-3/backup-and-webhooks.md

**Progress:** 0 / 9 work packages complete

---

## 1.1 — Fix Backup Encryption Salt
> depends: none
> spec: .odm/qa/reports/phase-3/backup-and-webhooks.md

- [x] Generate a random 16-byte salt per backup in `encrypt()` and prepend it to the ciphertext blob [critical]
  <!-- file: plugins/engine/backup/src/crypto.rs -->
  <!-- purpose: Prevent pre-computed dictionary attacks; same passphrase must produce different keys per backup -->
  <!-- requirements: 1 -->
  <!-- leverage: existing encrypt/decrypt functions, OsRng already imported -->
- [x] Update `decrypt()` to read the salt from the first 16 bytes of the ciphertext blob [critical]
  <!-- file: plugins/engine/backup/src/crypto.rs -->
  <!-- purpose: Enable decryption of backups with per-backup salts -->
  <!-- requirements: 1 -->
  <!-- leverage: existing decrypt function -->
- [x] Update `derive_key()` to accept a salt parameter instead of using the hardcoded constant [critical]
  <!-- file: plugins/engine/backup/src/crypto.rs -->
  <!-- purpose: Remove the fixed salt dependency -->
  <!-- requirements: 1 -->
  <!-- leverage: existing derive_key function -->
- [x] Update all callers of `derive_key` in `engine.rs` to pass the per-backup salt [critical]
  <!-- file: plugins/engine/backup/src/engine.rs -->
  <!-- purpose: Wire per-backup salt through backup/restore orchestration -->
  <!-- requirements: 1 -->
  <!-- leverage: existing create_full_backup, restore_full functions -->
- [x] Add migration path or version check so existing backups with fixed salt can still be decrypted [critical]
  <!-- file: plugins/engine/backup/src/crypto.rs -->
  <!-- purpose: Avoid breaking existing encrypted backups -->
  <!-- requirements: 1 -->
  <!-- leverage: none -->
- [x] Update crypto tests to verify different salts produce different keys [critical]
  <!-- file: plugins/engine/backup/src/crypto.rs -->
  <!-- purpose: Validate the fix -->
  <!-- requirements: 1 -->
  <!-- leverage: existing test suite -->

## 1.2 — Implement Webhook Sender HTTP Delivery
> depends: none
> spec: .odm/qa/reports/phase-3/backup-and-webhooks.md

- [ ] Implement HTTP POST dispatch in `handle_event` using `reqwest` [critical]
  <!-- file: plugins/engine/webhook-sender/src/lib.rs -->
  <!-- purpose: The sender currently only logs matched subscriptions but never sends HTTP requests -->
  <!-- requirements: 2 -->
  <!-- leverage: existing subscription matching logic at lib.rs:261-271, reqwest already in Cargo.toml -->
- [ ] Add HMAC-SHA256 signature generation for outbound payloads when `subscription.secret` is set [major]
  <!-- file: plugins/engine/webhook-sender/src/lib.rs -->
  <!-- purpose: Subscribers need to verify payload authenticity; secret field exists but is unused -->
  <!-- requirements: 13 -->
  <!-- leverage: existing WebhookSubscription.secret field -->
- [ ] Add `hmac` and `sha2` dependencies to webhook-sender Cargo.toml [major]
  <!-- file: plugins/engine/webhook-sender/Cargo.toml -->
  <!-- purpose: Required for HMAC-SHA256 signing of outbound payloads -->
  <!-- requirements: 13 -->
  <!-- leverage: webhook-receiver already uses these crates -->
- [ ] Implement exponential backoff retry logic using existing `RetryState` [major]
  <!-- file: plugins/engine/webhook-sender/src/lib.rs -->
  <!-- purpose: Failed deliveries should retry with backoff before marking exhausted -->
  <!-- requirements: 2 -->
  <!-- leverage: existing retry state tracking, record_delivery_success/failure methods -->
- [ ] Wire `WebhookSenderConfig` to control `max_retries` and `max_delivery_log_size` [minor]
  <!-- file: plugins/engine/webhook-sender/src/lib.rs, plugins/engine/webhook-sender/src/config.rs -->
  <!-- purpose: Config struct exists but is unused; delivery log hardcodes DEFAULT_MAX_CAPACITY -->
  <!-- requirements: 16 -->
  <!-- leverage: existing WebhookSenderConfig struct -->

## 1.3 — Backup Configuration and Plugin Wiring
> depends: none
> spec: .odm/qa/reports/phase-3/backup-and-webhooks.md

- [ ] Implement configuration loading in backup plugin `on_load` from `PluginContext` [major]
  <!-- file: plugins/engine/backup/src/lib.rs -->
  <!-- purpose: self.config is always None; no backup settings are loaded at startup -->
  <!-- requirements: 7 -->
  <!-- leverage: existing config field, BackupPluginConfig struct -->
- [ ] Remove or implement `encryption_enabled` config flag [minor]
  <!-- file: plugins/engine/backup/src/config.rs, plugins/engine/backup/src/engine.rs -->
  <!-- purpose: Flag exists but encryption is always applied regardless of setting -->
  <!-- requirements: 15 -->
  <!-- leverage: existing encryption_enabled field -->
- [ ] Fix `BackupTarget::S3` serialization to skip `secret_access_key` [major]
  <!-- file: plugins/engine/backup/src/types.rs -->
  <!-- purpose: Unlike S3BackupConfig, the enum variant's Serialize derives includes credentials -->
  <!-- requirements: 23 -->
  <!-- leverage: S3BackupConfig already uses serde(skip_serializing) pattern -->
- [ ] Align manifest.toml config schema with actual `BackupPluginConfig` fields (add passphrase, argon2 params; clarify retention) [minor]
  <!-- file: plugins/engine/backup/manifest.toml -->
  <!-- purpose: Schema advertises retention_days but implementation uses max_count -->
  <!-- requirements: 6 -->
  <!-- leverage: existing manifest.toml -->

## 1.4 — Manifest Encryption and Metadata Leakage
> depends: 1.1
> spec: .odm/qa/reports/phase-3/backup-and-webhooks.md

- [ ] Encrypt backup manifests or provide an option to encrypt them [critical]
  <!-- file: plugins/engine/backup/src/engine.rs -->
  <!-- purpose: Unencrypted manifests reveal collection names, record counts, timestamps to backend operators -->
  <!-- requirements: 3 -->
  <!-- leverage: existing manifest writing at engine.rs:83-85 -->
- [ ] If manifests remain unencrypted, document the security trade-off explicitly in code and docs [critical]
  <!-- file: plugins/engine/backup/src/engine.rs -->
  <!-- purpose: Users must understand what metadata is exposed -->
  <!-- requirements: 3 -->
  <!-- leverage: none -->
- [ ] Surface corrupted manifest errors in `list_backups` instead of silently dropping them [minor]
  <!-- file: plugins/engine/backup/src/engine.rs -->
  <!-- purpose: .flatten() on Results hides deserialization failures -->
  <!-- requirements: 18 -->
  <!-- leverage: existing list_backups at engine.rs:257 -->

## 1.5 — S3 Backend Fixes
> depends: none
> spec: .odm/qa/reports/phase-3/backup-and-webhooks.md

- [ ] Implement S3 list pagination using `ContinuationToken` loop until `is_truncated` is false [major]
  <!-- file: plugins/engine/backup/src/backend/s3.rs -->
  <!-- purpose: list_objects_v2 returns max 1000 objects; backups beyond that are silently truncated -->
  <!-- requirements: 4 -->
  <!-- leverage: existing list method at s3.rs:121-148 -->
- [ ] Cache S3 client in `S3Backend` struct, create once in `new()` [major]
  <!-- file: plugins/engine/backup/src/backend/s3.rs -->
  <!-- purpose: build_sdk_client() is called per operation, wasting HTTP connection setup and TLS handshakes -->
  <!-- requirements: 5 -->
  <!-- leverage: existing build_sdk_client at s3.rs:166-183 -->
- [ ] Remove unnecessary `head_object` check before `delete_object` [minor]
  <!-- file: plugins/engine/backup/src/backend/s3.rs -->
  <!-- purpose: TOCTOU race; delete_object on non-existent keys is already a no-op in S3 -->
  <!-- requirements: 20 -->
  <!-- leverage: existing delete method -->

## 1.6 — WebDAV Backend Fixes
> depends: none
> spec: .odm/qa/reports/phase-3/backup-and-webhooks.md

- [ ] Extract `<getcontentlength>` and `<getlastmodified>` from PROPFIND XML response [major]
  <!-- file: plugins/engine/backup/src/backend/webdav.rs -->
  <!-- purpose: StoredBackup.size is always 0 and last_modified always empty from WebDAV listings -->
  <!-- requirements: 11 -->
  <!-- leverage: existing parse_propfind_response function -->
- [ ] Add request timeouts to WebDAV HTTP client (e.g., 30 second timeout) [major]
  <!-- file: plugins/engine/backup/src/backend/webdav.rs -->
  <!-- purpose: No timeout means hung connections block indefinitely -->
  <!-- requirements: 12 -->
  <!-- leverage: existing reqwest::Client::new() at webdav.rs:39-41 -->
- [ ] Handle XML parse errors gracefully instead of silently breaking on `Err(_)` [minor]
  <!-- file: plugins/engine/backup/src/backend/webdav.rs -->
  <!-- purpose: Partial parse of valid-but-large response silently truncates results -->
  <!-- requirements: 11 -->
  <!-- leverage: existing parse_propfind_response -->

## 1.7 — Retention Policy and Decompression Safety
> depends: none
> spec: .odm/qa/reports/phase-3/backup-and-webhooks.md

- [ ] Implement age-based retention (`retention_days`) alongside count-based (`max_count`) [major]
  <!-- file: plugins/engine/backup/src/retention.rs -->
  <!-- purpose: Config advertises retention_days but only max_count is implemented -->
  <!-- requirements: 6 -->
  <!-- leverage: existing enforce_retention function, RetentionPolicy struct -->
- [ ] Handle partial deletion failures in `enforce_retention` (e.g., .enc deleted but .manifest.json fails) [minor]
  <!-- file: plugins/engine/backup/src/retention.rs -->
  <!-- purpose: Partial failures leave system in inconsistent state -->
  <!-- requirements: 6 -->
  <!-- leverage: existing deletion logic -->
- [ ] Add decompression size limit to prevent zip bombs [minor]
  <!-- file: plugins/engine/backup/src/crypto.rs -->
  <!-- purpose: decompress calls read_to_end with no size limit; malicious stream could exhaust memory -->
  <!-- requirements: 19 -->
  <!-- leverage: existing decompress function at crypto.rs:61-64 -->

## 1.8 — Webhook Receiver Completeness
> depends: none
> spec: .odm/qa/reports/phase-3/backup-and-webhooks.md

- [ ] Add `manifest.toml` for the webhook receiver plugin [major]
  <!-- file: plugins/engine/webhook-receiver/manifest.toml -->
  <!-- purpose: Plugin is not discoverable via the standard manifest system -->
  <!-- requirements: 8 -->
  <!-- leverage: webhook-sender/manifest.toml as template -->
- [ ] Implement WASM `Plugin` trait for the webhook receiver [major]
  <!-- file: plugins/engine/webhook-receiver/src/lib.rs -->
  <!-- purpose: Unlike other plugins, receiver cannot be loaded as WASM sandboxed plugin -->
  <!-- requirements: 9 -->
  <!-- leverage: backup and webhook-sender Plugin implementations as template -->
- [ ] Update Cargo.toml to include `crate-type = ["cdylib", "rlib"]` [major]
  <!-- file: plugins/engine/webhook-receiver/Cargo.toml -->
  <!-- purpose: Currently only builds as rlib; cannot be loaded as WASM -->
  <!-- requirements: 9 -->
  <!-- leverage: other plugins' Cargo.toml -->
- [ ] Add unique event ID to `WebhookReceivedEvent` for deduplication [minor]
  <!-- file: plugins/engine/webhook-receiver/src/models.rs -->
  <!-- purpose: No ID field makes deduplication of received webhooks impossible -->
  <!-- requirements: 25 -->
  <!-- leverage: none -->
- [ ] Implement event emission in `handle_event` or after `process_webhook` [minor]
  <!-- file: plugins/engine/webhook-receiver/src/lib.rs -->
  <!-- purpose: EventsEmit capability declared but handle_event is a no-op -->
  <!-- requirements: 21 -->
  <!-- leverage: none -->
- [ ] Add array index support to `resolve_path` in mapping (e.g., `items.0.name`) [minor]
  <!-- file: plugins/engine/webhook-receiver/src/mapping.rs -->
  <!-- purpose: Only object navigation supported; limits mapping for webhooks with array payloads -->
  <!-- requirements: 24 -->
  <!-- leverage: existing resolve_path at mapping.rs:32-37 -->

## 1.9 — Minor Cleanup and Hardening
> depends: none
> spec: .odm/qa/reports/phase-3/backup-and-webhooks.md

- [ ] Use `HashSet` for incremental backup deduplication instead of `Vec::contains` [minor]
  <!-- file: plugins/engine/backup/src/engine.rs -->
  <!-- purpose: Linear search inconsistency with full backup path which uses HashSet -->
  <!-- requirements: 14 -->
  <!-- leverage: existing incremental backup at engine.rs:106 -->
- [ ] Replace synchronous `path.exists()` with `tokio::fs::try_exists()` in local backend [minor]
  <!-- file: plugins/engine/backup/src/backend/local.rs -->
  <!-- purpose: Synchronous call blocks async runtime -->
  <!-- requirements: 17 -->
  <!-- leverage: existing exists method at local.rs:120 -->
- [ ] Add URL validation on `WebhookSubscription.url` [minor]
  <!-- file: plugins/engine/webhook-sender/src/models.rs -->
  <!-- purpose: Malformed URLs will cause runtime errors during delivery -->
  <!-- requirements: 22 -->
  <!-- leverage: none -->
- [ ] Store payload references or hashes in delivery records instead of full payloads [minor]
  <!-- file: plugins/engine/webhook-sender/src/models.rs -->
  <!-- purpose: Each retry stores a full copy of the payload; 5 retries = 5 copies -->
  <!-- requirements: 10 -->
  <!-- leverage: existing DeliveryRecord struct at models.rs:47 -->
- [ ] Remove unused `WebhookSenderStatus` type [minor]
  <!-- file: plugins/engine/webhook-sender/src/types.rs -->
  <!-- purpose: Defined but not referenced anywhere -->
  <!-- requirements: 16 -->
  <!-- leverage: none -->
- [ ] Refactor `process_webhook` to derive `body` from `raw_body` internally instead of accepting both [minor]
  <!-- file: plugins/engine/webhook-receiver/src/lib.rs -->
  <!-- purpose: Caller must ensure consistency; mismatch would verify signature against wrong data -->
  <!-- requirements: 27 -->
  <!-- leverage: existing process_webhook at lib.rs:64-69 -->
