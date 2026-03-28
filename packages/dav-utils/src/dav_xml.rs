//! Shared WebDAV XML response builders and request parsers.
//!
//! Provides reusable building blocks for constructing DAV multi-status
//! XML responses and parsing incoming PROPFIND/REPORT request bodies,
//! shared across CalDAV and CardDAV server plugins.

use quick_xml::events::Event;
use quick_xml::Reader;

/// A parsed PROPFIND request body.
#[derive(Debug, Clone, PartialEq)]
pub enum PropfindRequest {
    /// `<D:allprop/>` — client requests all properties.
    AllProp,
    /// `<D:propname/>` — client requests only property names.
    PropName,
    /// `<D:prop>` with specific property names — client requests listed properties.
    /// Each entry is `(namespace_uri, local_name)`.
    Prop(Vec<(String, String)>),
}

/// A parsed CalDAV/CardDAV REPORT request body.
#[derive(Debug, Clone, PartialEq)]
pub enum ReportRequest {
    /// `<C:calendar-query>` — CalDAV query with optional filter and requested properties.
    CalendarQuery {
        properties: Vec<(String, String)>,
    },
    /// `<C:calendar-multiget>` — CalDAV multiget with specific hrefs.
    CalendarMultiget {
        properties: Vec<(String, String)>,
        hrefs: Vec<String>,
    },
    /// `<CR:addressbook-query>` — CardDAV query with optional filter and requested properties.
    AddressbookQuery {
        properties: Vec<(String, String)>,
    },
    /// `<CR:addressbook-multiget>` — CardDAV multiget with specific hrefs.
    AddressbookMultiget {
        properties: Vec<(String, String)>,
        hrefs: Vec<String>,
    },
}

