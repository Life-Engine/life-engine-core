//! Delivery log for tracking webhook send attempts.
//!
//! Records every delivery attempt with status code, success/failure,
//! and retry count for audit and debugging purposes.

use std::collections::VecDeque;

use crate::models::{DeliveryRecord, DeliveryStatus};

/// Maximum number of delivery records kept in memory before oldest entries
/// are evicted.
const DEFAULT_MAX_CAPACITY: usize = 10_000;

/// In-memory delivery log that records webhook send attempts.
///
/// In production, this would be backed by Core's storage. For now,
/// the in-memory log is used for testing and will be replaced with
/// storage-backed persistence during plugin loading.
///
/// The log is bounded to `max_capacity` entries; when exceeded the oldest
/// entries are evicted via O(1) `pop_front` on the `VecDeque`.
#[derive(Debug)]
pub struct DeliveryLog {
    records: VecDeque<DeliveryRecord>,
    max_capacity: usize,
}

impl Default for DeliveryLog {
    fn default() -> Self {
        Self {
            records: VecDeque::new(),
            max_capacity: DEFAULT_MAX_CAPACITY,
        }
    }
}

impl DeliveryLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a delivery log with a custom maximum capacity.
    pub fn with_max_capacity(max_capacity: usize) -> Self {
        Self {
            records: VecDeque::new(),
            max_capacity,
        }
    }

    /// Record a delivery attempt. If the log exceeds its maximum capacity,
    /// the oldest entries are evicted via O(1) pop_front.
    pub fn record(&mut self, record: DeliveryRecord) {
        self.records.push_back(record);
        while self.records.len() > self.max_capacity {
            self.records.pop_front();
        }
    }

    /// Returns all delivery records as a slice pair (VecDeque may not be contiguous).
    pub fn all(&self) -> Vec<&DeliveryRecord> {
        self.records.iter().collect()
    }

    /// Returns delivery records for a specific subscription.
    pub fn for_subscription(&self, subscription_id: &str) -> Vec<&DeliveryRecord> {
        self.records
            .iter()
            .filter(|r| r.subscription_id == subscription_id)
            .collect()
    }

    /// Returns the most recent delivery records, up to `limit`.
    pub fn recent(&self, limit: usize) -> Vec<&DeliveryRecord> {
        self.records.iter().rev().take(limit).collect()
    }

    /// Returns the status of a delivery by looking at its record.
    pub fn delivery_status(record: &DeliveryRecord, max_retries: u32) -> DeliveryStatus {
        if record.success {
            DeliveryStatus::Success
        } else if record.attempt >= max_retries {
            DeliveryStatus::Exhausted
        } else {
            DeliveryStatus::Failed
        }
    }

    /// Returns the count of successful deliveries.
    pub fn success_count(&self) -> usize {
        self.records.iter().filter(|r| r.success).count()
    }

    /// Returns the count of failed deliveries.
    pub fn failure_count(&self) -> usize {
        self.records.iter().filter(|r| !r.success).count()
    }

    /// Total number of delivery records.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the log is empty.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::DeliveryRecord;

    fn success_record(sub_id: &str, status: u16) -> DeliveryRecord {
        DeliveryRecord::success(
            uuid::Uuid::new_v4().to_string(),
            sub_id.to_string(),
            "record.created".to_string(),
            serde_json::json!({"test": true}),
            status,
            1,
        )
    }

    fn failure_record(sub_id: &str, status: u16, attempt: u32) -> DeliveryRecord {
        DeliveryRecord::failure(
            uuid::Uuid::new_v4().to_string(),
            sub_id.to_string(),
            "record.created".to_string(),
            serde_json::json!({"test": true}),
            status,
            attempt,
            format!("HTTP {}", status),
        )
    }

    #[test]
    fn empty_log() {
        let log = DeliveryLog::new();
        assert!(log.is_empty());
        assert_eq!(log.len(), 0);
        assert_eq!(log.success_count(), 0);
        assert_eq!(log.failure_count(), 0);
    }

    #[test]
    fn record_and_retrieve() {
        let mut log = DeliveryLog::new();
        log.record(success_record("sub-1", 200));
        log.record(success_record("sub-1", 201));

        assert_eq!(log.len(), 2);
        assert_eq!(log.success_count(), 2);
        assert_eq!(log.failure_count(), 0);
    }

    #[test]
    fn records_status_codes() {
        let mut log = DeliveryLog::new();
        log.record(success_record("sub-1", 200));
        log.record(failure_record("sub-1", 500, 1));
        log.record(failure_record("sub-1", 502, 2));
        log.record(success_record("sub-1", 201));

        let records = log.all();
        assert_eq!(records[0].status_code, 200);
        assert_eq!(records[1].status_code, 500);
        assert_eq!(records[2].status_code, 502);
        assert_eq!(records[3].status_code, 201);
    }

    #[test]
    fn filter_by_subscription() {
        let mut log = DeliveryLog::new();
        log.record(success_record("sub-1", 200));
        log.record(success_record("sub-2", 200));
        log.record(failure_record("sub-1", 500, 1));

        let sub1_records = log.for_subscription("sub-1");
        assert_eq!(sub1_records.len(), 2);

        let sub2_records = log.for_subscription("sub-2");
        assert_eq!(sub2_records.len(), 1);

        let sub3_records = log.for_subscription("sub-3");
        assert!(sub3_records.is_empty());
    }

    #[test]
    fn recent_returns_latest_first() {
        let mut log = DeliveryLog::new();
        log.record(success_record("sub-1", 200));
        log.record(failure_record("sub-1", 500, 1));
        log.record(success_record("sub-1", 201));

        let recent = log.recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].status_code, 201); // most recent
        assert_eq!(recent[1].status_code, 500);
    }

    #[test]
    fn recent_with_limit_larger_than_records() {
        let mut log = DeliveryLog::new();
        log.record(success_record("sub-1", 200));

        let recent = log.recent(100);
        assert_eq!(recent.len(), 1);
    }

    #[test]
    fn delivery_status_success() {
        let record = success_record("sub-1", 200);
        assert_eq!(
            DeliveryLog::delivery_status(&record, 5),
            DeliveryStatus::Success
        );
    }

    #[test]
    fn delivery_status_failed_with_retries_remaining() {
        let record = failure_record("sub-1", 500, 2);
        assert_eq!(
            DeliveryLog::delivery_status(&record, 5),
            DeliveryStatus::Failed
        );
    }

    #[test]
    fn delivery_status_exhausted() {
        let record = failure_record("sub-1", 500, 5);
        assert_eq!(
            DeliveryLog::delivery_status(&record, 5),
            DeliveryStatus::Exhausted
        );
    }

    #[test]
    fn mixed_success_and_failure_counts() {
        let mut log = DeliveryLog::new();
        log.record(success_record("sub-1", 200));
        log.record(failure_record("sub-1", 500, 1));
        log.record(success_record("sub-2", 200));
        log.record(failure_record("sub-2", 503, 1));
        log.record(failure_record("sub-2", 503, 2));

        assert_eq!(log.success_count(), 2);
        assert_eq!(log.failure_count(), 3);
        assert_eq!(log.len(), 5);
    }

    #[test]
    fn default_impl() {
        let log = DeliveryLog::default();
        assert!(log.is_empty());
    }
}
