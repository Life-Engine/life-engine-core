//! vCard serialisation: CDM `Contact` to/from vCard 4.0 format.
//!
//! Used by the CardDAV server to serve contacts in the standard vCard
//! format and to parse incoming PUT requests from contacts clients.

use chrono::Utc;
use dav_utils::text::{decode_escaped_value, non_empty, unfold_lines};
use life_engine_types::{Contact, ContactAddress, ContactEmail, ContactInfoType, ContactName, ContactPhone, PhoneType};
use uuid::Uuid;

/// Convert a `ContactInfoType` to a vCard TYPE parameter string.
fn contact_info_type_to_vcard(t: &ContactInfoType) -> &str {
    match t {
        ContactInfoType::Home => "HOME",
        ContactInfoType::Work => "WORK",
        ContactInfoType::Other => "OTHER",
    }
}

/// Convert a `PhoneType` to a vCard TYPE parameter string.
fn phone_type_to_vcard(t: &PhoneType) -> &str {
    match t {
        PhoneType::Mobile => "CELL",
        PhoneType::Home => "HOME",
        PhoneType::Work => "WORK",
        PhoneType::Fax => "FAX",
        PhoneType::Other => "OTHER",
    }
}

/// Parse a vCard type string into a `ContactInfoType`.
fn parse_contact_info_type(s: &str) -> ContactInfoType {
    match s {
        "home" => ContactInfoType::Home,
        "work" => ContactInfoType::Work,
        _ => ContactInfoType::Other,
    }
}

/// Parse a vCard type string into a `PhoneType`.
fn parse_phone_type(s: &str) -> PhoneType {
    match s {
        "mobile" | "cell" => PhoneType::Mobile,
        "home" => PhoneType::Home,
        "work" => PhoneType::Work,
        "fax" => PhoneType::Fax,
        _ => PhoneType::Other,
    }
}

/// Build a display name from ContactName parts.
fn display_name(name: &ContactName) -> String {
    let mut parts = Vec::new();
    if let Some(ref p) = name.prefix {
        parts.push(p.as_str());
    }
    parts.push(&name.given);
    if let Some(ref m) = name.middle {
        parts.push(m.as_str());
    }
    parts.push(&name.family);
    if let Some(ref s) = name.suffix {
        parts.push(s.as_str());
    }
    parts.join(" ")
}

/// Serialise a CDM `Contact` to vCard 4.0 format.
///
/// Produces a complete vCard document suitable for serving via CardDAV
/// GET responses.
pub fn contact_to_vcard(contact: &Contact) -> String {
    let mut lines = vec![
        "BEGIN:VCARD".to_string(),
        "VERSION:4.0".to_string(),
        format!("UID:{}", contact.source_id),
        format!("FN:{}", escape_vcard_value(&display_name(&contact.name))),
        format!(
            "N:{};{};{};{};{}",
            escape_vcard_value(&contact.name.family),
            escape_vcard_value(&contact.name.given),
            escape_vcard_value(contact.name.middle.as_deref().unwrap_or("")),
            escape_vcard_value(contact.name.prefix.as_deref().unwrap_or("")),
            escape_vcard_value(contact.name.suffix.as_deref().unwrap_or("")),
        ),
    ];

    for email in &contact.emails {
        let mut prop = "EMAIL".to_string();
        if let Some(ref t) = email.email_type {
            prop.push_str(&format!(";TYPE={}", contact_info_type_to_vcard(t)));
        }
        if email.primary == Some(true) {
            prop.push_str(";PREF=1");
        }
        lines.push(format!("{prop}:{}", email.address));
    }

    for phone in &contact.phones {
        let mut prop = "TEL".to_string();
        if let Some(ref t) = phone.phone_type {
            prop.push_str(&format!(";TYPE={}", phone_type_to_vcard(t)));
        }
        lines.push(format!("{prop}:{}", phone.number));
    }

    for addr in &contact.addresses {
        let street = addr.street.as_deref().unwrap_or("");
        let city = addr.city.as_deref().unwrap_or("");
        let region = addr.region.as_deref().unwrap_or("");
        let postal_code = addr.postal_code.as_deref().unwrap_or("");
        let country = addr.country.as_deref().unwrap_or("");
        lines.push(format!("ADR:;;{street};{city};{region};{postal_code};{country}"));
    }

    if let Some(ref org) = contact.organization {
        lines.push(format!("ORG:{}", escape_vcard_value(org)));
    }

    let rev = contact.updated_at.format("%Y%m%dT%H%M%SZ").to_string();
    lines.push(format!("REV:{rev}"));

    lines.push("END:VCARD".to_string());

    // RFC 6350 §3.2: Lines longer than 75 octets SHOULD be folded with
    // CRLF followed by a single space.
    lines
        .iter()
        .map(|line| fold_line(line))
        .collect::<Vec<_>>()
        .join("\r\n")
}

