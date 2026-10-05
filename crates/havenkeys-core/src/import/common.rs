//! Building blocks the importers share: line cleaning, notes text, website
//! rules, login custom fields and card brands. Nothing here knows a file
//! format.

use super::ImportReport;
use crate::card::CardBrand;
use crate::custom_field::{
    clean_plain, FieldInput, FieldKind, FieldValueInput, SectionInput, MAX_FIELDS, MAX_LABEL_CHARS,
    MAX_SECTIONS, MAX_TOTAL_BYTES,
};
use crate::model::{check_password, normalize_url, MatchType, SecretUpdate, UrlRule, MAX_URLS};
use crate::secret::SecretString;
use crate::totp::parse_totp_input;
use serde_json::Value;
use zeroize::Zeroize;

pub(super) fn str_at<'a>(v: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut cur = v;
    for key in path {
        cur = cur.get(key)?;
    }
    cur.as_str()
}

/// Best-effort wipe of every string in a parsed JSON tree.
pub(super) fn wipe(v: &mut Value) {
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

pub(super) fn non_empty(s: Option<&str>) -> Option<&str> {
    s.map(str::trim).filter(|s| !s.is_empty())
}

pub(super) fn clean_line(s: &str, max_chars: usize) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .take(max_chars)
        .collect::<String>()
        .trim()
        .to_owned()
}

pub(super) fn set_or_keep(value: Option<SecretString>) -> SecretUpdate {
    value.map_or(SecretUpdate::Keep, SecretUpdate::Set)
}

/// Put `value` in `slot` if there is one. Whether it was taken.
pub(super) fn fill<T>(slot: &mut Option<T>, value: Option<T>) -> bool {
    match value {
        Some(v) => {
            *slot = Some(v);
            true
        }
        None => false,
    }
}

/// Collected free-text lines appended to notes.
#[derive(Default)]
pub(super) struct Extras {
    lines: Vec<String>,
    current_section: Option<String>,
}

impl Extras {
    pub(super) fn section(&mut self, title: Option<&str>) {
        self.current_section = non_empty(title).map(str::to_owned);
    }

    pub(super) fn push(&mut self, label: Option<&str>, value: &str) {
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

    pub(super) fn render(&self) -> String {
        self.lines.join("\n")
    }
}

impl Drop for Extras {
    fn drop(&mut self) {
        self.lines.iter_mut().for_each(|l| l.zeroize());
    }
}

/// `first` and the extra lines joined as one notes text, or `None` when
/// both are empty.
pub(super) fn join_notes(first: Option<&str>, extras: &Extras) -> Option<SecretString> {
    let mut rendered = extras.render();
    let out = match (non_empty(first), rendered.is_empty()) {
        (None, true) => None,
        (Some(n), true) => Some(n.to_owned()),
        (None, false) => Some(rendered.clone()),
        (Some(n), false) => Some(format!("{n}\n\n{rendered}")),
    };
    rendered.zeroize();
    out.map(SecretString::new)
}

/// A login's custom fields (spec 2026-09-30-login-custom-fields §6).
#[derive(Default)]
pub(super) struct LoginSections {
    sections: Vec<SectionInput>,
    count: usize,
    /// Bytes of titles, labels and values so far, counted strictly (at least
    /// what the vault counts) against `MAX_TOTAL_BYTES`.
    bytes: usize,
}

impl LoginSections {
    pub(super) fn start(&mut self, title: Option<&str>) {
        let title = non_empty(title)
            .map(|t| clean_line(t, MAX_LABEL_CHARS))
            .filter(|t| !t.is_empty())
            .map(SecretString::new);
        self.bytes += title.as_ref().map_or(0, |t| t.expose().len());
        self.sections.push(SectionInput {
            id: None,
            title,
            fields: Vec::new(),
        });
    }

