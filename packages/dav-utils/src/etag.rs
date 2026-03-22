//! ETag-based change detection for DAV resources.
//!
//! Both CalDAV and CardDAV use ETags to detect which individual
//! resources have been created or modified since the last sync. This
//! module provides a trait for DAV resources and a generic filter
//! function that works with any resource type implementing the trait.

use std::collections::HashMap;

/// A DAV resource that carries an href and an ETag.
///
/// Implement this trait for any fetched-resource type (CalDAV events,
/// CardDAV vCards) to use the generic [`filter_changed`] function.
pub trait DavResource: Clone {
    /// The server-side href (path) of this resource.
    fn href(&self) -> &str;
    /// The ETag (entity tag) for this version of the resource.
    fn etag(&self) -> &str;
}

/// Filter a list of fetched DAV resources to only those that are new
/// or have a changed ETag compared to the stored ETag map.
///
/// Resources whose href is absent from `stored_etags` are considered
/// new. Resources whose ETag differs from the stored value are
/// considered modified.
///
/// # Examples
///
/// ```
/// use dav_utils::etag::{DavResource, filter_changed};
/// use std::collections::HashMap;
///
/// #[derive(Clone)]
/// struct Res { href: String, etag: String }
///
/// impl DavResource for Res {
///     fn href(&self) -> &str { &self.href }
///     fn etag(&self) -> &str { &self.etag }
/// }
///
/// let stored = HashMap::from([("/a".to_string(), "\"e1\"".to_string())]);
/// let fetched = vec![
///     Res { href: "/a".into(), etag: "\"e1\"".into() }, // unchanged
///     Res { href: "/b".into(), etag: "\"e2\"".into() }, // new
/// ];
/// let changed = filter_changed(&stored, &fetched);
/// assert_eq!(changed.len(), 1);
/// assert_eq!(changed[0].href(), "/b");
/// ```
pub fn filter_changed<R: DavResource>(
    stored_etags: &HashMap<String, String>,
    fetched: &[R],
) -> Vec<R> {
    fetched
        .iter()
        .filter(|r| {
            match stored_etags.get(r.href()) {
                Some(stored_etag) => stored_etag != r.etag(),
                None => true, // new resource
            }
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    struct TestResource {
        href: String,
        etag: String,
    }

    impl DavResource for TestResource {
        fn href(&self) -> &str {
            &self.href
        }
        fn etag(&self) -> &str {
            &self.etag
        }
    }

    fn res(href: &str, etag: &str) -> TestResource {
        TestResource {
            href: href.into(),
            etag: etag.into(),
        }
    }

    #[test]
    fn all_new_resources_returned() {
        let stored = HashMap::new();
        let fetched = vec![res("/a", "\"e1\""), res("/b", "\"e2\"")];
        let changed = filter_changed(&stored, &fetched);
        assert_eq!(changed.len(), 2);
    }

    #[test]
    fn unchanged_resources_filtered_out() {
        let stored =
            HashMap::from([("/a".to_string(), "\"e1\"".to_string())]);
        let fetched = vec![res("/a", "\"e1\"")];
        let changed = filter_changed(&stored, &fetched);
        assert!(changed.is_empty());
    }

    #[test]
    fn modified_resources_returned() {
        let stored =
            HashMap::from([("/a".to_string(), "\"e-old\"".to_string())]);
        let fetched = vec![res("/a", "\"e-new\"")];
        let changed = filter_changed(&stored, &fetched);
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].etag(), "\"e-new\"");
    }

    #[test]
    fn mixed_new_modified_unchanged() {
        let stored = HashMap::from([
            ("/unchanged".to_string(), "\"same\"".to_string()),
            ("/modified".to_string(), "\"old\"".to_string()),
        ]);
        let fetched = vec![
            res("/unchanged", "\"same\""),
            res("/modified", "\"new\""),
            res("/brand-new", "\"fresh\""),
        ];
        let changed = filter_changed(&stored, &fetched);
        assert_eq!(changed.len(), 2);
        let hrefs: Vec<&str> = changed.iter().map(|r| r.href()).collect();
        assert!(hrefs.contains(&"/modified"));
        assert!(hrefs.contains(&"/brand-new"));
        assert!(!hrefs.contains(&"/unchanged"));
    }

    #[test]
    fn empty_fetched_returns_empty() {
        let stored =
            HashMap::from([("/a".to_string(), "\"e1\"".to_string())]);
        let fetched: Vec<TestResource> = vec![];
        let changed = filter_changed(&stored, &fetched);
        assert!(changed.is_empty());
    }

    #[test]
    fn empty_stored_returns_all() {
        let stored = HashMap::new();
        let fetched = vec![res("/a", "\"e1\""), res("/b", "\"e2\""), res("/c", "\"e3\"")];
        let changed = filter_changed(&stored, &fetched);
        assert_eq!(changed.len(), 3);
    }
}
