//! Module-internal types for the CardDAV transport crate.

/// Content-Type for vCard responses.
pub const CONTENT_TYPE_VCARD: &str = "text/vcard; charset=utf-8";

/// Content-Type for WebDAV XML responses.
pub const CONTENT_TYPE_XML: &str = "application/xml; charset=utf-8";

/// An addressbook collection exposed by this transport.
#[derive(Debug, Clone)]
pub struct AddressbookCollection {
    /// Display name shown to CardDAV clients.
    pub display_name: String,
    /// URL path segment for this addressbook (e.g. `"default"`).
    pub path: String,
    /// Optional description.
    pub description: Option<String>,
}

/// A single contact resource within an addressbook.
#[derive(Debug, Clone)]
pub struct ContactResource {
    /// Unique identifier (used as filename, e.g. `"contact-uuid.vcf"`).
    pub uid: String,
    /// Full vCard data.
    pub data: String,
    /// ETag for change detection.
    pub etag: String,
}

/// Depth header values for PROPFIND requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    Zero,
    One,
    Infinity,
}

impl Depth {
    pub fn parse(value: &str) -> Self {
        match value.trim() {
            "0" => Depth::Zero,
            "1" => Depth::One,
            _ => Depth::Infinity,
        }
    }
}
