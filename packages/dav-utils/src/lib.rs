//! Shared vCard/iCal parsing and DAV sync utilities.
//!
//! This crate provides common utilities used by both the CalDAV and CardDAV
//! connectors and API plugins, eliminating duplication of sync state tracking,
//! authentication, URL construction, ETag-based change detection, text
//! processing, iCal datetime parsing, vCard helpers, and DAV XML building.
//!
//! # Modules
//!
//! - [`sync_state`] -- Unified DAV sync state (sync-token, ctag, ETags)
//! - [`auth`] -- HTTP Basic authentication header construction
//! - [`url`] -- Slash-normalized DAV URL joining
//! - [`etag`] -- ETag-based change detection for DAV resources
//! - [`text`] -- vCard/iCal text utilities (line unfolding, escaping)
//! - [`ical`] -- iCalendar datetime parsing
//! - [`vcard`] -- vCard serialisation helpers (escape, property parsing)
//! - [`dav_xml`] -- WebDAV multi-status XML response builders

pub mod auth;
pub mod dav_xml;
pub mod etag;
pub mod ical;
pub mod sync_state;
pub mod text;
pub mod url;
pub mod vcard;
