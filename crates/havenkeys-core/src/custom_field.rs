//! A login's custom fields (spec 2026-09-30-login-custom-fields).
//!
//! Sections of labelled, typed fields inside the login's encrypted details;
//! nothing here is in the overview. Plain values reach the UI when a login
//! opens (as notes do). A Password value leaves the core only on an explicit
//! reveal or copy; an OTP secret never does, only its codes.

use crate::error::{Error, Result};
use crate::identity::{clean_value, parse_ymd, Allow};
use crate::model::{check_password, normalize_url, SecretUpdate, MAX_NOTES_BYTES, MAX_TITLE_CHARS};
use crate::secret::SecretString;
use crate::totp::{self, TotpCode, TotpConfig};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;
use uuid::Uuid;
use zeroize::Zeroize;

pub const MAX_SECTIONS: usize = 20;
pub const MAX_FIELDS: usize = 100;
pub const MAX_LABEL_CHARS: usize = MAX_TITLE_CHARS;
pub const MAX_SHORT_VALUE_CHARS: usize = 512;
/// Every value, label and title of one login together, in bytes.
pub const MAX_TOTAL_BYTES: usize = 256 * 1024;

/// An id that is not one of this login's fields or sections, sent twice, or
/// a kept value on a new field or with a changed type. Never names a value.
const BAD_LAYOUT: Error =
    Error::InvalidInput("the login's fields changed; reopen it and try again");

