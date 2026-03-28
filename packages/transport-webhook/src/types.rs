//! Module-internal types for the webhook transport crate.

/// Content-Type expected for inbound webhook payloads.
pub const CONTENT_TYPE_JSON: &str = "application/json";

/// Maximum age in seconds for webhook timestamp validation (5 minutes).
pub const MAX_TIMESTAMP_AGE_SECS: i64 = 300;

/// Header name for HMAC-SHA256 signature.
pub const HEADER_SIGNATURE: &str = "x-webhook-signature";

/// Header name for webhook timestamp.
pub const HEADER_TIMESTAMP: &str = "x-webhook-timestamp";

/// Header name for idempotency key.
pub const HEADER_IDEMPOTENCY_KEY: &str = "x-idempotency-key";
