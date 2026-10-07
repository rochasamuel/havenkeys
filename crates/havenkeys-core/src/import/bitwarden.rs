//! Bitwarden unencrypted `.json` export importer.
//!
//! Mapping:
//! * Login (type 1) → login. Every URI becomes a website; Bitwarden's match
//!   setting maps to one that is the same or stricter (host → origin, exact
//!   → exact, anything else → domain). Custom fields become the login's
//!   custom fields (hidden → password, linked fields are left out).
//! * Secure note (2) → secure note; custom fields are added to its text.
//! * Card (3) → card; values that do not pass the card's checks stay in its
//!   notes.
//! * Identity (4), SSH key (5) and anything else → secure note with every
//!   value written out.
//! * A folder becomes a tag (a nested one stays a single name).
//! * Trashed items, password history and passkeys are skipped and counted.
//! * Encrypted exports are refused: opening them would mean reimplementing
//!   Bitwarden's key scheme.

use super::common::{
    brand_from_name, clean_line, collect_urls, join_notes, non_empty, parse_utc_timestamp_ms,
    set_or_keep, str_at, validated, wipe, Extras, LoginSections,
};
use super::{split_tags, ImportReport, ImportedItem, Parsed, MAX_ITEMS};
use crate::card::{self, CardExpiry, CardInput, MAX_CARDHOLDER_CHARS};
use crate::custom_field::FieldValueInput;
use crate::error::{Error, Result};
use crate::model::{
    ItemInput, ItemType, MatchType, SecretUpdate, MAX_TITLE_CHARS, MAX_USERNAME_CHARS,
};
use crate::secret::SecretString;
use crate::totp::parse_totp_input;
use serde_json::Value;
use std::collections::HashMap;
use zeroize::Zeroize;

const INVALID: Error = Error::InvalidInput("not a valid Bitwarden export file");
const ENCRYPTED: Error = Error::InvalidInput("encrypted Bitwarden exports are not supported");

pub fn parse(bytes: &[u8]) -> Result<Parsed> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let mut root: Value = serde_json::from_slice(bytes).map_err(|_| INVALID)?;
    let result = convert(&root);
    wipe(&mut root);
    result
}

fn convert(root: &Value) -> Result<Parsed> {
    if root.get("encrypted").and_then(Value::as_bool) == Some(true) {
        return Err(ENCRYPTED);
    }
    let list = root.get("items").and_then(Value::as_array).ok_or(INVALID)?;
    if list.len() > MAX_ITEMS {
        return Err(Error::InvalidInput("export contains too many items"));
    }
    let folders: HashMap<&str, &str> = root
        .get("folders")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|f| Some((text(f, "id")?, text(f, "name")?)))
                .collect()
        })
        .unwrap_or_default();
    let mut report = ImportReport::default();
    let mut items = Vec::new();
    for item in list {
        if item.get("deletedDate").is_some_and(|d| !d.is_null()) {
            report.skipped_archived += 1;
            continue;
        }
        match convert_item(item, &folders, &mut report) {
            Some(parsed) => items.push(parsed),
            None => report.failed += 1,
        }
    }
    Ok(Parsed { items, report })
}

fn text<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    non_empty(str_at(v, &[key]))
}

/// Bitwarden's URI match setting, never looser than what it says.
fn match_type(uri: &Value) -> MatchType {
    match uri.get("match").and_then(Value::as_i64) {
        Some(1) => MatchType::Origin, // host
        Some(3) => MatchType::Exact,
        // null (default), base domain, starts with, regex, never.
        _ => MatchType::Domain,
    }
}

fn type_name(t: i64) -> &'static str {
    match t {
        3 => "Card",
        4 => "Identity",
        5 => "SSH key",
        _ => "Item",
    }
}