// ---------------------------------------------------------------- stored

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldSection {
    pub id: Uuid,
    /// `None`: an untitled section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<SecretString>,
    #[serde(default)]
    pub fields: Vec<CustomField>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomField {
    pub id: Uuid,
    pub label: SecretString,
    pub value: FieldValue,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum FieldValue {
    Text(SecretString),
    Url(SecretString),
    Email(SecretString),
    Phone(SecretString),
    /// `YYYY-MM-DD`, or empty.
    Date(SecretString),
    Address(Box<AddressValue>),
    /// `None`: cleared; the field and its label stay.
    Password(Option<SecretString>),
    /// `None`: cleared. The secret never leaves the core.
    Otp(Option<TotpConfig>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    Url,
    Email,
    Phone,
    Date,
    Address,
    Password,
    Otp,
}

/// The same parts as the Identity's address, so a later "Copy street" menu
/// needs no format change.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AddressValue {
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AddressPart {
    Street,
    Number,
    Complement,
    Neighborhood,
    City,
    State,
    PostalCode,
    Country,
}

impl AddressValue {
    pub fn part(&self, part: AddressPart) -> Option<&SecretString> {
        match part {
            AddressPart::Street => self.street.as_ref(),
            AddressPart::Number => self.number.as_ref(),
            AddressPart::Complement => self.complement.as_ref(),
            AddressPart::Neighborhood => self.neighborhood.as_ref(),
            AddressPart::City => self.city.as_ref(),
            AddressPart::State => self.state.as_ref(),
            AddressPart::PostalCode => self.postal_code.as_ref(),
            AddressPart::Country => self.country.as_ref(),
        }
    }

    /// One line each: "street, number", complement, neighborhood,
    /// "city - state", postal code, country; empty ones left out. The one
    /// address format, for the detail view and for Copy.
    pub fn formatted(&self) -> SecretString {
        fn pair(a: &Option<SecretString>, sep: &str, b: &Option<SecretString>) -> Option<String> {
            match (a, b) {
                (Some(a), Some(b)) => Some(format!("{}{sep}{}", a.expose(), b.expose())),
                (Some(x), None) | (None, Some(x)) => Some(x.expose().to_owned()),
                (None, None) => None,
            }
        }
        let one = |v: &Option<SecretString>| v.as_ref().map(|s| s.expose().to_owned());
        let mut lines: Vec<String> = [
            pair(&self.street, ", ", &self.number),
            one(&self.complement),
            one(&self.neighborhood),
            pair(&self.city, " - ", &self.state),
            one(&self.postal_code),
            one(&self.country),
        ]
        .into_iter()
        .flatten()
        .collect();
        let out = SecretString::new(lines.join("\n"));
        lines.iter_mut().for_each(|l| l.zeroize());
        out
    }

    fn clean(&self) -> Result<Self> {
        let c = |v: &Option<SecretString>| {
            clean_value(
                v.clone(),
                MAX_SHORT_VALUE_CHARS,
                Allow::Line,
                "address is too long or contains control characters",
            )
        };
        Ok(Self {
            street: c(&self.street)?,
            number: c(&self.number)?,
            complement: c(&self.complement)?,
            neighborhood: c(&self.neighborhood)?,
            city: c(&self.city)?,
            state: c(&self.state)?,
            postal_code: c(&self.postal_code)?,
            country: c(&self.country)?,
        })
    }
}

impl FieldValue {
    pub fn kind(&self) -> FieldKind {
        match self {
            FieldValue::Text(_) => FieldKind::Text,
            FieldValue::Url(_) => FieldKind::Url,
            FieldValue::Email(_) => FieldKind::Email,
            FieldValue::Phone(_) => FieldKind::Phone,
            FieldValue::Date(_) => FieldKind::Date,
            FieldValue::Address(_) => FieldKind::Address,
            FieldValue::Password(_) => FieldKind::Password,
            FieldValue::Otp(_) => FieldKind::Otp,
        }
    }

    fn byte_len(&self) -> usize {
        match self {
            FieldValue::Text(v)
            | FieldValue::Url(v)
            | FieldValue::Email(v)
            | FieldValue::Phone(v)
            | FieldValue::Date(v) => v.expose().len(),
            FieldValue::Address(a) => a.formatted().expose().len(),
            FieldValue::Password(p) => p.as_ref().map_or(0, |p| p.expose().len()),
            FieldValue::Otp(c) => c.as_ref().map_or(0, |c| c.secret.expose().len()),
        }
    }
}

impl CustomField {
    /// A Password field's value, for an explicit reveal.
    pub fn concealed(&self) -> Result<SecretString> {
        match &self.value {
            FieldValue::Password(Some(p)) => Ok(p.clone()),
            FieldValue::Password(None) => Err(Error::NotFound),
            _ => Err(Error::InvalidInput("only a password field can be revealed")),
        }
    }

    /// The current code of an OTP field.
    pub fn totp_code(&self, unix_seconds: u64) -> Result<TotpCode> {
        match &self.value {
            FieldValue::Otp(Some(cfg)) => totp::generate(cfg, unix_seconds),
            FieldValue::Otp(None) => Err(Error::NotFound),
            _ => Err(Error::InvalidInput("not a one-time password field")),
        }
    }

    /// What Copy puts on the clipboard: the value, an OTP field's current
    /// code, an address formatted or one of its parts. `NotFound` when empty.
    pub fn copy_value(&self, part: Option<AddressPart>, unix_seconds: u64) -> Result<SecretString> {
        let value = match (&self.value, part) {
            (FieldValue::Address(a), Some(p)) => a.part(p).cloned(),
            (FieldValue::Address(a), None) => Some(a.formatted()),
            (_, Some(_)) => return Err(Error::InvalidInput("only an address has parts")),
            (
                FieldValue::Text(v)
                | FieldValue::Url(v)
                | FieldValue::Email(v)
                | FieldValue::Phone(v)
                | FieldValue::Date(v),
                None,
            ) => Some(v.clone()),
            (FieldValue::Password(p), None) => p.clone(),
            (FieldValue::Otp(_), None) => return self.totp_code(unix_seconds).map(|c| c.code),
        };
        value.filter(|v| !v.is_empty()).ok_or(Error::NotFound)
    }

    /// A URL field's address, normalised again before the OS opens it.
    pub fn url(&self) -> Result<String> {
        match &self.value {
            FieldValue::Url(v) if !v.is_empty() => normalize_url(v.expose()),
            FieldValue::Url(_) => Err(Error::NotFound),
            _ => Err(Error::InvalidInput("not a URL field")),
        }
    }
}

impl fmt::Debug for FieldSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FieldSection")
            .field("id", &self.id)
            .field("fields", &self.fields)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for CustomField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CustomField")
            .field("id", &self.id)
            .field("kind", &self.value.kind())
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for FieldValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}(<redacted>)", self.kind())
    }
}

