//! The Identity item: the person's name, documents, contact details, address
//! and anything else they want to remember (spec 2026-09-29-identity-item).
//!
//! Every value is a [`SecretString`]: wiped on drop, never printed. The whole
//! identity lives in the item's details blob; only the display name and the
//! email are copied into the overview, for the list and search.

use crate::error::{Error, Result};
use crate::model::normalize_url;
use crate::secret::SecretString;
use serde::{Deserialize, Serialize};
use std::fmt;

pub const MAX_NAME_CHARS: usize = 256;
pub const MAX_DOCUMENT_CHARS: usize = 64;
pub const MAX_EMAIL_CHARS: usize = 512;
pub const MAX_PHONE_CHARS: usize = 32;
pub const MAX_SHORT_CHARS: usize = 32;
pub const MAX_CUSTOM_FIELDS: usize = 50;
pub const MAX_CUSTOM_LABEL_CHARS: usize = 128;
pub const MAX_CUSTOM_VALUE_CHARS: usize = 4096;
pub const MAX_IDENTITY_NOTES_BYTES: usize = 64 * 1024;
const MAX_WEBSITE_LEN: usize = 2048;
const MIN_BIRTH_YEAR: i64 = 1850;

/// A field the user added: anything else they want to remember.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CustomField {
    pub label: SecretString,
    pub value: SecretString,
    /// Masked in the UI until revealed, like a password.
    #[serde(default)]
    pub hidden: bool,
}

/// The identity's values. All optional; an empty identity is valid.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IdentityFields {
    // Identification
    #[serde(default)]
    pub first_name: Option<SecretString>,
    #[serde(default)]
    pub middle_name: Option<SecretString>,
    #[serde(default)]
    pub last_name: Option<SecretString>,
    #[serde(default)]
    pub gender: Option<SecretString>,
    /// `YYYY-MM-DD`.
    #[serde(default)]
    pub birth_date: Option<SecretString>,
    #[serde(default)]
    pub occupation: Option<SecretString>,
    #[serde(default)]
    pub company: Option<SecretString>,
    #[serde(default)]
    pub job_title: Option<SecretString>,
    // Documents
    #[serde(default)]
    pub cpf: Option<SecretString>,
    #[serde(default)]
    pub rg: Option<SecretString>,
    #[serde(default)]
    pub passport: Option<SecretString>,
    #[serde(default)]
    pub drivers_license: Option<SecretString>,
    // Contact
    #[serde(default)]
    pub email: Option<SecretString>,
    #[serde(default)]
    pub mobile_phone: Option<SecretString>,
    #[serde(default)]
    pub home_phone: Option<SecretString>,
    #[serde(default)]
    pub work_phone: Option<SecretString>,
    // Address
    #[serde(default)]
    pub street: Option<SecretString>,
    #[serde(default)]
    pub number: Option<SecretString>,
    #[serde(default)]
    pub complement: Option<SecretString>,
    #[serde(default)]
    pub neighborhood: Option<SecretString>,
    #[serde(default)]
    pub city: Option<SecretString>,
    #[serde(default)]
    pub state: Option<SecretString>,
    #[serde(default)]
    pub postal_code: Option<SecretString>,
    #[serde(default)]
    pub country: Option<SecretString>,
    // Internet
    #[serde(default)]
    pub username: Option<SecretString>,
    #[serde(default)]
    pub website: Option<SecretString>,
    // Everything else
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom: Vec<CustomField>,
    #[serde(default)]
    pub notes: Option<SecretString>,
}

impl fmt::Debug for IdentityFields {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IdentityFields(<redacted>)")
    }
}

impl fmt::Debug for CustomField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CustomField")
            .field("hidden", &self.hidden)
            .finish_non_exhaustive()
    }
}

/// A single identity value the desktop may copy. `Address` is the formatted
/// block; custom fields are addressed by index separately.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IdentityField {
    FirstName,
    MiddleName,
    LastName,
    Gender,
    BirthDate,
    Occupation,
    Company,
    JobTitle,
    Cpf,
    Rg,
    Passport,
    DriversLicense,
    Email,
    MobilePhone,
    HomePhone,
    WorkPhone,
    Street,
    Number,
    Complement,
    Neighborhood,
    City,
    State,
    PostalCode,
    Country,
    Username,
    Website,
    Notes,
    Address,
}