fn convert_item(
    item: &Value,
    folders: &HashMap<&str, &str>,
    report: &mut ImportReport,
) -> Option<ImportedItem> {
    if !item.is_object() {
        return None;
    }
    let kind = item.get("type").and_then(Value::as_i64).unwrap_or(0);
    let title = text(item, "name")
        .map(|t| clean_line(t, MAX_TITLE_CHARS))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "Untitled".to_owned());
    let notes = text(item, "notes");
    if let Some(history) = item.get("passwordHistory").and_then(Value::as_array) {
        report.password_history_skipped += history.len();
    }
    let created_at = text(item, "creationDate").and_then(parse_utc_timestamp_ms);
    let updated_at = text(item, "revisionDate")
        .and_then(parse_utc_timestamp_ms)
        .or(created_at);
    let custom: &[Value] = item
        .get("fields")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice);

    let mut extras = Extras::default();
    // The folder becomes a tag; a name that cannot be one stays in the notes.
    let folder = text(item, "folderId").and_then(|id| folders.get(id).copied());
    let (tags, rejected) = split_tags(folder.map(str::to_owned).into_iter().collect());
    for name in &rejected {
        extras.section(None);
        extras.push(Some("Folder"), name);
    }
    let mut input = match kind {
        1 => {
            let login = item.get("login").unwrap_or(&Value::Null);
            if let Some(passkeys) = login.get("fido2Credentials").and_then(Value::as_array) {
                report.passkeys_skipped += passkeys.len();
            }
            let uris: Vec<(&str, MatchType)> = login
                .get("uris")
                .and_then(Value::as_array)
                .map(|list| {
                    list.iter()
                        .filter_map(|u| text(u, "uri").map(|s| (s, match_type(u))))
                        .collect()
                })
                .unwrap_or_default();
            let urls = collect_urls(uris, report, &mut extras);
            let totp = match text(login, "totp") {
                Some(t) if parse_totp_input(t).is_ok() => Some(SecretString::from(t)),
                Some(t) => {
                    report.fields_to_notes += 1;
                    extras.section(None);
                    extras.push(Some("One-time password"), t);
                    None
                }
                None => None,
            };
            let mut sections = LoginSections::default();
            sections.start(None);
            for f in custom {
                let Some(value) = text(f, "value") else {
                    continue;
                };
                let name = text(f, "name");
                let candidate = match f.get("type").and_then(Value::as_i64) {
                    Some(1) => FieldValueInput::Password(SecretUpdate::Set(value.into())),
                    // A linked field only points at the username or password.
                    Some(3) => continue,
                    _ => FieldValueInput::Text(value.into()),
                };
                let kept = validated(candidate, || Some(value.to_owned()))
                    .is_some_and(|fv| sections.push(name, fv));
                if !kept {
                    report.fields_to_notes += 1;
                    extras.section(None);
                    extras.push(name, value);
                }
            }
            report.logins += 1;
            ItemInput {
                username: text(login, "username")
                    .map(|u| clean_line(u, MAX_USERNAME_CHARS))
                    .filter(|u| !u.is_empty()),
                urls,
                password: set_or_keep(
                    str_at(login, &["password"])
                        .filter(|p| !p.trim().is_empty())
                        .map(SecretString::from),
                ),
                totp: set_or_keep(totp),
                notes: set_or_keep(join_notes(notes, &extras)),
                sections: sections.finish(),
                ..ItemInput::blank(ItemType::Login, title)
            }
        }
        2 => {
            push_fields(custom, &mut extras);
            report.secure_notes += 1;
            ItemInput {
                content: SecretUpdate::Set(join_notes(notes, &extras).unwrap_or_default()),
                ..ItemInput::blank(ItemType::SecureNote, title)
            }
        }
        3 => {
            let c = item.get("card").unwrap_or(&Value::Null);
            let card = card_input(c, &mut extras);
            push_fields(custom, &mut extras);
            report.cards += 1;
            let untitled = text(item, "name").is_none();
            let has_number = matches!(card.number, SecretUpdate::Set(_));
            ItemInput {
                card: Some(CardInput {
                    notes: join_notes(notes, &extras),
                    ..card
                }),
                // An untitled card with a number is named after its brand.
                ..ItemInput::blank(
                    ItemType::Card,
                    if untitled && has_number {
                        String::new()
                    } else {
                        title
                    },
                )
            }
        }
        other => {
            // No native type for these (the vault's one Identity is the
            // user's own): keep every value as text in a secure note.
            let key = match other {
                4 => "identity",
                5 => "sshKey",
                _ => "",
            };
            if let Some(Value::Object(values)) = item.get(key) {
                for (name, value) in values {
                    if let Some(v) = non_empty(value.as_str()) {
                        extras.push(Some(&label(name)), v);
                    }
                }
            }
            push_fields(custom, &mut extras);
            report.converted_to_notes += 1;
            let mut body = format!("Imported from Bitwarden ({})", type_name(other));
            let mut rendered = extras.render();
            if !rendered.is_empty() {
                body.push_str("\n\n");
                body.push_str(&rendered);
            }
            rendered.zeroize();
            if let Some(n) = notes {
                body.push_str("\n\n");
                body.push_str(n);
            }
            let content = SecretString::new(body.clone());
            body.zeroize();
            ItemInput {
                content: SecretUpdate::Set(content),
                ..ItemInput::blank(ItemType::SecureNote, title)
            }
        }
    };
    input.tags = Some(tags);
    Some(ImportedItem {
        input,
        created_at,
        updated_at,
    })
}

