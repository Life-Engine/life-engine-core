//! Data export utilities.
//!
//! Provides full database export as a compressed `.tar.gz` archive and
//! per-service export in standard formats (JSON, `.ics`, `.vcf`, `.eml`).
//! Fulfils the "never locked in" principle by giving users complete control
//! over their data.

use std::io::Write;

use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::Compression;
use rusqlite::Connection;
use serde_json;
use tracing::warn;

use crate::audit::{self, AuditEvent, AuditEventType};
use crate::error::StorageError;

/// A single exported record from `plugin_data`.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ExportedRecord {
    pub id: String,
    pub plugin_id: String,
    pub collection: String,
    pub data: serde_json::Value,
    pub version: u64,
    pub created_at: String,
    pub updated_at: String,
}

/// Export the full database contents as a `.tar.gz` archive.
///
/// The archive contains:
/// - `plugin_data.json` — all rows from `plugin_data` as a JSON array.
/// - `audit_log.json` — all rows from `audit_log` as a JSON array.
/// - `metadata.json` — export timestamp and record counts.
///
/// Returns the archive bytes. The caller is responsible for writing them
/// to a file or streaming to the user.
///
/// An audit event of type `DataExport` is logged on success.
pub fn export_full_archive(conn: &Connection) -> Result<Vec<u8>, StorageError> {
    let plugin_data = query_all_plugin_data(conn)?;
    let audit_data = query_all_audit_log(conn)?;

    let plugin_json = serde_json::to_vec_pretty(&plugin_data)?;
    let audit_json = serde_json::to_vec_pretty(&audit_data)?;

    let metadata = serde_json::json!({
        "exported_at": Utc::now().to_rfc3339(),
        "plugin_data_count": plugin_data.len(),
        "audit_log_count": audit_data.len(),
        "format_version": 1,
    });
    let metadata_json = serde_json::to_vec_pretty(&metadata)?;

    let buf = Vec::new();
    let encoder = GzEncoder::new(buf, Compression::default());
    let mut archive = tar::Builder::new(encoder);

    append_bytes(&mut archive, "plugin_data.json", &plugin_json)?;
    append_bytes(&mut archive, "audit_log.json", &audit_json)?;
    append_bytes(&mut archive, "metadata.json", &metadata_json)?;

    let encoder = archive.into_inner().map_err(|e| {
        StorageError::InitFailed(format!("failed to finalise tar archive: {e}"))
    })?;
    let compressed = encoder.finish().map_err(|e| {
        StorageError::InitFailed(format!("failed to finish gzip compression: {e}"))
    })?;

    // Log the export event.
    let event = AuditEvent {
        event_type: AuditEventType::DataExport,
        collection: None,
        document_id: None,
        identity_subject: None,
        plugin_id: None,
        details: serde_json::json!({
            "type": "full",
            "plugin_data_count": plugin_data.len(),
            "audit_log_count": audit_data.len(),
        }),
    };
    if let Err(e) = audit::log_event(conn, event) {
        warn!("audit log write failed during export: {e}");
    }

    Ok(compressed)
}

