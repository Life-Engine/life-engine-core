//! Shared WebDAV multi-status XML response builders.
//!
//! Provides reusable building blocks for constructing DAV multi-status
//! XML responses, shared across CalDAV and CardDAV server plugins.

/// A single resource entry in a multi-status response.
#[derive(Debug, Clone)]
pub struct DavResourceEntry {
    /// The href/path of the resource.
    pub href: String,
    /// Properties to include in the response.
    pub properties: Vec<DavProperty>,
}

/// A DAV property in a propstat response.
#[derive(Debug, Clone)]
pub enum DavProperty {
    /// `<D:getetag>value</D:getetag>`
    ETag(String),
    /// `<D:getcontenttype>value</D:getcontenttype>`
    ContentType(String),
    /// Custom property with full XML element.
    Custom(String),
}

/// Write a `<D:response>` block for a resource entry.
pub fn write_response_entry(xml: &mut String, entry: &DavResourceEntry) {
    xml.push_str("  <D:response>\r\n");
    xml.push_str(&format!("    <D:href>{}</D:href>\r\n", entry.href));
    xml.push_str("    <D:propstat>\r\n");
    xml.push_str("      <D:prop>\r\n");

    for prop in &entry.properties {
        match prop {
            DavProperty::ETag(etag) => {
                xml.push_str(&format!("        <D:getetag>{etag}</D:getetag>\r\n"));
            }
            DavProperty::ContentType(ct) => {
                xml.push_str(&format!(
                    "        <D:getcontenttype>{ct}</D:getcontenttype>\r\n"
                ));
            }
            DavProperty::Custom(custom) => {
                xml.push_str(&format!("        {custom}\r\n"));
            }
        }
    }

    xml.push_str("      </D:prop>\r\n");
    xml.push_str("      <D:status>HTTP/1.1 200 OK</D:status>\r\n");
    xml.push_str("    </D:propstat>\r\n");
    xml.push_str("  </D:response>\r\n");
}

/// Start a multi-status XML document with the given namespace declarations.
pub fn open_multistatus(xml: &mut String, extra_namespaces: &str) {
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
    xml.push_str(&format!(
        "<D:multistatus xmlns:D=\"DAV:\"{extra_namespaces}>\r\n"
    ));
}

/// Close a multi-status XML document.
pub fn close_multistatus(xml: &mut String) {
    xml.push_str("</D:multistatus>");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_response_entry_produces_valid_xml() {
        let entry = DavResourceEntry {
            href: "/cal/test.ics".into(),
            properties: vec![
                DavProperty::ETag("\"etag-123\"".into()),
                DavProperty::ContentType("text/calendar".into()),
            ],
        };
        let mut xml = String::new();
        write_response_entry(&mut xml, &entry);

        assert!(xml.contains("<D:response>"));
        assert!(xml.contains("<D:href>/cal/test.ics</D:href>"));
        assert!(xml.contains("<D:getetag>\"etag-123\"</D:getetag>"));
        assert!(xml.contains("<D:getcontenttype>text/calendar</D:getcontenttype>"));
        assert!(xml.contains("HTTP/1.1 200 OK"));
        assert!(xml.contains("</D:response>"));
    }

    #[test]
    fn write_response_entry_with_custom_property() {
        let entry = DavResourceEntry {
            href: "/ab/ct.vcf".into(),
            properties: vec![
                DavProperty::ETag("\"e1\"".into()),
                DavProperty::Custom(
                    "<CR:address-data>BEGIN:VCARD\r\nEND:VCARD</CR:address-data>".into(),
                ),
            ],
        };
        let mut xml = String::new();
        write_response_entry(&mut xml, &entry);

        assert!(xml.contains("<CR:address-data>"));
        assert!(xml.contains("BEGIN:VCARD"));
    }

    #[test]
    fn open_close_multistatus() {
        let mut xml = String::new();
        open_multistatus(&mut xml, " xmlns:C=\"urn:ietf:params:xml:ns:caldav\"");
        close_multistatus(&mut xml);

        assert!(xml.starts_with("<?xml version=\"1.0\""));
        assert!(xml.contains("<D:multistatus"));
        assert!(xml.contains("xmlns:D=\"DAV:\""));
        assert!(xml.contains("xmlns:C=\"urn:ietf:params:xml:ns:caldav\""));
        assert!(xml.contains("</D:multistatus>"));
    }

    #[test]
    fn full_multistatus_document() {
        let mut xml = String::new();
        open_multistatus(&mut xml, "");

        write_response_entry(
            &mut xml,
            &DavResourceEntry {
                href: "/resource/1".into(),
                properties: vec![DavProperty::ETag("\"e1\"".into())],
            },
        );
        write_response_entry(
            &mut xml,
            &DavResourceEntry {
                href: "/resource/2".into(),
                properties: vec![DavProperty::ETag("\"e2\"".into())],
            },
        );

        close_multistatus(&mut xml);

        assert!(xml.contains("/resource/1"));
        assert!(xml.contains("/resource/2"));
        assert!(xml.contains("</D:multistatus>"));
    }
}