/// `firstName` → "First name".
fn label(key: &str) -> String {
    let mut out = String::new();
    for (i, c) in key.chars().enumerate() {
        if c.is_ascii_uppercase() {
            out.push(' ');
            out.push(c.to_ascii_lowercase());
        } else if i == 0 {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn push_fields(custom: &[Value], extras: &mut Extras) {
    for f in custom {
        if f.get("type").and_then(Value::as_i64) == Some(3) {
            continue;
        }
        if let Some(value) = text(f, "value") {
            extras.push(text(f, "name"), value);
        }
    }
}

/// A Bitwarden card's values. Each is taken only when it passes the card's
/// own checks; anything else goes to `extras`.
fn card_input(c: &Value, extras: &mut Extras) -> CardInput {
    let number = text(c, "number");
    let clean_number = number.and_then(|n| card::clean_number(n).ok());
    if let (Some(n), None) = (number, &clean_number) {
        extras.push(Some("Number"), n);
    }
    let code = text(c, "code");
    let clean_code = code.and_then(|v| card::clean_verification_number(v).ok());
    if let (Some(v), None) = (code, &clean_code) {
        extras.push(Some("Security code"), v);
    }
    let brand_text = text(c, "brand");
    let brand = brand_text.and_then(brand_from_name);
    if let (Some(b), None) = (brand_text, brand) {
        // "Other" carries nothing worth keeping.
        if !b.eq_ignore_ascii_case("other") {
            extras.push(Some("Brand"), b);
        }
    }
    let month = text(c, "expMonth");
    let year = text(c, "expYear");
    let expiry = match (month, year) {
        (Some(m), Some(y)) => {
            let parsed = m
                .parse::<u8>()
                .ok()
                .zip(y.parse::<u16>().ok())
                .and_then(|(m, y)| {
                    let y = if y < 100 { y + 2000 } else { y };
                    CardExpiry::new(y, m).ok()
                });
            if parsed.is_none() {
                extras.push(Some("Expiry"), &format!("{m}/{y}"));
            }
            parsed
        }
        (Some(v), None) | (None, Some(v)) => {
            extras.push(Some("Expiry"), v);
            None
        }
        (None, None) => None,
    };
    CardInput {
        cardholder_name: text(c, "cardholderName")
            .map(|t| clean_line(t, MAX_CARDHOLDER_CHARS))
            .filter(|t| !t.is_empty())
            .map(SecretString::new),
        brand,
        number: set_or_keep(clean_number.map(SecretString::new)),
        verification_number: set_or_keep(clean_code.map(SecretString::new)),
        expiry,
        notes: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::CardBrand;
    use crate::custom_field::FieldKind;
    use serde_json::json;

    fn set(u: &SecretUpdate) -> Option<&str> {
        match u {
            SecretUpdate::Set(v) => Some(v.expose()),
            _ => None,
        }
    }

    fn sample() -> Value {
        json!({"encrypted": false, "folders": [{"id": "f", "name": "Work"}], "items": [
            {
                "id": "1", "type": 1, "name": "GitHub", "notes": "old notes", "folderId": "f",
                "creationDate": "2024-01-01T00:00:00.000Z", "revisionDate": "2025-01-02T03:04:05.000Z",
                "deletedDate": null,
                "fields": [
                    {"name": "Recovery", "value": "RC-1", "type": 1, "linkedId": null},
                    {"name": "Branch", "value": "7", "type": 0, "linkedId": null},
                    {"name": "Remember", "value": "true", "type": 2, "linkedId": null},
                    {"name": "User", "value": null, "type": 3, "linkedId": 100}
                ],
                "login": {
                    "uris": [
                        {"match": null, "uri": "https://github.com/login"},
                        {"match": 1, "uri": "https://gist.github.com"},
                        {"match": 3, "uri": "https://github.com/exact"},
                        {"match": 4, "uri": "^https://.*"},
                        {"match": null, "uri": "androidapp://com.github.android"}
                    ],
                    "username": "octo", "password": "gh-pass",
                    "totp": "otpauth://totp/GitHub:octo?secret=JBSWY3DPEHPK3PXP&issuer=GitHub",
                    "fido2Credentials": [{"keyValue": "secret-key"}]
                },
                "passwordHistory": [{"password": "older", "lastUsedDate": "2020-01-01T00:00:00Z"}]
            },
            {"id": "2", "type": 2, "name": "Wifi", "notes": "pw is 1234", "secureNote": {"type": 0},
             "fields": [{"name": "SSID", "value": "home", "type": 0}]},
            {"id": "3", "type": 3, "name": "Visa", "notes": null, "card": {
                "cardholderName": "Sam Rocha", "brand": "Visa", "number": "4111 1111 1111 1111",
                "expMonth": "12", "expYear": "2030", "code": "123"}},
            {"id": "4", "type": 4, "name": "Me", "identity": {
                "firstName": "Sam", "lastName": "Rocha", "passportNumber": "X123", "email": null}},
            {"id": "5", "type": 1, "name": "Gone", "deletedDate": "2025-01-01T00:00:00Z", "login": {}},
            {"id": "6", "type": 5, "name": "Server key", "sshKey": {
                "privateKey": "-----BEGIN KEY-----", "publicKey": "ssh-ed25519 AAA", "keyFingerprint": "SHA256:x"}}
        ]})
    }

    fn parse_value(v: &Value) -> Parsed {
        parse(v.to_string().as_bytes()).unwrap()
    }

    #[test]
    fn a_folder_that_cannot_be_a_tag_stays_in_the_notes() {
        let parsed = parse_value(&json!({"folders": [{"id": "f", "name": "a,b"}], "items": [
            {"id": "1", "type": 1, "name": "Site", "folderId": "f",
             "login": {"username": "u", "password": "p", "uris": []}}
        ]}));
        let input = &parsed.items[0].input;
        assert_eq!(input.tags, Some(vec![]));
        assert_eq!(set(&input.notes), Some("Folder: a,b"));
    }

    #[test]
    fn maps_items() {
        let parsed = parse_value(&sample());
        let r = parsed.report;
        assert_eq!(parsed.items.len(), 5);
        assert_eq!(
            (r.logins, r.secure_notes, r.cards, r.converted_to_notes),
            (1, 1, 1, 2)
        );
        assert_eq!(r.skipped_archived, 1);
        assert_eq!(r.password_history_skipped, 1);
        assert_eq!(r.passkeys_skipped, 1);
        assert_eq!(r.urls_moved_to_notes, 2);

        assert_eq!(parsed.items[0].input.tags, Some(vec!["work".to_string()]));
        for other in &parsed.items[1..] {
            assert_eq!(other.input.tags, Some(vec![]));
        }
        let gh = &parsed.items[0];
        assert_eq!(gh.created_at, Some(1_704_067_200_000));
        assert_eq!(gh.updated_at, Some(1_735_787_045_000));
        let i = &gh.input;
        assert_eq!(i.username.as_deref(), Some("octo"));
        assert_eq!(set(&i.password), Some("gh-pass"));
        assert!(set(&i.totp).unwrap().starts_with("otpauth://"));
        let urls: Vec<(&str, MatchType)> = i
            .urls
            .iter()
            .map(|u| (u.url.as_str(), u.match_type))
            .collect();
        assert_eq!(
            urls,
            [
                ("https://github.com/login", MatchType::Domain),
                ("https://gist.github.com/", MatchType::Origin),
                ("https://github.com/exact", MatchType::Exact),
            ]
        );
        let notes = set(&i.notes).unwrap();
        assert!(notes.starts_with("old notes"));
        assert!(notes.contains("Website: ^https://.*"));
        assert!(notes.contains("Website: androidapp://com.github.android"));
        assert!(
            !notes.contains("secret-key"),
            "passkeys are never written out"
        );
        let fields = &i.sections.as_ref().unwrap()[0].fields;
        let kinds: Vec<(&str, FieldKind)> = fields
            .iter()
            .map(|f| (f.label.expose(), f.value.kind()))
            .collect();
        assert_eq!(
            kinds,
            [
                ("Recovery", FieldKind::Password),
                ("Branch", FieldKind::Text),
                ("Remember", FieldKind::Text)
            ]
        );

        let note = &parsed.items[1].input;
        assert_eq!(set(&note.content), Some("pw is 1234\n\nSSID: home"));

        let card = parsed.items[2].input.card.as_ref().unwrap();
        assert_eq!(set(&card.number), Some("4111111111111111"));
        assert_eq!(set(&card.verification_number), Some("123"));
        assert_eq!(card.brand, Some(CardBrand::Visa));
        assert_eq!(card.expiry, Some(CardExpiry::new(2030, 12).unwrap()));
        assert_eq!(card.cardholder_name.as_ref().unwrap().expose(), "Sam Rocha");
        assert!(card.notes.is_none());

        let me = set(&parsed.items[3].input.content).unwrap();
        assert!(me.starts_with("Imported from Bitwarden (Identity)"));
        assert!(me.contains("First name: Sam"));
        assert!(me.contains("Passport number: X123"));

        let key = set(&parsed.items[4].input.content).unwrap();
        assert!(key.contains("Private key: -----BEGIN KEY-----"));
    }

    #[test]
    fn odd_card_values_go_to_notes() {
        let data = json!({"items": [{"type": 3, "name": "", "card": {
            "brand": "Other", "number": "12", "code": "123456789", "expMonth": "13", "expYear": "26"}}]});
        let parsed = parse_value(&data);
        let item = &parsed.items[0].input;
        assert_eq!(item.title, "Untitled");
        let c = item.card.as_ref().unwrap();
        assert!(matches!(c.number, SecretUpdate::Keep));
        assert_eq!(c.brand, None);
        assert_eq!(c.expiry, None);
        let notes = c.notes.as_ref().unwrap().expose();
        assert!(notes.contains("Number: 12"));
        assert!(notes.contains("Security code: 123456789"));
        assert!(notes.contains("Expiry: 13/26"));
        assert!(!notes.contains("Other"));
    }

    #[test]
    fn short_year_and_bad_totp() {
        let data = json!({"items": [
            {"type": 3, "name": "C", "card": {"number": "4111111111111111", "expMonth": "1", "expYear": "31"}},
            {"type": 1, "name": "L", "login": {"totp": "nope nope"}}
        ]});
        let parsed = parse_value(&data);
        let c = parsed.items[0].input.card.as_ref().unwrap();
        assert_eq!(c.expiry, Some(CardExpiry::new(2031, 1).unwrap()));
        let l = &parsed.items[1].input;
        assert!(matches!(l.totp, SecretUpdate::Keep));
        assert_eq!(set(&l.notes), Some("One-time password: nope nope"));
        assert_eq!(parsed.report.fields_to_notes, 1);
    }

    #[test]
    fn refuses_encrypted_and_garbage() {
        let enc = json!({"encrypted": true, "passwordProtected": true, "data": "2.abc"});
        let e = parse(enc.to_string().as_bytes()).err().unwrap();
        assert_eq!(e.to_string(), ENCRYPTED.to_string());
        let account = json!({"encrypted": true, "items": [{"type": 1, "name": "2.xyz"}]});
        assert!(parse(account.to_string().as_bytes()).is_err());
        for bad in [&b""[..], b"name,url\n", b"{}", b"{\"items\": 5}", b"[]"] {
            assert!(parse(bad).is_err());
        }
    }

    #[test]
    fn tolerates_odd_shapes() {
        let data = json!({"items": [
            5, {"type": 1}, {"type": 1, "login": 7, "fields": "x"},
            {"type": 99, "name": "\u{0007}Bell"}, {"type": 4, "identity": [1, 2]}
        ]});
        let parsed = parse_value(&data);
        assert_eq!(parsed.items.len(), 4);
        assert_eq!(parsed.report.failed, 1);
        assert_eq!(parsed.items[2].input.title, "Bell");
    }

    #[test]
    fn errors_do_not_echo_content() {
        let e = parse(b"{\"items\": hunter2").err().unwrap();
        assert!(!e.to_string().contains("hunter2"));
    }

    /// Deterministic fuzz (CLAUDE.md §47): mutated exports never panic.
    #[test]
    fn fuzz_never_panics() {
        let mut state: u64 = 0xB17_3A2D;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let json = sample().to_string().into_bytes();
        for _ in 0..3_000 {
            let mut input = json.clone();
            for _ in 0..=(next() % 6) {
                let pos = (next() % input.len().max(1) as u64) as usize;
                match next() % 3 {
                    0 if pos < input.len() => input[pos] ^= 1 << (next() % 8),
                    1 if pos < input.len() => {
                        input.remove(pos);
                    }
                    _ => {
                        const BYTES: &[u8] = b"{}[]\":,0-9n";
                        input.insert(
                            pos.min(input.len()),
                            BYTES[(next() % BYTES.len() as u64) as usize],
                        );
                    }
                }
            }
            let _ = parse(&input);
        }
    }
}
