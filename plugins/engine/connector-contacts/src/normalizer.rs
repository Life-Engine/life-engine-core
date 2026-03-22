//! vCard normalizer: converts raw vCard text to CDM `Contact` type.
//!
//! Parses vCard 3.0 and 4.0 formats using line-by-line parsing.
//! Handles folded lines, property parameters, and multi-value fields.

use chrono::Utc;
use dav_utils::text::{decode_escaped_value as decode_value, non_empty, unfold_lines};
use life_engine_types::{Contact, ContactAddress, ContactEmail, ContactInfoType, ContactName, ContactPhone, PhoneType};
use uuid::Uuid;

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

/// Normalize a raw vCard string into the Life Engine CDM `Contact` type.
///
/// `raw` is the full vCard text (may contain folded lines).
/// `source` identifies the connector that produced this contact (e.g. "carddav").
pub fn normalize_vcard(raw: &str, source: &str) -> anyhow::Result<Contact> {
    let unfolded = unfold_lines(raw);
    let lines: Vec<&str> = unfolded.lines().collect();

    if !lines.iter().any(|l| l.trim().eq_ignore_ascii_case("BEGIN:VCARD"))
        || !lines.iter().any(|l| l.trim().eq_ignore_ascii_case("END:VCARD"))
    {
        return Err(anyhow::anyhow!("invalid vCard: missing BEGIN:VCARD or END:VCARD"));
    }

    let mut given = String::new();
    let mut family = String::new();
    let mut fn_value = String::new();
    let mut emails = Vec::new();
    let mut phones = Vec::new();
    let mut addresses = Vec::new();
    let mut organization = None;
    let mut source_id = String::new();
    let mut has_photo = false;

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
                fn_value = decode_value(value);
            }
            "N" => {
                // N:Family;Given;Additional;Prefix;Suffix
                let parts: Vec<&str> = value.split(';').collect();
                family = decode_value(parts.first().unwrap_or(&""));
                given = decode_value(parts.get(1).unwrap_or(&""));
            }
            "EMAIL" => {
                let email_type = extract_type_param(&params).map(|s| parse_contact_info_type(&s));
                // Check for PREF in three forms:
                //   vCard 3.0: TYPE=pref  or  TYPE=home,pref  (comma-separated)
                //   vCard 4.0: PREF=1
                let primary = params
                    .iter()
                    .any(|(k, v)| {
                        (k.eq_ignore_ascii_case("TYPE")
                            && v.split(',')
                                .any(|part| part.trim().eq_ignore_ascii_case("pref")))
                            || k.eq_ignore_ascii_case("PREF")
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
                // ADR:PO Box;Extended;Street;City;State;Postal Code;Country
                let parts: Vec<&str> = value.split(';').collect();
                let street = non_empty(parts.get(2).unwrap_or(&""));
                let city = non_empty(parts.get(3).unwrap_or(&""));
                let region = non_empty(parts.get(4).unwrap_or(&""));
                let postal_code = non_empty(parts.get(5).unwrap_or(&""));
                let country = non_empty(parts.get(6).unwrap_or(&""));

                // Only add if at least one field is populated
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
                let decoded = decode_value(org_value);
                if !decoded.is_empty() {
                    organization = Some(decoded);
                }
            }
            "UID" => {
                source_id = value.to_string();
            }
            "PHOTO" => {
                has_photo = true;
            }
            _ => {}
        }
    }

    // If no N property, try to parse from FN
    if given.is_empty() && family.is_empty() && !fn_value.is_empty() {
        let parts: Vec<&str> = fn_value.splitn(2, ' ').collect();
        given = parts.first().unwrap_or(&"").to_string();
        family = parts.get(1).unwrap_or(&"").to_string();
    }

    // If still empty, use a fallback
    if given.is_empty() && family.is_empty() {
        given = "(unnamed)".into();
    }

    // Generate source_id if not provided by UID
    if source_id.is_empty() {
        source_id = Uuid::new_v4().to_string();
    }

    let extensions = if has_photo {
        Some(serde_json::json!({"has_photo": true}))
    } else {
        None
    };

    let now = Utc::now();

    Ok(Contact {
        id: Uuid::new_v4(),
        name: ContactName {
            given,
            family,
            prefix: None,
            suffix: None,
            middle: None,
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
        source: source.into(),
        source_id,
        extensions,
        created_at: now,
        updated_at: now,
    })
}