impl fmt::Debug for AddressValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AddressValue(<redacted>)")
    }
}

// ---------------------------------------------------------------- input

/// A section as the editor sends it. `O` is how an OTP field's setup is
/// sent: `SecretUpdate` in the core; the desktop's wire type adds scanned
/// QR tokens and maps them with `try_map_otp`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SectionInput<O = SecretUpdate> {
    /// `None`: a new section.
    #[serde(default)]
    pub id: Option<Uuid>,
    #[serde(default)]
    pub title: Option<SecretString>,
    #[serde(default = "Vec::new")]
    pub fields: Vec<FieldInput<O>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FieldInput<O = SecretUpdate> {
    /// `None`: a new field.
    #[serde(default)]
    pub id: Option<Uuid>,
    pub label: SecretString,
    pub value: FieldValueInput<O>,
}

#[derive(Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum FieldValueInput<O = SecretUpdate> {
    Text(SecretString),
    Url(SecretString),
    Email(SecretString),
    Phone(SecretString),
    Date(SecretString),
    Address(Box<AddressValue>),
    Password(SecretUpdate),
    Otp(O),
}

impl<O> SectionInput<O> {
    pub fn try_map_otp<P, E, F: FnMut(O) -> std::result::Result<P, E>>(
        self,
        f: &mut F,
    ) -> std::result::Result<SectionInput<P>, E> {
        let fields = self
            .fields
            .into_iter()
            .map(|field| {
                Ok(FieldInput {
                    id: field.id,
                    label: field.label,
                    value: field.value.try_map_otp(&mut *f)?,
                })
            })
            .collect::<std::result::Result<Vec<_>, E>>()?;
        Ok(SectionInput {
            id: self.id,
            title: self.title,
            fields,
        })
    }

    pub fn otp_updates(&self) -> impl Iterator<Item = &O> {
        self.fields.iter().filter_map(|f| match &f.value {
            FieldValueInput::Otp(o) => Some(o),
            _ => None,
        })
    }
}

impl<O> FieldValueInput<O> {
    pub fn kind(&self) -> FieldKind {
        match self {
            FieldValueInput::Text(_) => FieldKind::Text,
            FieldValueInput::Url(_) => FieldKind::Url,
            FieldValueInput::Email(_) => FieldKind::Email,
            FieldValueInput::Phone(_) => FieldKind::Phone,
            FieldValueInput::Date(_) => FieldKind::Date,
            FieldValueInput::Address(_) => FieldKind::Address,
            FieldValueInput::Password(_) => FieldKind::Password,
            FieldValueInput::Otp(_) => FieldKind::Otp,
        }
    }

    pub fn try_map_otp<P, E, F: FnMut(O) -> std::result::Result<P, E>>(
        self,
        f: &mut F,
    ) -> std::result::Result<FieldValueInput<P>, E> {
        Ok(match self {
            FieldValueInput::Text(v) => FieldValueInput::Text(v),
            FieldValueInput::Url(v) => FieldValueInput::Url(v),
            FieldValueInput::Email(v) => FieldValueInput::Email(v),
            FieldValueInput::Phone(v) => FieldValueInput::Phone(v),
            FieldValueInput::Date(v) => FieldValueInput::Date(v),
            FieldValueInput::Address(a) => FieldValueInput::Address(a),
            FieldValueInput::Password(u) => FieldValueInput::Password(u),
            FieldValueInput::Otp(o) => FieldValueInput::Otp(f(o)?),
        })
    }
}

impl<O> fmt::Debug for SectionInput<O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SectionInput")
            .field("id", &self.id)
            .field("fields", &self.fields.len())
            .finish_non_exhaustive()
    }
}

impl<O> fmt::Debug for FieldInput<O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FieldInput")
            .field("id", &self.id)
            .field("kind", &self.value.kind())
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------- views

/// A section as the UI receives it when a login opens.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionView {
    pub id: Uuid,
    pub title: Option<SecretString>,
    pub fields: Vec<FieldView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldView {
    pub id: Uuid,
    pub label: SecretString,
    #[serde(flatten)]
    pub value: FieldValueView,
}