    /// Add a field to the current section; false when the login is full, so
    /// the caller keeps the value in the notes.
    pub(super) fn push(&mut self, label: Option<&str>, value: FieldValueInput) -> bool {
        let used = self
            .sections
            .iter()
            .filter(|s| !s.fields.is_empty())
            .count();
        let Some(section) = self.sections.last_mut() else {
            return false;
        };
        if self.count >= MAX_FIELDS || (section.fields.is_empty() && used >= MAX_SECTIONS) {
            return false;
        }
        let label = non_empty(label)
            .map(|l| clean_line(l, MAX_LABEL_CHARS))
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| default_label(value.kind()).to_owned());
        let added = label.len() + value_bytes(&value);
        if self.bytes + added > MAX_TOTAL_BYTES {
            return false;
        }
        self.bytes += added;
        section.fields.push(FieldInput {
            id: None,
            label: SecretString::new(label),
            value,
        });
        self.count += 1;
        true
    }

    pub(super) fn finish(self) -> Option<Vec<SectionInput>> {
        let sections: Vec<SectionInput> = self
            .sections
            .into_iter()
            .filter(|s| !s.fields.is_empty())
            .collect();
        (!sections.is_empty()).then_some(sections)
    }
}

fn value_bytes(value: &FieldValueInput) -> usize {
    match value {
        // The vault stores the normalised URL, which can be longer than what
        // was read (a trailing `/`, a scheme), so count whichever is more.
        FieldValueInput::Url(v) => normalize_url(v.expose())
            .map_or(0, |n| n.len())
            .max(v.expose().len()),
        FieldValueInput::Text(v)
        | FieldValueInput::Email(v)
        | FieldValueInput::Phone(v)
        | FieldValueInput::Date(v) => v.expose().len(),
        FieldValueInput::Address(a) => a.formatted().expose().len(),
        FieldValueInput::Password(SecretUpdate::Set(v))
        | FieldValueInput::Otp(SecretUpdate::Set(v)) => v.expose().len(),
        FieldValueInput::Password(_) | FieldValueInput::Otp(_) => 0,
    }
}

/// `candidate` if it passes its type's checks, else `fallback` as Text if
/// that passes, else `None` (the caller keeps the value in the notes).
pub(super) fn validated(
    candidate: FieldValueInput,
    fallback: impl FnOnce() -> Option<String>,
) -> Option<FieldValueInput> {
    let passes = match &candidate {
        FieldValueInput::Otp(SecretUpdate::Set(t)) => parse_totp_input(t.expose()).is_ok(),
        FieldValueInput::Password(SecretUpdate::Set(p)) => check_password(p).is_ok(),
        other => clean_plain(other).is_ok(),
    };
    if passes {
        return Some(candidate);
    }
    let as_text = FieldValueInput::Text(SecretString::new(fallback()?));
    clean_plain(&as_text).is_ok().then_some(as_text)
}

fn default_label(kind: FieldKind) -> &'static str {
    match kind {
        FieldKind::Text => "Text",
        FieldKind::Url => "URL",
        FieldKind::Email => "Email",
        FieldKind::Phone => "Phone",
        FieldKind::Date => "Date",
        FieldKind::Address => "Address",
        FieldKind::Password => "Password",
        FieldKind::Otp => "One-time password",
    }
}

