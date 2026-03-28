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
    xml.push_str(&format!(
        "        <D:current-user-principal><D:href>{CALDAV_PRINCIPAL_URL}</D:href></D:current-user-principal>\r\n"
    ));
    xml.push_str("        <D:supported-report-set>\r\n");
    xml.push_str("          <D:supported-report><D:report><C:calendar-multiget/></D:report></D:supported-report>\r\n");
    xml.push_str("          <D:supported-report><D:report><C:calendar-query/></D:report></D:supported-report>\r\n");
    xml.push_str("        </D:supported-report-set>\r\n");
    xml.push_str("      </D:prop>\r\n");
    xml.push_str("      <D:status>HTTP/1.1 200 OK</D:status>\r\n");
    xml.push_str("    </D:propstat>\r\n");
    xml.push_str("  </D:response>\r\n");
    xml.push_str("</D:multistatus>");
    xml
}

/// Build a 301 redirect response for the `.well-known/caldav` endpoint.
///
/// Per RFC 6764, the well-known URI redirects to the CalDAV principal URL.
/// Both GET and PROPFIND methods should be handled.
pub fn build_well_known_redirect() -> (u16, Vec<(&'static str, String)>) {
    (301, vec![("Location", CALDAV_PRINCIPAL_URL.to_string())])
}

/// Build the response headers for an OPTIONS request on CalDAV endpoints.
///
/// Per RFC 4791 Section 5.1, the server advertises CalDAV support via the
/// `DAV` response header.
pub fn build_options_headers() -> Vec<(&'static str, &'static str)> {
    vec![
        ("DAV", "1, calendar-access"),
        ("Allow", "OPTIONS, GET, PUT, DELETE, PROPFIND, REPORT"),
    ]
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
    fn principal_propfind_xml_contains_current_user_principal() {
        let xml = build_principal_propfind_xml();
        assert!(xml.contains("<D:current-user-principal>"));
        assert!(xml.contains(&format!(
            "<D:current-user-principal><D:href>{CALDAV_PRINCIPAL_URL}</D:href></D:current-user-principal>"
        )));
    }

    #[test]
    fn principal_propfind_xml_contains_supported_report_set() {
        let xml = build_principal_propfind_xml();
        assert!(xml.contains("<D:supported-report-set>"));
        assert!(xml.contains("<C:calendar-multiget/>"));
        assert!(xml.contains("<C:calendar-query/>"));
    }

    #[test]
    fn principal_propfind_xml_is_well_formed() {
        let xml = build_principal_propfind_xml();
        assert!(xml.starts_with("<?xml version=\"1.0\""));
        assert!(xml.contains("</D:multistatus>"));
    }

    #[test]
    fn well_known_redirect_returns_301_with_location() {
        let (status, headers) = build_well_known_redirect();
        assert_eq!(status, 301);
        let location = headers.iter().find(|(k, _)| *k == "Location");
        assert!(location.is_some());
        assert_eq!(location.unwrap().1, CALDAV_PRINCIPAL_URL);
    }

    #[test]
    fn options_headers_advertise_caldav_support() {
        let headers = build_options_headers();
        let dav = headers.iter().find(|(k, _)| *k == "DAV");
        assert!(dav.is_some());
        assert!(dav.unwrap().1.contains("calendar-access"));
    }

    #[test]
    fn options_headers_include_allow() {
        let headers = build_options_headers();
        let allow = headers.iter().find(|(k, _)| *k == "Allow");
        assert!(allow.is_some());
        assert!(allow.unwrap().1.contains("PROPFIND"));
        assert!(allow.unwrap().1.contains("REPORT"));
        assert!(allow.unwrap().1.contains("OPTIONS"));
    }
}
