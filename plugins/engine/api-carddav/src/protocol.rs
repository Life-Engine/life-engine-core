//! CardDAV protocol handlers for PROPFIND, REPORT, GET, PUT, DELETE.
//!
//! Implements the subset of RFC 6352 (CardDAV) needed for native contacts
//! clients to discover, read, create, update, and delete contacts.

use chrono::{DateTime, Utc};
use life_engine_types::Contact;

/// Response for a PROPFIND request on the address book collection.
#[derive(Debug, Clone)]
pub struct PropfindResponse {
    /// The address book display name.
    pub display_name: String,
    /// The address book CTag for sync.
    pub ctag: String,
    /// List of contact resource entries (href + etag).
    pub resources: Vec<ResourceEntry>,
}

/// A single resource entry returned in multi-status responses.
#[derive(Debug, Clone)]
pub struct ResourceEntry {
    /// The href/path of the resource.
    pub href: String,
    /// The ETag of the resource.
    pub etag: String,
    /// The content type (e.g. "text/vcard").
    pub content_type: String,
}

/// Build a PROPFIND multi-status XML response for the address book.
pub fn build_propfind_xml(response: &PropfindResponse) -> String {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
    xml.push_str("<D:multistatus xmlns:D=\"DAV:\" xmlns:CR=\"urn:ietf:params:xml:ns:carddav\" xmlns:CS=\"http://calendarserver.org/ns/\">\r\n");

    // Address book collection entry
    xml.push_str("  <D:response>\r\n");
    xml.push_str("    <D:href>/api/plugins/com.life-engine.api-carddav/addressbooks/default/</D:href>\r\n");
    xml.push_str("    <D:propstat>\r\n");
    xml.push_str("      <D:prop>\r\n");
    xml.push_str(&format!(
        "        <D:displayname>{}</D:displayname>\r\n",
        response.display_name
    ));
    xml.push_str("        <D:resourcetype><D:collection/><CR:addressbook/></D:resourcetype>\r\n");
    xml.push_str(&format!(
        "        <CS:getctag>{}</CS:getctag>\r\n",
        response.ctag
    ));
    xml.push_str("      </D:prop>\r\n");
    xml.push_str("      <D:status>HTTP/1.1 200 OK</D:status>\r\n");
    xml.push_str("    </D:propstat>\r\n");
    xml.push_str("  </D:response>\r\n");

    // Individual resource entries
    for entry in &response.resources {
        xml.push_str("  <D:response>\r\n");
        xml.push_str(&format!("    <D:href>{}</D:href>\r\n", entry.href));
        xml.push_str("    <D:propstat>\r\n");
        xml.push_str("      <D:prop>\r\n");
        xml.push_str(&format!(
            "        <D:getetag>{}</D:getetag>\r\n",
            entry.etag
        ));
        xml.push_str(&format!(
            "        <D:getcontenttype>{}</D:getcontenttype>\r\n",
            entry.content_type
        ));
        xml.push_str("      </D:prop>\r\n");
        xml.push_str("      <D:status>HTTP/1.1 200 OK</D:status>\r\n");
        xml.push_str("    </D:propstat>\r\n");
        xml.push_str("  </D:response>\r\n");
    }

    xml.push_str("</D:multistatus>");
    xml
}

/// Build an addressbook-multiget REPORT XML response.
pub fn build_report_xml(contacts: &[(String, String, String)]) -> String {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
    xml.push_str(
        "<D:multistatus xmlns:D=\"DAV:\" xmlns:CR=\"urn:ietf:params:xml:ns:carddav\">\r\n",
    );

    for (href, etag, vcard_data) in contacts {
        xml.push_str("  <D:response>\r\n");
        xml.push_str(&format!("    <D:href>{href}</D:href>\r\n"));
        xml.push_str("    <D:propstat>\r\n");
        xml.push_str("      <D:prop>\r\n");
        xml.push_str(&format!("        <D:getetag>{etag}</D:getetag>\r\n"));
        xml.push_str(&format!(
            "        <CR:address-data>{vcard_data}</CR:address-data>\r\n"
        ));
        xml.push_str("      </D:prop>\r\n");
        xml.push_str("      <D:status>HTTP/1.1 200 OK</D:status>\r\n");
        xml.push_str("    </D:propstat>\r\n");
        xml.push_str("  </D:response>\r\n");
    }

    xml.push_str("</D:multistatus>");
    xml
}

/// Generate an ETag for a contact based on its updated_at timestamp.
pub fn generate_etag(contact: &Contact) -> String {
    format!(
        "\"{}\"",
        contact.updated_at.format("%Y%m%dT%H%M%SZ")
    )
}

