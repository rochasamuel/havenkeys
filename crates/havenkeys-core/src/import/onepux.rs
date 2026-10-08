//! 1Password `.1pux` export importer.
//!
//! A `.1pux` file is a ZIP archive containing `export.data` (JSON with every
//! account, vault and item in plaintext), `export.attributes`, and attachments
//! under `files/`. Only `export.data` is read; attachments are counted and
//! skipped.
//!
//! Mapping:
//! * Login (001) and Password (005) → login. Username/password come from the
//!   designated login fields; the first TOTP field becomes the item's TOTP;
//!   every other non-empty field is appended to the notes so nothing is lost.
//! * Secure Note (003) → secure note.
//! * Credit Card (002) → card; fields with no place on a card go to its notes.
//! * Every other category (identity, SSH key, API credential, …)
//!   → secure note with all fields written out as text.
//! * Archived/deleted items, attachments and password history are skipped
//!   and counted.

use super::common::{
    self, clean_line, fill, format_date, non_empty, set_or_keep, str_at, validated, wipe, Extras,
    LoginSections,
};
use super::{split_tags, ImportReport, ImportedItem, Parsed, MAX_ITEMS};
use crate::card::{self, CardBrand, CardExpiry, CardInput, MAX_CARDHOLDER_CHARS};
use crate::custom_field::{AddressValue, FieldValueInput};
use crate::error::{Error, Result};
use crate::model::{
    ItemInput, ItemType, MatchType, SecretUpdate, UrlRule, MAX_TITLE_CHARS, MAX_USERNAME_CHARS,
};
use crate::secret::SecretString;
use crate::sso::{SignInWith, SsoProvider, MAX_ACCOUNT_CHARS};
use crate::totp::parse_totp_input;
use serde_json::Value;
use std::io::{Cursor, Read};
use zeroize::{Zeroize, Zeroizing};

/// Largest `.1pux` archive accepted.
pub const MAX_ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;
/// Largest uncompressed `export.data` accepted (zip-bomb guard).
pub const MAX_EXPORT_DATA_BYTES: u64 = 64 * 1024 * 1024;

/// Most entries (files) a `.1pux` may list. The ZIP reader builds metadata
/// for every entry before anything else can be checked, about 7× the
/// archive's size in memory for a crafted one (CR3); real exports have
/// `export.data`, `export.attributes` and one file per attachment.
pub const MAX_ARCHIVE_ENTRIES: u64 = 10_000;

pub(super) const INVALID: Error = Error::InvalidInput("not a valid 1Password export (.1pux) file");

pub fn parse(archive: &[u8]) -> Result<Parsed> {
    if archive.len() as u64 > MAX_ARCHIVE_BYTES {
        return Err(Error::InvalidInput("export file is too large"));
    }
    if declared_entries(archive) > MAX_ARCHIVE_ENTRIES {
        return Err(Error::InvalidInput("export file has too many files"));
    }
    let mut zip = zip::ZipArchive::new(Cursor::new(archive)).map_err(|_| INVALID)?;

    let attachments = zip.file_names().filter(|n| n.starts_with("files/")).count();

    let data = {
        let entry = zip.by_name("export.data").map_err(|_| INVALID)?;
        if entry.size() > MAX_EXPORT_DATA_BYTES {
            return Err(Error::InvalidInput("export file is too large"));
        }
        // The declared size can lie; cap what is actually decompressed.
        let mut buf = Zeroizing::new(Vec::new());
        entry
            .take(MAX_EXPORT_DATA_BYTES + 1)
            .read_to_end(&mut buf)
            .map_err(|_| INVALID)?;
        if buf.len() as u64 > MAX_EXPORT_DATA_BYTES {
            return Err(Error::InvalidInput("export file is too large"));
        }
        buf
    };

    let mut root: Value = serde_json::from_slice(&data).map_err(|_| INVALID)?;
    let result = convert(&root, attachments);
    wipe(&mut root);
    result
}

/// The largest entry count any end-of-central-directory record in
/// `archive` declares, read before the ZIP reader parses the directory.
/// Every candidate in the last 64 KiB is counted (a fake record in the
/// archive comment cannot hide the real one), and a ZIP64 record each
/// one points to as well. 0 when there is none; the ZIP reader then
/// refuses the archive itself.
fn declared_entries(archive: &[u8]) -> u64 {
    const EOCD: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];
    const LOCATOR: [u8; 4] = [0x50, 0x4b, 0x06, 0x07];
    const EOCD64: [u8; 4] = [0x50, 0x4b, 0x06, 0x06];
    let u16_at = |at: usize| -> Option<u64> {
        archive
            .get(at..at + 2)
            .map(|b| u64::from(u16::from_le_bytes([b[0], b[1]])))
    };
    let u64_at = |at: usize| -> Option<u64> {
        let b: [u8; 8] = archive.get(at..at + 8)?.try_into().ok()?;
        Some(u64::from_le_bytes(b))
    };
    // EOCD is 22 bytes plus a comment of at most 65,535.
    let start = archive.len().saturating_sub(22 + 65_535);
    let mut most = 0;
    for at in start..archive.len().saturating_sub(21) {
        if archive[at..at + 4] != EOCD {
            continue;
        }
        let this_disk = u16_at(at + 8).unwrap_or(0);
        let total = u16_at(at + 10).unwrap_or(0);
        most = most.max(this_disk).max(total);
        // ZIP64: a locator just before the EOCD names the ZIP64 record.
        if let Some(loc) = at.checked_sub(20) {
            if archive[loc..loc + 4] == LOCATOR {
                let record = u64_at(loc + 8)
                    .and_then(|o| usize::try_from(o).ok())
                    .filter(|&o| archive.get(o..o + 4) == Some(&EOCD64[..]));
                if let Some(r) = record {
                    let this_disk = u64_at(r + 24).unwrap_or(0);
                    let total = u64_at(r + 32).unwrap_or(0);
                    most = most.max(this_disk).max(total);
                }
            }
        }
    }
    most
}

