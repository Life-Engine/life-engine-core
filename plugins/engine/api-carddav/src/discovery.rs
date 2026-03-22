//! CardDAV service discovery endpoints.
//!
//! Implements `.well-known/carddav` (RFC 6764) to allow contacts clients
//! to auto-discover the CardDAV server location.

/// The CardDAV principal URL that `.well-known/carddav` redirects to.
pub const CARDDAV_PRINCIPAL_URL: &str = "/api/plugins/com.life-engine.api-carddav/addressbooks/";

/// The default address book collection URL.
pub const CARDDAV_ADDRESSBOOK_URL: &str =
    "/api/plugins/com.life-engine.api-carddav/addressbooks/default/";

/// Build the XML response for a PROPFIND on the principal URL.
///
/// Returns the addressbook-home-set pointing to the address book collection.
pub fn build_principal_propfind_xml() -> String {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
    xml.push_str(
        "<D:multistatus xmlns:D=\"DAV:\" xmlns:CR=\"urn:ietf:params:xml:ns:carddav\">\r\n",
    );
    xml.push_str("  <D:response>\r\n");
    xml.push_str(&format!(
        "    <D:href>{CARDDAV_PRINCIPAL_URL}</D:href>\r\n"
    ));
    xml.push_str("    <D:propstat>\r\n");
    xml.push_str("      <D:prop>\r\n");
    xml.push_str(
        "        <D:resourcetype><D:collection/><D:principal/></D:resourcetype>\r\n",
    );
    xml.push_str(&format!(
        "        <CR:addressbook-home-set><D:href>{CARDDAV_ADDRESSBOOK_URL}</D:href></CR:addressbook-home-set>\r\n"
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
    fn well_known_carddav_redirects_to_principal() {
        assert_eq!(
            CARDDAV_PRINCIPAL_URL,
            "/api/plugins/com.life-engine.api-carddav/addressbooks/"
        );
    }

    #[test]
    fn addressbook_url_points_to_default_collection() {
        assert_eq!(
            CARDDAV_ADDRESSBOOK_URL,
            "/api/plugins/com.life-engine.api-carddav/addressbooks/default/"
        );
    }

    #[test]
    fn principal_propfind_xml_contains_addressbook_home_set() {
        let xml = build_principal_propfind_xml();
        assert!(xml.contains("<CR:addressbook-home-set>"));
        assert!(xml.contains(CARDDAV_ADDRESSBOOK_URL));
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