/// Export data for a specific service (plugin) in standard formats.
///
/// Returns a list of `(filename, content_bytes)` pairs. The format depends
/// on the collection:
///
/// - `events` — `.ics` (iCalendar VCALENDAR wrapper with VEVENT entries)
/// - `contacts` — `.vcf` (one vCard per contact, concatenated)
/// - `emails` — `.eml` (RFC 5322-style, one per email)
/// - All others — `.json` (array of records)
///
/// An audit event of type `DataExport` is logged on success.
pub fn export_service_data(
    conn: &Connection,
    plugin_id: &str,
) -> Result<Vec<(String, Vec<u8>)>, StorageError> {
    let records = query_plugin_data_by_plugin(conn, plugin_id)?;

    // Group by collection.
    let mut by_collection: std::collections::BTreeMap<String, Vec<&ExportedRecord>> =
        std::collections::BTreeMap::new();
    for record in &records {
        by_collection
            .entry(record.collection.clone())
            .or_default()
            .push(record);
    }

    let mut files: Vec<(String, Vec<u8>)> = Vec::new();

    for (collection, items) in &by_collection {
        match collection.as_str() {
            "events" => {
                let ics = render_ical(items);
                files.push((format!("{plugin_id}-events.ics"), ics.into_bytes()));
            }
            "contacts" => {
                let vcf = render_vcf(items);
                files.push((format!("{plugin_id}-contacts.vcf"), vcf.into_bytes()));
            }
            "emails" => {
                let mbox = render_mbox(items);
                files.push((format!("{plugin_id}-emails.mbox"), mbox.into_bytes()));
            }
            _ => {
                let data: Vec<&serde_json::Value> = items.iter().map(|r| &r.data).collect();
                let json = serde_json::to_vec_pretty(&data)
                    .map_err(StorageError::Serialization)?;
                files.push((format!("{plugin_id}-{collection}.json"), json));
            }
        }
    }

    // Log the export event.
    let collections: Vec<&str> = by_collection.keys().map(|s| s.as_str()).collect();
    let event = AuditEvent {
        event_type: AuditEventType::DataExport,
        collection: None,
        document_id: None,
        identity_subject: None,
        plugin_id: Some(plugin_id.to_string()),
        details: serde_json::json!({
            "type": "per_service",
            "record_count": records.len(),
            "collections": collections,
        }),
    };
    if let Err(e) = audit::log_event(conn, event) {
        warn!("audit log write failed during export: {e}");
    }

    Ok(files)
}

// --- Internal helpers ---

