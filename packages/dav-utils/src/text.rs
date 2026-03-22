//! Text processing utilities shared by vCard and iCal parsers.
//!
//! Handles RFC 6350 / RFC 2425 line unfolding, vCard/iCal escape
//! sequences, and common string helpers.

/// Unfold continuation lines per RFC 6350 / RFC 2425.
///
/// Lines that start with a single space or tab are continuations of the
/// previous logical line. The leading whitespace character is stripped
/// and the content is appended to the previous line.
///
/// # Examples
///
/// ```
/// let raw = "FN:Long Name\n That Wraps";
/// let unfolded = dav_utils::text::unfold_lines(raw);
/// assert_eq!(unfolded, "FN:Long NameThat Wraps");
/// ```
pub fn unfold_lines(raw: &str) -> String {
    // Normalize CRLF → LF first per RFC 6350 §3.2, then unfold.
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let mut result = String::with_capacity(normalized.len());
    for line in normalized.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            // Continuation: append without the leading whitespace
            result.push_str(&line[1..]);
        } else {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(line);
        }
    }
    result
}

/// Decode vCard/iCal escaped values.
///
/// Handles the standard escape sequences:
/// - `\\n` becomes a literal newline
/// - `\\,` becomes a literal comma
/// - `\\;` becomes a literal semicolon
/// - `\\\\` becomes a literal backslash
///
/// # Examples
///
/// ```
/// assert_eq!(dav_utils::text::decode_escaped_value("hello\\, world"), "hello, world");
/// assert_eq!(dav_utils::text::decode_escaped_value("line1\\nline2"), "line1\nline2");
/// ```
pub fn decode_escaped_value(value: &str) -> String {
    // Process backslash escapes in a single pass to avoid ordering bugs.
    // A chained `.replace()` approach fails because replacing `\\` before
    // or after `\n` can cause double-unescaping (e.g. `\\n` → `\n` instead
    // of literal backslash + 'n').
    let mut result = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some('n') => {
                    result.push('\n');
                    chars.next();
                }
                Some(',') => {
                    result.push(',');
                    chars.next();
                }
                Some(';') => {
                    result.push(';');
                    chars.next();
                }
                Some('\\') => {
                    result.push('\\');
                    chars.next();
                }
                _ => {
                    // Unknown escape sequence: preserve as-is
                    result.push('\\');
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Return `Some(s)` if the string is non-empty after trimming, `None` otherwise.
///
/// When the value is non-empty, escape sequences are decoded via
/// [`decode_escaped_value`].
///
/// # Examples
///
/// ```
/// assert_eq!(dav_utils::text::non_empty("  hello  "), Some("hello".to_string()));
/// assert_eq!(dav_utils::text::non_empty("   "), None);
/// assert_eq!(dav_utils::text::non_empty(""), None);
/// ```
pub fn non_empty(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(decode_escaped_value(trimmed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- unfold_lines ---

    #[test]
    fn unfold_space_continuation() {
        let input = "LINE1\n LINE2";
        assert_eq!(unfold_lines(input), "LINE1LINE2");
    }

    #[test]
    fn unfold_tab_continuation() {
        let input = "LINE1\n\tLINE2";
        assert_eq!(unfold_lines(input), "LINE1LINE2");
    }

    #[test]
    fn unfold_no_continuation() {
        let input = "LINE1\nLINE2";
        assert_eq!(unfold_lines(input), "LINE1\nLINE2");
    }

    #[test]
    fn unfold_multiple_continuations() {
        let input = "START\n part1\n part2\n part3";
        assert_eq!(unfold_lines(input), "STARTpart1part2part3");
    }

    #[test]
    fn unfold_mixed_normal_and_continuation() {
        let input = "A:1\n continued\nB:2\n also continued";
        assert_eq!(unfold_lines(input), "A:1continued\nB:2also continued");
    }

    #[test]
    fn unfold_empty_string() {
        assert_eq!(unfold_lines(""), "");
    }

    #[test]
    fn unfold_single_line() {
        assert_eq!(unfold_lines("HELLO"), "HELLO");
    }

    #[test]
    fn unfold_preserves_crlf_stripped() {
        // lines() handles both \n and \r\n
        let input = "FN:Name\r\n continued";
        assert_eq!(unfold_lines(input), "FN:Namecontinued");
    }

    // --- decode_escaped_value ---

    #[test]
    fn decode_newline() {
        assert_eq!(decode_escaped_value("a\\nb"), "a\nb");
    }

    #[test]
    fn decode_comma() {
        assert_eq!(decode_escaped_value("a\\,b"), "a,b");
    }

    #[test]
    fn decode_semicolon() {
        assert_eq!(decode_escaped_value("a\\;b"), "a;b");
    }

    #[test]
    fn decode_backslash() {
        assert_eq!(decode_escaped_value("a\\\\b"), "a\\b");
    }

    #[test]
    fn decode_multiple_escapes() {
        assert_eq!(
            decode_escaped_value("hello\\, world\\; end\\n"),
            "hello, world; end\n"
        );
    }

    #[test]
    fn decode_no_escapes() {
        assert_eq!(decode_escaped_value("plain text"), "plain text");
    }

    #[test]
    fn decode_empty_string() {
        assert_eq!(decode_escaped_value(""), "");
    }

    // --- non_empty ---

    #[test]
    fn non_empty_with_content() {
        assert_eq!(non_empty("hello"), Some("hello".to_string()));
    }

    #[test]
    fn non_empty_trims_whitespace() {
        assert_eq!(non_empty("  hello  "), Some("hello".to_string()));
    }

    #[test]
    fn non_empty_returns_none_for_empty() {
        assert_eq!(non_empty(""), None);
    }

    #[test]
    fn non_empty_returns_none_for_whitespace() {
        assert_eq!(non_empty("   "), None);
    }

    #[test]
    fn non_empty_decodes_escapes() {
        assert_eq!(non_empty("hello\\, world"), Some("hello, world".to_string()));
    }

    #[test]
    fn non_empty_tab_only() {
        assert_eq!(non_empty("\t\t"), None);
    }

    // --- F-068: escape ordering regression tests ---

    #[test]
    fn decode_double_backslash_before_n() {
        // `\\n` in vCard means literal backslash followed by 'n', not a newline.
        assert_eq!(decode_escaped_value("a\\\\nb"), "a\\nb");
    }

    #[test]
    fn decode_backslash_then_newline_escape() {
        // `\\\n` means literal backslash + newline escape
        assert_eq!(decode_escaped_value("a\\\\\\nb"), "a\\\nb");
    }

    #[test]
    fn decode_unknown_escape_preserved() {
        // Unknown escape sequences are preserved as-is
        assert_eq!(decode_escaped_value("a\\xb"), "a\\xb");
    }

    // --- F-069: CRLF normalization in unfold_lines ---

    #[test]
    fn unfold_crlf_input() {
        let input = "FN:Name\r\n continued\r\nEMAIL:a@b.com";
        assert_eq!(unfold_lines(input), "FN:Namecontinued\nEMAIL:a@b.com");
    }

    #[test]
    fn unfold_bare_cr() {
        let input = "FN:Name\r continued";
        assert_eq!(unfold_lines(input), "FN:Namecontinued");
    }
}
