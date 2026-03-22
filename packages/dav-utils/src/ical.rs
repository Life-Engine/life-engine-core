//! iCalendar datetime parsing utilities.
//!
//! Parses iCal DATE and DATE-TIME values into `DateTime<Utc>`, handling
//! all-day events (DATE), UTC timestamps (Z suffix), local times, and
//! TZID parameters.

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};

/// Check if a date/time string is date-only (`YYYYMMDD`) vs date-time.
///
/// DATE format is exactly 8 ASCII digits. DATE-TIME is at least 15
/// characters (e.g. `YYYYMMDDTHHmmSS`).
///
/// # Examples
///
/// ```
/// assert!(dav_utils::ical::is_date_only("20260321"));
/// assert!(!dav_utils::ical::is_date_only("20260321T100000"));
/// assert!(!dav_utils::ical::is_date_only("20260321T100000Z"));
/// ```
pub fn is_date_only(value: &str) -> bool {
    value.len() == 8 && value.chars().all(|c| c.is_ascii_digit())
}

/// Parse an iCal date or date-time string into a `DateTime<Utc>`.
///
/// Supports:
/// - `YYYYMMDD` (all-day, interpreted as midnight UTC)
/// - `YYYYMMDDTHHmmSS` (local time, treated as UTC if no TZID)
/// - `YYYYMMDDTHHmmSSZ` (explicit UTC)
/// - `VALUE=DATE` parameter indicating an all-day event
/// - `TZID` parameter is noted but currently treated as UTC
///   (full timezone conversion requires a tz database)
///
/// # Errors
///
/// Returns an error if the value cannot be parsed as a valid date or
/// date-time.
///
/// # Examples
///
/// ```
/// let dt = dav_utils::ical::parse_ical_datetime("20260321T100000Z", &None).unwrap();
/// assert_eq!(dt.to_rfc3339(), "2026-03-21T10:00:00+00:00");
/// ```
pub fn parse_ical_datetime(
    value: &str,
    params: &Option<Vec<(String, Vec<String>)>>,
) -> anyhow::Result<DateTime<Utc>> {
    let value = value.trim();

    // Check for VALUE=DATE parameter indicating all-day event
    let is_date_param = params
        .as_ref()
        .map(|ps| {
            ps.iter()
                .any(|(k, v)| k == "VALUE" && v.iter().any(|val| val == "DATE"))
        })
        .unwrap_or(false);

    if is_date_only(value) || is_date_param {
        let date = NaiveDate::parse_from_str(value, "%Y%m%d")
            .map_err(|e| anyhow::anyhow!("invalid DATE '{}': {}", value, e))?;
        let datetime = date.and_time(NaiveTime::MIN);
        return Ok(Utc.from_utc_datetime(&datetime));
    }

    // Try UTC format first (ends with Z)
    if let Some(without_z) = value.strip_suffix('Z') {
        let naive = NaiveDateTime::parse_from_str(without_z, "%Y%m%dT%H%M%S")
            .map_err(|e| anyhow::anyhow!("invalid DATE-TIME '{}': {}", value, e))?;
        return Ok(Utc.from_utc_datetime(&naive));
    }

    // Local time (no Z suffix) — convert using TZID if present.
    if let Some(ps) = params
        && let Some((_key, values)) = ps.iter().find(|(k, _)| k == "TZID")
        && let Some(tz_name) = values.first()
    {
        let naive = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S")
            .map_err(|e| anyhow::anyhow!("invalid DATE-TIME '{}': {}", value, e))?;

        if let Ok(tz) = tz_name.parse::<chrono_tz::Tz>() {
            let local = tz.from_local_datetime(&naive)
                .earliest()
                .ok_or_else(|| anyhow::anyhow!("ambiguous or invalid local time '{value}' in timezone '{tz_name}'"))?;
            return Ok(local.with_timezone(&Utc));
        } else {
            tracing::warn!(
                tzid = %tz_name,
                "unknown timezone, treating as UTC"
            );
        }
    }

    let naive = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S")
        .map_err(|e| anyhow::anyhow!("invalid DATE-TIME '{}': {}", value, e))?;
    Ok(Utc.from_utc_datetime(&naive))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    // --- is_date_only ---

    #[test]
    fn date_only_valid() {
        assert!(is_date_only("20260321"));
    }

    #[test]
    fn date_only_false_for_datetime() {
        assert!(!is_date_only("20260321T100000"));
    }

    #[test]
    fn date_only_false_for_utc_datetime() {
        assert!(!is_date_only("20260321T100000Z"));
    }

    #[test]
    fn date_only_false_for_short_string() {
        assert!(!is_date_only("2026032"));
    }

    #[test]
    fn date_only_false_for_non_digits() {
        assert!(!is_date_only("2026-03-"));
    }

    #[test]
    fn date_only_false_for_empty() {
        assert!(!is_date_only(""));
    }

    // --- parse_ical_datetime ---

    #[test]
    fn parse_utc_datetime() {
        let dt = parse_ical_datetime("20260321T100000Z", &None).unwrap();
        assert_eq!(dt.to_rfc3339(), "2026-03-21T10:00:00+00:00");
    }

    #[test]
    fn parse_local_datetime() {
        let dt = parse_ical_datetime("20260321T153000", &None).unwrap();
        assert_eq!(dt.to_rfc3339(), "2026-03-21T15:30:00+00:00");
    }

    #[test]
    fn parse_date_only() {
        let dt = parse_ical_datetime("20260325", &None).unwrap();
        assert_eq!(dt.date_naive().to_string(), "2026-03-25");
        assert_eq!(dt.hour(), 0);
        assert_eq!(dt.minute(), 0);
        assert_eq!(dt.second(), 0);
    }

    #[test]
    fn parse_date_with_value_param() {
        let params = Some(vec![("VALUE".to_string(), vec!["DATE".to_string()])]);
        let dt = parse_ical_datetime("20260401", &params).unwrap();
        assert_eq!(dt.date_naive().to_string(), "2026-04-01");
        assert_eq!(dt.hour(), 0);
    }

    #[test]
    fn parse_with_tzid_param() {
        let params = Some(vec![(
            "TZID".to_string(),
            vec!["America/New_York".to_string()],
        )]);
        let dt = parse_ical_datetime("20260321T100000", &params).unwrap();
        // 10:00 AM EDT (America/New_York in March = UTC-4) = 14:00 UTC
        assert_eq!(dt.to_rfc3339(), "2026-03-21T14:00:00+00:00");
    }

    #[test]
    fn parse_invalid_returns_error() {
        let result = parse_ical_datetime("not-a-date", &None);
        assert!(result.is_err());
    }

    #[test]
    fn parse_invalid_date_format() {
        let result = parse_ical_datetime("2026-03-21", &None);
        assert!(result.is_err());
    }

    #[test]
    fn parse_trimmed_whitespace() {
        let dt = parse_ical_datetime("  20260321T100000Z  ", &None).unwrap();
        assert_eq!(dt.to_rfc3339(), "2026-03-21T10:00:00+00:00");
    }

    #[test]
    fn parse_midnight_utc() {
        let dt = parse_ical_datetime("20260101T000000Z", &None).unwrap();
        assert_eq!(dt.to_rfc3339(), "2026-01-01T00:00:00+00:00");
    }

    #[test]
    fn parse_end_of_day() {
        let dt = parse_ical_datetime("20261231T235959Z", &None).unwrap();
        assert_eq!(dt.hour(), 23);
        assert_eq!(dt.minute(), 59);
        assert_eq!(dt.second(), 59);
    }

    #[test]
    fn parse_none_params() {
        // Ensure None params don't cause issues
        let dt = parse_ical_datetime("20260601T120000Z", &None).unwrap();
        assert_eq!(dt.hour(), 12);
    }

    #[test]
    fn parse_empty_params() {
        let params: Option<Vec<(String, Vec<String>)>> = Some(vec![]);
        let dt = parse_ical_datetime("20260601T120000Z", &params).unwrap();
        assert_eq!(dt.hour(), 12);
    }
}