/// Query all rows from `plugin_data`.
fn query_all_plugin_data(conn: &Connection) -> Result<Vec<ExportedRecord>, StorageError> {
    let mut stmt = conn.prepare(
        "SELECT id, plugin_id, collection, data, version, created_at, updated_at \
         FROM plugin_data ORDER BY collection, created_at",
    )?;

    let rows = stmt
        .query_map([], |row| {
            let data_str: String = row.get(3)?;
            let data: serde_json::Value =
                serde_json::from_str(&data_str).unwrap_or(serde_json::Value::String(data_str));
            Ok(ExportedRecord {
                id: row.get(0)?,
                plugin_id: row.get(1)?,
                collection: row.get(2)?,
                data,
                version: row.get(3 + 1)?,
                created_at: row.get(3 + 2)?,
                updated_at: row.get(3 + 3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

/// Query all rows from `audit_log`.
fn query_all_audit_log(conn: &Connection) -> Result<Vec<serde_json::Value>, StorageError> {
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, event_type, plugin_id, details, created_at \
         FROM audit_log ORDER BY timestamp",
    )?;

    let rows = stmt
        .query_map([], |row| {
            let details_str: String = row.get(4)?;
            let details: serde_json::Value =
                serde_json::from_str(&details_str).unwrap_or(serde_json::Value::String(details_str));
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "timestamp": row.get::<_, String>(1)?,
                "event_type": row.get::<_, String>(2)?,
                "plugin_id": row.get::<_, Option<String>>(3)?,
                "details": details,
                "created_at": row.get::<_, String>(5)?,
            }))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

/// Query `plugin_data` rows for a specific plugin.
fn query_plugin_data_by_plugin(
    conn: &Connection,
    plugin_id: &str,
) -> Result<Vec<ExportedRecord>, StorageError> {
    let mut stmt = conn.prepare(
        "SELECT id, plugin_id, collection, data, version, created_at, updated_at \
         FROM plugin_data WHERE plugin_id = ?1 ORDER BY collection, created_at",
    )?;

    let rows = stmt
        .query_map([plugin_id], |row| {
            let data_str: String = row.get(3)?;
            let data: serde_json::Value =
                serde_json::from_str(&data_str).unwrap_or(serde_json::Value::String(data_str));
            Ok(ExportedRecord {
                id: row.get(0)?,
                plugin_id: row.get(1)?,
                collection: row.get(2)?,
                data,
                version: row.get(3 + 1)?,
                created_at: row.get(3 + 2)?,
                updated_at: row.get(3 + 3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

/// Append an in-memory byte slice to a tar archive as a file entry.
fn append_bytes<W: Write>(
    archive: &mut tar::Builder<W>,
    path: &str,
    data: &[u8],
) -> Result<(), StorageError> {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_mtime(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    header.set_cksum();

    archive
        .append_data(&mut header, path, data)
        .map_err(|e| StorageError::InitFailed(format!("failed to append {path} to archive: {e}")))?;

    Ok(())
}

/// Render event records as an iCalendar (`.ics`) string.
///
/// Produces a minimal VCALENDAR with one VEVENT per record. Fields are
/// mapped from the CDM Event schema where available.
fn render_ical(records: &[&ExportedRecord]) -> String {
    let mut out = String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Life Engine//Export//EN\r\n");
    for record in records {
        out.push_str("BEGIN:VEVENT\r\n");

        if let Some(uid) = record.data.get("id").and_then(|v| v.as_str()) {
            out.push_str(&format!("UID:{uid}\r\n"));
        } else {
            out.push_str(&format!("UID:{}\r\n", record.id));
        }

        if let Some(summary) = record.data.get("title").and_then(|v| v.as_str()) {
            out.push_str(&format!("SUMMARY:{}\r\n", ical_escape(summary)));
        }

        if let Some(desc) = record.data.get("description").and_then(|v| v.as_str()) {
            out.push_str(&format!("DESCRIPTION:{}\r\n", ical_escape(desc)));
        }

        if let Some(start) = record.data.get("start_time").and_then(|v| v.as_str()) {
            out.push_str(&format!("DTSTART:{}\r\n", to_ical_datetime(start)));
        }

        if let Some(end) = record.data.get("end_time").and_then(|v| v.as_str()) {
            out.push_str(&format!("DTEND:{}\r\n", to_ical_datetime(end)));
        }

        if let Some(loc) = record.data.get("location").and_then(|v| v.as_str()) {
            out.push_str(&format!("LOCATION:{}\r\n", ical_escape(loc)));
        }

        out.push_str("END:VEVENT\r\n");
    }
    out.push_str("END:VCALENDAR\r\n");
    out
}

/// Render contact records as a vCard (`.vcf`) string.
///
/// Produces one vCard 3.0 entry per contact, mapped from the CDM Contact
/// schema.
fn render_vcf(records: &[&ExportedRecord]) -> String {
    let mut out = String::new();
    for record in records {
        out.push_str("BEGIN:VCARD\r\nVERSION:3.0\r\n");

        let first = record.data.get("first_name").and_then(|v| v.as_str()).unwrap_or("");
        let last = record.data.get("last_name").and_then(|v| v.as_str()).unwrap_or("");
        let display = record.data.get("display_name").and_then(|v| v.as_str());

        out.push_str(&format!("N:{last};{first};;;\r\n"));
        if let Some(dn) = display {
            out.push_str(&format!("FN:{}\r\n", vcf_escape(dn)));
        } else if !first.is_empty() || !last.is_empty() {
            out.push_str(&format!("FN:{} {}\r\n", vcf_escape(first), vcf_escape(last)));
        }

        if let Some(emails) = record.data.get("emails").and_then(|v| v.as_array()) {
            for email in emails {
                if let Some(addr) = email.get("address").and_then(|v| v.as_str()) {
                    out.push_str(&format!("EMAIL:{addr}\r\n"));
                }
            }
        }

        if let Some(phones) = record.data.get("phone_numbers").and_then(|v| v.as_array()) {
            for phone in phones {
                if let Some(num) = phone.get("number").and_then(|v| v.as_str()) {
                    out.push_str(&format!("TEL:{num}\r\n"));
                }
            }
        }

        if let Some(org) = record.data.get("organization").and_then(|v| v.as_str()) {
            out.push_str(&format!("ORG:{}\r\n", vcf_escape(org)));
        }

        out.push_str("END:VCARD\r\n");
    }
    out
}

/// Render email records as an mbox-format string.
///
/// Each email is separated by a `From ` line as per the mbox convention.
/// Fields are mapped from the CDM Email schema.
fn render_mbox(records: &[&ExportedRecord]) -> String {
    let mut out = String::new();
    for record in records {
        let from_addr = record
            .data
            .get("from")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown@unknown");
        let date = record
            .data
            .get("date")
            .and_then(|v| v.as_str())
            .unwrap_or(&record.created_at);

        // mbox separator line.
        out.push_str(&format!("From {from_addr} {date}\r\n"));

        if let Some(from) = record.data.get("from").and_then(|v| v.as_str()) {
            out.push_str(&format!("From: {from}\r\n"));
        }

        if let Some(to) = record.data.get("to").and_then(|v| v.as_str()) {
            out.push_str(&format!("To: {to}\r\n"));
        } else if let Some(to_arr) = record.data.get("to").and_then(|v| v.as_array()) {
            let addrs: Vec<&str> = to_arr.iter().filter_map(|v| v.as_str()).collect();
            out.push_str(&format!("To: {}\r\n", addrs.join(", ")));
        }

        if let Some(subject) = record.data.get("subject").and_then(|v| v.as_str()) {
            out.push_str(&format!("Subject: {subject}\r\n"));
        }

        if let Some(d) = record.data.get("date").and_then(|v| v.as_str()) {
            out.push_str(&format!("Date: {d}\r\n"));
        }

        if let Some(msg_id) = record.data.get("message_id").and_then(|v| v.as_str()) {
            out.push_str(&format!("Message-ID: {msg_id}\r\n"));
        }

        // Blank line separates headers from body.
        out.push_str("\r\n");

        if let Some(body) = record.data.get("body").and_then(|v| v.as_str()) {
            // mbox format requires "From " at start of line in body to be escaped as ">From ".
            for line in body.lines() {
                if line.starts_with("From ") {
                    out.push('>');
                }
                out.push_str(line);
                out.push_str("\r\n");
            }
        }

        out.push_str("\r\n");
    }
    out
}

/// Escape special characters for iCalendar text values.
fn ical_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\n', "\\n")
}

/// Escape special characters for vCard text values.
fn vcf_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(',', "\\,")
        .replace(';', "\\;")
        .replace('\n', "\\n")
}

/// Convert an RFC 3339 timestamp to iCalendar datetime format.
///
/// Input: `2026-03-21T10:00:00Z` → Output: `20260321T100000Z`
fn to_ical_datetime(rfc3339: &str) -> String {
    // Strip dashes, colons, and anything after seconds (fractional seconds, timezone offset).
    let cleaned: String = rfc3339
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == 'T' || *c == 'Z')
        .collect();

    // Truncate to YYYYMMDDTHHmmSSZ (16 chars max with Z, 15 without).
    if cleaned.len() >= 15 {
        cleaned[..15.min(cleaned.len())].to_string()
            + if rfc3339.ends_with('Z') { "Z" } else { "" }
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::ALL_DDL;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        for ddl in ALL_DDL {
            conn.execute_batch(ddl).expect("apply DDL");
        }
        conn
    }

    fn insert_plugin_data(conn: &Connection, id: &str, plugin_id: &str, collection: &str, data: &str) {
        conn.execute(
            "INSERT INTO plugin_data (id, plugin_id, collection, data, version, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            rusqlite::params![id, plugin_id, collection, data],
        )
        .expect("insert plugin_data");
    }

    #[test]
    fn export_full_archive_empty_database() {
        let conn = setup_db();
        let archive = export_full_archive(&conn).expect("export should succeed");
        assert!(!archive.is_empty(), "archive should not be empty even with no data");

        // Decompress and verify structure.
        let decoder = flate2::read::GzDecoder::new(&archive[..]);
        let mut tar = tar::Archive::new(decoder);
        let entries: Vec<String> = tar
            .entries()
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                e.path().unwrap().to_string_lossy().into_owned()
            })
            .collect();

        assert!(entries.contains(&"plugin_data.json".to_string()));
        assert!(entries.contains(&"audit_log.json".to_string()));
        assert!(entries.contains(&"metadata.json".to_string()));
    }

    #[test]
    fn export_full_archive_with_data() {
        let conn = setup_db();
        insert_plugin_data(&conn, "r1", "plugin-a", "events", r#"{"title":"Meeting"}"#);
        insert_plugin_data(&conn, "r2", "plugin-b", "contacts", r#"{"first_name":"Alice"}"#);

        let archive = export_full_archive(&conn).expect("export should succeed");

        let decoder = flate2::read::GzDecoder::new(&archive[..]);
        let mut tar = tar::Archive::new(decoder);

        for entry in tar.entries().unwrap() {
            let mut entry = entry.unwrap();
            let path = entry.path().unwrap().to_string_lossy().into_owned();
            if path == "plugin_data.json" {
                let mut content = String::new();
                std::io::Read::read_to_string(&mut entry, &mut content).unwrap();
                let records: Vec<serde_json::Value> = serde_json::from_str(&content).unwrap();
                assert_eq!(records.len(), 2);
            }
            if path == "metadata.json" {
                let mut content = String::new();
                std::io::Read::read_to_string(&mut entry, &mut content).unwrap();
                let meta: serde_json::Value = serde_json::from_str(&content).unwrap();
                assert_eq!(meta["plugin_data_count"], 2);
                assert_eq!(meta["format_version"], 1);
            }
        }
    }

    #[test]
    fn export_full_archive_logs_audit_event() {
        let conn = setup_db();
        export_full_archive(&conn).expect("export should succeed");

        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM audit_log WHERE event_type = 'data_export'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn export_service_data_json_fallback() {
        let conn = setup_db();
        insert_plugin_data(&conn, "r1", "my-plugin", "custom_items", r#"{"key":"value"}"#);

        let files = export_service_data(&conn, "my-plugin").expect("export should succeed");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, "my-plugin-custom_items.json");

        let content: Vec<serde_json::Value> = serde_json::from_slice(&files[0].1).unwrap();
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["key"], "value");
    }

    #[test]
    fn export_service_data_events_as_ics() {
        let conn = setup_db();
        let event_json = serde_json::json!({
            "id": "evt-1",
            "title": "Team Standup",
            "start_time": "2026-03-21T10:00:00Z",
            "end_time": "2026-03-21T10:30:00Z",
            "description": "Daily standup",
            "location": "Room 101"
        })
        .to_string();

        insert_plugin_data(&conn, "r1", "cal-plugin", "events", &event_json);

        let files = export_service_data(&conn, "cal-plugin").expect("export should succeed");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, "cal-plugin-events.ics");

        let content = String::from_utf8(files[0].1.clone()).unwrap();
        assert!(content.contains("BEGIN:VCALENDAR"));
        assert!(content.contains("BEGIN:VEVENT"));
        assert!(content.contains("SUMMARY:Team Standup"));
        assert!(content.contains("DTSTART:20260321T100000Z"));
        assert!(content.contains("DTEND:20260321T103000Z"));
        assert!(content.contains("LOCATION:Room 101"));
        assert!(content.contains("END:VCALENDAR"));
    }

    #[test]
    fn export_service_data_contacts_as_vcf() {
        let conn = setup_db();
        let contact_json = serde_json::json!({
            "first_name": "Alice",
            "last_name": "Smith",
            "display_name": "Alice Smith",
            "emails": [{"address": "alice@example.com"}],
            "phone_numbers": [{"number": "+1234567890"}],
            "organization": "Acme Corp"
        })
        .to_string();

        insert_plugin_data(&conn, "r1", "contacts-plugin", "contacts", &contact_json);

        let files = export_service_data(&conn, "contacts-plugin").expect("export should succeed");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, "contacts-plugin-contacts.vcf");

        let content = String::from_utf8(files[0].1.clone()).unwrap();
        assert!(content.contains("BEGIN:VCARD"));
        assert!(content.contains("N:Smith;Alice;;;"));
        assert!(content.contains("FN:Alice Smith"));
        assert!(content.contains("EMAIL:alice@example.com"));
        assert!(content.contains("TEL:+1234567890"));
        assert!(content.contains("ORG:Acme Corp"));
        assert!(content.contains("END:VCARD"));
    }

    #[test]
    fn export_service_data_emails_as_mbox() {
        let conn = setup_db();
        let email_json = serde_json::json!({
            "from": "sender@example.com",
            "to": "recipient@example.com",
            "subject": "Test Email",
            "date": "2026-03-21T10:00:00Z",
            "message_id": "<msg-1@example.com>",
            "body": "Hello, world!\nFrom the test suite."
        })
        .to_string();

        insert_plugin_data(&conn, "r1", "email-plugin", "emails", &email_json);

        let files = export_service_data(&conn, "email-plugin").expect("export should succeed");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, "email-plugin-emails.mbox");

        let content = String::from_utf8(files[0].1.clone()).unwrap();
        assert!(content.contains("From sender@example.com"));
        assert!(content.contains("From: sender@example.com"));
        assert!(content.contains("To: recipient@example.com"));
        assert!(content.contains("Subject: Test Email"));
        assert!(content.contains("Message-ID: <msg-1@example.com>"));
        assert!(content.contains("Hello, world!"));
        // "From " at start of body line should be escaped.
        assert!(content.contains(">From the test suite."));
    }

    #[test]
    fn export_service_data_empty_plugin() {
        let conn = setup_db();
        let files = export_service_data(&conn, "nonexistent").expect("export should succeed");
        assert!(files.is_empty());
    }

    #[test]
    fn export_service_data_multiple_collections() {
        let conn = setup_db();
        insert_plugin_data(&conn, "r1", "multi", "events", r#"{"title":"E1"}"#);
        insert_plugin_data(&conn, "r2", "multi", "contacts", r#"{"first_name":"Bob"}"#);
        insert_plugin_data(&conn, "r3", "multi", "custom", r#"{"x":1}"#);

        let files = export_service_data(&conn, "multi").expect("export should succeed");
        assert_eq!(files.len(), 3);

        let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"multi-events.ics"));
        assert!(names.contains(&"multi-contacts.vcf"));
        assert!(names.contains(&"multi-custom.json"));
    }

    #[test]
    fn export_service_data_logs_audit_event() {
        let conn = setup_db();
        insert_plugin_data(&conn, "r1", "audited", "events", r#"{"title":"E"}"#);

        export_service_data(&conn, "audited").expect("export should succeed");

        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM audit_log WHERE event_type = 'data_export'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn to_ical_datetime_converts_rfc3339() {
        assert_eq!(to_ical_datetime("2026-03-21T10:00:00Z"), "20260321T100000Z");
        assert_eq!(to_ical_datetime("2026-12-31T23:59:59Z"), "20261231T235959Z");
    }

    #[test]
    fn ical_escape_special_chars() {
        assert_eq!(ical_escape("a;b,c\\d\ne"), "a\\;b\\,c\\\\d\\ne");
    }

    #[test]
    fn vcf_escape_special_chars() {
        assert_eq!(vcf_escape("a;b,c\\d\ne"), "a\\;b\\,c\\\\d\\ne");
    }
}
