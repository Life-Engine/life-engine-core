//! Shared vCard serialisation helpers.
//!
//! Provides escape/unescape functions and property parsing utilities
//! used by both the CardDAV connector (inbound sync) and the CardDAV
//! API server (outbound serving).

/// Escape special characters in a vCard property value.
///
/// Per RFC 6350, the following characters must be escaped:
/// - Backslash → `\\`
/// - Comma → `\,`
/// - Semicolon → `\;`
/// - Newline → `\n`
pub fn escape_value(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(',', "\\,")
        .replace(';', "\\;")
        .replace('\n', "\\n")
}

/// Parse a vCard property line into (property_name, parameters, value).
///
/// Handles `PROP;PARAM1=val1;PARAM2:value` format.
/// Returns `None` if the line has no colon separator.
#[allow(clippy::type_complexity)]
pub fn parse_property_line(line: &str) -> Option<(&str, Vec<(&str, &str)>, &str)> {
    // Strip trailing CR for lines that still have CRLF endings (RFC 6350).
    let line = line.strip_suffix('\r').unwrap_or(line);
    let (prop_with_params, value) = line.split_once(':')?;

    let mut parts = prop_with_params.split(';');
    let prop_name = parts.next()?;

    let params: Vec<(&str, &str)> = parts
        .map(|p| {
            if let Some((k, v)) = p.split_once('=') {
                (k, v)
            } else {
                ("TYPE", p)
            }
        })
        .collect();

    Some((prop_name, params, value))
}

/// Check whether a parameter list contains a TYPE parameter with a specific value.
pub fn has_type_param(params: &[(&str, &str)], value: &str) -> bool {
    params.iter().any(|(k, v)| {
        k.eq_ignore_ascii_case("TYPE")
            && v.split(',')
                .any(|t| t.eq_ignore_ascii_case(value))
    })
}

/// Extract the first TYPE parameter value (excluding "pref").
pub fn extract_type(params: &[(&str, &str)]) -> Option<String> {
    for (key, value) in params {
        if key.eq_ignore_ascii_case("TYPE") {
            for t in value.split(',') {
                if !t.eq_ignore_ascii_case("pref") {
                    return Some(t.to_lowercase());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_backslash() {
        assert_eq!(escape_value("a\\b"), "a\\\\b");
    }

    #[test]
    fn escape_comma() {
        assert_eq!(escape_value("a,b"), "a\\,b");
    }

    #[test]
    fn escape_semicolon() {
        assert_eq!(escape_value("a;b"), "a\\;b");
    }

    #[test]
    fn escape_newline() {
        assert_eq!(escape_value("a\nb"), "a\\nb");
    }

    #[test]
    fn escape_combined() {
        assert_eq!(escape_value("a,b;c\\d\ne"), "a\\,b\\;c\\\\d\\ne");
    }

    #[test]
    fn escape_no_special_chars() {
        assert_eq!(escape_value("plain text"), "plain text");
    }

    #[test]
    fn parse_simple_property() {
        let (name, params, value) = parse_property_line("FN:Jane Doe").unwrap();
        assert_eq!(name, "FN");
        assert!(params.is_empty());
        assert_eq!(value, "Jane Doe");
    }

    #[test]
    fn parse_property_with_params() {
        let (name, params, value) = parse_property_line("EMAIL;TYPE=work:jane@example.com").unwrap();
        assert_eq!(name, "EMAIL");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0], ("TYPE", "work"));
        assert_eq!(value, "jane@example.com");
    }

    #[test]
    fn parse_property_with_bare_type() {
        let (name, params, value) = parse_property_line("TEL;CELL:+1-555-0100").unwrap();
        assert_eq!(name, "TEL");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0], ("TYPE", "CELL"));
        assert_eq!(value, "+1-555-0100");
    }

    #[test]
    fn parse_property_no_colon() {
        assert!(parse_property_line("NO_COLON_HERE").is_none());
    }

    #[test]
    fn has_type_param_found() {
        let params = vec![("TYPE", "work,pref")];
        assert!(has_type_param(&params, "work"));
        assert!(has_type_param(&params, "pref"));
    }

    #[test]
    fn has_type_param_not_found() {
        let params = vec![("TYPE", "work")];
        assert!(!has_type_param(&params, "home"));
    }

    #[test]
    fn extract_type_excludes_pref() {
        let params = vec![("TYPE", "home,pref")];
        assert_eq!(extract_type(&params), Some("home".to_string()));
    }

    #[test]
    fn extract_type_returns_first() {
        let params = vec![("TYPE", "work")];
        assert_eq!(extract_type(&params), Some("work".to_string()));
    }

    #[test]
    fn extract_type_only_pref_returns_none() {
        let params = vec![("TYPE", "pref")];
        assert_eq!(extract_type(&params), None);
    }

    #[test]
    fn extract_type_no_type_param() {
        let params: Vec<(&str, &str)> = vec![("VALUE", "uri")];
        assert_eq!(extract_type(&params), None);
    }

    // --- F-069: CRLF handling in parse_property_line ---

    #[test]
    fn parse_property_line_strips_trailing_cr() {
        let (name, params, value) = parse_property_line("FN:Jane Doe\r").unwrap();
        assert_eq!(name, "FN");
        assert!(params.is_empty());
        assert_eq!(value, "Jane Doe");
    }
}
