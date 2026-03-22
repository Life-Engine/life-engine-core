//! Slash-normalized DAV URL construction.
//!
//! DAV servers are sensitive to double-slashes in paths. This module
//! provides a helper that joins a base URL and a path segment while
//! normalizing the slash boundary so exactly one separator appears.

/// Join a base URL and a path segment with exactly one `/` separator.
///
/// Trims trailing slashes from `base` and leading slashes from `path`,
/// then joins them with a single `/`.
///
/// # Examples
///
/// ```
/// let url = dav_utils::url::join_dav_url("http://localhost:5232", "/user/cal/");
/// assert_eq!(url, "http://localhost:5232/user/cal/");
///
/// let url = dav_utils::url::join_dav_url("http://localhost:5232/", "/user/cal/");
/// assert_eq!(url, "http://localhost:5232/user/cal/");
/// ```
pub fn join_dav_url(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    let joined = format!("{base}/{path}");
    // Normalize any internal double slashes (preserving the scheme's "://").
    if let Some(idx) = joined.find("://") {
        let (scheme, rest) = joined.split_at(idx + 3);
        let normalized: String = rest.chars().fold(String::with_capacity(rest.len()), |mut acc, c| {
            if c == '/' && acc.ends_with('/') {
                // skip duplicate slash
            } else {
                acc.push(c);
            }
            acc
        });
        format!("{scheme}{normalized}")
    } else {
        joined
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_no_trailing_no_leading() {
        assert_eq!(
            join_dav_url("http://localhost:5232", "user/cal/"),
            "http://localhost:5232/user/cal/"
        );
    }

    #[test]
    fn join_trailing_base_slash() {
        assert_eq!(
            join_dav_url("http://localhost:5232/", "user/cal/"),
            "http://localhost:5232/user/cal/"
        );
    }

    #[test]
    fn join_leading_path_slash() {
        assert_eq!(
            join_dav_url("http://localhost:5232", "/user/cal/"),
            "http://localhost:5232/user/cal/"
        );
    }

    #[test]
    fn join_both_slashes() {
        assert_eq!(
            join_dav_url("http://localhost:5232/", "/user/cal/"),
            "http://localhost:5232/user/cal/"
        );
    }

    #[test]
    fn join_multiple_trailing_slashes_on_base() {
        assert_eq!(
            join_dav_url("http://localhost:5232///", "/user/cal/"),
            "http://localhost:5232/user/cal/"
        );
    }

    #[test]
    fn join_multiple_leading_slashes_on_path() {
        assert_eq!(
            join_dav_url("http://localhost:5232", "///user/cal/"),
            "http://localhost:5232/user/cal/"
        );
    }

    #[test]
    fn join_preserves_trailing_path_slash() {
        assert_eq!(
            join_dav_url("https://dav.example.com", "/addressbooks/user/default/"),
            "https://dav.example.com/addressbooks/user/default/"
        );
    }

    #[test]
    fn join_empty_path() {
        assert_eq!(
            join_dav_url("http://localhost:5232", ""),
            "http://localhost:5232/"
        );
    }

    #[test]
    fn join_empty_base() {
        assert_eq!(join_dav_url("", "/path/"), "/path/");
    }

    #[test]
    fn join_with_https() {
        assert_eq!(
            join_dav_url("https://dav.example.com", "/contacts/"),
            "https://dav.example.com/contacts/"
        );
    }
}
