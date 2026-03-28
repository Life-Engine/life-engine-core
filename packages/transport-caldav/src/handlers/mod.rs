//! CalDAV request handlers.
//!
//! Implements WebDAV/CalDAV protocol handlers for PROPFIND, REPORT,
//! GET, PUT, DELETE, and MKCALENDAR methods.

use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use dav_utils::dav_xml::{
    close_multistatus, open_multistatus, write_response_entry, DavProperty, DavResourceEntry,
    parse_propfind, parse_report, PropfindRequest, ReportRequest,
};
use std::collections::HashMap;
use std::sync::Arc;

use crate::types::{CalendarCollection, CalendarResource, Depth, CONTENT_TYPE_XML, CONTENT_TYPE_CALENDAR};

/// Shared application state for CalDAV handlers.
#[derive(Debug, Clone)]
pub struct CaldavState {
    /// Base path prefix for CalDAV URLs (e.g. `"/caldav"`).
    pub base_path: String,
    /// Principal path for the current user.
    pub principal: String,
    /// Available calendar collections.
    pub calendars: Vec<CalendarCollection>,
    /// Calendar resources indexed by collection path, then resource UID.
    pub resources: HashMap<String, Vec<CalendarResource>>,
}

impl CaldavState {
    /// Look up a calendar collection by path segment.
    pub fn find_calendar(&self, path: &str) -> Option<&CalendarCollection> {
        self.calendars.iter().find(|c| c.path == path)
    }

    /// Look up a specific resource in a collection.
    pub fn find_resource(&self, collection: &str, uid: &str) -> Option<&CalendarResource> {
        self.resources
            .get(collection)
            .and_then(|rs| rs.iter().find(|r| r.uid == uid))
    }
}

/// CalDAV namespace constants for XML responses.
const CALDAV_NS: &[(&str, &str)] = &[("C", "urn:ietf:params:xml:ns:caldav")];

/// Parse the Depth header from request headers.
fn extract_depth(headers: &HeaderMap) -> Depth {
    headers
        .get("depth")
        .and_then(|v| v.to_str().ok())
        .map(Depth::parse)
        .unwrap_or(Depth::Infinity)
}

/// PROPFIND handler for calendar discovery and property retrieval.
///
/// Supports Depth 0 (resource itself) and Depth 1 (resource + children).
pub async fn handle_propfind(
    State(state): State<Arc<CaldavState>>,
    Path(params): Path<HashMap<String, String>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let depth = extract_depth(&headers);
    let collection_path = params.get("collection").map(|s| s.as_str());

    let propfind = match parse_propfind(&body) {
        Ok(pf) => pf,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, e).into_response();
        }
    };

    let mut xml = String::new();
    open_multistatus(&mut xml, CALDAV_NS);

    match collection_path {
        None => {
            // Root PROPFIND — list calendar home
            let root_entry = DavResourceEntry {
                href: format!("{}/", state.base_path),
                properties: build_home_properties(&state, &propfind),
            };
            write_response_entry(&mut xml, &root_entry);

            if depth == Depth::One {
                for cal in &state.calendars {
                    let entry = DavResourceEntry {
                        href: format!("{}/{}/", state.base_path, cal.path),
                        properties: build_collection_properties(cal, &propfind),
                    };
                    write_response_entry(&mut xml, &entry);
                }
            }
        }
        Some(col) => {
            if let Some(cal) = state.find_calendar(col) {
                let col_entry = DavResourceEntry {
                    href: format!("{}/{}/", state.base_path, cal.path),
                    properties: build_collection_properties(cal, &propfind),
                };
                write_response_entry(&mut xml, &col_entry);

                if depth == Depth::One {
                    if let Some(resources) = state.resources.get(col) {
                        for res in resources {
                            let entry = DavResourceEntry {
                                href: format!("{}/{}/{}", state.base_path, col, res.uid),
                                properties: build_resource_properties(res, &propfind),
                            };
                            write_response_entry(&mut xml, &entry);
                        }
                    }
                }
            } else {
                return (StatusCode::NOT_FOUND, "Calendar collection not found").into_response();
            }
        }
    }

    close_multistatus(&mut xml);
    multistatus_response(xml)
}

