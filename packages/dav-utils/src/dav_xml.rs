//! Shared WebDAV multi-status XML response builders.
//!
//! Provides reusable building blocks for constructing DAV multi-status
//! XML responses, shared across CalDAV and CardDAV server plugins.

/// Escape XML special characters in a string value.
pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

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
    xml.push_str(&format!("    <D:href>{}</D:href>\r\n", xml_escape(&entry.href)));
    xml.push_str("    <D:propstat>\r\n");
    xml.push_str("      <D:prop>\r\n");

    for prop in &entry.properties {
        match prop {
            DavProperty::ETag(etag) => {
                xml.push_str(&format!("        <D:getetag>{}</D:getetag>\r\n", xml_escape(etag)));
            }
            DavProperty::ContentType(ct) => {
                xml.push_str(&format!(
                    "        <D:getcontenttype>{}</D:getcontenttype>\r\n",
                    xml_escape(ct)
                ));
            }
            DavProperty::Custom(custom) => {
                // Custom properties are assumed to be pre-escaped XML fragments.
                xml.push_str(&format!("        {custom}\r\n"));
            }
        }
    }

    xml.push_str("      </D:prop>\r\n");
    xml.push_str("      <D:status>HTTP/1.1 200 OK</D:status>\r\n");
    xml.push_str("    </D:propstat>\r\n");
    xml.push_str("  </D:response>\r\n");
}

/// Validate that an XML namespace declaration string contains only valid
/// `xmlns:PREFIX="URI"` entries.
///
/// Returns `true` if the string is empty or all whitespace-separated tokens
/// match the expected pattern. Returns `false` if any token is malformed.
fn validate_namespace_declarations(ns: &str) -> bool {
    let trimmed = ns.trim();
    if trimmed.is_empty() {
        return true;
    }

    // Split on whitespace; each token should be xmlns:PREFIX="URI"
    for token in trimmed.split_whitespace() {
        if !token.starts_with("xmlns:") {
            return false;
        }
        let after_xmlns = &token["xmlns:".len()..];
        let Some((prefix, uri)) = after_xmlns.split_once('=') else {
            return false;
        };

        // Prefix must be non-empty and contain only valid XML NCName characters
        // (simplified: alphanumeric, hyphen, underscore, period)
        if prefix.is_empty()
            || !prefix
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return false;
        }

        // URI must be quoted
        if !(uri.starts_with('"') && uri.ends_with('"') && uri.len() >= 2) {
            return false;
        }
    }

    true
}

/// Start a multi-status XML document with the given namespace declarations.
///
/// `extra_namespaces` must contain only valid `xmlns:PREFIX="URI"` entries
/// (space-separated). Invalid declarations are silently dropped with a
/// debug-level log to prevent malformed XML output.
pub fn open_multistatus(xml: &mut String, extra_namespaces: &str) {
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");

    let ns = if validate_namespace_declarations(extra_namespaces) {
        extra_namespaces.to_string()
    } else {
        // Drop invalid namespace declarations to prevent malformed XML.
        String::new()
    };

    xml.push_str(&format!(
        "<D:multistatus xmlns:D=\"DAV:\"{ns}>\r\n"
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
        assert!(xml.contains("<D:getetag>&quot;etag-123&quot;</D:getetag>"));
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

    // --- validate_namespace_declarations ---

    #[test]
    fn validate_ns_empty_string() {
        assert!(validate_namespace_declarations(""));
        assert!(validate_namespace_declarations("   "));
    }

    #[test]
    fn validate_ns_valid_single() {
        assert!(validate_namespace_declarations("xmlns:C=\"urn:ietf:params:xml:ns:caldav\""));
    }

    #[test]
    fn validate_ns_valid_multiple() {
        assert!(validate_namespace_declarations(
            "xmlns:C=\"urn:ietf:params:xml:ns:caldav\" xmlns:CR=\"urn:ietf:params:xml:ns:carddav\""
        ));
    }

    #[test]
    fn validate_ns_rejects_non_xmlns() {
        assert!(!validate_namespace_declarations("foo=\"bar\""));
    }

    #[test]
    fn validate_ns_rejects_missing_uri() {
        assert!(!validate_namespace_declarations("xmlns:C"));
    }

    #[test]
    fn validate_ns_rejects_unquoted_uri() {
        assert!(!validate_namespace_declarations("xmlns:C=urn:foo"));
    }

    #[test]
    fn validate_ns_rejects_empty_prefix() {
        assert!(!validate_namespace_declarations("xmlns:=\"urn:foo\""));
    }

    #[test]
    fn open_multistatus_drops_invalid_ns() {
        let mut xml = String::new();
        open_multistatus(&mut xml, "INVALID_NAMESPACE");
        // Invalid namespace should be dropped; only DAV: namespace remains
        assert!(xml.contains("xmlns:D=\"DAV:\""));
        assert!(!xml.contains("INVALID_NAMESPACE"));
    }
}
