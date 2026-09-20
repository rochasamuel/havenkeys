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
//! * Every other category (credit card, identity, SSH key, API credential, …)
//!   → secure note with all fields written out as text.
//! * Archived/deleted items, attachments and password history are skipped
//!   and counted.

use super::{ImportReport, ImportedItem};
use crate::error::{Error, Result};
use crate::model::{
    normalize_url, ItemInput, ItemType, MatchType, SecretUpdate, UrlRule, MAX_TITLE_CHARS,
    MAX_URLS, MAX_USERNAME_CHARS,
};
use crate::secret::SecretString;
use crate::totp::parse_totp_input;
use serde_json::Value;
use std::io::{Cursor, Read};
use zeroize::{Zeroize, Zeroizing};

/// Largest `.1pux` archive accepted.
pub const MAX_ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;
/// Largest uncompressed `export.data` accepted (zip-bomb guard).
pub const MAX_EXPORT_DATA_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_ITEMS: usize = 50_000;

const INVALID: Error = Error::InvalidInput("not a valid 1Password export (.1pux) file");

/// Parse result: items to store plus what was skipped while parsing.
/// `report.imported`/`failed`/`skipped_duplicates` are filled in by the vault.
pub struct Parsed {
    pub items: Vec<ImportedItem>,
    pub report: ImportReport,
}