fn convert(root: &Value, attachments: usize) -> Result<Parsed> {
    let accounts = root
        .get("accounts")
        .and_then(Value::as_array)
        .ok_or(INVALID)?;
    let mut report = ImportReport {
        attachments_skipped: attachments,
        ..Default::default()
    };
    let mut items = Vec::new();

    for account in accounts {
        let vaults = account
            .get("vaults")
            .and_then(Value::as_array)
            .ok_or(INVALID)?;
        for vault in vaults {
            let Some(vault_items) = vault.get("items").and_then(Value::as_array) else {
                continue;
            };
            for item in vault_items {
                if items.len() >= MAX_ITEMS {
                    return Err(Error::InvalidInput("export contains too many items"));
                }
                if str_at(item, &["state"]).is_some_and(|s| s != "active") {
                    report.skipped_archived += 1;
                    continue;
                }
                if let Some(parsed) = convert_item(item, &mut report) {
                    items.push(parsed);
                }
            }
        }
    }
    Ok(Parsed { items, report })
}

/// 1Password stores seconds; the vault stores milliseconds.
fn timestamp_ms(item: &Value, key: &str) -> Option<i64> {
    item.get(key)?
        .as_i64()
        .filter(|s| *s > 0)
        .map(|s| s.saturating_mul(1000))
}

fn category_name(uuid: &str) -> &'static str {
    match uuid {
        "002" => "Credit card",
        "004" => "Identity",
        "006" => "Document",
        "100" => "Software license",
        "101" => "Bank account",
        "102" => "Database",
        "103" => "Driver licence",
        "104" => "Outdoor licence",
        "105" => "Membership",
        "106" => "Passport",
        "107" => "Reward program",
        "108" => "Social security number",
        "109" => "Wireless router",
        "110" => "Server",
        "111" => "Email account",
        "112" => "API credential",
        "113" => "Medical record",
        "114" => "SSH key",
        "115" => "Crypto wallet",
        _ => "Item",
    }
}

/// A 1Password section value as a custom field. `None` for kinds with
/// nothing to keep (files are counted by `render_value`). A value that does
/// not pass its type's checks becomes Text; one that is not valid text
/// either is `None`, and the caller keeps it in the notes.
fn custom_field_from(value: &Value, report: &mut ImportReport) -> Option<FieldValueInput> {
    let obj = value.as_object()?;
    let (kind, v) = obj.iter().next()?;
    let text = |t: Option<&str>| non_empty(t).map(SecretString::from);
    let candidate = match kind.as_str() {
        "file" => return None,
        "concealed" => FieldValueInput::Password(SecretUpdate::Set(text(v.as_str())?)),
        "totp" => FieldValueInput::Otp(SecretUpdate::Set(text(v.as_str())?)),
        "email" => FieldValueInput::Email(text(str_at(v, &["email_address"]).or(v.as_str()))?),
        "phone" => FieldValueInput::Phone(text(v.as_str())?),
        "url" => FieldValueInput::Url(text(v.as_str())?),
        "date" => FieldValueInput::Date(SecretString::new(format_date(v.as_i64()?))),
        "address" => {
            let part = |k: &str| text(str_at(v, &[k]));
            let a = AddressValue {
                street: part("street"),
                city: part("city"),
                state: part("state"),
                postal_code: part("zip"),
                country: part("country"),
                ..AddressValue::default()
            };
            if a.street.is_none()
                && a.city.is_none()
                && a.state.is_none()
                && a.postal_code.is_none()
                && a.country.is_none()
            {
                return None;
            }
            FieldValueInput::Address(Box::new(a))
        }
        "sshKey" => FieldValueInput::Password(SecretUpdate::Set(SecretString::new(render_value(
            value, report,
        )?))),
        _ => FieldValueInput::Text(SecretString::new(render_value(value, report)?)),
    };
    validated(candidate, || render_value(value, report))
}
/// The notes line the importer wrote for a "Sign in with" login before
/// sign-in-with existed. A re-import clears notes that are exactly this.
pub(crate) fn legacy_sso_note(name: &str) -> String {
    format!("Sign in with {name}")
}

