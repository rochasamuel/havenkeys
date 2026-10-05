//! CSV exports: Bitwarden, Chromium browsers (Chrome, Edge, Brave),
//! Firefox, KeePassXC and LastPass.
//!
//! One reader for all of them. Header names are matched case-insensitively
//! and in any order; extra columns are ignored. Each source names the
//! columns it must have, so a file from another source is refused rather
//! than imported with the wrong columns. A row the reader cannot use (wrong
//! number of fields, not UTF-8) counts as failed and the rest go on.
//!
//! Mapping:
//! * Every row is a login, except Bitwarden rows of type `note`, LastPass
//!   rows whose url is `http://sn` (its secure notes) and KeePassXC entries
//!   with nothing but notes, which become secure notes.
//! * A TOTP value that does not parse stays in the notes.
//! * Folders/groups, favourites and LastPass's `grouping` are left out.

use super::common::{
    clean_line, collect_urls, join_notes, non_empty, parse_utc_timestamp_ms, set_or_keep, Extras,
};
use super::{ImportReport, ImportSource, ImportedItem, Parsed, MAX_ITEMS};
use crate::error::{Error, Result};
use crate::model::{
    ItemInput, ItemType, MatchType, SecretUpdate, MAX_TITLE_CHARS, MAX_USERNAME_CHARS,
};
use crate::secret::SecretString;
use crate::totp::parse_totp_input;
use std::collections::HashMap;

const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";

/// What a source's header must contain (lower case).
fn required_columns(source: ImportSource) -> &'static [&'static str] {
    match source {
        ImportSource::BitwardenCsv => &[
            "type",
            "name",
            "login_uri",
            "login_username",
            "login_password",
        ],
        ImportSource::Chrome => &["name", "url", "username", "password"],
        ImportSource::Firefox => &["url", "username", "password", "httprealm"],
        ImportSource::KeePassXc => &["title", "username", "password", "url"],
        ImportSource::LastPass => &["url", "username", "password", "extra", "name"],
        ImportSource::OnePassword | ImportSource::BitwardenJson => &[],
    }
}

const INVALID_BITWARDEN: Error = Error::InvalidInput("not a valid Bitwarden export file");
const INVALID_CHROME: Error = Error::InvalidInput("not a valid Chrome export file");
const INVALID_FIREFOX: Error = Error::InvalidInput("not a valid Firefox export file");
const INVALID_KEEPASSXC: Error = Error::InvalidInput("not a valid KeePassXC export file");
const INVALID_LASTPASS: Error = Error::InvalidInput("not a valid LastPass export file");

fn invalid(source: ImportSource) -> Error {
    match source {
        ImportSource::BitwardenCsv | ImportSource::BitwardenJson => INVALID_BITWARDEN,
        ImportSource::Chrome => INVALID_CHROME,
        ImportSource::Firefox => INVALID_FIREFOX,
        ImportSource::KeePassXc => INVALID_KEEPASSXC,
        ImportSource::LastPass => INVALID_LASTPASS,
        ImportSource::OnePassword => super::onepux::INVALID,
    }
}

/// One row, by lower-case column name. Values are borrowed from the reader's
/// record.
struct Row<'a> {
    columns: &'a HashMap<String, usize>,
    fields: Vec<&'a str>,
}

impl<'a> Row<'a> {
    /// The value as written, if it has anything but whitespace.
    fn get(&self, column: &str) -> Option<&'a str> {
        let v = *self.fields.get(*self.columns.get(column)?)?;
        (!v.trim().is_empty()).then_some(v)
    }
}