/// Normalize multiple vCards from a single response body.
///
/// Splits on `BEGIN:VCARD` boundaries and normalizes each individually.
/// Returns successfully parsed contacts and skips any that fail.
pub fn normalize_vcards(raw: &str, source: &str) -> Vec<anyhow::Result<Contact>> {
    let mut results = Vec::new();
    let mut current = String::new();
    let mut in_vcard = false;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("BEGIN:VCARD") {
            in_vcard = true;
            current.clear();
            current.push_str(line);
            current.push('\n');
        } else if trimmed.eq_ignore_ascii_case("END:VCARD") {
            current.push_str(line);
            current.push('\n');
            in_vcard = false;
            results.push(normalize_vcard(&current, source));
        } else if in_vcard {
            current.push_str(line);
            current.push('\n');
        }
    }

    results
}

/// Parse a property name from its parameters.
///
/// e.g. `EMAIL;TYPE=work;PREF` becomes `("EMAIL", [("TYPE","work"), ("PREF","")])`
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

/// Extract the first TYPE parameter value, excluding "pref".
fn extract_type_param(params: &[(String, String)]) -> Option<String> {
    for (key, value) in params {
        if key.eq_ignore_ascii_case("TYPE") && !value.eq_ignore_ascii_case("pref") {
            // TYPE may contain comma-separated values like "work,voice"
            let first_type = value.split(',').next().unwrap_or(value);
            return Some(first_type.to_lowercase());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_contact_normalization() {
        let vcard = "\
BEGIN:VCARD
VERSION:4.0
FN:Jane Doe
N:Doe;Jane;;;
EMAIL:jane@example.com
UID:contact-001
END:VCARD";

        let contact = normalize_vcard(vcard, "carddav").expect("should parse");
        assert_eq!(contact.name.given, "Jane");
        assert_eq!(contact.name.family, "Doe");
        assert_eq!(contact.emails.len(), 1);
        assert_eq!(contact.emails[0].address, "jane@example.com");
        assert_eq!(contact.source, "carddav");
        assert_eq!(contact.source_id, "contact-001");
    }

    #[test]
    fn vcard_with_phone_numbers() {
        let vcard = "\
BEGIN:VCARD
VERSION:4.0
FN:John Smith
N:Smith;John;;;
TEL;TYPE=work:+1-555-0100
TEL;TYPE=home:+1-555-0200
TEL;TYPE=cell:+1-555-0300
UID:contact-002
END:VCARD";

        let contact = normalize_vcard(vcard, "carddav").expect("should parse");
        assert_eq!(contact.phones.len(), 3);
        assert_eq!(contact.phones[0].number, "+1-555-0100");
        assert_eq!(contact.phones[0].phone_type, Some(PhoneType::Work));
        assert_eq!(contact.phones[1].number, "+1-555-0200");
        assert_eq!(contact.phones[1].phone_type, Some(PhoneType::Home));
        assert_eq!(contact.phones[2].number, "+1-555-0300");
        assert_eq!(contact.phones[2].phone_type, Some(PhoneType::Mobile));
    }

    #[test]
    fn vcard_with_addresses() {
        let vcard = "\
BEGIN:VCARD
VERSION:4.0
FN:Alice Worker
N:Worker;Alice;;;
ADR;TYPE=work:;;123 Business Ave;Metropolis;NY;10001;US
ADR;TYPE=home:;;456 Home St;Suburbia;CA;90210;US
UID:contact-003
END:VCARD";

        let contact = normalize_vcard(vcard, "carddav").expect("should parse");
        assert_eq!(contact.addresses.len(), 2);

        let work = &contact.addresses[0];
        assert_eq!(work.street.as_deref(), Some("123 Business Ave"));
        assert_eq!(work.city.as_deref(), Some("Metropolis"));
        assert_eq!(work.region.as_deref(), Some("NY"));
        assert_eq!(work.postal_code.as_deref(), Some("10001"));
        assert_eq!(work.country.as_deref(), Some("US"));

        let home = &contact.addresses[1];
        assert_eq!(home.street.as_deref(), Some("456 Home St"));
        assert_eq!(home.city.as_deref(), Some("Suburbia"));
    }

    #[test]
    fn vcard_with_organisation() {
        let vcard = "\
BEGIN:VCARD
VERSION:4.0
FN:Bob Builder
N:Builder;Bob;;;
ORG:Acme Corp;Engineering
UID:contact-004
END:VCARD";

        let contact = normalize_vcard(vcard, "carddav").expect("should parse");
        assert_eq!(contact.organization.as_deref(), Some("Acme Corp"));
    }

    #[test]
    fn vcard_with_multiple_emails() {
        let vcard = "\
BEGIN:VCARD
VERSION:4.0
FN:Multi Email
N:Email;Multi;;;
EMAIL;TYPE=work:work@example.com
EMAIL;TYPE=home:home@example.com
EMAIL;TYPE=work,pref:primary@example.com
UID:contact-005
END:VCARD";

        let contact = normalize_vcard(vcard, "carddav").expect("should parse");
        assert_eq!(contact.emails.len(), 3);
        assert_eq!(contact.emails[0].address, "work@example.com");
        assert_eq!(contact.emails[0].email_type, Some(ContactInfoType::Work));
        assert_eq!(contact.emails[1].address, "home@example.com");
        assert_eq!(contact.emails[1].email_type, Some(ContactInfoType::Home));
        assert_eq!(contact.emails[2].address, "primary@example.com");
    }

    #[test]
    fn vcard_v3_format() {
        let vcard = "\
BEGIN:VCARD
VERSION:3.0
FN:Version Three
N:Three;Version;;;
EMAIL;TYPE=internet:v3@example.com
TEL;TYPE=CELL:+1-555-0333
UID:contact-v3
END:VCARD";

        let contact = normalize_vcard(vcard, "carddav").expect("should parse");
        assert_eq!(contact.name.given, "Version");
        assert_eq!(contact.name.family, "Three");
        assert_eq!(contact.emails.len(), 1);
        assert_eq!(contact.emails[0].email_type, Some(ContactInfoType::Other));
        assert_eq!(contact.phones.len(), 1);
        assert_eq!(contact.phones[0].phone_type, Some(PhoneType::Mobile));
    }

    #[test]
    fn vcard_v4_format() {
        let vcard = "\
BEGIN:VCARD
VERSION:4.0
FN:Version Four
N:Four;Version;;;
EMAIL;TYPE=work:v4@example.com
TEL;VALUE=uri;TYPE=cell:tel:+1-555-0444
UID:contact-v4
END:VCARD";

        let contact = normalize_vcard(vcard, "carddav").expect("should parse");
        assert_eq!(contact.name.given, "Version");
        assert_eq!(contact.name.family, "Four");
        assert_eq!(contact.emails.len(), 1);
        assert_eq!(contact.phones.len(), 1);
    }

    #[test]
    fn vcard_with_folded_lines() {
        let vcard = "\
BEGIN:VCARD
VERSION:4.0
FN:Long Name That Is Very Long Indeed And Needs To Be
 Folded Across Lines
N:Folded;Long;;;
EMAIL:long@example.com
UID:contact-folded
END:VCARD";

        let contact = normalize_vcard(vcard, "carddav").expect("should parse");
        assert_eq!(contact.name.given, "Long");
        assert_eq!(contact.name.family, "Folded");
    }

    #[test]
    fn vcard_with_tab_folding() {
        let vcard = "BEGIN:VCARD\nVERSION:4.0\nFN:Tab\n\tFolded\nN:Folded;Tab;;;\nUID:tab-fold\nEND:VCARD";

        let contact = normalize_vcard(vcard, "carddav").expect("should parse");
        assert_eq!(contact.name.given, "Tab");
        assert_eq!(contact.name.family, "Folded");
    }

    #[test]
    fn vcard_with_missing_optional_fields() {
        let vcard = "\
BEGIN:VCARD
VERSION:4.0
FN:Minimal Contact
N:Contact;Minimal;;;
UID:contact-minimal
END:VCARD";

        let contact = normalize_vcard(vcard, "carddav").expect("should parse");
        assert_eq!(contact.name.given, "Minimal");
        assert_eq!(contact.name.family, "Contact");
        assert!(contact.emails.is_empty());
        assert!(contact.phones.is_empty());
        assert!(contact.addresses.is_empty());
        assert!(contact.organization.is_none());
        assert!(contact.extensions.is_none());
    }

    #[test]
    fn vcard_with_utf8_values() {
        let vcard = "\
BEGIN:VCARD
VERSION:4.0
FN:Rene Muller
N:Muller;Rene;;;
ORG:Cafe Zurich
UID:contact-utf8
END:VCARD";

        let contact = normalize_vcard(vcard, "carddav").expect("should parse");
        assert_eq!(contact.name.given, "Rene");
        assert_eq!(contact.name.family, "Muller");
        assert_eq!(contact.organization.as_deref(), Some("Cafe Zurich"));
    }

    #[test]
    fn vcard_with_escaped_values() {
        let vcard = "\
BEGIN:VCARD
VERSION:4.0
FN:Escaped\\, Name
N:Name;Escaped\\,;;;
UID:contact-escaped
END:VCARD";

        let contact = normalize_vcard(vcard, "carddav").expect("should parse");
        assert_eq!(contact.name.given, "Escaped,");
        assert_eq!(contact.name.family, "Name");
    }

    #[test]
    fn multiple_vcards_from_response() {
        let raw = "\
BEGIN:VCARD
VERSION:4.0
FN:Contact One
N:One;Contact;;;
UID:multi-001
END:VCARD
BEGIN:VCARD
VERSION:4.0
FN:Contact Two
N:Two;Contact;;;
UID:multi-002
END:VCARD
BEGIN:VCARD
VERSION:4.0
FN:Contact Three
N:Three;Contact;;;
UID:multi-003
END:VCARD";

        let results = normalize_vcards(raw, "carddav");
        assert_eq!(results.len(), 3);

        let contacts: Vec<Contact> = results
            .into_iter()
            .map(|r| r.expect("should parse"))
            .collect();
        assert_eq!(contacts[0].name.given, "Contact");
        assert_eq!(contacts[0].name.family, "One");
        assert_eq!(contacts[1].name.given, "Contact");
        assert_eq!(contacts[1].name.family, "Two");
        assert_eq!(contacts[2].name.given, "Contact");
        assert_eq!(contacts[2].name.family, "Three");
    }

    #[test]
    fn invalid_vcard_returns_error() {
        let result = normalize_vcard("not a vcard at all", "carddav");
        assert!(result.is_err());
    }

    #[test]
    fn empty_string_returns_error() {
        let result = normalize_vcard("", "carddav");
        assert!(result.is_err());
    }

    #[test]
    fn vcard_with_photo_sets_extension() {
        let vcard = "\
BEGIN:VCARD
VERSION:4.0
FN:Photo Person
N:Person;Photo;;;
PHOTO;VALUE=uri:https://example.com/photo.jpg
UID:contact-photo
END:VCARD";

        let contact = normalize_vcard(vcard, "carddav").expect("should parse");
        assert!(contact.extensions.is_some());
        let ext = contact.extensions.unwrap();
        assert_eq!(ext["has_photo"], true);
    }

    #[test]
    fn vcard_fn_fallback_from_n() {
        let vcard = "\
BEGIN:VCARD
VERSION:4.0
N:Doe;Jane;;;
UID:contact-no-fn
END:VCARD";

        let contact = normalize_vcard(vcard, "carddav").expect("should parse");
        assert_eq!(contact.name.given, "Jane");
        assert_eq!(contact.name.family, "Doe");
    }

    #[test]
    fn vcard_generates_source_id_when_no_uid() {
        let vcard = "\
BEGIN:VCARD
VERSION:4.0
FN:No UID Contact
N:Contact;No UID;;;
END:VCARD";

        let contact = normalize_vcard(vcard, "carddav").expect("should parse");
        assert!(!contact.source_id.is_empty());
    }

    #[test]
    fn normalized_contact_has_valid_uuid() {
        let vcard = "\
BEGIN:VCARD
VERSION:4.0
FN:UUID Test
N:Test;UUID;;;
UID:uuid-test
END:VCARD";

        let contact = normalize_vcard(vcard, "carddav").expect("should parse");
        assert!(!contact.id.is_nil());
    }

    #[test]
    fn normalized_contact_serializes_to_json() {
        let vcard = "\
BEGIN:VCARD
VERSION:4.0
FN:JSON Test
N:Test;JSON;;;
EMAIL:json@example.com
UID:json-test
END:VCARD";

        let contact = normalize_vcard(vcard, "carddav").expect("should parse");
        let json = serde_json::to_string(&contact).expect("should serialize");
        let restored: Contact = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(restored.name.given, contact.name.given);
        assert_eq!(restored.emails[0].address, contact.emails[0].address);
    }

    #[test]
    fn vcard_address_with_partial_fields() {
        let vcard = "\
BEGIN:VCARD
VERSION:4.0
FN:Partial Address
N:Address;Partial;;;
ADR:;;;Melbourne;;3000;Australia
UID:partial-addr
END:VCARD";

        let contact = normalize_vcard(vcard, "carddav").expect("should parse");
        assert_eq!(contact.addresses.len(), 1);
        assert!(contact.addresses[0].street.is_none());
        assert_eq!(contact.addresses[0].city.as_deref(), Some("Melbourne"));
        assert!(contact.addresses[0].region.is_none());
        assert_eq!(contact.addresses[0].postal_code.as_deref(), Some("3000"));
        assert_eq!(contact.addresses[0].country.as_deref(), Some("Australia"));
    }

    #[test]
    fn vcard_empty_address_not_added() {
        let vcard = "\
BEGIN:VCARD
VERSION:4.0
FN:Empty Addr
N:Addr;Empty;;;
ADR:;;;;;;
UID:empty-addr
END:VCARD";

        let contact = normalize_vcard(vcard, "carddav").expect("should parse");
        assert!(contact.addresses.is_empty());
    }

    #[test]
    fn vcard_pref_email_marked_primary() {
        let vcard = "\
BEGIN:VCARD
VERSION:4.0
FN:Pref Test
N:Test;Pref;;;
EMAIL;TYPE=work:work@example.com
EMAIL;TYPE=home,pref:home@example.com
UID:pref-test
END:VCARD";

        let contact = normalize_vcard(vcard, "carddav").expect("should parse");
        assert_eq!(contact.emails.len(), 2);
        assert!(contact.emails[0].primary.is_none());
        // TYPE=home,pref — comma-separated values are split; "pref" is detected.
        assert_eq!(contact.emails[1].primary, Some(true));
        assert_eq!(contact.emails[1].email_type, Some(ContactInfoType::Home));
    }
}