/// Render a section field value as text. Returns `None` for kinds that carry
/// no importable text (attachments are counted separately).
fn render_value(value: &Value, report: &mut ImportReport) -> Option<String> {
    let obj = value.as_object()?;
    let (kind, v) = obj.iter().next()?;
    let text = match kind.as_str() {
        "string" | "concealed" | "phone" | "url" | "menu" | "creditCardType"
        | "creditCardNumber" | "totp" | "gender" => v.as_str().map(str::to_owned),
        "email" => str_at(v, &["email_address"])
            .map(str::to_owned)
            .or_else(|| v.as_str().map(str::to_owned)),
        "monthYear" => v
            .as_i64()
            .filter(|n| *n > 0)
            .map(|n| format!("{:02}/{}", n % 100, n / 100)),
        "date" => v.as_i64().map(format_date),
        "address" => {
            let parts: Vec<&str> = ["street", "city", "state", "zip", "country"]
                .iter()
                .filter_map(|k| non_empty(str_at(v, &[k])))
                .collect();
            (!parts.is_empty()).then(|| parts.join(", "))
        }
        "sshKey" => {
            let mut out = Vec::new();
            if let Some(k) = non_empty(str_at(v, &["metadata", "keyType"])) {
                out.push(format!("Key type: {k}"));
            }
            if let Some(k) = non_empty(str_at(v, &["metadata", "fingerprint"])) {
                out.push(format!("Fingerprint: {k}"));
            }
            if let Some(k) = non_empty(str_at(v, &["metadata", "publicKey"])) {
                out.push(format!("Public key: {k}"));
            }
            if let Some(k) = non_empty(str_at(v, &["privateKey"]))
                .or_else(|| non_empty(str_at(v, &["metadata", "privateKey"])))
            {
                out.push(format!("Private key:\n{k}"));
            }
            (!out.is_empty()).then(|| out.join("\n"))
        }
        "ssoLogin" => non_empty(str_at(v, &["provider"])).map(legacy_sso_note),
        "file" => {
            report.attachments_skipped += 1;
            None
        }
        _ => v.as_str().map(str::to_owned),
    };
    text.filter(|t| !t.trim().is_empty())
}

/// Valid http(s) websites become URL rules; anything else (app links, bare
/// words) is kept as text in the notes rather than dropped.
fn collect_urls(overview: &Value, report: &mut ImportReport, extras: &mut Extras) -> Vec<UrlRule> {
    let mut raw: Vec<&str> = Vec::new();
    if let Some(list) = overview.get("urls").and_then(Value::as_array) {
        raw.extend(list.iter().filter_map(|u| non_empty(str_at(u, &["url"]))));
    }
    if let Some(u) = non_empty(str_at(overview, &["url"])) {
        raw.push(u);
    }
    common::collect_urls(
        raw.into_iter().map(|u| (u, MatchType::Domain)),
        report,
        extras,
    )
}

/// The values of a 1Password credit card that a Card holds. A field is
/// taken only when it passes the card's own checks; anything else stays in
/// the item's notes.
#[derive(Default)]
struct CardParts {
    cardholder: Option<SecretString>,
    brand: Option<CardBrand>,
    number: Option<SecretString>,
    verification: Option<SecretString>,
    expiry: Option<CardExpiry>,
}

impl CardParts {
    fn take(&mut self, field: &Value, value: &Value) -> bool {
        let id = str_at(field, &["id"]).unwrap_or("");
        let title = str_at(field, &["title"]).unwrap_or("").to_lowercase();
        let text = || {
            ["string", "concealed", "creditCardNumber", "creditCardType"]
                .iter()
                .find_map(|k| non_empty(str_at(value, &[k])))
        };
        if self.number.is_none() && (id == "ccnum" || value.get("creditCardNumber").is_some()) {
            let number = text().and_then(|t| card::clean_number(t).ok());
            return fill(&mut self.number, number.map(SecretString::new));
        }
        if self.verification.is_none() && id == "cvv" {
            let code = text().and_then(|t| card::clean_verification_number(t).ok());
            return fill(&mut self.verification, code.map(SecretString::new));
        }
        if self.cardholder.is_none() && id == "cardholder" {
            let name = text()
                .map(|t| clean_line(t, MAX_CARDHOLDER_CHARS))
                .filter(|t| !t.is_empty());
            return fill(&mut self.cardholder, name.map(SecretString::new));
        }
        if self.brand.is_none() && (id == "type" || value.get("creditCardType").is_some()) {
            return fill(&mut self.brand, text().and_then(common::brand_from_name));
        }
        let expiry_field = id == "expiry" || (id.is_empty() && title.contains("expir"));
        if self.expiry.is_none() && expiry_field {
            let parsed = value
                .get("monthYear")
                .and_then(Value::as_i64)
                .filter(|n| *n > 0)
                .and_then(|n| {
                    let year = u16::try_from(n / 100).ok()?;
                    let month = u8::try_from(n % 100).ok()?;
                    CardExpiry::new(year, month).ok()
                });
            if let Some(e) = parsed {
                self.expiry = Some(e);
                return true;
            }
        }
        false
    }
}

/// A login's username and password from `loginFields`: the fields
/// designated as such first, then the first text/email and password
/// fields. Any other field with a value goes to `extras`, except
/// checkboxes, radio buttons, buttons and images, which are UI state.
fn login_fields(
    fields: &[Value],
    extras: &mut Extras,
    sections: &mut LoginSections,
    report: &mut ImportReport,
) -> (Option<String>, Option<SecretString>) {
    let mut username: Option<String> = None;
    let mut password: Option<SecretString> = None;
    // Designated fields first.
    for f in fields {
        let value = non_empty(str_at(f, &["value"]));
        match (str_at(f, &["designation"]), value) {
            (Some("username"), Some(v)) if username.is_none() => username = Some(v.to_owned()),
            (Some("password"), Some(v)) if password.is_none() => {
                password = Some(SecretString::from(v))
            }
            _ => {}
        }
    }
    // Fallbacks, then keep the rest as extra lines.
    extras.section(None);
    for f in fields {
        let Some(value) = non_empty(str_at(f, &["value"])) else {
            continue;
        };
        let designation = str_at(f, &["designation"]);
        if matches!(designation, Some("username") | Some("password")) {
            continue;
        }
        let field_type = str_at(f, &["fieldType"]).unwrap_or("");
        if username.is_none() && matches!(field_type, "T" | "E") {
            username = Some(value.to_owned());
        } else if password.is_none() && field_type == "P" {
            password = Some(SecretString::from(value));
        } else if !matches!(field_type, "C" | "R" | "B" | "I") {
            let name = non_empty(str_at(f, &["name"]));
            let v = SecretString::from(value);
            let candidate = if field_type == "P" {
                FieldValueInput::Password(SecretUpdate::Set(v))
            } else {
                FieldValueInput::Text(v)
            };
            let kept = validated(candidate, || Some(value.to_owned()))
                .is_some_and(|fv| sections.push(name, fv));
            if !kept {
                report.fields_to_notes += 1;
                extras.push(name, value);
            }
        }
    }
    (username, password)
}