pub fn parse(source: ImportSource, bytes: &[u8]) -> Result<Parsed> {
    let required = required_columns(source);
    if required.is_empty() {
        return Err(invalid(source));
    }
    let bytes = bytes.strip_prefix(UTF8_BOM).unwrap_or(bytes);
    let mut reader = ::csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(bytes);

    let mut record = ::csv::ByteRecord::new();
    if !reader
        .read_byte_record(&mut record)
        .map_err(|_| invalid(source))?
    {
        return Err(invalid(source));
    }
    let mut columns: HashMap<String, usize> = HashMap::new();
    for (i, name) in record.iter().enumerate() {
        let name = std::str::from_utf8(name).map_err(|_| invalid(source))?;
        columns.entry(name.trim().to_ascii_lowercase()).or_insert(i);
    }
    if !required.iter().all(|c| columns.contains_key(*c)) {
        return Err(invalid(source));
    }
    let width = record.len();

    let mut report = ImportReport::default();
    let mut items = Vec::new();
    loop {
        match reader.read_byte_record(&mut record) {
            Ok(true) => {}
            Ok(false) => break,
            // The reader moves past a bad record; an I/O error cannot happen
            // on a slice, but stop rather than spin if one does.
            Err(e) if e.is_io_error() => return Err(invalid(source)),
            Err(_) => {
                report.failed += 1;
                continue;
            }
        }
        if record.iter().all(|f| f.iter().all(u8::is_ascii_whitespace)) {
            continue; // blank line
        }
        if items.len() >= MAX_ITEMS {
            return Err(Error::InvalidInput("export contains too many items"));
        }
        if record.len() != width {
            report.failed += 1;
            continue;
        }
        let Ok(fields) = record
            .iter()
            .map(std::str::from_utf8)
            .collect::<std::result::Result<Vec<&str>, _>>()
        else {
            report.failed += 1;
            continue;
        };
        let row = Row {
            columns: &columns,
            fields,
        };
        match convert_row(source, &row, &mut report) {
            Some(item) => items.push(item),
            None => report.failed += 1,
        }
    }
    // Best effort: the reader's buffer held the last row.
    record.clear();
    Ok(Parsed { items, report })
}

/// The parts of a login every source fills in its own way.
#[derive(Default)]
struct Login<'a> {
    title: Option<&'a str>,
    username: Option<&'a str>,
    password: Option<&'a str>,
    urls: Vec<&'a str>,
    totp: Option<&'a str>,
    notes: Option<&'a str>,
    /// Text kept in the notes after `notes` (e.g. Bitwarden's custom fields).
    more_notes: Option<&'a str>,
    created_at: Option<i64>,
    updated_at: Option<i64>,
}

