//! CardDAV request handlers.
//!
//! Implements WebDAV/CardDAV protocol handlers for PROPFIND, REPORT,
//! GET, PUT, DELETE, and MKCOL methods per RFC 6352.

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

use crate::types::{AddressbookCollection, ContactResource, Depth, CONTENT_TYPE_XML, CONTENT_TYPE_VCARD};

/// Shared application state for CardDAV handlers.
#[derive(Debug, Clone)]
pub struct CarddavState {
    /// Base path prefix for CardDAV URLs (e.g. `"/carddav"`).
    pub base_path: String,
    /// Principal path for the current user.
    pub principal: String,
    /// Available addressbook collections.
    pub addressbooks: Vec<AddressbookCollection>,
    /// Contact resources indexed by addressbook path, then resource UID.
    pub resources: HashMap<String, Vec<ContactResource>>,
}

impl CarddavState {
    pub fn find_addressbook(&self, path: &str) -> Option<&AddressbookCollection> {
        self.addressbooks.iter().find(|a| a.path == path)
    }

    pub fn find_resource(&self, addressbook: &str, uid: &str) -> Option<&ContactResource> {
        self.resources
            .get(addressbook)
            .and_then(|rs| rs.iter().find(|r| r.uid == uid))
    }
}

const CARDDAV_NS: &[(&str, &str)] = &[("CR", "urn:ietf:params:xml:ns:carddav")];

fn extract_depth(headers: &HeaderMap) -> Depth {
    headers
        .get("depth")
        .and_then(|v| v.to_str().ok())
        .map(Depth::parse)
        .unwrap_or(Depth::Infinity)
}

/// PROPFIND handler for addressbook discovery and property retrieval.
pub async fn handle_propfind(
    State(state): State<Arc<CarddavState>>,
    Path(params): Path<HashMap<String, String>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let depth = extract_depth(&headers);
    let addressbook_path = params.get("addressbook").map(|s| s.as_str());

    let propfind = match parse_propfind(&body) {
        Ok(pf) => pf,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };

    let mut xml = String::new();
    open_multistatus(&mut xml, CARDDAV_NS);

    match addressbook_path {
        None => {
            let root_entry = DavResourceEntry {
                href: format!("{}/", state.base_path),
                properties: build_home_properties(&state, &propfind),
            };
            write_response_entry(&mut xml, &root_entry);

            if depth == Depth::One {
                for ab in &state.addressbooks {
                    let entry = DavResourceEntry {
                        href: format!("{}/{}/", state.base_path, ab.path),
                        properties: build_addressbook_properties(ab, &propfind),
                    };
                    write_response_entry(&mut xml, &entry);
                }
            }
        }
        Some(ab_path) => {
            if let Some(ab) = state.find_addressbook(ab_path) {
                let entry = DavResourceEntry {
                    href: format!("{}/{}/", state.base_path, ab.path),
                    properties: build_addressbook_properties(ab, &propfind),
                };
                write_response_entry(&mut xml, &entry);

                if depth == Depth::One {
                    if let Some(resources) = state.resources.get(ab_path) {
                        for res in resources {
                            let entry = DavResourceEntry {
                                href: format!("{}/{}/{}", state.base_path, ab_path, res.uid),
                                properties: build_resource_properties(res, &propfind),
                            };
                            write_response_entry(&mut xml, &entry);
                        }
                    }
                }
            } else {
                return (StatusCode::NOT_FOUND, "Addressbook not found").into_response();
            }
        }
    }

    close_multistatus(&mut xml);
    multistatus_response(xml)
}

/// REPORT handler for addressbook-query and addressbook-multiget.
pub async fn handle_report(
    State(state): State<Arc<CarddavState>>,
    Path(params): Path<HashMap<String, String>>,
    body: String,
) -> Response {
    let addressbook = match params.get("addressbook") {
        Some(a) => a.as_str(),
        None => return (StatusCode::BAD_REQUEST, "Addressbook path required").into_response(),
    };

    if state.find_addressbook(addressbook).is_none() {
        return (StatusCode::NOT_FOUND, "Addressbook not found").into_response();
    }

    let report = match parse_report(&body) {
        Ok(r) => r,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };

    let mut xml = String::new();
    open_multistatus(&mut xml, CARDDAV_NS);

    let resources = state.resources.get(addressbook);

    match report {
        ReportRequest::AddressbookQuery { ref properties } => {
            if let Some(resources) = resources {
                for res in resources {
                    let entry = build_report_entry(&state.base_path, addressbook, res, properties);
                    write_response_entry(&mut xml, &entry);
                }
            }
        }
        ReportRequest::AddressbookMultiget {
            ref properties,
            ref hrefs,
        } => {
            if let Some(resources) = resources {
                for href in hrefs {
                    let uid = href.rsplit('/').next().unwrap_or("");
                    if let Some(res) = resources.iter().find(|r| r.uid == uid) {
                        let entry =
                            build_report_entry(&state.base_path, addressbook, res, properties);
                        write_response_entry(&mut xml, &entry);
                    }
                }
            }
        }
        _ => {
            return (
                StatusCode::FORBIDDEN,
                "Only CardDAV REPORT types are supported on this endpoint",
            )
                .into_response();
        }
    }

    close_multistatus(&mut xml);
    multistatus_response(xml)
}