/// What a web form field asks for, as the browser extension classifies it
/// (spec 2026-09-29-identity-autofill §4). Some roles are derived from
/// several identity values; the extension never gets more than a role's
/// value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FillRole {
    FullName,
    FirstName,
    MiddleName,
    LastName,
    Email,
    Phone,
    BirthDate,
    BirthDay,
    BirthMonth,
    BirthYear,
    Company,
    Street,
    Number,
    Complement,
    AddressLine1,
    AddressLine2,
    Neighborhood,
    City,
    State,
    PostalCode,
    Country,
    Username,
    Cpf,
    Rg,
    Passport,
    DriversLicense,
}

impl FillRole {
    pub const ALL: [FillRole; 26] = [
        FillRole::FullName,
        FillRole::FirstName,
        FillRole::MiddleName,
        FillRole::LastName,
        FillRole::Email,
        FillRole::Phone,
        FillRole::BirthDate,
        FillRole::BirthDay,
        FillRole::BirthMonth,
        FillRole::BirthYear,
        FillRole::Company,
        FillRole::Street,
        FillRole::Number,
        FillRole::Complement,
        FillRole::AddressLine1,
        FillRole::AddressLine2,
        FillRole::Neighborhood,
        FillRole::City,
        FillRole::State,
        FillRole::PostalCode,
        FillRole::Country,
        FillRole::Username,
        FillRole::Cpf,
        FillRole::Rg,
        FillRole::Passport,
        FillRole::DriversLicense,
    ];

    /// Document numbers: filled only after the user confirms them, and only
    /// on https pages.
    pub fn is_document(self) -> bool {
        matches!(
            self,
            FillRole::Cpf | FillRole::Rg | FillRole::Passport | FillRole::DriversLicense
        )
    }
}

/// Which characters a field may hold.
#[derive(Clone, Copy)]
pub(crate) enum Allow {
    /// One line: no control characters.
    Line,
    /// Several lines: newlines and tabs allowed, no other control characters.
    Text,
    /// Digits, spaces and `+ ( ) - .`.
    Phone,
}

fn allowed(value: &str, allow: Allow) -> bool {
    match allow {
        Allow::Line => !value.chars().any(char::is_control),
        Allow::Text => !value
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t' && c != '\r'),
        Allow::Phone => value
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, ' ' | '+' | '(' | ')' | '-' | '.')),
    }
}

/// Trim; drop when empty; reject when too long or holding forbidden characters.
pub(crate) fn clean_value(
    value: Option<SecretString>,
    max_chars: usize,
    allow: Allow,
    what: &'static str,
) -> Result<Option<SecretString>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.expose().trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > max_chars || !allowed(trimmed, allow) {
        return Err(Error::InvalidInput(what));
    }
    Ok(Some(SecretString::from(trimmed)))
}

/// Days since 1970-01-01 of a proleptic Gregorian date (H. Hinnant's
/// `days_from_civil`).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        _ => 28,
    }
}

/// `YYYY-MM-DD` as a real calendar date: `(year, month, day)`. The one
/// date check for identity birth dates and Date custom fields.
pub(crate) fn parse_ymd(value: &str) -> Option<(i64, i64, i64)> {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let num = |range: std::ops::Range<usize>| -> Option<i64> {
        let part = &value[range];
        if !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        part.parse().ok()
    };
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    if y < 1 || !(1..=12).contains(&m) || d < 1 || d > days_in_month(y, m) {
        return None;
    }
    Some((y, m, d))
}

/// `YYYY-MM-DD`, a real calendar date, from 1850 up to today (UTC).
fn check_birth_date(value: &str, now_ms: i64) -> Result<()> {
    const BAD: Error = Error::InvalidInput("birth date must be a past date as YYYY-MM-DD");
    let (y, m, d) = parse_ymd(value).ok_or(BAD)?;
    if y < MIN_BIRTH_YEAR || days_from_civil(y, m, d) > now_ms.div_euclid(86_400_000) {
        return Err(BAD);
    }
    Ok(())
}