fn convert_row(
    source: ImportSource,
    row: &Row<'_>,
    report: &mut ImportReport,
) -> Option<ImportedItem> {
    match source {
        ImportSource::Chrome => Some(login(
            Login {
                title: row.get("name"),
                username: row.get("username"),
                password: row.get("password"),
                urls: row.get("url").into_iter().collect(),
                notes: row.get("note"),
                ..Login::default()
            },
            report,
        )),
        ImportSource::Firefox => {
            let ms = |c: &str| {
                row.get(c)
                    .and_then(|v| v.trim().parse::<i64>().ok())
                    .filter(|v| *v > 0)
            };
            let created_at = ms("timecreated");
            Some(login(
                Login {
                    username: row.get("username"),
                    password: row.get("password"),
                    urls: row.get("url").into_iter().collect(),
                    created_at,
                    updated_at: ms("timepasswordchanged").or(created_at),
                    ..Login::default()
                },
                report,
            ))
        }
        ImportSource::KeePassXc => {
            let created_at = row.get("created").and_then(parse_utc_timestamp_ms);
            let updated_at = row.get("last modified").and_then(parse_utc_timestamp_ms);
            let fields = Login {
                title: row.get("title"),
                username: row.get("username"),
                password: row.get("password"),
                urls: row.get("url").into_iter().collect(),
                totp: row.get("totp"),
                notes: row.get("notes"),
                created_at,
                updated_at,
                ..Login::default()
            };
            let notes_only = fields.username.is_none()
                && fields.password.is_none()
                && fields.urls.is_empty()
                && fields.totp.is_none()
                && fields.notes.is_some();
            Some(if notes_only {
                note(
                    fields.title,
                    fields.notes,
                    None,
                    created_at,
                    updated_at,
                    report,
                )
            } else {
                login(fields, report)
            })
        }
        ImportSource::LastPass => {
            let url = row.get("url");
            if url.map(str::trim) == Some("http://sn") {
                return Some(note(
                    row.get("name"),
                    row.get("extra"),
                    None,
                    None,
                    None,
                    report,
                ));
            }
            Some(login(
                Login {
                    title: row.get("name"),
                    username: row.get("username"),
                    password: row.get("password"),
                    urls: url.into_iter().collect(),
                    totp: row.get("totp"),
                    notes: row.get("extra"),
                    ..Login::default()
                },
                report,
            ))
        }
        ImportSource::BitwardenCsv => {
            match row
                .get("type")
                .map(|t| t.trim().to_ascii_lowercase())
                .as_deref()
            {
                Some("note") => Some(note(
                    row.get("name"),
                    row.get("notes"),
                    row.get("fields"),
                    None,
                    None,
                    report,
                )),
                Some("login") => Some(login(
                    Login {
                        title: row.get("name"),
                        username: row.get("login_username"),
                        password: row.get("login_password"),
                        urls: row
                            .get("login_uri")
                            .map(|u| u.split(',').collect())
                            .unwrap_or_default(),
                        totp: row.get("login_totp"),
                        notes: row.get("notes"),
                        more_notes: row.get("fields"),
                        ..Login::default()
                    },
                    report,
                )),
                _ => None,
            }
        }
        ImportSource::OnePassword | ImportSource::BitwardenJson => None,
    }
}

fn title_or(title: Option<&str>, fallback: impl FnOnce() -> Option<String>) -> String {
    title
        .map(|t| clean_line(t, MAX_TITLE_CHARS))
        .filter(|t| !t.is_empty())
        .or_else(fallback)
        .unwrap_or_else(|| "Untitled".to_owned())
}

fn login(fields: Login<'_>, report: &mut ImportReport) -> ImportedItem {
    let mut extras = Extras::default();
    let urls = collect_urls(
        fields.urls.iter().map(|u| (*u, MatchType::Domain)),
        report,
        &mut extras,
    );
    let totp = match fields.totp {
        Some(t) if parse_totp_input(t).is_ok() => Some(SecretString::from(t.trim())),
        Some(t) => {
            report.fields_to_notes += 1;
            extras.section(None);
            extras.push(Some("One-time password"), t.trim());
            None
        }
        None => None,
    };
    if let Some(more) = non_empty(fields.more_notes) {
        extras.section(None);
        extras.push(None, more);
    }
    // Untitled browser logins are named after their website.
    let title = title_or(fields.title, || {
        let first = urls.first()?;
        let host = url::Url::parse(&first.url).ok()?.host_str()?.to_owned();
        Some(host.strip_prefix("www.").map(str::to_owned).unwrap_or(host))
    });
    report.logins += 1;
    ImportedItem {
        input: ItemInput {
            username: fields
                .username
                .map(|u| clean_line(u, MAX_USERNAME_CHARS))
                .filter(|u| !u.is_empty()),
            urls,
            password: set_or_keep(fields.password.map(SecretString::from)),
            totp: set_or_keep(totp),
            notes: set_or_keep(join_notes(fields.notes, &extras)),
            ..ItemInput::blank(ItemType::Login, title)
        },
        created_at: fields.created_at,
        updated_at: fields.updated_at.or(fields.created_at),
    }
}