/// Parse a PROPFIND request body.
///
/// Returns `AllProp` for empty bodies (per RFC 4918 Section 9.1,
/// an absent body is treated as allprop).
pub fn parse_propfind(body: &str) -> Result<PropfindRequest, String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Ok(PropfindRequest::AllProp);
    }

    let mut reader = Reader::from_str(trimmed);
    let mut found_allprop = false;
    let mut found_propname = false;
    let mut in_prop = false;
    let mut properties: Vec<(String, String)> = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let local = local_name(e.name().as_ref());
                match local.as_str() {
                    "allprop" => found_allprop = true,
                    "propname" => found_propname = true,
                    "prop" if !in_prop => in_prop = true,
                    name if in_prop => {
                        let ns = resolve_namespace(e.name().as_ref());
                        properties.push((ns, name.to_string()));
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = local_name(e.name().as_ref());
                match local.as_str() {
                    "allprop" => found_allprop = true,
                    "propname" => found_propname = true,
                    name if in_prop => {
                        let ns = resolve_namespace(e.name().as_ref());
                        properties.push((ns, name.to_string()));
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let local = local_name(e.name().as_ref());
                if local == "prop" {
                    in_prop = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {e}")),
            _ => {}
        }
    }

    if found_allprop {
        Ok(PropfindRequest::AllProp)
    } else if found_propname {
        Ok(PropfindRequest::PropName)
    } else if !properties.is_empty() {
        Ok(PropfindRequest::Prop(properties))
    } else {
        Ok(PropfindRequest::AllProp)
    }
}

/// Parse a REPORT request body (CalDAV or CardDAV).
pub fn parse_report(body: &str) -> Result<ReportRequest, String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Err("REPORT request body is empty".to_string());
    }

    let mut reader = Reader::from_str(trimmed);
    let mut report_type: Option<String> = None;
    let mut in_prop = false;
    let mut in_href = false;
    let mut properties: Vec<(String, String)> = Vec::new();
    let mut hrefs: Vec<String> = Vec::new();
    let mut href_buf = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let local = local_name(e.name().as_ref());
                match local.as_str() {
                    "calendar-query" | "calendar-multiget"
                    | "addressbook-query" | "addressbook-multiget"
                        if report_type.is_none() =>
                    {
                        report_type = Some(local);
                    }
                    "prop" if !in_prop => in_prop = true,
                    "href" => {
                        in_href = true;
                        href_buf.clear();
                    }
                    name if in_prop => {
                        let ns = resolve_namespace(e.name().as_ref());
                        properties.push((ns, name.to_string()));
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = local_name(e.name().as_ref());
                match local.as_str() {
                    "calendar-query" | "calendar-multiget"
                    | "addressbook-query" | "addressbook-multiget"
                        if report_type.is_none() =>
                    {
                        report_type = Some(local);
                    }
                    name if in_prop => {
                        let ns = resolve_namespace(e.name().as_ref());
                        properties.push((ns, name.to_string()));
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_href {
                    if let Ok(text) = e.unescape() {
                        href_buf.push_str(&text);
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let local = local_name(e.name().as_ref());
                match local.as_str() {
                    "prop" => in_prop = false,
                    "href" => {
                        in_href = false;
                        let h = href_buf.trim().to_string();
                        if !h.is_empty() {
                            hrefs.push(h);
                        }
                        href_buf.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {e}")),
            _ => {}
        }
    }

    match report_type.as_deref() {
        Some("calendar-query") => Ok(ReportRequest::CalendarQuery { properties }),
        Some("calendar-multiget") => Ok(ReportRequest::CalendarMultiget { properties, hrefs }),
        Some("addressbook-query") => Ok(ReportRequest::AddressbookQuery { properties }),
        Some("addressbook-multiget") => Ok(ReportRequest::AddressbookMultiget { properties, hrefs }),
        Some(other) => Err(format!("Unknown REPORT type: {other}")),
        None => Err("No recognized REPORT element found".to_string()),
    }
}

/// Extract the local name from a possibly-prefixed XML element name.
fn local_name(name: &[u8]) -> String {
    let s = std::str::from_utf8(name).unwrap_or("");
    match s.rsplit_once(':') {
        Some((_, local)) => local.to_string(),
        None => s.to_string(),
    }
}

/// Resolve namespace URI from a prefixed XML element name.
///
/// Maps common DAV prefixes to their standard URIs.
fn resolve_namespace(name: &[u8]) -> String {
    let s = std::str::from_utf8(name).unwrap_or("");
    match s.split_once(':') {
        Some(("D", _)) => "DAV:".to_string(),
        Some(("C", _)) => "urn:ietf:params:xml:ns:caldav".to_string(),
        Some(("CR", _)) => "urn:ietf:params:xml:ns:carddav".to_string(),
        Some(("CS", _)) => "http://calendarserver.org/ns/".to_string(),
        _ => "DAV:".to_string(),
    }
}

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
    /// Custom property with a namespace-prefixed element name and text content.
    /// The content is escaped automatically during rendering.
    CustomText {
        /// Full element name including prefix, e.g. `"C:calendar-data"`.
        element: String,
        /// Text content (will be XML-escaped).
        content: String,
    },
    /// Raw pre-escaped XML fragment. Use with caution — content is inserted
    /// verbatim. Prefer `CustomText` for simple text properties.
    CustomRaw(String),
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
            DavProperty::CustomText { element, content } => {
                xml.push_str(&format!(
                    "        <{element}>{}</{element}>\r\n",
                    xml_escape(content)
                ));
            }
            DavProperty::CustomRaw(raw) => {
                xml.push_str(&format!("        {raw}\r\n"));
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
/// Each entry in `extra_namespaces` is a `(prefix, uri)` pair, e.g.
/// `&[("C", "urn:ietf:params:xml:ns:caldav")]`.
pub fn open_multistatus(xml: &mut String, extra_namespaces: &[(&str, &str)]) {
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");

    let mut ns = String::new();
    for (prefix, uri) in extra_namespaces {
        ns.push_str(&format!(" xmlns:{prefix}=\"{uri}\""));
    }

    xml.push_str(&format!(
        "<D:multistatus xmlns:D=\"DAV:\"{ns}>\r\n"
    ));
}

/// Start a multi-status XML document from a raw namespace string (legacy API).
///
/// `extra_namespaces` must contain only valid `xmlns:PREFIX="URI"` entries
/// (space-separated). Invalid declarations are dropped with a warning log.
pub fn open_multistatus_raw(xml: &mut String, extra_namespaces: &str) {
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");

    let ns = if validate_namespace_declarations(extra_namespaces) {
        // Ensure a space separator before namespace declarations
        let trimmed = extra_namespaces.trim_start();
        if trimmed.is_empty() {
            String::new()
        } else {
            format!(" {trimmed}")
        }
    } else {
        tracing::warn!(
            namespaces = extra_namespaces,
            "invalid namespace declarations dropped from multistatus element"
        );
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
    fn write_response_entry_with_custom_text_property() {
        let entry = DavResourceEntry {
            href: "/ab/ct.vcf".into(),
            properties: vec![
                DavProperty::ETag("\"e1\"".into()),
                DavProperty::CustomText {
                    element: "CR:address-data".into(),
                    content: "BEGIN:VCARD\r\nEND:VCARD".into(),
                },
            ],
        };
        let mut xml = String::new();
        write_response_entry(&mut xml, &entry);

        assert!(xml.contains("<CR:address-data>"));
        assert!(xml.contains("BEGIN:VCARD"));
        assert!(xml.contains("</CR:address-data>"));
    }

    #[test]
    fn write_response_entry_custom_text_escapes_content() {
        let entry = DavResourceEntry {
            href: "/test".into(),
            properties: vec![DavProperty::CustomText {
                element: "D:displayname".into(),
                content: "Meeting <&> Notes".into(),
            }],
        };
        let mut xml = String::new();
        write_response_entry(&mut xml, &entry);

        assert!(xml.contains("Meeting &lt;&amp;&gt; Notes"));
    }

    #[test]
    fn write_response_entry_with_custom_raw_property() {
        let entry = DavResourceEntry {
            href: "/ab/ct.vcf".into(),
            properties: vec![DavProperty::CustomRaw(
                "<CR:address-data>BEGIN:VCARD\r\nEND:VCARD</CR:address-data>".into(),
            )],
        };
        let mut xml = String::new();
        write_response_entry(&mut xml, &entry);

        assert!(xml.contains("<CR:address-data>"));
        assert!(xml.contains("BEGIN:VCARD"));
    }

    #[test]
    fn open_close_multistatus() {
        let mut xml = String::new();
        open_multistatus(&mut xml, &[("C", "urn:ietf:params:xml:ns:caldav")]);
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
        open_multistatus(&mut xml, &[]);

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
    fn open_multistatus_raw_drops_invalid_ns() {
        let mut xml = String::new();
        open_multistatus_raw(&mut xml, "INVALID_NAMESPACE");
        // Invalid namespace should be dropped; only DAV: namespace remains
        assert!(xml.contains("xmlns:D=\"DAV:\""));
        assert!(!xml.contains("INVALID_NAMESPACE"));
    }

    // --- parse_propfind ---

    #[test]
    fn parse_propfind_empty_body_is_allprop() {
        assert_eq!(parse_propfind("").unwrap(), PropfindRequest::AllProp);
        assert_eq!(parse_propfind("  ").unwrap(), PropfindRequest::AllProp);
    }

    #[test]
    fn parse_propfind_allprop() {
        let body = r#"<?xml version="1.0" encoding="UTF-8"?>
            <D:propfind xmlns:D="DAV:">
                <D:allprop/>
            </D:propfind>"#;
        assert_eq!(parse_propfind(body).unwrap(), PropfindRequest::AllProp);
    }

    #[test]
    fn parse_propfind_propname() {
        let body = r#"<?xml version="1.0" encoding="UTF-8"?>
            <D:propfind xmlns:D="DAV:">
                <D:propname/>
            </D:propfind>"#;
        assert_eq!(parse_propfind(body).unwrap(), PropfindRequest::PropName);
    }

    #[test]
    fn parse_propfind_specific_properties() {
        let body = r#"<?xml version="1.0" encoding="UTF-8"?>
            <D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
                <D:prop>
                    <D:getetag/>
                    <D:getcontenttype/>
                    <C:calendar-data/>
                </D:prop>
            </D:propfind>"#;
        let result = parse_propfind(body).unwrap();
        match result {
            PropfindRequest::Prop(props) => {
                assert_eq!(props.len(), 3);
                assert_eq!(props[0], ("DAV:".to_string(), "getetag".to_string()));
                assert_eq!(props[1], ("DAV:".to_string(), "getcontenttype".to_string()));
                assert_eq!(
                    props[2],
                    (
                        "urn:ietf:params:xml:ns:caldav".to_string(),
                        "calendar-data".to_string()
                    )
                );
            }
            other => panic!("Expected Prop, got {other:?}"),
        }
    }

    // --- parse_report ---

    #[test]
    fn parse_report_empty_body_is_error() {
        assert!(parse_report("").is_err());
    }

    #[test]
    fn parse_report_calendar_query() {
        let body = r#"<?xml version="1.0" encoding="UTF-8"?>
            <C:calendar-query xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
                <D:prop>
                    <D:getetag/>
                    <C:calendar-data/>
                </D:prop>
            </C:calendar-query>"#;
        let result = parse_report(body).unwrap();
        match result {
            ReportRequest::CalendarQuery { properties } => {
                assert_eq!(properties.len(), 2);
                assert_eq!(properties[0].1, "getetag");
                assert_eq!(properties[1].1, "calendar-data");
            }
            other => panic!("Expected CalendarQuery, got {other:?}"),
        }
    }

    #[test]
    fn parse_report_calendar_multiget() {
        let body = r#"<?xml version="1.0" encoding="UTF-8"?>
            <C:calendar-multiget xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
                <D:prop>
                    <D:getetag/>
                </D:prop>
                <D:href>/cal/event1.ics</D:href>
                <D:href>/cal/event2.ics</D:href>
            </C:calendar-multiget>"#;
        let result = parse_report(body).unwrap();
        match result {
            ReportRequest::CalendarMultiget { properties, hrefs } => {
                assert_eq!(properties.len(), 1);
                assert_eq!(properties[0].1, "getetag");
                assert_eq!(hrefs.len(), 2);
                assert_eq!(hrefs[0], "/cal/event1.ics");
                assert_eq!(hrefs[1], "/cal/event2.ics");
            }
            other => panic!("Expected CalendarMultiget, got {other:?}"),
        }
    }

    #[test]
    fn parse_report_addressbook_query() {
        let body = r#"<?xml version="1.0" encoding="UTF-8"?>
            <CR:addressbook-query xmlns:D="DAV:" xmlns:CR="urn:ietf:params:xml:ns:carddav">
                <D:prop>
                    <D:getetag/>
                    <CR:address-data/>
                </D:prop>
            </CR:addressbook-query>"#;
        let result = parse_report(body).unwrap();
        match result {
            ReportRequest::AddressbookQuery { properties } => {
                assert_eq!(properties.len(), 2);
                assert_eq!(properties[0].1, "getetag");
                assert_eq!(properties[1].1, "address-data");
            }
            other => panic!("Expected AddressbookQuery, got {other:?}"),
        }
    }

    #[test]
    fn parse_report_addressbook_multiget() {
        let body = r#"<?xml version="1.0" encoding="UTF-8"?>
            <CR:addressbook-multiget xmlns:D="DAV:" xmlns:CR="urn:ietf:params:xml:ns:carddav">
                <D:prop>
                    <CR:address-data/>
                </D:prop>
                <D:href>/contacts/alice.vcf</D:href>
                <D:href>/contacts/bob.vcf</D:href>
            </CR:addressbook-multiget>"#;
        let result = parse_report(body).unwrap();
        match result {
            ReportRequest::AddressbookMultiget { properties, hrefs } => {
                assert_eq!(properties.len(), 1);
                assert_eq!(properties[0].1, "address-data");
                assert_eq!(hrefs.len(), 2);
                assert_eq!(hrefs[0], "/contacts/alice.vcf");
                assert_eq!(hrefs[1], "/contacts/bob.vcf");
            }
            other => panic!("Expected AddressbookMultiget, got {other:?}"),
        }
    }
}
