//! CalDAV service discovery endpoints.
//!
//! Implements `.well-known/caldav` (RFC 6764) to allow calendar clients
//! to auto-discover the CalDAV server location.

/// The CalDAV principal URL that `.well-known/caldav` redirects to.
pub const CALDAV_PRINCIPAL_URL: &str = "/api/plugins/com.life-engine.api-caldav/calendars/";

/// The default calendar collection URL.
pub const CALDAV_CALENDAR_URL: &str = "/api/plugins/com.life-engine.api-caldav/calendars/default/";

/// Build the XML response for a PROPFIND on the principal URL.
///
/// Returns the calendar-home-set pointing to the calendar collection.
pub fn build_principal_propfind_xml() -> String {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
    xml.push_str("<D:multistatus xmlns:D=\"DAV:\" xmlns:C=\"urn:ietf:params:xml:ns:caldav\">\r\n");
    xml.push_str("  <D:response>\r\n");
    xml.push_str(&format!("    <D:href>{CALDAV_PRINCIPAL_URL}</D:href>\r\n"));
    xml.push_str("    <D:propstat>\r\n");
    xml.push_str("      <D:prop>\r\n");
    xml.push_str("        <D:resourcetype><D:collection/><D:principal/></D:resourcetype>\r\n");
    xml.push_str(&format!(
        "        <C:calendar-home-set><D:href>{CALDAV_CALENDAR_URL}</D:href></C:calendar-home-set>\r\n"
    ));
    xml.push_str("      </D:prop>\r\n");
    xml.push_str("      <D:status>HTTP/1.1 200 OK</D:status>\r\n");
    xml.push_str("    </D:propstat>\r\n");
    xml.push_str("  </D:response>\r\n");
    xml.push_str("</D:multistatus>");
    xml
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn well_known_caldav_redirects_to_principal() {
        // The .well-known/caldav endpoint should redirect (301) to the principal URL
        assert_eq!(
            CALDAV_PRINCIPAL_URL,
            "/api/plugins/com.life-engine.api-caldav/calendars/"
        );
    }

    #[test]
    fn calendar_url_points_to_default_collection() {
        assert_eq!(
            CALDAV_CALENDAR_URL,
            "/api/plugins/com.life-engine.api-caldav/calendars/default/"
        );
    }

    #[test]
    fn principal_propfind_xml_contains_calendar_home_set() {
        let xml = build_principal_propfind_xml();
        assert!(xml.contains("<C:calendar-home-set>"));
        assert!(xml.contains(CALDAV_CALENDAR_URL));
    }

    #[test]
    fn principal_propfind_xml_identifies_as_principal() {
        let xml = build_principal_propfind_xml();
        assert!(xml.contains("<D:principal/>"));
        assert!(xml.contains("<D:collection/>"));
    }

    #[test]
    fn principal_propfind_xml_is_well_formed() {
        let xml = build_principal_propfind_xml();
        assert!(xml.starts_with("<?xml version=\"1.0\""));
        assert!(xml.contains("</D:multistatus>"));
    }
}
