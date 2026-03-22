//! vCard serialisation: CDM `Contact` to/from vCard 4.0 format.
//!
//! Used by the CardDAV server to serve contacts in the standard vCard
//! format and to parse incoming PUT requests from contacts clients.

use chrono::Utc;
use dav_utils::text::{decode_escaped_value, non_empty, unfold_lines};
use life_engine_types::{Contact, ContactName, EmailAddress, PhoneNumber, PostalAddress};
use uuid::Uuid;

/// Serialise a CDM `Contact` to vCard 4.0 format.
///
/// Produces a complete vCard document suitable for serving via CardDAV
/// GET responses.
pub fn contact_to_vcard(contact: &Contact) -> String {
    let mut lines = vec![
        "BEGIN:VCARD".to_string(),
        "VERSION:4.0".to_string(),
        format!("UID:{}", contact.source_id),
        format!("FN:{}", escape_vcard_value(&contact.name.display)),
        format!(
            "N:{};{};;;",
            escape_vcard_value(&contact.name.family),
            escape_vcard_value(&contact.name.given)
        ),
    ];

    for email in &contact.emails {
        let mut prop = "EMAIL".to_string();
        if let Some(ref t) = email.email_type {
            prop.push_str(&format!(";TYPE={}", t.to_uppercase()));
        }
        if email.primary == Some(true) {
            prop.push_str(";PREF=1");
        }
        lines.push(format!("{prop}:{}", email.address));
    }

    for phone in &contact.phones {
        let mut prop = "TEL".to_string();
        if let Some(ref t) = phone.phone_type {
            prop.push_str(&format!(";TYPE={}", t.to_uppercase()));
        }
        lines.push(format!("{prop}:{}", phone.number));
    }

    for addr in &contact.addresses {
        let street = addr.street.as_deref().unwrap_or("");
        let city = addr.city.as_deref().unwrap_or("");
        let state = addr.state.as_deref().unwrap_or("");
        let postcode = addr.postcode.as_deref().unwrap_or("");
        let country = addr.country.as_deref().unwrap_or("");
        lines.push(format!("ADR:;;{street};{city};{state};{postcode};{country}"));
    }

    if let Some(ref org) = contact.organisation {
        lines.push(format!("ORG:{}", escape_vcard_value(org)));
    }

    let rev = contact.updated_at.format("%Y%m%dT%H%M%SZ").to_string();
    lines.push(format!("REV:{rev}"));

    lines.push("END:VCARD".to_string());
    lines.join("\r\n")
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
    let mut display = String::new();
    let mut emails = Vec::new();
    let mut phones = Vec::new();
    let mut addresses = Vec::new();
    let mut organisation = None;
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
                display = decode_escaped_value(value);
            }
            "N" => {
                let parts: Vec<&str> = value.split(';').collect();
                family = decode_escaped_value(parts.first().unwrap_or(&""));
                given = decode_escaped_value(parts.get(1).unwrap_or(&""));
            }
            "EMAIL" => {
                let email_type = extract_type_param(&params);
                let primary = params
                    .iter()
                    .any(|(k, _)| k.eq_ignore_ascii_case("PREF"))
                    .then_some(true);
                emails.push(EmailAddress {
                    address: value.to_string(),
                    email_type,
                    primary,
                });
            }
            "TEL" => {
                let phone_type = extract_type_param(&params);
                phones.push(PhoneNumber {
                    number: value.to_string(),
                    phone_type,
                });
            }
            "ADR" => {
                let parts: Vec<&str> = value.split(';').collect();
                let street = non_empty(parts.get(2).unwrap_or(&""));
                let city = non_empty(parts.get(3).unwrap_or(&""));
                let state = non_empty(parts.get(4).unwrap_or(&""));
                let postcode = non_empty(parts.get(5).unwrap_or(&""));
                let country = non_empty(parts.get(6).unwrap_or(&""));

                if street.is_some()
                    || city.is_some()
                    || state.is_some()
                    || postcode.is_some()
                    || country.is_some()
                {
                    addresses.push(PostalAddress {
                        street,
                        city,
                        state,
                        postcode,
                        country,
                    });
                }
            }
            "ORG" => {
                let org_value = value.split(';').next().unwrap_or("");
                let decoded = decode_escaped_value(org_value);
                if !decoded.is_empty() {
                    organisation = Some(decoded);
                }
            }
            "UID" => {
                source_id = value.to_string();
            }
            _ => {}
        }
    }

    if display.is_empty() {
        display = format!("{given} {family}").trim().to_string();
    }
    if display.is_empty() {
        display = "(unnamed)".into();
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
            display,
        },
        emails,
        phones,
        addresses,
        organisation,
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
                display: "Jane Doe".into(),
            },
            emails: vec![
                EmailAddress {
                    address: "jane@example.com".into(),
                    email_type: Some("work".into()),
                    primary: Some(true),
                },
                EmailAddress {
                    address: "jane.personal@example.com".into(),
                    email_type: Some("home".into()),
                    primary: None,
                },
            ],
            phones: vec![PhoneNumber {
                number: "+1-555-0100".into(),
                phone_type: Some("cell".into()),
            }],
            addresses: vec![PostalAddress {
                street: Some("123 Main St".into()),
                city: Some("Springfield".into()),
                state: Some("IL".into()),
                postcode: Some("62704".into()),
                country: Some("US".into()),
            }],
            organisation: Some("Acme Corp".into()),
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
                display: "Min Contact".into(),
            },
            emails: vec![],
            phones: vec![],
            addresses: vec![],
            organisation: None,
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
                display: "Test, User; Jr.".into(),
            },
            emails: vec![],
            phones: vec![],
            addresses: vec![],
            organisation: Some("Org; Inc.".into()),
            source: "local".into(),
            source_id: "escape-001".into(),
            extensions: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let vcard = contact_to_vcard(&contact);
        assert!(vcard.contains("FN:Test\\, User\\; Jr."));
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
        assert_eq!(contact.name.display, "Parsed Contact");
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
        assert_eq!(contact.name.display, "Full Contact");
        assert_eq!(contact.emails.len(), 2);
        assert_eq!(contact.phones.len(), 1);
        assert_eq!(contact.addresses.len(), 1);
        assert_eq!(contact.organisation.as_deref(), Some("Acme Corp"));
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

        assert_eq!(restored.name.display, original.name.display);
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
        assert_eq!(restored.organisation, original.organisation);
    }

    #[test]
    fn round_trip_minimal_contact() {
        let contact = Contact {
            id: Uuid::new_v4(),
            name: ContactName {
                given: "Solo".into(),
                family: "Person".into(),
                display: "Solo Person".into(),
            },
            emails: vec![],
            phones: vec![],
            addresses: vec![],
            organisation: None,
            source: "local".into(),
            source_id: "solo-001".into(),
            extensions: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let vcard = contact_to_vcard(&contact);
        let restored = vcard_to_contact(&vcard).expect("should round-trip");

        assert_eq!(restored.name.display, "Solo Person");
        assert_eq!(restored.source_id, "solo-001");
        assert!(restored.emails.is_empty());
        assert!(restored.phones.is_empty());
        assert!(restored.addresses.is_empty());
        assert!(restored.organisation.is_none());
    }
}