/// Fold a content line per RFC 6350 §3.2.
///
/// Lines longer than 75 octets are split: the first chunk is 75 octets,
/// continuation chunks are 74 octets (the leading space counts as one).
/// Fold points never split multi-byte UTF-8 characters.
fn fold_line(line: &str) -> String {
    if line.len() <= 75 {
        return line.to_string();
    }

    let mut result = String::with_capacity(line.len() + line.len() / 74 * 3);
    let mut chunk_start = 0;
    let mut chunk_byte_len = 0;
    let mut first = true;

    for (idx, ch) in line.char_indices() {
        let char_len = ch.len_utf8();
        let limit = if first { 75 } else { 74 };

        if chunk_byte_len + char_len > limit {
            if !first {
                result.push_str("\r\n ");
            }
            result.push_str(&line[chunk_start..idx]);
            chunk_start = idx;
            chunk_byte_len = 0;
            first = false;
        }
        chunk_byte_len += char_len;
    }

    if chunk_start < line.len() {
        if !first {
            result.push_str("\r\n ");
        }
        result.push_str(&line[chunk_start..]);
    }

    result
}

/// Parse a vCard string into a CDM `Contact`.
///
/// Used to process PUT requests from CardDAV clients creating or
/// updating contacts.
pub fn vcard_to_contact(vcard_data: &str) -> anyhow::Result<Contact> {
    let unfolded = unfold_lines(vcard_data);
    let lines: Vec<&str> = unfolded.lines().collect();

    if !lines
        .iter()
        .any(|l| l.trim().eq_ignore_ascii_case("BEGIN:VCARD"))
        || !lines
            .iter()
            .any(|l| l.trim().eq_ignore_ascii_case("END:VCARD"))
    {
        return Err(anyhow::anyhow!(
            "invalid vCard: missing BEGIN:VCARD or END:VCARD"
        ));
    }

    let mut given = String::new();
    let mut family = String::new();
    let mut middle = None;
    let mut prefix = None;
    let mut suffix = None;
    let mut emails = Vec::new();
    let mut phones = Vec::new();
    let mut addresses = Vec::new();
    let mut organization = None;
    let mut source_id = String::new();

    for line in &lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let (prop_with_params, value) = match line.split_once(':') {
            Some((p, v)) => (p, v),
            None => continue,
        };

        let (prop_name, params) = parse_property_name(prop_with_params);

        match prop_name.to_uppercase().as_str() {
            "FN" => {
                // FN is used for display — we don't store it, but use it as
                // fallback for given/family if N is absent.
            }
            "N" => {
                let parts: Vec<&str> = value.split(';').collect();
                family = decode_escaped_value(parts.first().unwrap_or(&""));
                given = decode_escaped_value(parts.get(1).unwrap_or(&""));
                let mid = decode_escaped_value(parts.get(2).unwrap_or(&""));
                if !mid.is_empty() {
                    middle = Some(mid);
                }
                let pfx = decode_escaped_value(parts.get(3).unwrap_or(&""));
                if !pfx.is_empty() {
                    prefix = Some(pfx);
                }
                let sfx = decode_escaped_value(parts.get(4).unwrap_or(&""));
                if !sfx.is_empty() {
                    suffix = Some(sfx);
                }
            }
            "EMAIL" => {
                let email_type = extract_type_param(&params).map(|s| parse_contact_info_type(&s));
                // Handle preference in both vCard formats:
                //   vCard 4.0: PREF=1
                //   vCard 3.0: TYPE=PREF  or  TYPE=home,pref  (comma-separated)
                let primary = params
                    .iter()
                    .any(|(k, v)| {
                        k.eq_ignore_ascii_case("PREF")
                            || (k.eq_ignore_ascii_case("TYPE")
                                && v.split(',')
                                    .any(|part| part.trim().eq_ignore_ascii_case("pref")))
                    })
                    .then_some(true);
                emails.push(ContactEmail {
                    address: value.to_string(),
                    email_type,
                    primary,
                });
            }
            "TEL" => {
                let phone_type = extract_type_param(&params).map(|s| parse_phone_type(&s));
                phones.push(ContactPhone {
                    number: value.to_string(),
                    phone_type,
                    primary: None,
                });
            }
            "ADR" => {
                let parts: Vec<&str> = value.split(';').collect();
                let street = non_empty(parts.get(2).unwrap_or(&""));
                let city = non_empty(parts.get(3).unwrap_or(&""));
                let region = non_empty(parts.get(4).unwrap_or(&""));
                let postal_code = non_empty(parts.get(5).unwrap_or(&""));
                let country = non_empty(parts.get(6).unwrap_or(&""));

                if street.is_some()
                    || city.is_some()
                    || region.is_some()
                    || postal_code.is_some()
                    || country.is_some()
                {
                    addresses.push(ContactAddress {
                        street,
                        city,
                        region,
                        postal_code,
                        country,
                        address_type: None,
                    });
                }
            }
            "ORG" => {
                let org_value = value.split(';').next().unwrap_or("");
                let decoded = decode_escaped_value(org_value);
                if !decoded.is_empty() {
                    organization = Some(decoded);
                }
            }
            "UID" => {
                source_id = value.to_string();
            }
            _ => {}
        }
    }

    // If N was missing, try to parse given/family from FN
    if given.is_empty() && family.is_empty() {
        // Re-parse to get FN value
        let unfolded = unfold_lines(vcard_data);
        for line in unfolded.lines() {
            let line = line.trim();
            if let Some((prop, val)) = line.split_once(':') {
                let (name, _) = parse_property_name(prop);
                if name.eq_ignore_ascii_case("FN") {
                    let decoded = decode_escaped_value(val);
                    let parts: Vec<&str> = decoded.splitn(2, ' ').collect();
                    given = parts.first().unwrap_or(&"").to_string();
                    family = parts.get(1).unwrap_or(&"").to_string();
                    break;
                }
            }
        }
    }

    if given.is_empty() && family.is_empty() {
        given = "(unnamed)".into();
    }

    if source_id.is_empty() {
        source_id = Uuid::new_v4().to_string();
    }

    let now = Utc::now();

    Ok(Contact {
        id: Uuid::new_v4(),
        name: ContactName {
            given,
            family,
            display: None,
            prefix,
            suffix,
            middle,
        },
        emails,
        phones,
        addresses,
        organization,
        title: None,
        birthday: None,
        photo_url: None,
        notes: None,
        groups: vec![],
        source: "carddav-api".into(),
        source_id,
        extensions: None,
        created_at: now,
        updated_at: now,
    })
}