pub fn parse(archive: &[u8]) -> Result<Parsed> {
    if archive.len() as u64 > MAX_ARCHIVE_BYTES {
        return Err(Error::InvalidInput("export file is too large"));
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

/// Best-effort wipe of every string in the parsed JSON tree.
fn wipe(v: &mut Value) {
    match v {
        Value::String(s) => s.zeroize(),
        Value::Array(a) => a.iter_mut().for_each(wipe),
        Value::Object(o) => {
            for (_, x) in o.iter_mut() {
                wipe(x);
            }
        }
        _ => {}
    }
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

fn str_at<'a>(v: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut cur = v;
    for key in path {
        cur = cur.get(key)?;
    }
    cur.as_str()
}

fn non_empty(s: Option<&str>) -> Option<&str> {
    s.map(str::trim).filter(|s| !s.is_empty())
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

fn clean_line(s: &str, max_chars: usize) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .take(max_chars)
        .collect::<String>()
        .trim()
        .to_owned()
}

/// Collected free-text lines appended to notes.
#[derive(Default)]
struct Extras {
    lines: Vec<String>,
    current_section: Option<String>,
}

impl Extras {
    fn section(&mut self, title: Option<&str>) {
        self.current_section = non_empty(title).map(str::to_owned);
    }

    fn push(&mut self, label: Option<&str>, value: &str) {
        if value.trim().is_empty() {
            return;
        }
        if let Some(section) = self.current_section.take() {
            if !self.lines.is_empty() {
                self.lines.push(String::new());
            }
            self.lines.push(format!("[{section}]"));
        }
        match non_empty(label) {
            Some(l) => self.lines.push(format!("{l}: {value}")),
            None => self.lines.push(value.to_owned()),
        }
    }

    fn render(&self) -> String {
        self.lines.join("\n")
    }
}

impl Drop for Extras {
    fn drop(&mut self) {
        self.lines.iter_mut().for_each(|l| l.zeroize());
    }
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
        "ssoLogin" => non_empty(str_at(v, &["provider"])).map(|p| format!("Sign in with {p}")),
        "file" => {
            report.attachments_skipped += 1;
            None
        }
        _ => v.as_str().map(str::to_owned),
    };
    text.filter(|t| !t.trim().is_empty())
}

/// Unix seconds → YYYY-MM-DD (proleptic Gregorian, UTC).
fn format_date(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    // Howard Hinnant's days-from-civil inverse.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
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
    let mut out: Vec<UrlRule> = Vec::new();
    for candidate in raw {
        match normalize_url(candidate) {
            Ok(url) if !out.iter().any(|r| r.url == url) => {
                if out.len() < MAX_URLS {
                    out.push(UrlRule {
                        url,
                        match_type: MatchType::Domain,
                    });
                }
            }
            Ok(_) => {}
            Err(_) => {
                report.urls_moved_to_notes += 1;
                extras.section(None);
                extras.push(Some("Website"), candidate);
            }
        }
    }
    out
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
    let mut totp: Option<SecretString> = None;

    // Section fields: first valid TOTP becomes the item's TOTP (logins only);
    // everything else is rendered into the notes.
    let is_login = matches!(category, "001" | "005");
    if let Some(sections) = details.get("sections").and_then(Value::as_array) {
        for section in sections {
            extras.section(str_at(section, &["title"]));
            let Some(fields) = section.get("fields").and_then(Value::as_array) else {
                continue;
            };
            for field in fields {
                let label = str_at(field, &["title"]);
                let value = field.get("value").unwrap_or(&Value::Null);
                if is_login && totp.is_none() {
                    if let Some(t) = non_empty(str_at(value, &["totp"])) {
                        if parse_totp_input(t).is_ok() {
                            totp = Some(SecretString::from(t));
                            continue;
                        }
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

    if let Some(tags) = overview.get("tags").and_then(Value::as_array) {
        let tags: Vec<&str> = tags.iter().filter_map(Value::as_str).collect();
        if !tags.is_empty() {
            extras.section(None);
            extras.push(Some("Tags"), &tags.join(", "));
        }
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

    let input = match category {
        "001" | "005" => {
            let mut username: Option<String> = None;
            let mut password: Option<SecretString> = None;
            if let Some(fields) = details.get("loginFields").and_then(Value::as_array) {
                // Designated fields first.
                for f in fields {
                    let value = non_empty(str_at(f, &["value"]));
                    match (str_at(f, &["designation"]), value) {
                        (Some("username"), Some(v)) if username.is_none() => {
                            username = Some(v.to_owned())
                        }
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
                        extras.push(non_empty(str_at(f, &["name"])), value);
                    }
                }
            }
            // "Password" category keeps its value in details.password.
            if password.is_none() {
                if let Some(p) = non_empty(str_at(details, &["password"])) {
                    password = Some(SecretString::from(p));
                }
            }
            report.logins += 1;
            ItemInput {
                item_type: ItemType::Login,
                title,
                username: username
                    .map(|u| clean_line(&u, MAX_USERNAME_CHARS))
                    .filter(|u| !u.is_empty()),
                urls: collect_urls(overview, report, &mut extras),
                password: password.map_or(SecretUpdate::Keep, SecretUpdate::Set),
                totp: totp.map_or(SecretUpdate::Keep, SecretUpdate::Set),
                notes: join_notes(extras.render()).map_or(SecretUpdate::Keep, SecretUpdate::Set),
                content: SecretUpdate::Keep,
            }
        }
        "003" => {
            report.secure_notes += 1;
            ItemInput {
                item_type: ItemType::SecureNote,
                title,
                username: None,
                urls: Vec::new(),
                password: SecretUpdate::Keep,
                totp: SecretUpdate::Keep,
                notes: SecretUpdate::Keep,
                content: SecretUpdate::Set(join_notes(extras.render()).unwrap_or_default()),
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
                item_type: ItemType::SecureNote,
                title,
                username: None,
                urls: Vec::new(),
                password: SecretUpdate::Keep,
                totp: SecretUpdate::Keep,
                notes: SecretUpdate::Keep,
                content: SecretUpdate::Set(content),
            }
        }
    };

    Some(ImportedItem {
        input,
        created_at,
        updated_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
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
                    "tags": ["work"]},
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

    #[test]
    fn maps_items() {
        let parsed = parse(&make_1pux(&sample(), 1)).unwrap();
        let r = parsed.report;
        assert_eq!(parsed.items.len(), 4);
        assert_eq!((r.logins, r.secure_notes, r.converted_to_notes), (2, 1, 1));
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
        let notes = set_value(&gh.input.notes).unwrap();
        assert!(notes.starts_with("old notes"));
        assert!(notes.contains("[Security]\nRecovery code: RC-123"));
        assert!(notes.contains("Sign in with Google"));
        assert!(notes.contains("Tags: work"));
        assert!(
            notes.contains("Website: javascript:alert(1)"),
            "non-http URLs are kept as text"
        );
        assert!(
            !notes.contains("remember"),
            "checkboxes are UI state, not data"
        );

        let card = &parsed.items[1];
        assert_eq!(card.input.item_type, ItemType::SecureNote);
        let body = set_value(&card.input.content).unwrap();
        assert!(body.starts_with("Imported from 1Password (Credit card)"));
        assert!(body.contains("number: 4111111111111111"));
        assert!(body.contains("expiry date: 12/2026"));
        assert!(!body.contains("valid from"));

        assert_eq!(
            set_value(&parsed.items[2].input.content),
            Some("pw is 1234")
        );

        let pw = &parsed.items[3];
        assert_eq!(pw.input.title, "Untitled");
        assert_eq!(set_value(&pw.input.password), Some("standalone-pw"));
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

    // Storing parsed items in a vault is exercised where the write path
    // lives now (`VaultService::stage_create`/`commit_write`); import itself
    // is rebuilt on top of staged writes once there is a server to send them
    // to (spec 2026-09-20 §8.4, §13). Until then `import_1pux` refuses with
    // `Error::Offline` (see `apps/desktop/src-tauri/src/import.rs`), so there
    // is nothing left here to store `parse`'s output into.

    #[test]
    fn dates_format() {
        assert_eq!(format_date(0), "1970-01-01");
        assert_eq!(format_date(951_782_400), "2000-02-29");
        assert_eq!(format_date(1_758_240_000), "2025-09-19");
    }

    #[test]
    fn errors_do_not_echo_content() {
        let Err(e) = parse(b"PK\x03\x04 secret-ish bytes") else {
            panic!("accepted garbage")
        };
        assert!(!e.to_string().contains("secret"));
    }
}