/// Valid http(s) websites become URL rules with their match type; anything
/// else (app links, bare words) is kept as text in the notes rather than
/// dropped.
pub(super) fn collect_urls<'a>(
    raw: impl IntoIterator<Item = (&'a str, MatchType)>,
    report: &mut ImportReport,
    extras: &mut Extras,
) -> Vec<UrlRule> {
    let mut out: Vec<UrlRule> = Vec::new();
    for (candidate, match_type) in raw {
        let candidate = candidate.trim();
        if candidate.is_empty() {
            continue;
        }
        match normalize_url(candidate) {
            Ok(url) if !out.iter().any(|r| r.url == url) => {
                if out.len() < MAX_URLS {
                    out.push(UrlRule { url, match_type });
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

/// A card brand by its name as password managers write it.
pub(super) fn brand_from_name(value: &str) -> Option<CardBrand> {
    Some(match value.trim().to_ascii_lowercase().as_str() {
        "visa" | "visaelectron" | "electron" => CardBrand::Visa,
        "mc" | "mastercard" | "master" => CardBrand::Mastercard,
        "amex" | "americanexpress" | "american express" => CardBrand::Amex,
        "elo" => CardBrand::Elo,
        "hipercard" | "hiper" => CardBrand::Hipercard,
        "diners" | "dinersclub" | "diners club" | "carteblanche" => CardBrand::Diners,
        "discover" => CardBrand::Discover,
        "jcb" => CardBrand::Jcb,
        "unionpay" | "china unionpay" => CardBrand::Unionpay,
        "maestro" => CardBrand::Maestro,
        _ => return None,
    })
}

/// Unix days → (year, month, day), proleptic Gregorian.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
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
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// (year, month, day) → Unix days, proleptic Gregorian.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Unix seconds → YYYY-MM-DD (UTC).
pub(super) fn format_date(secs: i64) -> String {
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

/// A UTC timestamp written as `YYYY-MM-DDTHH:MM:SS[.fff]Z` (what Bitwarden
/// and KeePassXC export) → Unix ms. Anything else, or a date before 1970,
/// is `None`: the item then gets the import time.
pub(super) fn parse_utc_timestamp_ms(s: &str) -> Option<i64> {
    let s = s.trim();
    let b = s.as_bytes();
    if b.len() < 20 || !s.is_ascii() {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> {
        let part = &s[r];
        part.bytes()
            .all(|c| c.is_ascii_digit())
            .then(|| part.parse().ok())?
    };
    if b[4] != b'-'
        || b[7] != b'-'
        || !matches!(b[10], b'T' | b' ')
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, sec) = (num(11..13)?, num(14..16)?, num(17..19)?);
    let rest = &s[19..];
    let (frac, zone) = match rest.strip_prefix('.') {
        Some(r) => {
            let digits = r.bytes().take_while(u8::is_ascii_digit).count();
            (&r[..digits], &r[digits..])
        }
        None => ("", rest),
    };
    if zone != "Z"
        || !(1..=12).contains(&mo)
        || !(1..=31).contains(&d)
        || h > 23
        || mi > 59
        || sec > 60
    {
        return None;
    }
    let ms: i64 = if frac.is_empty() {
        0
    } else {
        format!("{:0<3}", &frac[..frac.len().min(3)]).parse().ok()?
    };
    let days = days_from_civil(y, mo, d);
    let total = days * 86_400_000 + (h * 3600 + mi * 60 + sec) * 1000 + ms;
    (total > 0).then_some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_format() {
        assert_eq!(format_date(0), "1970-01-01");
        assert_eq!(format_date(951_782_400), "2000-02-29");
        assert_eq!(format_date(1_758_240_000), "2025-09-19");
    }

    #[test]
    fn utc_timestamps_parse() {
        assert_eq!(
            parse_utc_timestamp_ms("2025-09-19T00:00:00Z"),
            Some(1_758_240_000_000)
        );
        assert_eq!(
            parse_utc_timestamp_ms("2000-02-29T00:00:01.5Z"),
            Some(951_782_401_500)
        );
        assert_eq!(
            parse_utc_timestamp_ms("2023-11-14T22:13:20.123456Z"),
            Some(1_700_000_000_123)
        );
        for bad in [
            "",
            "2025-09-19",
            "2025-13-01T00:00:00Z",
            "2025-09-19T00:00:00+02:00",
            "1960-01-01T00:00:00Z",
            "20x5-09-19T00:00:00Z",
            "2025-09-19T00:00:00Zjunk",
            "２025-09-19T00:00:00Z",
        ] {
            assert_eq!(parse_utc_timestamp_ms(bad), None, "{bad}");
        }
    }

    #[test]
    fn notes_join() {
        let mut extras = Extras::default();
        assert!(join_notes(Some("  "), &extras).is_none());
        assert_eq!(join_notes(Some("a"), &extras).unwrap().expose(), "a");
        extras.push(Some("k"), "v");
        assert_eq!(join_notes(None, &extras).unwrap().expose(), "k: v");
        assert_eq!(
            join_notes(Some("a"), &extras).unwrap().expose(),
            "a\n\nk: v"
        );
    }
}