fn convert_item(item: &Value, report: &mut ImportReport) -> Option<ImportedItem> {
    let category = str_at(item, &["categoryUuid"]).unwrap_or("");
    let overview = item.get("overview").unwrap_or(&Value::Null);
    let details = item.get("details").unwrap_or(&Value::Null);

    let title = non_empty(str_at(overview, &["title"]))
        .map(|t| clean_line(t, MAX_TITLE_CHARS))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "Untitled".to_owned());

    if let Some(history) = details.get("passwordHistory").and_then(Value::as_array) {
        report.password_history_skipped += history.len();
    }

    let mut extras = Extras::default();
    let mut fields = LoginSections::default();
    let mut totp: Option<SecretString> = None;
    let mut sso: Option<SignInWith> = None;

    // Section fields: first valid TOTP becomes the item's TOTP (logins only);
    // the first recognized "Sign in with" provider becomes sign_in_with
    // (logins only); everything else is rendered into the notes.
    let is_login = matches!(category, "001" | "005");
    let is_card = category == "002";
    let mut card_parts = CardParts::default();
    if let Some(sections) = details.get("sections").and_then(Value::as_array) {
        for section in sections {
            extras.section(str_at(section, &["title"]));
            if is_login {
                fields.start(str_at(section, &["title"]));
            }
            let Some(section_fields) = section.get("fields").and_then(Value::as_array) else {
                continue;
            };
            for field in section_fields {
                let label = str_at(field, &["title"]);
                let value = field.get("value").unwrap_or(&Value::Null);
                if is_card && card_parts.take(field, value) {
                    continue;
                }
                if is_login && totp.is_none() {
                    if let Some(t) = non_empty(str_at(value, &["totp"])) {
                        if parse_totp_input(t).is_ok() {
                            totp = Some(SecretString::from(t));
                            continue;
                        }
                    }
                }
                if is_login && sso.is_none() {
                    if let Some(p) = value
                        .get("ssoLogin")
                        .and_then(|s| str_at(s, &["provider"]))
                        .and_then(SsoProvider::from_name)
                    {
                        let account = non_empty(str_at(&value["ssoLogin"], &["username"]))
                            .map(|a| clean_line(a, MAX_ACCOUNT_CHARS))
                            .filter(|a| !a.is_empty());
                        sso = Some(SignInWith {
                            provider: p,
                            account,
                        });
                        continue;
                    }
                }
                if is_login {
                    if let Some(fv) = custom_field_from(value, report) {
                        if fields.push(label, fv) {
                            continue;
                        }
                        report.fields_to_notes += 1;
                    }
                }
                if let Some(text) = render_value(value, report) {
                    let label = if value.get("totp").is_some() {
                        Some(
                            label
                                .filter(|l| !l.trim().is_empty())
                                .unwrap_or("One-time password"),
                        )
                    } else {
                        label
                    };
                    extras.push(label, &text);
                    let mut text = text;
                    text.zeroize();
                }
            }
        }
    }

    let (tags, rejected) = split_tags(
        overview
            .get("tags")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
    );
    if !rejected.is_empty() {
        extras.section(None);
        extras.push(Some("Tags"), &rejected.join(", "));
    }

    let notes_plain = non_empty(str_at(details, &["notesPlain"]));
    let join_notes = |extra: String| -> Option<SecretString> {
        let mut parts = Vec::new();
        if let Some(n) = notes_plain {
            parts.push(n.to_owned());
        }
        if !extra.is_empty() {
            parts.push(extra);
        }
        (!parts.is_empty()).then(|| SecretString::new(parts.join("\n\n")))
    };

    let created_at = timestamp_ms(item, "createdAt");
    let updated_at = timestamp_ms(item, "updatedAt");

    let mut input = match category {
        "001" | "005" => {
            fields.start(None);
            let (username, mut password) =
                match details.get("loginFields").and_then(Value::as_array) {
                    Some(form) => login_fields(form, &mut extras, &mut fields, report),
                    None => (None, None),
                };
            // "Password" category keeps its value in details.password.
            if password.is_none() {
                if let Some(p) = non_empty(str_at(details, &["password"])) {
                    password = Some(SecretString::from(p));
                }
            }
            report.logins += 1;
            // Field order matters: `collect_urls` may add lines to the notes.
            ItemInput {
                username: username
                    .map(|u| clean_line(&u, MAX_USERNAME_CHARS))
                    .filter(|u| !u.is_empty()),
                urls: collect_urls(overview, report, &mut extras),
                password: set_or_keep(password),
                totp: set_or_keep(totp),
                notes: set_or_keep(join_notes(extras.render())),
                sign_in_with: sso,
                sections: fields.finish(),
                ..ItemInput::blank(ItemType::Login, title)
            }
        }
        "003" => {
            report.secure_notes += 1;
            ItemInput {
                content: SecretUpdate::Set(join_notes(extras.render()).unwrap_or_default()),
                ..ItemInput::blank(ItemType::SecureNote, title)
            }
        }
        "002" => {
            report.cards += 1;
            for rule in collect_urls(overview, report, &mut extras) {
                extras.section(None);
                extras.push(Some("Website"), &rule.url);
            }
            let untitled = non_empty(str_at(overview, &["title"])).is_none();
            // An untitled card with a number is named after its brand.
            let title = if untitled && card_parts.number.is_some() {
                String::new()
            } else {
                title
            };
            ItemInput {
                card: Some(CardInput {
                    cardholder_name: card_parts.cardholder,
                    brand: card_parts.brand,
                    number: set_or_keep(card_parts.number),
                    verification_number: set_or_keep(card_parts.verification),
                    expiry: card_parts.expiry,
                    notes: join_notes(extras.render()),
                }),
                ..ItemInput::blank(ItemType::Card, title)
            }
        }
        other => {
            // No native type yet: keep every field as text in a secure note.
            report.converted_to_notes += 1;
            let mut body = format!("Imported from 1Password ({})", category_name(other));
            let urls: Vec<String> = collect_urls(overview, report, &mut extras)
                .into_iter()
                .map(|r| r.url.clone())
                .collect();
            if !urls.is_empty() {
                body.push_str(&format!("\nWebsite: {}", urls.join(", ")));
            }
            let rendered = extras.render();
            if !rendered.is_empty() {
                body.push_str("\n\n");
                body.push_str(&rendered);
            }
            if let Some(n) = notes_plain {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::custom_field::{FieldKind, FieldValueInput, MAX_FIELDS};
    use serde_json::json;
    use std::io::Write;

    pub(crate) fn make_1pux(data: &Value, attachments: usize) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buf);
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file("export.attributes", opts).unwrap();
            zip.write_all(b"{\"version\":3}").unwrap();
            zip.start_file("export.data", opts).unwrap();
            zip.write_all(data.to_string().as_bytes()).unwrap();
            for i in 0..attachments {
                zip.start_file(format!("files/doc{i}__file.bin"), opts)
                    .unwrap();
                zip.write_all(b"attachment").unwrap();
            }
            zip.finish().unwrap();
        }
        buf.into_inner()
    }

    pub(crate) fn sample() -> Value {
        json!({"accounts": [{"attrs": {}, "vaults": [{"attrs": {"name": "Private"}, "items": [
            {
                "uuid": "a", "categoryUuid": "001", "state": "active",
                "createdAt": 1_600_000_000, "updatedAt": 1_700_000_000,
                "overview": {"title": "GitHub", "url": "https://github.com/login",
                    "urls": [{"label": "", "url": "https://github.com/login", "mode": "default"},
                             {"label": "", "url": "javascript:alert(1)", "mode": "default"}],
                    "tags": ["Work", "a,b"]},
                "details": {
                    "loginFields": [
                        {"value": "octo", "id": "", "name": "login", "fieldType": "T", "designation": "username"},
                        {"value": "gh-pass", "id": "", "name": "password", "fieldType": "P", "designation": "password"},
                        {"value": "✓", "id": "", "name": "remember", "fieldType": "C"}
                    ],
                    "notesPlain": "old notes",
                    "passwordHistory": [{"value": "older", "time": 1}],
                    "sections": [{"title": "Security", "name": "s", "fields": [
                        {"title": "one-time password", "id": "t", "value": {"totp": "otpauth://totp/GitHub:octo?secret=JBSWY3DPEHPK3PXP&issuer=GitHub"}},
                        {"title": "Recovery code", "id": "r", "value": {"concealed": "RC-123"}},
                        {"title": "", "id": "sso", "value": {"ssoLogin": {"provider": "Google"}}}
                    ]}]
                }
            },
            {
                "uuid": "b", "categoryUuid": "002", "state": "active",
                "createdAt": 1_600_000_000, "updatedAt": 1_600_000_000,
                "overview": {"title": "Visa"},
                "details": {"sections": [{"title": "", "fields": [
                    {"title": "number", "value": {"creditCardNumber": "4111111111111111"}},
                    {"title": "expiry date", "value": {"monthYear": 202612}},
                    {"title": "valid from", "value": {"monthYear": null}},
                    {"title": "scan", "value": {"file": {"fileName": "x.png"}}}
                ]}]}
            },
            {
                "uuid": "c", "categoryUuid": "003", "state": "active",
                "overview": {"title": "Wifi"}, "details": {"notesPlain": "pw is 1234"}
            },
            {
                "uuid": "d", "categoryUuid": "001", "state": "archived",
                "overview": {"title": "Old"}, "details": {}
            },
            {
                "uuid": "e", "categoryUuid": "005", "state": "active",
                "overview": {"title": ""}, "details": {"password": "standalone-pw"}
            }
        ]}]}]})
    }

    /// Deterministic fuzz (CLAUDE.md §47): an export file is attacker-supplied
    /// input. Mutated archives and mutated `export.data` JSON must never
    /// panic the importer.
    #[test]
    fn fuzz_never_panics() {
        let mut state: u64 = 0x1_9E37_79B9;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let archive = make_1pux(&sample(), 1);
        let json = sample().to_string().into_bytes();
        for i in 0..2_000 {
            let (mut input, zipped) = if i % 2 == 0 {
                (archive.clone(), true)
            } else {
                (json.clone(), false)
            };
            for _ in 0..=(next() % 6) {
                let pos = (next() % input.len().max(1) as u64) as usize;
                match next() % 3 {
                    0 if pos < input.len() => input[pos] ^= 1 << (next() % 8),
                    1 if pos < input.len() => {
                        input.remove(pos);
                    }
                    _ => {
                        const BYTES: &[u8] = b"{}[]\":,0-9";
                        input.insert(
                            pos.min(input.len()),
                            BYTES[(next() % BYTES.len() as u64) as usize],
                        );
                    }
                }
            }
            let bytes = if zipped {
                input
            } else {
                match serde_json::from_slice::<Value>(&input) {
                    Ok(v) => make_1pux(&v, 0),
                    Err(_) => continue,
                }
            };
            let _ = parse(&bytes);
        }
    }

    fn set_value(u: &SecretUpdate) -> Option<&str> {
        match u {
            SecretUpdate::Set(v) => Some(v.expose()),
            _ => None,
        }
    }

    fn login_with(fields: Value) -> Value {
        json!({"accounts": [{"attrs": {}, "vaults": [{"attrs": {"name": "P"}, "items": [{
            "uuid": "a", "categoryUuid": "001", "state": "active",
            "overview": {"title": "Bank"},
            "details": {
                "loginFields": [
                    {"value": "me", "name": "u", "fieldType": "T", "designation": "username"},
                    {"value": "pw", "name": "p", "fieldType": "P", "designation": "password"},
                    {"value": "branch-7", "name": "branch", "fieldType": "T"},
                    {"value": "4321", "name": "pin", "fieldType": "P"}
                ],
                "sections": [{"title": "More", "fields": fields}]
            }
        }]}]}]})
    }

    fn only_login(data: Value) -> (ImportedItem, ImportReport) {
        let mut parsed = parse(&make_1pux(&data, 0)).unwrap();
        (parsed.items.remove(0), parsed.report)
    }

    #[test]
    fn every_field_kind_maps_to_its_type() {
        let (item, _) = only_login(login_with(json!([
            {"title": "Note", "value": {"string": "hello"}},
            {"title": "Mail", "value": {"email": {"email_address": "a@b.c"}}},
            {"title": "Tel", "value": {"phone": "+55 11 5555"}},
            {"title": "Site", "value": {"url": "https://bank.example"}},
            {"title": "Opened", "value": {"date": 1_700_000_000}},
            {"title": "Home", "value": {"address": {"street": "Rua A", "city": "Recife", "zip": "50000"}}},
            {"title": "", "value": {"totp": "JBSWY3DPEHPK3PXP"}},
            {"title": "Backup", "value": {"totp": "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"}},
            {"title": "Card exp", "value": {"monthYear": 202612}},
            {"title": "Key", "value": {"sshKey": {"privateKey": "-----BEGIN KEY-----"}}}
        ])));
        let s = item.input.sections.as_ref().unwrap();
        let kinds: Vec<FieldKind> = s[0].fields.iter().map(|f| f.value.kind()).collect();
        assert_eq!(
            kinds,
            [
                FieldKind::Text,
                FieldKind::Email,
                FieldKind::Phone,
                FieldKind::Url,
                FieldKind::Date,
                FieldKind::Address,
                FieldKind::Otp,
                FieldKind::Text,
                FieldKind::Password,
            ],
            "the first TOTP stays the login's own"
        );
        assert!(set_value(&item.input.totp).is_some());
        assert!(
            matches!(&s[0].fields[4].value, FieldValueInput::Date(d) if d.expose() == "2023-11-14")
        );
        match &s[0].fields[5].value {
            FieldValueInput::Address(a) => {
                assert_eq!(a.postal_code.as_ref().unwrap().expose(), "50000");
                assert_eq!(a.city.as_ref().unwrap().expose(), "Recife");
            }
            _ => panic!("address"),
        }
        // Extra form fields: an untitled section after the 1Password ones.
        let form = &s[1];
        assert!(form.title.is_none());
        assert_eq!(form.fields[0].label.expose(), "branch");
        assert_eq!(form.fields[0].value.kind(), FieldKind::Text);
        assert_eq!(form.fields[1].value.kind(), FieldKind::Password);
    }

    #[test]
    fn odd_values_become_text_and_overflow_goes_to_notes() {
        let mut fields: Vec<Value> = vec![
            json!({"title": "Bad otp", "value": {"totp": "not base32 !!"}}),
            json!({"title": "Bad site", "value": {"url": "javascript:alert(1)"}}),
            json!({"title": "Far", "value": {"date": 400_000_000_000i64}}),
        ];
        for i in 0..110 {
            fields.push(json!({"title": format!("f{i}"), "value": {"string": format!("v{i}")}}));
        }
        let (item, report) = only_login(login_with(Value::Array(fields)));
        let s = item.input.sections.as_ref().unwrap();
        let first: Vec<FieldKind> = s[0].fields.iter().take(3).map(|f| f.value.kind()).collect();
        assert_eq!(first, [FieldKind::Text, FieldKind::Text, FieldKind::Text]);
        let total: usize = s.iter().map(|x| x.fields.len()).sum();
        assert_eq!(total, MAX_FIELDS);
        assert!(report.fields_to_notes > 0);
        let notes = set_value(&item.input.notes).unwrap();
        assert!(
            notes.contains("f109: v109"),
            "overflow is kept in the notes"
        );
    }

    fn card_item(fields: Value, title: &str) -> Value {
        json!({"accounts": [{"vaults": [{"items": [{
            "uuid": "k", "categoryUuid": "002", "state": "active",
            "overview": {"title": title},
            "details": {"notesPlain": "old", "sections": [{"title": "", "fields": fields}]}
        }]}]}]})
    }

    #[test]
    fn a_1password_card_maps_every_field() {
        let data = card_item(
            json!([
                {"title": "cardholder name", "id": "cardholder", "value": {"string": "Samuel S Rocha"}},
                {"title": "type", "id": "type", "value": {"creditCardType": "mc"}},
                {"title": "number", "id": "ccnum", "value": {"creditCardNumber": "5200 8282 8282 8210"}},
                {"title": "verification number", "id": "cvv", "value": {"concealed": "123"}},
                {"title": "expiry date", "id": "expiry", "value": {"monthYear": 203311}},
                {"title": "valid from", "id": "validFrom", "value": {"monthYear": 202011}},
                {"title": "issuing bank", "id": "bank", "value": {"string": "Nubank"}},
                {"title": "PIN", "id": "pin", "value": {"concealed": "4321"}}
            ]),
            "Mastercard",
        );
        let parsed = parse(&make_1pux(&data, 0)).unwrap();
        assert_eq!(parsed.report.cards, 1);
        let c = parsed.items[0].input.card.as_ref().unwrap();
        assert_eq!(
            c.cardholder_name.as_ref().unwrap().expose(),
            "Samuel S Rocha"
        );
        assert_eq!(c.brand, Some(crate::card::CardBrand::Mastercard));
        assert_eq!(set_value(&c.number), Some("5200828282828210"));
        assert_eq!(set_value(&c.verification_number), Some("123"));
        assert_eq!(
            c.expiry,
            Some(crate::card::CardExpiry::new(2033, 11).unwrap())
        );
        let notes = c.notes.as_ref().unwrap().expose();
        assert!(notes.starts_with("old"));
        assert!(notes.contains("valid from: 11/2020"));
        assert!(notes.contains("issuing bank: Nubank"));
        assert!(notes.contains("PIN: 4321"));
    }

    #[test]
    fn card_values_out_of_range_go_to_notes() {
        let data = card_item(
            json!([
                {"title": "type", "id": "type", "value": {"creditCardType": "laser"}},
                {"title": "number", "id": "ccnum", "value": {"creditCardNumber": "41111111111111111111"}},
                {"title": "verification number", "id": "cvv", "value": {"concealed": "123456789"}}
            ]),
            "Odd card",
        );
        let parsed = parse(&make_1pux(&data, 0)).unwrap();
        let item = &parsed.items[0];
        assert_eq!(item.input.item_type, ItemType::Card);
        let c = item.input.card.as_ref().unwrap();
        assert!(matches!(c.number, SecretUpdate::Keep));
        assert!(matches!(c.verification_number, SecretUpdate::Keep));
        assert_eq!(c.brand, None);
        let notes = c.notes.as_ref().unwrap().expose();
        assert!(notes.contains("type: laser"));
        assert!(notes.contains("number: 41111111111111111111"));
        assert!(notes.contains("verification number: 123456789"));
    }

    #[test]
    fn maps_items() {
        let parsed = parse(&make_1pux(&sample(), 1)).unwrap();
        let r = parsed.report;
        assert_eq!(parsed.items.len(), 4);
        assert_eq!(
            (r.logins, r.secure_notes, r.converted_to_notes, r.cards),
            (2, 1, 0, 1)
        );
        assert_eq!(r.skipped_archived, 1);
        assert_eq!(r.attachments_skipped, 2, "1 archive entry + 1 file field");
        assert_eq!(r.password_history_skipped, 1);
        assert_eq!(r.urls_moved_to_notes, 1);

        let gh = &parsed.items[0];
        assert_eq!(gh.input.title, "GitHub");
        assert_eq!(gh.input.username.as_deref(), Some("octo"));
        assert_eq!(set_value(&gh.input.password), Some("gh-pass"));
        assert!(set_value(&gh.input.totp).unwrap().starts_with("otpauth://"));
        assert_eq!(gh.input.urls.len(), 1);
        assert_eq!(gh.input.urls[0].url, "https://github.com/login");
        assert_eq!(gh.created_at, Some(1_600_000_000_000));
        let sso = gh
            .input
            .sign_in_with
            .as_ref()
            .expect("ssoLogin becomes sign_in_with");
        assert_eq!(sso.provider, crate::sso::SsoProvider::Google);
        assert_eq!(sso.account, None);
        let notes = set_value(&gh.input.notes).unwrap();
        assert!(notes.starts_with("old notes"));
        assert!(
            !notes.contains("RC-123"),
            "section fields are custom fields now"
        );
        let sections = gh.input.sections.as_ref().expect("login sections");
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title.as_ref().unwrap().expose(), "Security");
        assert_eq!(sections[0].fields.len(), 1, "TOTP and sso are taken first");
        assert_eq!(sections[0].fields[0].label.expose(), "Recovery code");
        assert!(matches!(
            &sections[0].fields[0].value,
            FieldValueInput::Password(SecretUpdate::Set(v)) if v.expose() == "RC-123"
        ));
        assert!(
            !notes.contains("Sign in with Google"),
            "ssoLogin no longer falls through to notes"
        );
        assert_eq!(gh.input.tags, Some(vec!["Work".to_string()]));
        assert!(
            notes.contains("Tags: a,b"),
            "a tag that cannot be one stays in the notes"
        );
        assert!(
            !notes.contains("work"),
            "a valid tag is not repeated in the notes"
        );
        assert!(
            notes.contains("Website: javascript:alert(1)"),
            "non-http URLs are kept as text"
        );
        assert!(
            !notes.contains("remember"),
            "checkboxes are UI state, not data"
        );

        let card = &parsed.items[1];
        assert_eq!(card.input.item_type, ItemType::Card);
        assert_eq!(card.input.title, "Visa");
        let c = card.input.card.as_ref().unwrap();
        assert_eq!(set_value(&c.number), Some("4111111111111111"));
        assert_eq!(
            c.expiry,
            Some(crate::card::CardExpiry::new(2026, 12).unwrap())
        );
        assert!(
            c.notes.is_none(),
            "nothing left over: valid from is empty, scan is a file"
        );

        assert_eq!(
            set_value(&parsed.items[2].input.content),
            Some("pw is 1234")
        );

        let pw = &parsed.items[3];
        assert_eq!(pw.input.title, "Untitled");
        assert_eq!(set_value(&pw.input.password), Some("standalone-pw"));
    }

    #[test]
    fn sso_login_with_account_and_unknown_provider() {
        let field =
            |v: serde_json::Value| json!({"title": "", "id": "sso", "value": {"ssoLogin": v}});
        for (value, expect) in [
            (
                json!({"provider": "GitHub", "username": "octo"}),
                Some(("github", Some("octo"))),
            ),
            (json!({"provider": "Okta"}), None),
        ] {
            let data = json!({"accounts": [{"attrs": {}, "vaults": [{"attrs": {"name": "V"}, "items": [{
                "uuid": "a", "categoryUuid": "001", "state": "active",
                "overview": {"title": "Site", "url": "https://site.example"},
                "details": {"loginFields": [], "sections": [{"title": "", "fields": [field(value)]}]}
            }]}]}]});
            let parsed = parse(&make_1pux(&data, 0)).unwrap();
            let item = &parsed.items[0];
            match expect {
                Some((p, account)) => {
                    let s = item.input.sign_in_with.as_ref().unwrap();
                    assert_eq!(serde_json::to_value(s.provider).unwrap(), p);
                    assert_eq!(s.account.as_deref(), account);
                }
                None => assert!(item.input.sign_in_with.is_none()),
            }
        }
    }

    /// CR3: an archive listing more entries than any real export is refused
    /// before the ZIP reader builds its directory.
    #[test]
    fn rejects_too_many_entries() {
        let small = make_1pux(&sample(), 3);
        assert_eq!(declared_entries(&small), 5);
        assert!(parse(&small).is_ok());

        let many = make_1pux(&sample(), MAX_ARCHIVE_ENTRIES as usize);
        assert_eq!(
            parse(&many).err(),
            Some(Error::InvalidInput("export file has too many files"))
        );
    }

    /// CR3: the ZIP64 count is read too, and a fake record in the comment
    /// does not hide the real one.
    #[test]
    fn counts_zip64_and_every_candidate_record() {
        // ZIP64 record at 0 claiming 2^40 entries, its locator, then an EOCD
        // saying "see ZIP64" (0xFFFF).
        let mut a = Vec::new();
        a.extend_from_slice(&[0x50, 0x4b, 0x06, 0x06]);
        a.extend_from_slice(&[0; 20]);
        a.extend_from_slice(&(1u64 << 40).to_le_bytes()); // this disk
        a.extend_from_slice(&(1u64 << 40).to_le_bytes()); // total
        a.extend_from_slice(&[0; 16]);
        a.extend_from_slice(&[0x50, 0x4b, 0x06, 0x07, 0, 0, 0, 0]);
        a.extend_from_slice(&0u64.to_le_bytes());
        a.extend_from_slice(&[1, 0, 0, 0]);
        a.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0]);
        a.extend_from_slice(&[0xff, 0xff, 0xff, 0xff]);
        a.extend_from_slice(&[0xff; 8]);
        // A comment holding a fake EOCD that claims one entry.
        let fake = [
            0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        a.extend_from_slice(&(fake.len() as u16).to_le_bytes());
        a.extend_from_slice(&fake);
        assert_eq!(declared_entries(&a), 1 << 40);
        assert!(parse(&a).is_err());
        assert_eq!(declared_entries(b"short"), 0);
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse(b"not a zip").is_err());
        assert!(parse(&[]).is_err());
        assert!(parse(&make_1pux(&json!({"nope": 1}), 0)).is_err());
        assert!(parse(&make_1pux(&json!({"accounts": "x"}), 0)).is_err());
        // Zip without export.data.
        let mut buf = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buf);
            zip.start_file("other", zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.finish().unwrap();
        }
        assert!(parse(&buf.into_inner()).is_err());
    }

    #[test]
    fn tolerates_odd_shapes() {
        // Items with missing/odd fields must not panic.
        let data = json!({"accounts": [{"vaults": [{"items": [
            {"categoryUuid": "001"},
            {"categoryUuid": "001", "overview": 5, "details": {"loginFields": "x", "sections": [{"fields": [{"value": 1}, {"value": {}}]}]}},
            {"categoryUuid": "999", "overview": {"title": "\u{0007}Bell"}, "details": {"sections": [{"fields": [{"value": {"weird": [1,2]}}]}]}}
        ]}, {"attrs": {}}]}]});
        let parsed = parse(&make_1pux(&data, 0)).unwrap();
        assert_eq!(parsed.items.len(), 3);
        assert_eq!(parsed.items[2].input.title, "Bell");
    }

    // Storing parsed items is tested with the staged write path
    // (`tests/writes.rs`, `tests/import_formats.rs`).

    #[test]
    fn errors_do_not_echo_content() {
        let Err(e) = parse(b"PK\x03\x04 secret-ish bytes") else {
            panic!("accepted garbage")
        };
        assert!(!e.to_string().contains("secret"));
    }
}