impl IdentityFields {
    /// Validate and normalize every field. `now_ms` bounds the birth date.
    pub fn clean(self, now_ms: i64) -> Result<Self> {
        let name = |v, what| clean_value(v, MAX_NAME_CHARS, Allow::Line, what);
        let doc = |v, what| clean_value(v, MAX_DOCUMENT_CHARS, Allow::Line, what);
        let phone = |v, what| clean_value(v, MAX_PHONE_CHARS, Allow::Phone, what);
        let short = |v, what| clean_value(v, MAX_SHORT_CHARS, Allow::Line, what);

        let birth_date = clean_value(self.birth_date, 10, Allow::Line, "invalid birth date")?;
        if let Some(b) = &birth_date {
            check_birth_date(b.expose(), now_ms)?;
        }
        let website = match clean_value(
            self.website,
            MAX_WEBSITE_LEN,
            Allow::Line,
            "invalid website address",
        )? {
            Some(w) => Some(SecretString::new(normalize_url(w.expose())?)),
            None => None,
        };
        let notes = match self.notes {
            Some(n) if n.expose().trim().is_empty() => None,
            Some(n) if n.expose().len() > MAX_IDENTITY_NOTES_BYTES => {
                return Err(Error::InvalidInput("notes are too long"))
            }
            other => other,
        };

        if self.custom.len() > MAX_CUSTOM_FIELDS {
            return Err(Error::InvalidInput("too many custom fields"));
        }
        let mut custom = Vec::with_capacity(self.custom.len());
        for field in self.custom {
            // A row with no value is dropped, whatever its label (an
            // abandoned "Add field"); one with a value needs a label.
            let Some(value) = clean_value(
                Some(field.value),
                MAX_CUSTOM_VALUE_CHARS,
                Allow::Text,
                "custom field value is too long or contains control characters",
            )?
            else {
                continue;
            };
            let label = clean_value(
                Some(field.label),
                MAX_CUSTOM_LABEL_CHARS,
                Allow::Line,
                "custom field label is too long or contains control characters",
            )?
            .ok_or(Error::InvalidInput("a custom field needs a label"))?;
            custom.push(CustomField {
                label,
                value,
                hidden: field.hidden,
            });
        }

        Ok(Self {
            first_name: name(self.first_name, "first name is invalid")?,
            middle_name: name(self.middle_name, "middle name is invalid")?,
            last_name: name(self.last_name, "last name is invalid")?,
            gender: name(self.gender, "gender is invalid")?,
            birth_date,
            occupation: name(self.occupation, "occupation is invalid")?,
            company: name(self.company, "company is invalid")?,
            job_title: name(self.job_title, "job title is invalid")?,
            cpf: doc(self.cpf, "CPF is invalid")?,
            rg: doc(self.rg, "RG is invalid")?,
            passport: doc(self.passport, "passport number is invalid")?,
            drivers_license: doc(self.drivers_license, "driver's license is invalid")?,
            email: clean_value(self.email, MAX_EMAIL_CHARS, Allow::Line, "email is invalid")?,
            mobile_phone: phone(self.mobile_phone, "mobile phone is invalid")?,
            home_phone: phone(self.home_phone, "home phone is invalid")?,
            work_phone: phone(self.work_phone, "work phone is invalid")?,
            street: name(self.street, "street is invalid")?,
            number: short(self.number, "address number is invalid")?,
            complement: name(self.complement, "address complement is invalid")?,
            neighborhood: name(self.neighborhood, "neighborhood is invalid")?,
            city: name(self.city, "city is invalid")?,
            state: name(self.state, "state is invalid")?,
            postal_code: short(self.postal_code, "postal code is invalid")?,
            country: name(self.country, "country is invalid")?,
            username: clean_value(
                self.username,
                MAX_EMAIL_CHARS,
                Allow::Line,
                "username is invalid",
            )?,
            website,
            custom,
            notes,
        })
    }