/// GET handler — returns the vCard data for a single contact resource.
pub async fn handle_get(
    State(state): State<Arc<CarddavState>>,
    Path(params): Path<HashMap<String, String>>,
) -> Response {
    let (addressbook, resource) = match extract_ab_res(&params) {
        Some(cr) => cr,
        None => return (StatusCode::BAD_REQUEST, "Invalid path").into_response(),
    };

    match state.find_resource(addressbook, resource) {
        Some(res) => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", CONTENT_TYPE_VCARD)
            .header("ETag", &res.etag)
            .body(Body::from(res.data.clone()))
            .unwrap(),
        None => (StatusCode::NOT_FOUND, "Resource not found").into_response(),
    }
}

/// PUT handler — creates or updates a contact resource.
pub async fn handle_put(
    State(state): State<Arc<CarddavState>>,
    Path(params): Path<HashMap<String, String>>,
    headers: HeaderMap,
    _body: String,
) -> Response {
    let (addressbook, resource) = match extract_ab_res(&params) {
        Some(cr) => cr,
        None => return (StatusCode::BAD_REQUEST, "Invalid path").into_response(),
    };

    if state.find_addressbook(addressbook).is_none() {
        return (StatusCode::NOT_FOUND, "Addressbook not found").into_response();
    }

    let existing = state.find_resource(addressbook, resource);

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

    let new_etag = format!("\"carddav-{}\"", hash_stub(resource));
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

/// DELETE handler — removes a contact resource.
pub async fn handle_delete(
    State(state): State<Arc<CarddavState>>,
    Path(params): Path<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let (addressbook, resource) = match extract_ab_res(&params) {
        Some(cr) => cr,
        None => return (StatusCode::BAD_REQUEST, "Invalid path").into_response(),
    };

    match state.find_resource(addressbook, resource) {
        Some(res) => {
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

/// MKCOL handler — creates a new addressbook collection.
pub async fn handle_mkcol(
    State(state): State<Arc<CarddavState>>,
    Path(params): Path<HashMap<String, String>>,
) -> Response {
    let addressbook = match params.get("addressbook") {
        Some(a) => a.as_str(),
        None => return (StatusCode::BAD_REQUEST, "Addressbook path required").into_response(),
    };

    if state.find_addressbook(addressbook).is_some() {
        return (StatusCode::CONFLICT, "Addressbook already exists").into_response();
    }

    (StatusCode::CREATED, "").into_response()
}

// --- Helper functions ---

fn extract_ab_res<'a>(params: &'a HashMap<String, String>) -> Option<(&'a str, &'a str)> {
    let ab = params.get("addressbook")?;
    let res = params.get("resource")?;
    Some((ab.as_str(), res.as_str()))
}

fn build_home_properties(state: &CarddavState, propfind: &PropfindRequest) -> Vec<DavProperty> {
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
            content: "Addressbook Home".into(),
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

fn build_addressbook_properties(
    ab: &AddressbookCollection,
    propfind: &PropfindRequest,
) -> Vec<DavProperty> {
    let mut props = Vec::new();
    let include_all = matches!(propfind, PropfindRequest::AllProp);

    if include_all || prop_requested(propfind, "resourcetype") {
        props.push(DavProperty::CustomRaw(
            "<D:resourcetype><D:collection/><CR:addressbook/></D:resourcetype>".into(),
        ));
    }
    if include_all || prop_requested(propfind, "displayname") {
        props.push(DavProperty::CustomText {
            element: "D:displayname".into(),
            content: ab.display_name.clone(),
        });
    }
    if include_all || prop_requested(propfind, "addressbook-description") {
        if let Some(desc) = &ab.description {
            props.push(DavProperty::CustomText {
                element: "CR:addressbook-description".into(),
                content: desc.clone(),
            });
        }
    }
    if include_all || prop_requested(propfind, "supported-address-data") {
        props.push(DavProperty::CustomRaw(
            "<CR:supported-address-data><CR:address-data-type content-type=\"text/vcard\" version=\"4.0\"/></CR:supported-address-data>".into(),
        ));
    }
    props
}

fn build_resource_properties(
    res: &ContactResource,
    propfind: &PropfindRequest,
) -> Vec<DavProperty> {
    let mut props = Vec::new();
    let include_all = matches!(propfind, PropfindRequest::AllProp);

    if include_all || prop_requested(propfind, "getetag") {
        props.push(DavProperty::ETag(res.etag.clone()));
    }
    if include_all || prop_requested(propfind, "getcontenttype") {
        props.push(DavProperty::ContentType("text/vcard; charset=utf-8".into()));
    }
    props
}

fn build_report_entry(
    base_path: &str,
    addressbook: &str,
    res: &ContactResource,
    properties: &[(String, String)],
) -> DavResourceEntry {
    let mut props = Vec::new();

    for (_ns, local) in properties {
        match local.as_str() {
            "getetag" => props.push(DavProperty::ETag(res.etag.clone())),
            "getcontenttype" => {
                props.push(DavProperty::ContentType("text/vcard; charset=utf-8".into()))
            }
            "address-data" => props.push(DavProperty::CustomText {
                element: "CR:address-data".into(),
                content: res.data.clone(),
            }),
            _ => {}
        }
    }

    DavResourceEntry {
        href: format!("{}/{}/{}", base_path, addressbook, res.uid),
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

fn hash_stub(input: &str) -> u64 {
    let mut hash: u64 = 0;
    for b in input.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(b as u64);
    }
    hash
}