/// REPORT handler for calendar-query and calendar-multiget.
pub async fn handle_report(
    State(state): State<Arc<CaldavState>>,
    Path(params): Path<HashMap<String, String>>,
    body: String,
) -> Response {
    let collection = match params.get("collection") {
        Some(c) => c.as_str(),
        None => return (StatusCode::BAD_REQUEST, "Collection path required").into_response(),
    };

    if state.find_calendar(collection).is_none() {
        return (StatusCode::NOT_FOUND, "Calendar collection not found").into_response();
    }

    let report = match parse_report(&body) {
        Ok(r) => r,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };

    let mut xml = String::new();
    open_multistatus(&mut xml, CALDAV_NS);

    let resources = state.resources.get(collection);

    match report {
        ReportRequest::CalendarQuery { ref properties } => {
            if let Some(resources) = resources {
                for res in resources {
                    let entry = build_report_entry(&state.base_path, collection, res, properties);
                    write_response_entry(&mut xml, &entry);
                }
            }
        }
        ReportRequest::CalendarMultiget {
            ref properties,
            ref hrefs,
        } => {
            if let Some(resources) = resources {
                for href in hrefs {
                    let uid = href.rsplit('/').next().unwrap_or("");
                    if let Some(res) = resources.iter().find(|r| r.uid == uid) {
                        let entry =
                            build_report_entry(&state.base_path, collection, res, properties);
                        write_response_entry(&mut xml, &entry);
                    }
                }
            }
        }
        _ => {
            return (
                StatusCode::FORBIDDEN,
                "Only CalDAV REPORT types are supported on this endpoint",
            )
                .into_response();
        }
    }

    close_multistatus(&mut xml);
    multistatus_response(xml)
}

/// GET handler — returns the iCalendar data for a single resource.
pub async fn handle_get(
    State(state): State<Arc<CaldavState>>,
    Path(params): Path<HashMap<String, String>>,
) -> Response {
    let (collection, resource) = match extract_col_res(&params) {
        Some(cr) => cr,
        None => return (StatusCode::BAD_REQUEST, "Invalid path").into_response(),
    };

    match state.find_resource(collection, resource) {
        Some(res) => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", CONTENT_TYPE_CALENDAR)
            .header("ETag", &res.etag)
            .body(Body::from(res.data.clone()))
            .unwrap(),
        None => (StatusCode::NOT_FOUND, "Resource not found").into_response(),
    }
}

/// PUT handler — creates or updates a calendar resource.
///
/// Returns 201 Created for new resources, 204 No Content for updates.
/// Supports If-Match for conditional updates.
pub async fn handle_put(
    State(state): State<Arc<CaldavState>>,
    Path(params): Path<HashMap<String, String>>,
    headers: HeaderMap,
    _body: String,
) -> Response {
    let (collection, resource) = match extract_col_res(&params) {
        Some(cr) => cr,
        None => return (StatusCode::BAD_REQUEST, "Invalid path").into_response(),
    };

    if state.find_calendar(collection).is_none() {
        return (StatusCode::NOT_FOUND, "Calendar collection not found").into_response();
    }

    let existing = state.find_resource(collection, resource);

    // Check If-Match precondition
    if let Some(if_match) = headers.get("if-match").and_then(|v| v.to_str().ok()) {
        match existing {
            Some(res) if res.etag != if_match => {
                return (StatusCode::PRECONDITION_FAILED, "ETag mismatch").into_response();
            }
            None => {
                return (StatusCode::PRECONDITION_FAILED, "Resource does not exist").into_response();
            }
            _ => {}
        }
    }

    // In a real implementation this would store the resource. We generate
    // a new ETag and return the appropriate status.
    let new_etag = format!("\"caldav-{}\"", uuid_stub(resource));
    let status = if existing.is_some() {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::CREATED
    };

    Response::builder()
        .status(status)
        .header("ETag", new_etag)
        .body(Body::empty())
        .unwrap()
}

/// DELETE handler — removes a calendar resource.
pub async fn handle_delete(
    State(state): State<Arc<CaldavState>>,
    Path(params): Path<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let (collection, resource) = match extract_col_res(&params) {
        Some(cr) => cr,
        None => return (StatusCode::BAD_REQUEST, "Invalid path").into_response(),
    };

    match state.find_resource(collection, resource) {
        Some(res) => {
            // Check If-Match
            if let Some(if_match) = headers.get("if-match").and_then(|v| v.to_str().ok()) {
                if res.etag != if_match {
                    return (StatusCode::PRECONDITION_FAILED, "ETag mismatch").into_response();
                }
            }
            (StatusCode::NO_CONTENT, "").into_response()
        }
        None => (StatusCode::NOT_FOUND, "Resource not found").into_response(),
    }
}

/// OPTIONS handler — returns DAV compliance headers per RFC 4791.
pub async fn handle_options() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header("Allow", "OPTIONS, GET, PUT, DELETE, PROPFIND, REPORT, MKCALENDAR")
        .header("DAV", "1, 2, calendar-access")
        .body(Body::empty())
        .unwrap()
}