    /// First, middle and last name joined with single spaces; empty when none.
    pub fn display_name(&self) -> String {
        [&self.first_name, &self.middle_name, &self.last_name]
            .into_iter()
            .flatten()
            .map(|s| s.expose().trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The value for a form field of `role`, derived as the spec says; `None`
    /// when the identity has nothing for it.
    pub fn fill_value(&self, role: FillRole) -> Option<SecretString> {
        let get = |v: &Option<SecretString>| v.as_ref().map(|s| s.expose().to_owned());
        let date_part = |i: usize| -> Option<String> {
            let d = self.birth_date.as_ref()?.expose().to_owned();
            let part = d.split('-').nth(i)?.to_owned();
            // Day and month unpadded ("04" → "4"); the year as is.
            Some(if i == 0 {
                part
            } else {
                part.trim_start_matches('0').to_owned()
            })
        };
        let value = match role {
            FillRole::FullName => Some(self.display_name()).filter(|n| !n.is_empty()),
            FillRole::FirstName => get(&self.first_name),
            FillRole::MiddleName => get(&self.middle_name),
            FillRole::LastName => get(&self.last_name),
            FillRole::Email => get(&self.email),
            FillRole::Phone => get(&self.mobile_phone)
                .or_else(|| get(&self.home_phone))
                .or_else(|| get(&self.work_phone)),
            FillRole::BirthDate => get(&self.birth_date),
            FillRole::BirthDay => date_part(2),
            FillRole::BirthMonth => date_part(1),
            FillRole::BirthYear => date_part(0),
            FillRole::Company => get(&self.company),
            FillRole::Street => get(&self.street),
            FillRole::Number => get(&self.number),
            FillRole::Complement | FillRole::AddressLine2 => get(&self.complement),
            FillRole::AddressLine1 => match (get(&self.street), get(&self.number)) {
                (Some(s), Some(n)) => Some(format!("{s}, {n}")),
                (s, n) => s.or(n),
            },
            FillRole::Neighborhood => get(&self.neighborhood),
            FillRole::City => get(&self.city),
            FillRole::State => get(&self.state),
            FillRole::PostalCode => get(&self.postal_code),
            FillRole::Country => get(&self.country),
            FillRole::Username => get(&self.username),
            FillRole::Cpf => get(&self.cpf),
            FillRole::Rg => get(&self.rg),
            FillRole::Passport => get(&self.passport),
            FillRole::DriversLicense => get(&self.drivers_license),
        };
        value.map(SecretString::new)
    }

    /// The snake_case names of every non-empty value, in declaration order.
    /// Custom fields are addressed by index and not listed.
    pub fn filled_field_names(&self) -> Vec<&'static str> {
        [
            ("first_name", &self.first_name),
            ("middle_name", &self.middle_name),
            ("last_name", &self.last_name),
            ("gender", &self.gender),
            ("birth_date", &self.birth_date),
            ("occupation", &self.occupation),
            ("company", &self.company),
            ("job_title", &self.job_title),
            ("cpf", &self.cpf),
            ("rg", &self.rg),
            ("passport", &self.passport),
            ("drivers_license", &self.drivers_license),
            ("email", &self.email),
            ("mobile_phone", &self.mobile_phone),
            ("home_phone", &self.home_phone),
            ("work_phone", &self.work_phone),
            ("street", &self.street),
            ("number", &self.number),
            ("complement", &self.complement),
            ("neighborhood", &self.neighborhood),
            ("city", &self.city),
            ("state", &self.state),
            ("postal_code", &self.postal_code),
            ("country", &self.country),
            ("username", &self.username),
            ("website", &self.website),
            ("notes", &self.notes),
        ]
        .into_iter()
        .filter(|(_, v)| v.as_ref().is_some_and(|v| !v.is_empty()))
        .map(|(name, _)| name)
        .collect()
    }

    /// One value, for copying. `Address` is the formatted block.
    pub fn value(&self, field: IdentityField) -> Option<SecretString> {
        let v = match field {
            IdentityField::FirstName => &self.first_name,
            IdentityField::MiddleName => &self.middle_name,
            IdentityField::LastName => &self.last_name,
            IdentityField::Gender => &self.gender,
            IdentityField::BirthDate => &self.birth_date,
            IdentityField::Occupation => &self.occupation,
            IdentityField::Company => &self.company,
            IdentityField::JobTitle => &self.job_title,
            IdentityField::Cpf => &self.cpf,
            IdentityField::Rg => &self.rg,
            IdentityField::Passport => &self.passport,
            IdentityField::DriversLicense => &self.drivers_license,
            IdentityField::Email => &self.email,
            IdentityField::MobilePhone => &self.mobile_phone,
            IdentityField::HomePhone => &self.home_phone,
            IdentityField::WorkPhone => &self.work_phone,
            IdentityField::Street => &self.street,
            IdentityField::Number => &self.number,
            IdentityField::Complement => &self.complement,
            IdentityField::Neighborhood => &self.neighborhood,
            IdentityField::City => &self.city,
            IdentityField::State => &self.state,
            IdentityField::PostalCode => &self.postal_code,
            IdentityField::Country => &self.country,
            IdentityField::Username => &self.username,
            IdentityField::Website => &self.website,
            IdentityField::Notes => &self.notes,
            IdentityField::Address => return self.formatted_address(),
        };
        v.clone()
    }

    /// The address as one block, laid out the Brazilian way when the country
    /// is Brazil, otherwise in a common international order. `None` when no
    /// address part is filled.
    pub fn formatted_address(&self) -> Option<SecretString> {
        let get = |v: &Option<SecretString>| v.as_ref().map(|s| s.expose().to_owned());
        let join = |parts: &[Option<String>], sep: &str| {
            let kept: Vec<String> = parts.iter().flatten().cloned().collect();
            (!kept.is_empty()).then(|| kept.join(sep))
        };
        let street = get(&self.street);
        let number = get(&self.number);
        let complement = get(&self.complement);
        let neighborhood = get(&self.neighborhood);
        let city = get(&self.city);
        let state = get(&self.state);
        let postal = get(&self.postal_code);
        let country = get(&self.country);
        let brazil = country.as_deref().is_some_and(|c| {
            matches!(c.trim().to_lowercase().as_str(), "brazil" | "brasil" | "br")
        });

        let lines: Vec<Option<String>> = if brazil {
            let street_line = join(&[join(&[street, number], ", "), complement], " – ");
            vec![
                street_line,
                neighborhood,
                join(&[city, state], " – "),
                postal.map(|p| format!("CEP {p}")),
                country,
            ]
        } else {
            let street_line = join(&[join(&[street, number], " "), complement], ", ");
            let city_line = join(&[join(&[city, state], ", "), postal], " ");
            vec![street_line, neighborhood, city_line, country]
        };
        join(&lines, "\n").map(SecretString::new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-09-29T12:00:00Z
    const NOW: i64 = 1_790_683_200_000;

    fn s(v: &str) -> Option<SecretString> {
        Some(SecretString::from(v))
    }

    fn expose(v: &Option<SecretString>) -> Option<&str> {
        v.as_ref().map(SecretString::expose)
    }

    #[test]
    fn filled_field_names_lists_non_empty_values_in_order() {
        let f = IdentityFields {
            notes: s("n"),
            first_name: s("Ana"),
            cpf: s("123"),
            city: s(""),
            drivers_license: s("D1"),
            ..Default::default()
        };
        assert_eq!(
            f.filled_field_names(),
            ["first_name", "cpf", "drivers_license", "notes"]
        );
        assert!(IdentityFields::default().filled_field_names().is_empty());
    }

    #[test]
    fn an_empty_identity_is_valid() {
        let clean = IdentityFields::default().clean(NOW).unwrap();
        assert_eq!(clean.display_name(), "");
        assert!(clean.formatted_address().is_none());
    }

    #[test]
    fn values_are_trimmed_and_blank_ones_dropped() {
        let clean = IdentityFields {
            first_name: s("  Samuel "),
            last_name: s("   "),
            ..Default::default()
        }
        .clean(NOW)
        .unwrap();
        assert_eq!(expose(&clean.first_name), Some("Samuel"));
        assert!(clean.last_name.is_none());
    }

    #[test]
    fn display_name_joins_the_parts_present() {
        let f = IdentityFields {
            first_name: s("Samuel"),
            last_name: s("Rocha"),
            ..Default::default()
        };
        assert_eq!(f.display_name(), "Samuel Rocha");
    }

    #[test]
    fn birth_dates() {
        let with = |d: &str| {
            IdentityFields {
                birth_date: s(d),
                ..Default::default()
            }
            .clean(NOW)
        };
        assert!(with("2000-04-20").is_ok());
        assert!(with("2024-02-29").is_ok(), "leap day");
        assert!(with("2026-09-29").is_ok(), "today");
        for bad in [
            "2023-02-29",
            "2000-13-01",
            "2000-04-31",
            "2000-4-20",
            "20/04/2000",
            "2026-09-30",
            "1849-12-31",
            "２０００-04-20",
            "2000-04-2x",
        ] {
            assert!(with(bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn days_from_civil_matches_known_dates() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        assert_eq!(days_from_civil(2026, 9, 29), NOW / 86_400_000);
    }

    #[test]
    fn phones_allow_only_dialling_characters() {
        let with = |p: &str| {
            IdentityFields {
                mobile_phone: s(p),
                ..Default::default()
            }
            .clean(NOW)
        };
        assert!(with("+55 (61) 99999-0000").is_ok());
        assert!(with("61.3333.4444").is_ok());
        assert!(with("call me").is_err());
        assert!(with(&"1".repeat(MAX_PHONE_CHARS + 1)).is_err());
    }

    #[test]
    fn limits_and_control_characters() {
        let long = "x".repeat(MAX_NAME_CHARS + 1);
        assert!(IdentityFields {
            city: s(&long),
            ..Default::default()
        }
        .clean(NOW)
        .is_err());
        assert!(IdentityFields {
            street: s("Rua A\nApto 2"),
            ..Default::default()
        }
        .clean(NOW)
        .is_err());
        assert!(IdentityFields {
            cpf: s(&"1".repeat(MAX_DOCUMENT_CHARS + 1)),
            ..Default::default()
        }
        .clean(NOW)
        .is_err());
        assert!(IdentityFields {
            notes: s(&"n".repeat(MAX_IDENTITY_NOTES_BYTES + 1)),
            ..Default::default()
        }
        .clean(NOW)
        .is_err());
        assert!(IdentityFields {
            notes: s("line one\nline two"),
            ..Default::default()
        }
        .clean(NOW)
        .is_ok());
    }

    #[test]
    fn websites_are_normalized_like_login_websites() {
        let clean = IdentityFields {
            website: s("example.com"),
            ..Default::default()
        }
        .clean(NOW)
        .unwrap();
        assert_eq!(expose(&clean.website), Some("https://example.com/"));
        assert!(IdentityFields {
            website: s("javascript:alert(1)"),
            ..Default::default()
        }
        .clean(NOW)
        .is_err());
    }

    #[test]
    fn custom_fields() {
        let field = |label: &str, value: &str| CustomField {
            label: label.into(),
            value: value.into(),
            hidden: false,
        };
        let clean = IdentityFields {
            custom: vec![field("Blood type", "O+"), field("Empty", "  ")],
            ..Default::default()
        }
        .clean(NOW)
        .unwrap();
        assert_eq!(clean.custom.len(), 1, "an empty value is dropped");
        assert!(
            IdentityFields {
                custom: vec![field("", "")],
                ..Default::default()
            }
            .clean(NOW)
            .unwrap()
            .custom
            .is_empty(),
            "a blank row is dropped, not refused"
        );
        assert!(IdentityFields {
            custom: vec![field(" ", "value")],
            ..Default::default()
        }
        .clean(NOW)
        .is_err());
        assert!(IdentityFields {
            custom: (0..=MAX_CUSTOM_FIELDS).map(|_| field("a", "b")).collect(),
            ..Default::default()
        }
        .clean(NOW)
        .is_err());
    }

    #[test]
    fn unknown_fields_are_rejected() {
        assert!(serde_json::from_str::<IdentityFields>(r#"{"firstName":"a","ssn":"x"}"#).is_err());
        assert!(serde_json::from_str::<IdentityFields>(r#"{"postalCode":"70000-000"}"#).is_ok());
    }

    #[test]
    fn brazilian_address_block() {
        let f = IdentityFields {
            street: s("Quadra 02 Conjunto 01"),
            number: s("10"),
            complement: s("Setor Especial"),
            neighborhood: s("Estrutural"),
            city: s("Brasília"),
            state: s("DF"),
            postal_code: s("71266-105"),
            country: s("Brasil"),
            ..Default::default()
        };
        assert_eq!(
            f.formatted_address().unwrap().expose(),
            "Quadra 02 Conjunto 01, 10 – Setor Especial\nEstrutural\nBrasília – DF\nCEP 71266-105\nBrasil"
        );
    }

    #[test]
    fn international_address_block() {
        let f = IdentityFields {
            street: s("Main St"),
            number: s("221"),
            city: s("Springfield"),
            state: s("IL"),
            postal_code: s("62701"),
            country: s("USA"),
            ..Default::default()
        };
        assert_eq!(
            f.formatted_address().unwrap().expose(),
            "Main St 221\nSpringfield, IL 62701\nUSA"
        );
    }

    #[test]
    fn values_by_field_and_nothing_in_debug() {
        let f = IdentityFields {
            cpf: s("123.456.789-00"),
            custom: vec![CustomField {
                label: "PIN".into(),
                value: "4321".into(),
                hidden: true,
            }],
            ..Default::default()
        };
        assert_eq!(
            f.value(IdentityField::Cpf).unwrap().expose(),
            "123.456.789-00"
        );
        assert!(f.value(IdentityField::Rg).is_none());
        let debug = format!("{f:?} {:?}", f.custom[0]);
        assert!(!debug.contains("123.456") && !debug.contains("4321") && !debug.contains("PIN"));
    }

    #[test]
    fn fill_values_are_derived_as_the_spec_says() {
        let f = IdentityFields {
            first_name: s("Samuel"),
            last_name: s("Rocha"),
            birth_date: s("2000-04-20"),
            home_phone: s("61 3333-4444"),
            work_phone: s("61 2222-0000"),
            street: s("Quadra 02"),
            number: s("10"),
            complement: s("Apto 3"),
            cpf: s("123.456.789-00"),
            ..Default::default()
        };
        let v = |r| f.fill_value(r).map(|x| x.expose().to_owned());
        assert_eq!(v(FillRole::FullName).as_deref(), Some("Samuel Rocha"));
        assert_eq!(
            v(FillRole::Phone).as_deref(),
            Some("61 3333-4444"),
            "home when no mobile"
        );
        assert_eq!(v(FillRole::BirthDay).as_deref(), Some("20"));
        assert_eq!(v(FillRole::BirthMonth).as_deref(), Some("4"));
        assert_eq!(v(FillRole::BirthYear).as_deref(), Some("2000"));
        assert_eq!(v(FillRole::AddressLine1).as_deref(), Some("Quadra 02, 10"));
        assert_eq!(v(FillRole::AddressLine2).as_deref(), Some("Apto 3"));
        assert_eq!(v(FillRole::Cpf).as_deref(), Some("123.456.789-00"));
        assert_eq!(v(FillRole::Email), None);
        assert_eq!(v(FillRole::MiddleName), None);
    }

    #[test]
    fn mobile_wins_and_address_line_takes_what_exists() {
        let f = IdentityFields {
            mobile_phone: s("+55 61 99999-0000"),
            home_phone: s("61 3333-4444"),
            number: s("10"),
            ..Default::default()
        };
        assert_eq!(
            f.fill_value(FillRole::Phone).unwrap().expose(),
            "+55 61 99999-0000"
        );
        assert_eq!(f.fill_value(FillRole::AddressLine1).unwrap().expose(), "10");
        assert!(IdentityFields::default()
            .fill_value(FillRole::FullName)
            .is_none());
    }

    #[test]
    fn document_roles() {
        let docs: Vec<_> = FillRole::ALL.iter().filter(|r| r.is_document()).collect();
        assert_eq!(
            docs,
            [
                &FillRole::Cpf,
                &FillRole::Rg,
                &FillRole::Passport,
                &FillRole::DriversLicense
            ]
        );
    }
}