/// Plain values as they are; a Password or OTP field only says whether it
/// holds something. This type cannot carry a concealed value.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FieldValueView {
    Text {
        value: SecretString,
    },
    Url {
        value: SecretString,
    },
    Email {
        value: SecretString,
    },
    Phone {
        value: SecretString,
    },
    Date {
        value: SecretString,
    },
    Address {
        parts: Box<AddressValue>,
        formatted: SecretString,
    },
    Password {
        #[serde(rename = "hasValue")]
        has_value: bool,
    },
    Otp {
        #[serde(rename = "hasOtp")]
        has_otp: bool,
    },
}

pub fn views(sections: Vec<FieldSection>) -> Vec<SectionView> {
    sections
        .into_iter()
        .map(|s| SectionView {
            id: s.id,
            title: s.title,
            fields: s
                .fields
                .into_iter()
                .map(|f| FieldView {
                    id: f.id,
                    label: f.label,
                    value: match f.value {
                        FieldValue::Text(value) => FieldValueView::Text { value },
                        FieldValue::Url(value) => FieldValueView::Url { value },
                        FieldValue::Email(value) => FieldValueView::Email { value },
                        FieldValue::Phone(value) => FieldValueView::Phone { value },
                        FieldValue::Date(value) => FieldValueView::Date { value },
                        FieldValue::Address(parts) => {
                            let formatted = parts.formatted();
                            FieldValueView::Address { parts, formatted }
                        }
                        FieldValue::Password(p) => FieldValueView::Password {
                            has_value: p.is_some(),
                        },
                        FieldValue::Otp(c) => FieldValueView::Otp {
                            has_otp: c.is_some(),
                        },
                    },
                })
                .collect(),
        })
        .collect()
}

// ---------------------------------------------------------------- validation

/// Check one plain (not Password or OTP) value. Used by `apply_sections`,
/// and by the importer to decide whether a value keeps its type or becomes
/// Text.
pub(crate) fn clean_plain(value: &FieldValueInput) -> Result<FieldValue> {
    let short = |v: &SecretString, what: &'static str| -> Result<SecretString> {
        Ok(
            clean_value(Some(v.clone()), MAX_SHORT_VALUE_CHARS, Allow::Line, what)?
                .unwrap_or_default(),
        )
    };
    Ok(match value {
        FieldValueInput::Text(v) => {
            if v.expose().len() > MAX_NOTES_BYTES {
                return Err(Error::InvalidInput("a text field is too long"));
            }
            FieldValue::Text(v.clone())
        }
        FieldValueInput::Url(v) if v.expose().trim().is_empty() => {
            FieldValue::Url(SecretString::default())
        }
        FieldValueInput::Url(v) => FieldValue::Url(SecretString::new(normalize_url(v.expose())?)),
        FieldValueInput::Email(v) => FieldValue::Email(short(
            v,
            "email is too long or contains control characters",
        )?),
        FieldValueInput::Phone(v) => FieldValue::Phone(short(
            v,
            "phone is too long or contains control characters",
        )?),
        FieldValueInput::Date(v) => {
            const BAD: Error = Error::InvalidInput("date must be YYYY-MM-DD");
            let v = short(v, "date must be YYYY-MM-DD")?;
            if !v.is_empty() && parse_ymd(v.expose()).is_none() {
                return Err(BAD);
            }
            FieldValue::Date(v)
        }
        FieldValueInput::Address(a) => FieldValue::Address(Box::new(a.clean()?)),
        FieldValueInput::Password(_) | FieldValueInput::Otp(_) => {
            return Err(Error::InvalidInput("not a plain value"))
        }
    })
}