/// MKCALENDAR handler — creates a new calendar collection.
pub async fn handle_mkcalendar(
    State(state): State<Arc<CaldavState>>,
    Path(params): Path<HashMap<String, String>>,
) -> Response {
    let collection = match params.get("collection") {
        Some(c) => c.as_str(),
        None => return (StatusCode::BAD_REQUEST, "Collection path required").into_response(),
    };

    if state.find_calendar(collection).is_some() {
        return (StatusCode::CONFLICT, "Calendar already exists").into_response();
    }

    // In a real implementation, this would create the collection in storage.
    (StatusCode::CREATED, "").into_response()
}

// --- Helper functions ---

fn extract_col_res<'a>(params: &'a HashMap<String, String>) -> Option<(&'a str, &'a str)> {
    let col = params.get("collection")?;
    let res = params.get("resource")?;
    Some((col.as_str(), res.as_str()))
}

fn build_home_properties(state: &CaldavState, propfind: &PropfindRequest) -> Vec<DavProperty> {
    let mut props = Vec::new();
    let include_all = matches!(propfind, PropfindRequest::AllProp);

    if include_all || prop_requested(propfind, "resourcetype") {
        props.push(DavProperty::CustomRaw(
            "<D:resourcetype><D:collection/></D:resourcetype>".into(),
        ));
    }
    if include_all || prop_requested(propfind, "displayname") {
        props.push(DavProperty::CustomText {
            element: "D:displayname".into(),
            content: "Calendar Home".into(),
        });
    }
    if include_all || prop_requested(propfind, "current-user-principal") {
        props.push(DavProperty::CustomRaw(format!(
            "<D:current-user-principal><D:href>{}</D:href></D:current-user-principal>",
            state.principal
        )));
    }
    props
}

fn build_collection_properties(
    cal: &CalendarCollection,
    propfind: &PropfindRequest,
) -> Vec<DavProperty> {
    let mut props = Vec::new();
    let include_all = matches!(propfind, PropfindRequest::AllProp);

    if include_all || prop_requested(propfind, "resourcetype") {
        props.push(DavProperty::CustomRaw(
            "<D:resourcetype><D:collection/><C:calendar/></D:resourcetype>".into(),
        ));
    }
    if include_all || prop_requested(propfind, "displayname") {
        props.push(DavProperty::CustomText {
            element: "D:displayname".into(),
            content: cal.display_name.clone(),
        });
    }
    if include_all || prop_requested(propfind, "calendar-description") {
        if let Some(desc) = &cal.description {
            props.push(DavProperty::CustomText {
                element: "C:calendar-description".into(),
                content: desc.clone(),
            });
        }
    }
    if include_all || prop_requested(propfind, "supported-calendar-component-set") {
        props.push(DavProperty::CustomRaw(
            "<C:supported-calendar-component-set><C:comp name=\"VEVENT\"/></C:supported-calendar-component-set>".into(),
        ));
    }
    props
}

fn build_resource_properties(
    res: &CalendarResource,
    propfind: &PropfindRequest,
) -> Vec<DavProperty> {
    let mut props = Vec::new();
    let include_all = matches!(propfind, PropfindRequest::AllProp);

    if include_all || prop_requested(propfind, "getetag") {
        props.push(DavProperty::ETag(res.etag.clone()));
    }
    if include_all || prop_requested(propfind, "getcontenttype") {
        props.push(DavProperty::ContentType(
            "text/calendar; charset=utf-8".into(),
        ));
    }
    props
}

fn build_report_entry(
    base_path: &str,
    collection: &str,
    res: &CalendarResource,
    properties: &[(String, String)],
) -> DavResourceEntry {
    let mut props = Vec::new();

    for (_ns, local) in properties {
        match local.as_str() {
            "getetag" => props.push(DavProperty::ETag(res.etag.clone())),
            "getcontenttype" => {
                props.push(DavProperty::ContentType(
                    "text/calendar; charset=utf-8".into(),
                ))
            }
            "calendar-data" => props.push(DavProperty::CustomText {
                element: "C:calendar-data".into(),
                content: res.data.clone(),
            }),
            _ => {}
        }
    }

    DavResourceEntry {
        href: format!("{}/{}/{}", base_path, collection, res.uid),
        properties: props,
    }
}

fn prop_requested(propfind: &PropfindRequest, name: &str) -> bool {
    match propfind {
        PropfindRequest::AllProp => true,
        PropfindRequest::PropName => true,
        PropfindRequest::Prop(props) => props.iter().any(|(_, local)| local == name),
    }
}

fn multistatus_response(xml: String) -> Response {
    Response::builder()
        .status(StatusCode::MULTI_STATUS)
        .header("Content-Type", CONTENT_TYPE_XML)
        .body(Body::from(xml))
        .unwrap()
}

/// Simple deterministic stub for generating ETags in the absence of real storage.
fn uuid_stub(input: &str) -> u64 {
    let mut hash: u64 = 0;
    for b in input.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(b as u64);
    }
    hash
}