/// Generate a CTag for the address book collection.
pub fn generate_ctag(last_modified: DateTime<Utc>) -> String {
    format!(
        "life-engine-{}",
        last_modified.format("%Y%m%dT%H%M%SZ")
    )
}

/// Build the href path for a contact resource.
pub fn contact_href(source_id: &str) -> String {
    format!(
        "/api/plugins/com.life-engine.api-carddav/addressbooks/default/{}.vcf",
        source_id
    )
}

/// Parse a UID from a resource href path.
pub fn uid_from_href(href: &str) -> Option<&str> {
    let filename = href.rsplit('/').next()?;
    filename.strip_suffix(".vcf")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use life_engine_types::ContactName;

    fn sample_contact() -> Contact {
        Contact {
            id: uuid::Uuid::new_v4(),
            name: ContactName {
                given: "Test".into(),
                family: "Contact".into(),
                display: "Test Contact".into(),
            },
            emails: vec![],
            phones: vec![],
            addresses: vec![],
            organisation: None,
            source: "local".into(),
            source_id: "ct-001".into(),
            extensions: None,
            created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            updated_at: Utc.with_ymd_and_hms(2026, 3, 15, 12, 0, 0).unwrap(),
        }
    }

    // --- PROPFIND tests ---

    #[test]
    fn propfind_xml_contains_addressbook_metadata() {
        let response = PropfindResponse {
            display_name: "My Contacts".into(),
            ctag: "ctag-abc".into(),
            resources: vec![],
        };
        let xml = build_propfind_xml(&response);
        assert!(xml.contains("<D:multistatus"));
        assert!(xml.contains("<D:displayname>My Contacts</D:displayname>"));
        assert!(xml.contains("<CR:addressbook/>"));
        assert!(xml.contains("<CS:getctag>ctag-abc</CS:getctag>"));
    }

    #[test]
    fn propfind_xml_includes_resource_entries() {
        let response = PropfindResponse {
            display_name: "Contacts".into(),
            ctag: "ct".into(),
            resources: vec![
                ResourceEntry {
                    href: "/addressbooks/default/ct-001.vcf".into(),
                    etag: "\"etag-1\"".into(),
                    content_type: "text/vcard".into(),
                },
            ],
        };
        let xml = build_propfind_xml(&response);
        assert!(xml.contains("ct-001.vcf"));
        assert!(xml.contains("<D:getetag>\"etag-1\"</D:getetag>"));
        assert!(xml.contains("text/vcard"));
    }

    // --- REPORT tests ---

    #[test]
    fn report_xml_contains_address_data() {
        let contacts = vec![(
            "/ab/ct-001.vcf".into(),
            "\"etag-1\"".into(),
            "BEGIN:VCARD\r\nFN:Test\r\nEND:VCARD".into(),
        )];
        let xml = build_report_xml(&contacts);
        assert!(xml.contains("<CR:address-data>"));
        assert!(xml.contains("BEGIN:VCARD"));
    }

    #[test]
    fn report_xml_empty_collection() {
        let contacts: Vec<(String, String, String)> = vec![];
        let xml = build_report_xml(&contacts);
        assert!(xml.contains("<D:multistatus"));
        assert!(!xml.contains("<D:response>"));
    }

    // --- ETag/CTag tests ---

    #[test]
    fn generate_etag_from_contact() {
        let contact = sample_contact();
        let etag = generate_etag(&contact);
        assert!(etag.starts_with('"'));
        assert!(etag.ends_with('"'));
        assert!(etag.contains("20260315T120000Z"));
    }

    #[test]
    fn generate_ctag_from_timestamp() {
        let ts = Utc.with_ymd_and_hms(2026, 3, 20, 14, 30, 0).unwrap();
        let ctag = generate_ctag(ts);
        assert_eq!(ctag, "life-engine-20260320T143000Z");
    }

    // --- Href/UID helpers ---

    #[test]
    fn contact_href_construction() {
        let href = contact_href("ct-123");
        assert_eq!(
            href,
            "/api/plugins/com.life-engine.api-carddav/addressbooks/default/ct-123.vcf"
        );
    }

    #[test]
    fn uid_from_href_extracts_uid() {
        let href = "/api/plugins/com.life-engine.api-carddav/addressbooks/default/ct-123.vcf";
        assert_eq!(uid_from_href(href), Some("ct-123"));
    }

    #[test]
    fn uid_from_href_returns_none_for_non_vcf() {
        assert_eq!(uid_from_href("/some/path/without/extension"), None);
    }

    // --- GET operation tests ---

    #[test]
    fn get_contact_returns_vcard_format() {
        let contact = sample_contact();
        let vcard = crate::serializer::contact_to_vcard(&contact);
        assert!(vcard.contains("BEGIN:VCARD"));
        assert!(vcard.contains("FN:Test Contact"));
        assert!(vcard.contains("END:VCARD"));
    }

    // --- PUT operation tests ---

    #[test]
    fn put_contact_parses_vcard_to_cdm() {
        let vcard = "\
BEGIN:VCARD\r\n\
VERSION:4.0\r\n\
UID:new-ct-001\r\n\
FN:New Contact\r\n\
N:Contact;New;;;\r\n\
EMAIL:new@example.com\r\n\
END:VCARD\r\n";

        let contact = crate::serializer::vcard_to_contact(vcard).expect("should parse");
        assert_eq!(contact.name.display, "New Contact");
        assert_eq!(contact.source_id, "new-ct-001");
    }

    // --- DELETE operation tests ---

    #[test]
    fn delete_contact_identifies_by_uid() {
        let uid = uid_from_href(
            "/api/plugins/com.life-engine.api-carddav/addressbooks/default/ct-to-delete.vcf",
        );
        assert_eq!(uid, Some("ct-to-delete"));
    }

    // --- Thunderbird compatibility tests ---

    #[test]
    fn thunderbird_propfind_response_has_required_elements() {
        // Thunderbird requires: displayname, resourcetype with addressbook,
        // and getctag for change detection
        let response = PropfindResponse {
            display_name: "Life Engine Contacts".into(),
            ctag: "ctag-tb-test".into(),
            resources: vec![ResourceEntry {
                href: "/api/plugins/com.life-engine.api-carddav/addressbooks/default/ct-001.vcf"
                    .into(),
                etag: "\"etag-1\"".into(),
                content_type: "text/vcard".into(),
            }],
        };
        let xml = build_propfind_xml(&response);

        // Required DAV namespace declarations
        assert!(xml.contains("xmlns:D=\"DAV:\""));
        assert!(xml.contains("xmlns:CR=\"urn:ietf:params:xml:ns:carddav\""));
        assert!(xml.contains("xmlns:CS=\"http://calendarserver.org/ns/\""));

        // Required properties for Thunderbird
        assert!(xml.contains("<D:displayname>"));
        assert!(xml.contains("<D:resourcetype>"));
        assert!(xml.contains("<D:collection/>"));
        assert!(xml.contains("<CR:addressbook/>"));
        assert!(xml.contains("<CS:getctag>"));

        // Individual resources must have getetag and getcontenttype
        assert!(xml.contains("<D:getetag>"));
        assert!(xml.contains("<D:getcontenttype>text/vcard</D:getcontenttype>"));

        // Status lines
        assert!(xml.contains("HTTP/1.1 200 OK"));
    }

    #[test]
    fn thunderbird_vcard_output_is_rfc6350_compliant() {
        // Thunderbird expects RFC 6350 (vCard 4.0) format
        let contact = sample_contact();
        let vcard = crate::serializer::contact_to_vcard(&contact);

        // Required vCard properties
        assert!(vcard.contains("BEGIN:VCARD"));
        assert!(vcard.contains("VERSION:4.0"));
        assert!(vcard.contains("UID:"));
        assert!(vcard.contains("FN:"));
        assert!(vcard.contains("N:"));
        assert!(vcard.contains("REV:"));
        assert!(vcard.contains("END:VCARD"));

        // Must use CRLF line endings
        assert!(vcard.contains("\r\n"));
    }

    #[test]
    fn thunderbird_report_contains_address_data() {
        // Thunderbird uses addressbook-multiget REPORT
        let contact = sample_contact();
        let vcard = crate::serializer::contact_to_vcard(&contact);
        let contacts = vec![(
            contact_href("ct-001"),
            generate_etag(&contact),
            vcard,
        )];
        let xml = build_report_xml(&contacts);

        assert!(xml.contains("xmlns:CR=\"urn:ietf:params:xml:ns:carddav\""));
        assert!(xml.contains("<CR:address-data>"));
        assert!(xml.contains("BEGIN:VCARD"));
    }

    // --- iOS Contacts compatibility tests ---

    #[test]
    fn ios_contacts_propfind_response_compatible() {
        let response = PropfindResponse {
            display_name: "Contacts".into(),
            ctag: "ctag-ios".into(),
            resources: vec![],
        };
        let xml = build_propfind_xml(&response);

        assert!(xml.contains("<D:collection/>"));
        assert!(xml.contains("<CR:addressbook/>"));
        assert!(xml.contains("<D:displayname>Contacts</D:displayname>"));
    }
}