/// A login's new fields from the editor's layout (spec §4.5). `None` keeps
/// `current` as it is. Ids are looked up only among `current`, so a save
/// can never reach another login's secrets.
pub(crate) fn apply_sections(
    input: Option<Vec<SectionInput>>,
    current: Vec<FieldSection>,
) -> Result<Vec<FieldSection>> {
    let Some(input) = input else {
        return Ok(current);
    };
    if input.len() > MAX_SECTIONS {
        return Err(Error::InvalidInput("too many sections"));
    }
    if input.iter().map(|s| s.fields.len()).sum::<usize>() > MAX_FIELDS {
        return Err(Error::InvalidInput("too many fields"));
    }
    let mut section_ids: HashSet<Uuid> = current.iter().map(|s| s.id).collect();
    let mut prior: HashMap<Uuid, FieldValue> = current
        .into_iter()
        .flat_map(|s| s.fields)
        .map(|f| (f.id, f.value))
        .collect();

    let mut out = Vec::with_capacity(input.len());
    for section in input {
        let id = match section.id {
            None => Uuid::new_v4(),
            Some(id) if section_ids.remove(&id) => id,
            Some(_) => return Err(BAD_LAYOUT),
        };
        let title = clean_value(
            section.title,
            MAX_LABEL_CHARS,
            Allow::Line,
            "section title is too long or contains control characters",
        )?;
        let mut fields = Vec::with_capacity(section.fields.len());
        for field in section.fields {
            let (id, before) = match field.id {
                None => (Uuid::new_v4(), None),
                // `remove`: an id sent twice is unknown the second time.
                Some(id) => (id, Some(prior.remove(&id).ok_or(BAD_LAYOUT)?)),
            };
            if before
                .as_ref()
                .is_some_and(|b| b.kind() != field.value.kind())
            {
                return Err(BAD_LAYOUT);
            }
            let label = clean_value(
                Some(field.label),
                MAX_LABEL_CHARS,
                Allow::Line,
                "field label is too long or contains control characters",
            )?
            .ok_or(Error::InvalidInput("a field needs a label"))?;
            let value = match field.value {
                FieldValueInput::Password(update) => {
                    if before.is_none() && update.is_keep() {
                        return Err(BAD_LAYOUT);
                    }
                    let current = match before {
                        Some(FieldValue::Password(p)) => p,
                        _ => None,
                    };
                    let p = update.apply(current);
                    if let Some(p) = &p {
                        check_password(p)?;
                    }
                    FieldValue::Password(p)
                }
                FieldValueInput::Otp(update) => {
                    if before.is_none() && update.is_keep() {
                        return Err(BAD_LAYOUT);
                    }
                    let current = match before {
                        Some(FieldValue::Otp(c)) => c,
                        _ => None,
                    };
                    FieldValue::Otp(update.apply_totp(current)?)
                }
                plain => clean_plain(&plain)?,
            };
            fields.push(CustomField { id, label, value });
        }
        out.push(FieldSection { id, title, fields });
    }

    let total: usize = out
        .iter()
        .map(|s| {
            s.title.as_ref().map_or(0, |t| t.expose().len())
                + s.fields
                    .iter()
                    .map(|f| f.label.expose().len() + f.value.byte_len())
                    .sum::<usize>()
        })
        .sum();
    if total > MAX_TOTAL_BYTES {
        return Err(Error::InvalidInput("the login's fields are too large"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RFC_SECRET: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

    fn s(v: &str) -> SecretString {
        SecretString::from(v)
    }

    fn field(id: Option<Uuid>, label: &str, value: FieldValueInput) -> FieldInput {
        FieldInput {
            id,
            label: s(label),
            value,
        }
    }

    fn section(id: Option<Uuid>, title: Option<&str>, fields: Vec<FieldInput>) -> SectionInput {
        SectionInput {
            id,
            title: title.map(s),
            fields,
        }
    }

    /// One section "Bank" with a PIN (Password), an OTP and a Text field.
    fn bank() -> Vec<FieldSection> {
        apply_sections(
            Some(vec![section(
                None,
                Some("Bank"),
                vec![
                    field(
                        None,
                        "PIN",
                        FieldValueInput::Password(SecretUpdate::Set(s("4321"))),
                    ),
                    field(
                        None,
                        "Token",
                        FieldValueInput::Otp(SecretUpdate::Set(s(RFC_SECRET))),
                    ),
                    field(None, "Account", FieldValueInput::Text(s("12345-6"))),
                ],
            )]),
            Vec::new(),
        )
        .unwrap()
    }

    fn ids(sections: &[FieldSection]) -> (Uuid, Uuid, Uuid, Uuid) {
        let sec = &sections[0];
        (sec.id, sec.fields[0].id, sec.fields[1].id, sec.fields[2].id)
    }

    #[test]
    fn none_keeps_the_current_fields() {
        let current = bank();
        let (sid, ..) = ids(&current);
        let kept = apply_sections(None, current).unwrap();
        assert_eq!(kept[0].id, sid);
        assert_eq!(kept[0].fields.len(), 3);
    }

    #[test]
    fn new_fields_get_ids_and_keep_order() {
        let out = bank();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].title.as_ref().unwrap().expose(), "Bank");
        let labels: Vec<&str> = out[0].fields.iter().map(|f| f.label.expose()).collect();
        assert_eq!(labels, ["PIN", "Token", "Account"]);
        assert_ne!(out[0].fields[0].id, out[0].fields[1].id);
    }

    #[test]
    fn keep_keeps_and_reorder_is_the_list_order() {
        let (sid, pin, token, account) = ids(&bank());
        let out = apply_sections(
            Some(vec![section(
                Some(sid),
                Some("Bank"),
                vec![
                    field(
                        Some(account),
                        "Account",
                        FieldValueInput::Text(s("12345-6")),
                    ),
                    field(
                        Some(token),
                        "Token",
                        FieldValueInput::Otp(SecretUpdate::Keep),
                    ),
                    field(
                        Some(pin),
                        "PIN",
                        FieldValueInput::Password(SecretUpdate::Keep),
                    ),
                ],
            )]),
            bank_with_ids(sid, pin, token, account),
        )
        .unwrap();
        let f = &out[0].fields;
        assert_eq!(f[0].id, account);
        assert_eq!(f[2].concealed().unwrap().expose(), "4321");
        assert_eq!(f[1].totp_code(59).unwrap().code.expose(), "287082");
    }

    /// `bank()` again with fixed ids, so a test can refer to them.
    fn bank_with_ids(sid: Uuid, pin: Uuid, token: Uuid, account: Uuid) -> Vec<FieldSection> {
        let mut b = bank();
        b[0].id = sid;
        b[0].fields[0].id = pin;
        b[0].fields[1].id = token;
        b[0].fields[2].id = account;
        b
    }

    #[test]
    fn a_field_moved_to_another_section_keeps_its_secret() {
        let (sid, pin, token, account) = ids(&bank());
        let current = bank_with_ids(sid, pin, token, account);
        let out = apply_sections(
            Some(vec![
                section(
                    Some(sid),
                    Some("Bank"),
                    vec![field(
                        Some(account),
                        "Account",
                        FieldValueInput::Text(s("12345-6")),
                    )],
                ),
                section(
                    None,
                    Some("Secrets"),
                    vec![
                        field(
                            Some(pin),
                            "PIN",
                            FieldValueInput::Password(SecretUpdate::Keep),
                        ),
                        field(
                            Some(token),
                            "Token",
                            FieldValueInput::Otp(SecretUpdate::Keep),
                        ),
                    ],
                ),
            ]),
            current,
        )
        .unwrap();
        assert_eq!(out[1].fields[0].concealed().unwrap().expose(), "4321");
        assert!(out[1].fields[1].totp_code(59).is_ok());
    }

    #[test]
    fn unknown_duplicate_or_retyped_ids_are_refused() {
        let (sid, pin, token, account) = ids(&bank());
        let current = || bank_with_ids(sid, pin, token, account);
        let keep_pin = |id| {
            field(
                Some(id),
                "PIN",
                FieldValueInput::Password(SecretUpdate::Keep),
            )
        };
        // A field id this login does not have (e.g. another login's).
        let other = Uuid::new_v4();
        let one = |f| Some(vec![section(Some(sid), None, vec![f])]);
        assert_eq!(
            apply_sections(one(keep_pin(other)), current()).err(),
            Some(BAD_LAYOUT)
        );
        // The same field twice.
        let twice = Some(vec![section(
            Some(sid),
            None,
            vec![keep_pin(pin), keep_pin(pin)],
        )]);
        assert_eq!(apply_sections(twice, current()).err(), Some(BAD_LAYOUT));
        // An OTP id sent as Password (would turn a hidden secret revealable).
        assert_eq!(
            apply_sections(one(keep_pin(token)), current()).err(),
            Some(BAD_LAYOUT)
        );
        // A Password id sent as Text with a new value is still a type change.
        let retyped = field(Some(pin), "PIN", FieldValueInput::Text(s("x")));
        assert_eq!(
            apply_sections(one(retyped), current()).err(),
            Some(BAD_LAYOUT)
        );
        // An unknown section id.
        let bad_section = Some(vec![section(Some(other), None, vec![])]);
        assert_eq!(
            apply_sections(bad_section, current()).err(),
            Some(BAD_LAYOUT)
        );
        // Keep on a new field.
        let new_keep = field(None, "PIN", FieldValueInput::Password(SecretUpdate::Keep));
        assert_eq!(
            apply_sections(Some(vec![section(None, None, vec![new_keep])]), Vec::new()).err(),
            Some(BAD_LAYOUT)
        );
    }

    #[test]
    fn a_cleared_secret_field_keeps_its_label() {
        let (sid, pin, token, account) = ids(&bank());
        let out = apply_sections(
            Some(vec![section(
                Some(sid),
                None,
                vec![
                    field(
                        Some(pin),
                        "PIN",
                        FieldValueInput::Password(SecretUpdate::Clear),
                    ),
                    field(
                        Some(token),
                        "Token",
                        FieldValueInput::Otp(SecretUpdate::Set(s("  "))),
                    ),
                ],
            )]),
            bank_with_ids(sid, pin, token, account),
        )
        .unwrap();
        assert_eq!(out[0].fields[0].label.expose(), "PIN");
        assert_eq!(out[0].fields[0].concealed().err(), Some(Error::NotFound));
        assert_eq!(out[0].fields[1].totp_code(59).err(), Some(Error::NotFound));
        let v = serde_json::to_value(views(out)).unwrap();
        assert_eq!(v[0]["fields"][0]["hasValue"], false);
        assert_eq!(v[0]["fields"][1]["hasOtp"], false);
    }

    #[test]
    fn values_are_checked_per_type() {
        let one = |value| {
            apply_sections(
                Some(vec![section(None, None, vec![field(None, "x", value)])]),
                Vec::new(),
            )
        };
        assert!(one(FieldValueInput::Url(s("javascript:alert(1)"))).is_err());
        let url = one(FieldValueInput::Url(s("example.com"))).unwrap();
        assert_eq!(url[0].fields[0].url().unwrap(), "https://example.com/");
        assert!(one(FieldValueInput::Url(s(""))).is_ok());
        assert!(one(FieldValueInput::Date(s("2026-02-30"))).is_err());
        assert!(one(FieldValueInput::Date(s("2024-02-29"))).is_ok());
        assert!(one(FieldValueInput::Date(s(""))).is_ok());
        assert!(one(FieldValueInput::Email(s("a\u{0}b"))).is_err());
        assert!(one(FieldValueInput::Phone(s(&"9".repeat(513)))).is_err());
        assert!(one(FieldValueInput::Otp(SecretUpdate::Set(s("not base32!")))).is_err());
        assert!(one(FieldValueInput::Text(s(&"a".repeat(64 * 1024 + 1)))).is_err());
        let no_label = apply_sections(
            Some(vec![section(
                None,
                None,
                vec![field(None, "  ", FieldValueInput::Text(s("x")))],
            )]),
            Vec::new(),
        );
        assert_eq!(
            no_label.err(),
            Some(Error::InvalidInput("a field needs a label"))
        );
    }

    #[test]
    fn limits() {
        let text = || field(None, "x", FieldValueInput::Text(s("x")));
        let sections = (0..21).map(|_| section(None, None, vec![])).collect();
        assert!(apply_sections(Some(sections), Vec::new()).is_err());
        let many = vec![section(None, None, (0..101).map(|_| text()).collect())];
        assert!(apply_sections(Some(many), Vec::new()).is_err());
        let big = (0..5)
            .map(|_| field(None, "x", FieldValueInput::Text(s(&"a".repeat(60 * 1024)))))
            .collect();
        assert_eq!(
            apply_sections(Some(vec![section(None, None, big)]), Vec::new()).err(),
            Some(Error::InvalidInput("the login's fields are too large"))
        );
    }

    #[test]
    fn views_never_carry_concealed_values() {
        let json = serde_json::to_string(&views(bank())).unwrap();
        assert!(!json.contains("4321"));
        assert!(!json.contains(RFC_SECRET));
        assert!(json.contains("\"hasValue\":true"));
        assert!(json.contains("\"hasOtp\":true"));
        assert!(json.contains("12345-6"), "plain values are shown");
    }

    #[test]
    fn address_format_parts_and_copy() {
        let addr = AddressValue {
            street: Some(s("Rua A")),
            number: Some(s("10")),
            city: Some(s("São Paulo")),
            state: Some(s("SP")),
            postal_code: Some(s("01000-000")),
            ..AddressValue::default()
        };
        let out = apply_sections(
            Some(vec![section(
                None,
                None,
                vec![field(
                    None,
                    "Home",
                    FieldValueInput::Address(Box::new(addr)),
                )],
            )]),
            Vec::new(),
        )
        .unwrap();
        let f = &out[0].fields[0];
        assert_eq!(
            f.copy_value(None, 0).unwrap().expose(),
            "Rua A, 10\nSão Paulo - SP\n01000-000"
        );
        assert_eq!(
            f.copy_value(Some(AddressPart::City), 0).unwrap().expose(),
            "São Paulo"
        );
        assert_eq!(
            f.copy_value(Some(AddressPart::Country), 0).err(),
            Some(Error::NotFound)
        );
    }

    #[test]
    fn wrong_operations_are_invalid_input() {
        let b = bank();
        let (pin, token, text) = (&b[0].fields[0], &b[0].fields[1], &b[0].fields[2]);
        assert!(matches!(text.concealed(), Err(Error::InvalidInput(_))));
        assert!(matches!(token.concealed(), Err(Error::InvalidInput(_))));
        assert!(matches!(pin.totp_code(59), Err(Error::InvalidInput(_))));
        assert!(matches!(
            pin.copy_value(Some(AddressPart::City), 0),
            Err(Error::InvalidInput(_))
        ));
        assert!(matches!(text.url(), Err(Error::InvalidInput(_))));
        assert_eq!(token.copy_value(None, 59).unwrap().expose(), "287082");
        assert_eq!(pin.copy_value(None, 0).unwrap().expose(), "4321");
    }

    #[test]
    fn debug_never_prints_labels_or_values() {
        let b = bank();
        let dbg = format!("{b:?}");
        for secret in [
            "Bank", "PIN", "4321", "Token", RFC_SECRET, "Account", "12345-6",
        ] {
            assert!(!dbg.contains(secret), "Debug printed {secret}");
        }
        let input = section(
            None,
            Some("Bank"),
            vec![field(None, "PIN", FieldValueInput::Text(s("4321")))],
        );
        let dbg = format!("{input:?}");
        assert!(!dbg.contains("Bank") && !dbg.contains("4321"));
    }

    #[test]
    fn stored_form_round_trips() {
        let json = serde_json::to_string(&bank()).unwrap();
        let back: Vec<FieldSection> = serde_json::from_str(&json).unwrap();
        assert_eq!(back[0].fields[0].concealed().unwrap().expose(), "4321");
        assert!(json.contains("\"type\":\"password\""));
    }

    #[test]
    fn input_json_shape() {
        let input: SectionInput = serde_json::from_value(serde_json::json!({
            "title": "Bank",
            "fields": [
                {"label": "PIN", "value": {"type": "password", "value": {"op": "set", "value": "1"}}},
                {"label": "Home", "value": {"type": "address", "value": {"street": "Rua A", "postalCode": "1"}}},
                {"label": "Day", "value": {"type": "date", "value": "2026-09-30"}}
            ]
        }))
        .unwrap();
        assert_eq!(input.fields.len(), 3);
        let bad = serde_json::from_value::<SectionInput>(serde_json::json!({
            "fields": [{"label": "x", "value": {"type": "text", "value": "y"}, "extra": 1}]
        }));
        assert!(bad.is_err(), "unknown keys are refused");
    }
}