fn note(
    title: Option<&str>,
    content: Option<&str>,
    more: Option<&str>,
    created_at: Option<i64>,
    updated_at: Option<i64>,
    report: &mut ImportReport,
) -> ImportedItem {
    let mut extras = Extras::default();
    if let Some(more) = non_empty(more) {
        extras.push(None, more);
    }
    report.secure_notes += 1;
    ImportedItem {
        input: ItemInput {
            content: SecretUpdate::Set(join_notes(content, &extras).unwrap_or_default()),
            ..ItemInput::blank(ItemType::SecureNote, title_or(title, || None))
        },
        created_at,
        updated_at: updated_at.or(created_at),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(u: &SecretUpdate) -> Option<&str> {
        match u {
            SecretUpdate::Set(v) => Some(v.expose()),
            _ => None,
        }
    }

    const TOTP_URI: &str = "otpauth://totp/Site:me?secret=JBSWY3DPEHPK3PXP&issuer=Site";

    #[test]
    fn chrome() {
        let csv = "\u{feff}name,url,username,password,note\n\
            github.com,https://github.com/login,octo,\"pa,ss\"\"word\",\"line 1\nline 2\"\n\
            ,https://www.example.com/,me@example.com,pw2,\n\
            app,android://abc@com.example/,u,p,\n";
        let parsed = parse(ImportSource::Chrome, csv.as_bytes()).unwrap();
        let r = parsed.report;
        assert_eq!((r.logins, r.failed, r.urls_moved_to_notes), (3, 0, 1));
        let gh = &parsed.items[0].input;
        assert_eq!(gh.title, "github.com");
        assert_eq!(gh.username.as_deref(), Some("octo"));
        assert_eq!(set(&gh.password), Some("pa,ss\"word"));
        assert_eq!(gh.urls[0].url, "https://github.com/login");
        assert_eq!(gh.urls[0].match_type, MatchType::Domain);
        assert_eq!(set(&gh.notes), Some("line 1\nline 2"));
        assert_eq!(
            parsed.items[1].input.title, "example.com",
            "untitled → host"
        );
        assert!(matches!(parsed.items[1].input.notes, SecretUpdate::Keep));
        let app = &parsed.items[2].input;
        assert!(app.urls.is_empty());
        assert_eq!(set(&app.notes), Some("Website: android://abc@com.example/"));
    }

    #[test]
    fn chrome_without_note_column_and_reordered() {
        let csv = "password,username,url,name\npw,me,https://a.example,A\n";
        let parsed = parse(ImportSource::Chrome, csv.as_bytes()).unwrap();
        let a = &parsed.items[0].input;
        assert_eq!((a.title.as_str(), a.username.as_deref()), ("A", Some("me")));
        assert_eq!(set(&a.password), Some("pw"));
    }

    #[test]
    fn firefox() {
        let csv = "\"url\",\"username\",\"password\",\"httpRealm\",\"formActionOrigin\",\"guid\",\"timeCreated\",\"timeLastUsed\",\"timePasswordChanged\"\n\
            \"https://accounts.firefox.com\",\"me@example.com\",\"pw\",,\"https://accounts.firefox.com\",\"{1}\",\"1600000000000\",\"1700000000000\",\"1650000000000\"\n";
        let parsed = parse(ImportSource::Firefox, csv.as_bytes()).unwrap();
        let item = &parsed.items[0];
        assert_eq!(item.input.title, "accounts.firefox.com");
        assert_eq!(item.created_at, Some(1_600_000_000_000));
        assert_eq!(item.updated_at, Some(1_650_000_000_000));
    }

    #[test]
    fn keepassxc() {
        let csv = format!(
            "\"Group\",\"Title\",\"Username\",\"Password\",\"URL\",\"Notes\",\"TOTP\",\"Icon\",\"Last Modified\",\"Created\"\n\
            \"Root\",\"Site\",\"me\",\"pw\",\"https://site.example\",\"\",\"{TOTP_URI}\",\"0\",\"2025-01-02T03:04:05Z\",\"2024-01-01T00:00:00Z\"\n\
            \"Root\",\"Raw\",\"me\",\"pw\",\"\",\"\",\"JBSWY3DPEHPK3PXP\",\"0\",\"\",\"\"\n\
            \"Root\",\"Bad\",\"me\",\"pw\",\"\",\"n\",\"not a code\",\"0\",\"\",\"\"\n\
            \"Root\",\"Wifi\",\"\",\"\",\"\",\"pw is 1234\",\"\",\"0\",\"\",\"\"\n"
        );
        let parsed = parse(ImportSource::KeePassXc, csv.as_bytes()).unwrap();
        let r = parsed.report;
        assert_eq!((r.logins, r.secure_notes, r.fields_to_notes), (3, 1, 1));
        assert_eq!(set(&parsed.items[0].input.totp), Some(TOTP_URI));
        assert_eq!(parsed.items[0].created_at, Some(1_704_067_200_000));
        assert_eq!(parsed.items[0].updated_at, Some(1_735_787_045_000));
        assert_eq!(set(&parsed.items[1].input.totp), Some("JBSWY3DPEHPK3PXP"));
        let bad = &parsed.items[2].input;
        assert!(matches!(bad.totp, SecretUpdate::Keep));
        assert_eq!(set(&bad.notes), Some("n\n\nOne-time password: not a code"));
        let wifi = &parsed.items[3].input;
        assert_eq!(wifi.item_type, ItemType::SecureNote);
        assert_eq!(set(&wifi.content), Some("pw is 1234"));
    }

    #[test]
    fn lastpass() {
        let csv = format!(
            "url,username,password,totp,extra,name,grouping,fav\n\
            https://site.example/login,me,pw,{TOTP_URI},some notes,Site,Work,0\n\
            http://sn,,,,\"NoteType:Credit Card\nNumber:4111\",Visa,Cards,0\n"
        );
        let parsed = parse(ImportSource::LastPass, csv.as_bytes()).unwrap();
        let r = parsed.report;
        assert_eq!((r.logins, r.secure_notes), (1, 1));
        let site = &parsed.items[0].input;
        assert_eq!(set(&site.notes), Some("some notes"));
        assert_eq!(set(&site.totp), Some(TOTP_URI));
        let card = &parsed.items[1].input;
        assert_eq!(card.item_type, ItemType::SecureNote);
        assert_eq!(card.title, "Visa");
        assert_eq!(
            set(&card.content),
            Some("NoteType:Credit Card\nNumber:4111")
        );
    }

    #[test]
    fn bitwarden_csv() {
        let csv = "folder,favorite,type,name,notes,fields,reprompt,login_uri,login_username,login_password,login_totp\n\
            ,,login,Site,n,\"PIN: 1234\",0,\"https://a.example,https://b.example\",me,pw,JBSWY3DPEHPK3PXP\n\
            Docs,1,note,Memo,body,,0,,,,\n\
            ,,card,Visa,,,0,,,,\n";
        let parsed = parse(ImportSource::BitwardenCsv, csv.as_bytes()).unwrap();
        let r = parsed.report;
        assert_eq!((r.logins, r.secure_notes, r.failed), (1, 1, 1));
        let site = &parsed.items[0].input;
        assert_eq!(site.urls.len(), 2);
        assert_eq!(set(&site.notes), Some("n\n\nPIN: 1234"));
        assert_eq!(set(&parsed.items[1].input.content), Some("body"));
    }

    #[test]
    fn broken_rows_fail_alone() {
        let mut csv = b"name,url,username,password\nA,https://a.example,u,p\nB,only-two\n".to_vec();
        csv.extend_from_slice(b"C,https://c.example,\xff\xfe,p\nD,https://d.example,u,p\n");
        let parsed = parse(ImportSource::Chrome, &csv).unwrap();
        assert_eq!((parsed.report.logins, parsed.report.failed), (2, 2));
        // An unclosed quote swallows the rest of the file into one row.
        let csv = "name,url,username,password\nA,https://a.example,u,p\n\"B,x,y,z\nC,d,e,f\n";
        let parsed = parse(ImportSource::Chrome, csv.as_bytes()).unwrap();
        assert_eq!((parsed.report.logins, parsed.report.failed), (1, 1));
    }

    #[test]
    fn refuses_other_sources_files() {
        let chrome = "name,url,username,password\nA,https://a.example,u,p\n";
        let firefox = "url,username,password,httpRealm\nhttps://a.example,u,p,\n";
        for (source, file) in [
            (ImportSource::Firefox, chrome),
            (ImportSource::Chrome, firefox),
            (ImportSource::KeePassXc, chrome),
            (ImportSource::LastPass, chrome),
            (ImportSource::BitwardenCsv, chrome),
            (ImportSource::Chrome, ""),
            (ImportSource::Chrome, "\u{feff}"),
            (ImportSource::Chrome, "{\"items\": []}"),
        ] {
            assert!(parse(source, file.as_bytes()).is_err(), "{source:?}");
        }
        assert!(parse(ImportSource::Chrome, b"\xffname,url,username,password\n").is_err());
        assert!(parse(ImportSource::OnePassword, chrome.as_bytes()).is_err());
    }

    #[test]
    fn header_only_and_blank_lines() {
        let csv = "name,url,username,password\n\n,,,\n";
        let parsed = parse(ImportSource::Chrome, csv.as_bytes()).unwrap();
        assert!(parsed.items.is_empty());
        assert_eq!(parsed.report.failed, 0);
    }

    #[test]
    fn errors_do_not_echo_content() {
        let e = parse(ImportSource::Chrome, b"secret-header\nhunter2\n")
            .err()
            .unwrap();
        assert!(!e.to_string().contains("hunter2") && !e.to_string().contains("secret"));
    }

    #[test]
    fn too_many_rows() {
        let mut csv = String::from("name,url,username,password\n");
        for _ in 0..=MAX_ITEMS {
            csv.push_str("a,,u,p\n");
        }
        assert!(parse(ImportSource::Chrome, csv.as_bytes()).is_err());
    }

    /// Deterministic fuzz (CLAUDE.md §47): mutated exports never panic.
    #[test]
    fn fuzz_never_panics() {
        let mut state: u64 = 0x5EED_C5F0;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let samples = [
            (ImportSource::Chrome, "name,url,username,password,note\nA,https://a.example,u,\"p,\"\"q\",n\n".to_owned()),
            (ImportSource::Firefox, "url,username,password,httpRealm,timeCreated,timePasswordChanged\nhttps://a.example,u,p,,1600000000000,9\n".to_owned()),
            (ImportSource::KeePassXc, format!("Title,Username,Password,URL,Notes,TOTP,Created\nA,u,p,https://a.example,n,{TOTP_URI},2024-01-01T00:00:00Z\n")),
            (ImportSource::LastPass, "url,username,password,totp,extra,name\nhttp://sn,,,,x,N\nhttps://a.example,u,p,,e,A\n".to_owned()),
            (ImportSource::BitwardenCsv, "type,name,notes,fields,login_uri,login_username,login_password,login_totp\nlogin,A,n,f,\"https://a.example,x\",u,p,JBSWY3DPEHPK3PXP\n".to_owned()),
        ];
        for i in 0..5_000 {
            let (source, sample) = &samples[i % samples.len()];
            let mut input = sample.clone().into_bytes();
            for _ in 0..=(next() % 6) {
                let pos = (next() % input.len().max(1) as u64) as usize;
                match next() % 3 {
                    0 if pos < input.len() => input[pos] ^= 1 << (next() % 8),
                    1 if pos < input.len() => {
                        input.remove(pos);
                    }
                    _ => {
                        const BYTES: &[u8] = b",\"\n\r:/ \xEF\xBB\xBF0";
                        input.insert(
                            pos.min(input.len()),
                            BYTES[(next() % BYTES.len() as u64) as usize],
                        );
                    }
                }
            }
            let _ = parse(*source, &input);
        }
    }
}