/// Escape special characters for vCard property values.
fn escape_vcard_value(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(',', "\\,")
        .replace(';', "\\;")
        .replace('\n', "\\n")
}

fn parse_property_name(prop_with_params: &str) -> (&str, Vec<(String, String)>) {
    let parts: Vec<&str> = prop_with_params.split(';').collect();
    let name = parts[0];
    let params = parts[1..]
        .iter()
        .map(|p| {
            if let Some((k, v)) = p.split_once('=') {
                (k.to_string(), v.to_string())
            } else {
                ("TYPE".to_string(), p.to_string())
            }
        })
        .collect();
    (name, params)
}

fn extract_type_param(params: &[(String, String)]) -> Option<String> {
    for (key, value) in params {
        if key.eq_ignore_ascii_case("TYPE") {
            let first_type = value.split(',').next().unwrap_or(value);
            return Some(first_type.to_lowercase());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample_contact() -> Contact {
        Contact {
            id: Uuid::new_v4(),
            name: ContactName {
                given: "Jane".into(),
                family: "Doe".into(),
                display: None,
                prefix: None,
                suffix: None,
                middle: None,
            },
            emails: vec![
                ContactEmail {
                    address: "jane@example.com".into(),
                    email_type: Some(ContactInfoType::Work),
                    primary: Some(true),
                },
                ContactEmail {
                    address: "jane.personal@example.com".into(),
                    email_type: Some(ContactInfoType::Home),
                    primary: None,
                },
            ],
            phones: vec![ContactPhone {
                number: "+1-555-0100".into(),
                phone_type: Some(PhoneType::Mobile),
                primary: None,
            }],
            addresses: vec![ContactAddress {
                street: Some("123 Main St".into()),
                city: Some("Springfield".into()),
                region: Some("IL".into()),
                postal_code: Some("62704".into()),
                country: Some("US".into()),
                address_type: None,
            }],
            organization: Some("Acme Corp".into()),
            title: None,
            birthday: None,
            photo_url: None,
            notes: None,
            groups: vec![],
            source: "local".into(),
            source_id: "contact-rt-001".into(),
            extensions: None,
            created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            updated_at: Utc.with_ymd_and_hms(2026, 3, 15, 12, 0, 0).unwrap(),
        }
    }

    // --- Serialisation (CDM -> vCard) ---

    #[test]
    fn contact_to_vcard_contains_wrapper() {
        let vcard = contact_to_vcard(&sample_contact());
        assert!(vcard.starts_with("BEGIN:VCARD"));
        assert!(vcard.contains("END:VCARD"));
        assert!(vcard.contains("VERSION:4.0"));
    }

    #[test]
    fn contact_to_vcard_contains_name_properties() {
        let vcard = contact_to_vcard(&sample_contact());
        assert!(vcard.contains("FN:Jane Doe"));
        assert!(vcard.contains("N:Doe;Jane;;;"));
        assert!(vcard.contains("UID:contact-rt-001"));
    }

    #[test]
    fn contact_to_vcard_includes_emails() {
        let vcard = contact_to_vcard(&sample_contact());
        assert!(vcard.contains("EMAIL;TYPE=WORK;PREF=1:jane@example.com"));
        assert!(vcard.contains("EMAIL;TYPE=HOME:jane.personal@example.com"));
    }

    #[test]
    fn contact_to_vcard_includes_phone() {
        let vcard = contact_to_vcard(&sample_contact());
        assert!(vcard.contains("TEL;TYPE=CELL:+1-555-0100"));
    }

    #[test]
    fn contact_to_vcard_includes_address() {
        let vcard = contact_to_vcard(&sample_contact());
        assert!(vcard.contains("ADR:;;123 Main St;Springfield;IL;62704;US"));
    }

    #[test]
    fn contact_to_vcard_includes_org() {
        let vcard = contact_to_vcard(&sample_contact());
        assert!(vcard.contains("ORG:Acme Corp"));
    }

    #[test]
    fn contact_to_vcard_includes_rev() {
        let vcard = contact_to_vcard(&sample_contact());
        assert!(vcard.contains("REV:20260315T120000Z"));
    }

    #[test]
    fn contact_to_vcard_minimal() {
        let contact = Contact {
            id: Uuid::new_v4(),
            name: ContactName {
                given: "Min".into(),
                family: "Contact".into(),
                display: None,
                prefix: None,
                suffix: None,
                middle: None,
            },
            emails: vec![],
            phones: vec![],
            addresses: vec![],
            organization: None,
            title: None,
            birthday: None,
            photo_url: None,
            notes: None,
            groups: vec![],
            source: "local".into(),
            source_id: "min-001".into(),
            extensions: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let vcard = contact_to_vcard(&contact);
        assert!(vcard.contains("FN:Min Contact"));
        assert!(!vcard.contains("EMAIL"));
        assert!(!vcard.contains("TEL"));
        assert!(!vcard.contains("ADR"));
        assert!(!vcard.contains("ORG"));
    }

    #[test]
    fn contact_to_vcard_uses_crlf() {
        let vcard = contact_to_vcard(&sample_contact());
        assert!(vcard.contains("\r\n"));
    }

    #[test]
    fn contact_to_vcard_escapes_special_chars() {
        let contact = Contact {
            id: Uuid::new_v4(),
            name: ContactName {
                given: "Test".into(),
                family: "User".into(),
                display: None,
                prefix: None,
                suffix: Some("Jr.".into()),
                middle: None,
            },
            emails: vec![],
            phones: vec![],
            addresses: vec![],
            organization: Some("Org; Inc.".into()),
            title: None,
            birthday: None,
            photo_url: None,
            notes: None,
            groups: vec![],
            source: "local".into(),
            source_id: "escape-001".into(),
            extensions: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let vcard = contact_to_vcard(&contact);
        assert!(vcard.contains("FN:Test User Jr."));
        assert!(vcard.contains("ORG:Org\\; Inc."));
    }

    // --- Deserialisation (vCard -> CDM) ---

    #[test]
    fn vcard_to_contact_parses_simple() {
        let vcard = "\
BEGIN:VCARD\r\n\
VERSION:4.0\r\n\
UID:parse-001\r\n\
FN:Parsed Contact\r\n\
N:Contact;Parsed;;;\r\n\
EMAIL:parsed@example.com\r\n\
END:VCARD\r\n";

        let contact = vcard_to_contact(vcard).expect("should parse");
        assert_eq!(contact.name.given, "Parsed");
        assert_eq!(contact.name.family, "Contact");
        assert_eq!(contact.source_id, "parse-001");
        assert_eq!(contact.source, "carddav-api");
        assert_eq!(contact.emails.len(), 1);
    }

    #[test]
    fn vcard_to_contact_parses_full() {
        let vcard = "\
BEGIN:VCARD\r\n\
VERSION:4.0\r\n\
UID:full-001\r\n\
FN:Full Contact\r\n\
N:Contact;Full;;;\r\n\
EMAIL;TYPE=WORK;PREF=1:work@example.com\r\n\
EMAIL;TYPE=HOME:home@example.com\r\n\
TEL;TYPE=CELL:+1-555-0100\r\n\
ADR:;;123 Main St;Springfield;IL;62704;US\r\n\
ORG:Acme Corp\r\n\
END:VCARD\r\n";

        let contact = vcard_to_contact(vcard).expect("should parse");
        assert_eq!(contact.name.given, "Full");
        assert_eq!(contact.name.family, "Contact");
        assert_eq!(contact.emails.len(), 2);
        assert_eq!(contact.phones.len(), 1);
        assert_eq!(contact.addresses.len(), 1);
        assert_eq!(contact.organization.as_deref(), Some("Acme Corp"));
    }

    #[test]
    fn vcard_to_contact_errors_on_invalid() {
        let result = vcard_to_contact("not a vcard");
        assert!(result.is_err());
    }

    #[test]
    fn vcard_to_contact_errors_on_empty() {
        let result = vcard_to_contact("");
        assert!(result.is_err());
    }

    // --- Round-trip ---

    #[test]
    fn round_trip_serialisation() {
        let original = sample_contact();
        let vcard = contact_to_vcard(&original);
        let restored = vcard_to_contact(&vcard).expect("should round-trip");

        assert_eq!(restored.name.given, original.name.given);
        assert_eq!(restored.name.family, original.name.family);
        assert_eq!(restored.source_id, original.source_id);
        assert_eq!(restored.emails.len(), original.emails.len());
        assert_eq!(restored.emails[0].address, original.emails[0].address);
        assert_eq!(restored.phones.len(), original.phones.len());
        assert_eq!(restored.phones[0].number, original.phones[0].number);
        assert_eq!(restored.addresses.len(), original.addresses.len());
        assert_eq!(restored.addresses[0].street, original.addresses[0].street);
        assert_eq!(restored.addresses[0].city, original.addresses[0].city);
        assert_eq!(restored.organization, original.organization);
    }

    #[test]
    fn round_trip_minimal_contact() {
        let contact = Contact {
            id: Uuid::new_v4(),
            name: ContactName {
                given: "Solo".into(),
                family: "Person".into(),
                display: None,
                prefix: None,
                suffix: None,
                middle: None,
            },
            emails: vec![],
            phones: vec![],
            addresses: vec![],
            organization: None,
            title: None,
            birthday: None,
            photo_url: None,
            notes: None,
            groups: vec![],
            source: "local".into(),
            source_id: "solo-001".into(),
            extensions: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let vcard = contact_to_vcard(&contact);
        let restored = vcard_to_contact(&vcard).expect("should round-trip");

        assert_eq!(restored.name.given, "Solo");
        assert_eq!(restored.name.family, "Person");
        assert_eq!(restored.source_id, "solo-001");
        assert!(restored.emails.is_empty());
        assert!(restored.phones.is_empty());
        assert!(restored.addresses.is_empty());
        assert!(restored.organization.is_none());
    }

    #[test]
    fn fold_line_preserves_multibyte_utf8() {
        // CJK characters are 3 bytes each — common in contact names
        let cjk = "田中太郎".repeat(8); // 96 bytes, 32 chars
        let line = format!("FN:{cjk}");
        let folded = fold_line(&line);

        assert!(
            !folded.contains('\u{FFFD}'),
            "fold_line must not split multi-byte characters"
        );

        let unfolded: String = folded.replace("\r\n ", "");
        assert_eq!(unfolded, line);
    }

    #[test]
    fn fold_line_preserves_emoji() {
        let emoji_line = format!("NOTE:{}", "🎉".repeat(20)); // 5 + 80 = 85 bytes
        let folded = fold_line(&emoji_line);

        assert!(
            !folded.contains('\u{FFFD}'),
            "fold_line must not split emoji characters"
        );

        let unfolded: String = folded.replace("\r\n ", "");
        assert_eq!(unfolded, emoji_line);
    }

    #[test]
    fn fold_line_preserves_accented_names() {
        // European accented names are extremely common in contact data
        let name = "José María García López José María García López José María García López";
        let line = format!("FN:{name}");
        let folded = fold_line(&line);

        assert!(
            !folded.contains('\u{FFFD}'),
            "fold_line must not split accented characters"
        );

        let unfolded: String = folded.replace("\r\n ", "");
        assert_eq!(unfolded, line);
    }
}
