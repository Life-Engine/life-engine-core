//! Backup scheduling — converts schedule configs to cron expressions
//! and computes next run times.

use std::str::FromStr;

use chrono::{DateTime, Utc};

use crate::types::BackupSchedule;

/// Convert a BackupSchedule to a cron expression string.
pub fn to_cron_expression(schedule: &BackupSchedule) -> String {
    match schedule {
        BackupSchedule::Daily { hour } => {
            format!("0 0 {hour} * * *")
        }
        BackupSchedule::Weekly { day, hour } => {
            format!("0 0 {hour} * * {day}")
        }
        BackupSchedule::Cron { expression } => expression.clone(),
    }
}

/// Compute the next scheduled run time from a cron expression.
pub fn next_run(cron_expr: &str, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let schedule = cron::Schedule::from_str(cron_expr).ok()?;
    schedule.after(&after).next()
}

/// Check if the schedule is due (next run time is in the past or now).
pub fn is_due(cron_expr: &str, last_run: Option<DateTime<Utc>>) -> bool {
    let reference = last_run.unwrap_or_else(|| DateTime::from_timestamp(0, 0).unwrap());
    match next_run(cron_expr, reference) {
        Some(next) => next <= Utc::now(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daily_schedule_to_cron() {
        let schedule = BackupSchedule::Daily { hour: 3 };
        let cron = to_cron_expression(&schedule);
        assert_eq!(cron, "0 0 3 * * *");
    }

    #[test]
    fn weekly_schedule_to_cron() {
        let schedule = BackupSchedule::Weekly { day: 0, hour: 2 };
        let cron = to_cron_expression(&schedule);
        assert_eq!(cron, "0 0 2 * * 0");
    }

    #[test]
    fn custom_cron_passthrough() {
        let schedule = BackupSchedule::Cron {
            expression: "0 30 4 * * 1-5".into(),
        };
        let cron = to_cron_expression(&schedule);
        assert_eq!(cron, "0 30 4 * * 1-5");
    }

    #[test]
    fn next_run_returns_future_time() {
        let now = Utc::now();
        let result = next_run("0 0 * * * *", now);
        assert!(result.is_some());
        assert!(result.unwrap() > now);
    }

    #[test]
    fn next_run_invalid_cron_returns_none() {
        let result = next_run("invalid cron", Utc::now());
        assert!(result.is_none());
    }

    #[test]
    fn is_due_with_no_last_run() {
        // With no last run (epoch), a daily schedule should be due.
        let due = is_due("0 0 * * * *", None);
        assert!(due);
    }

    #[test]
    fn is_due_with_recent_last_run() {
        // If last run was just now, next run should be in the future.
        let due = is_due("0 0 * * * *", Some(Utc::now()));
        assert!(!due);
    }
}
