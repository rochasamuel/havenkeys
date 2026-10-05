//! Unencrypted Bitwarden JSON, the format Bitwarden, Proton Pass, KeePassXC
//! and 1Password import. Never carries passkeys (`fido2Credentials` is
//! always empty).

use super::{count, for_each_item, ExportFormat, ExportSummary, Rendered};
use crate::custom_field::FieldValue;
use crate::error::{Error, Result};
use crate::import::common::format_utc_timestamp_ms;
use crate::model::{ItemDetails, ItemOverview, MatchType};
use crate::secret::SecretString;
use crate::vault::VaultService;
use serde::Serialize;
use std::io::Write;
use zeroize::Zeroizing;

#[derive(Serialize)]
struct Field {
    name: String,
    value: SecretString,
    r#type: u8,
    #[serde(rename = "linkedId")]
    linked_id: Option<()>,
}

#[derive(Serialize)]
struct Uri {
    r#match: Option<u8>,
    uri: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct History {
    last_used_date: String,
    password: SecretString,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Login {
    uris: Vec<Uri>,
    username: Option<String>,
    password: Option<SecretString>,
    totp: Option<SecretString>,
    fido2_credentials: [(); 0],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Card {
    cardholder_name: Option<SecretString>,
    brand: Option<&'static str>,
    number: Option<SecretString>,
    exp_month: Option<String>,
    exp_year: Option<String>,
    code: Option<SecretString>,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct Identity {
    title: Option<()>,
    first_name: Option<SecretString>,
    middle_name: Option<SecretString>,
    last_name: Option<SecretString>,
    address1: Option<SecretString>,
    address2: Option<SecretString>,
    address3: Option<SecretString>,
    city: Option<SecretString>,
    state: Option<SecretString>,
    postal_code: Option<SecretString>,
    country: Option<SecretString>,
    company: Option<SecretString>,
    email: Option<SecretString>,
    phone: Option<SecretString>,
    ssn: Option<SecretString>,
    username: Option<SecretString>,
    passport_number: Option<SecretString>,
    license_number: Option<SecretString>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Item {
    id: uuid::Uuid,
    organization_id: Option<()>,
    folder_id: Option<()>,
    r#type: u8,
    reprompt: u8,
    name: String,
    notes: Option<SecretString>,
    favorite: bool,
    fields: Vec<Field>,
    #[serde(skip_serializing_if = "Option::is_none")]
    login: Option<Login>,
    #[serde(skip_serializing_if = "Option::is_none")]
    secure_note: Option<SecureNote>,
    #[serde(skip_serializing_if = "Option::is_none")]
    card: Option<Card>,
    #[serde(skip_serializing_if = "Option::is_none")]
    identity: Option<Identity>,
    collection_ids: Option<()>,
    password_history: Vec<History>,
    creation_date: String,
    revision_date: String,
}

#[derive(Serialize)]
struct SecureNote {
    r#type: u8,
}

fn field(name: &str, value: SecretString, hidden: bool) -> Field {
    Field {
        name: name.to_owned(),
        value,
        r#type: u8::from(hidden),
        linked_id: None,
    }
}

/// Bitwarden refuses an item without a name, so an empty title gets one.
fn name(ov: &ItemOverview, details: &ItemDetails) -> String {
    if !ov.title.trim().is_empty() {
        return ov.title.clone();
    }
    match details {
        ItemDetails::Identity(_) => "Identity".to_owned(),
        ItemDetails::Card(f) => f.effective_brand().display_name().to_owned(),
        _ => "Untitled".to_owned(),
    }
}

fn item(ov: &ItemOverview, details: ItemDetails) -> Item {
    let mut it = Item {
        id: ov.id,
        organization_id: None,
        folder_id: None,
        r#type: 1,
        reprompt: 0,
        name: name(ov, &details),
        notes: None,
        favorite: false,
        fields: Vec::new(),
        login: None,
        secure_note: None,
        card: None,
        identity: None,
        collection_ids: None,
        password_history: Vec::new(),
        creation_date: format_utc_timestamp_ms(ov.created_at),
        revision_date: format_utc_timestamp_ms(ov.updated_at),
    };
    match details {
        ItemDetails::Login {
            password,
            totp,
            notes,
            password_history,
            sections,
            app_bindings,
            ..
        } => {
            let mut uris: Vec<Uri> = ov
                .urls
                .iter()
                .map(|r| Uri {
                    r#match: match r.match_type {
                        MatchType::Domain => None,
                        MatchType::Origin => Some(1),
                        MatchType::Exact => Some(3),
                    },
                    uri: r.url.clone(),
                })
                .collect();
            let mut packages: Vec<&str> = app_bindings.iter().map(|b| b.package.as_str()).collect();
            packages.sort_unstable();
            packages.dedup();
            uris.extend(packages.into_iter().map(|p| Uri {
                r#match: None,
                uri: format!("androidapp://{p}"),
            }));
            for f in sections.into_iter().flat_map(|s| s.fields) {
                let label = f.label.expose().to_owned();
                match f.value {
                    FieldValue::Text(v)
                    | FieldValue::Url(v)
                    | FieldValue::Email(v)
                    | FieldValue::Phone(v)
                    | FieldValue::Date(v) => it.fields.push(field(&label, v, false)),
                    FieldValue::Address(a) => it.fields.push(field(&label, a.formatted(), false)),
                    FieldValue::Password(Some(p)) => it.fields.push(field(&label, p, true)),
                    FieldValue::Otp(Some(c)) => {
                        it.fields.push(field(&label, c.to_otpauth_uri(), true))
                    }
                    FieldValue::Password(None) | FieldValue::Otp(None) => {}
                }
            }
            it.password_history = password_history
                .into_iter()
                .map(|h| History {
                    last_used_date: format_utc_timestamp_ms(h.replaced_at),
                    password: h.password,
                })
                .collect();
            it.notes = notes;
            it.login = Some(Login {
                uris,
                username: ov.username.clone(),
                password,
                totp: totp.map(|c| c.to_otpauth_uri()),
                fido2_credentials: [],
            });
        }
        ItemDetails::SecureNote { content } => {
            it.r#type = 2;
            it.notes = Some(content);
            it.secure_note = Some(SecureNote { r#type: 0 });
        }
        ItemDetails::Card(f) => {
            let f = *f;
            it.r#type = 3;
            it.notes = f.notes.clone();
            it.card = Some(Card {
                brand: Some(f.effective_brand().display_name()),
                cardholder_name: f.cardholder_name,
                number: f.number,
                exp_month: f.expiry.map(|e| e.month.to_string()),
                exp_year: f.expiry.map(|e| e.year.to_string()),
                code: f.verification_number,
            });
        }
        ItemDetails::Identity(f) => {
            let f = *f;
            it.r#type = 4;
            it.notes = f.notes;
            let address1 = match (f.street, f.number) {
                (Some(s), Some(n)) => {
                    Some(SecretString::new(format!("{} {}", s.expose(), n.expose())))
                }
                (s, n) => s.or(n),
            };
            let mut extra = |name: &str, v: Option<SecretString>| {
                if let Some(v) = v {
                    it.fields.push(field(name, v, false));
                }
            };
            extra("Gender", f.gender);
            extra("Birth date", f.birth_date);
            extra("Occupation", f.occupation);
            extra("Job title", f.job_title);
            extra("RG", f.rg);
            extra("Home phone", f.home_phone);
            extra("Work phone", f.work_phone);
            extra("Website", f.website);
            for c in f.custom {
                it.fields.push(field(c.label.expose(), c.value, c.hidden));
            }
            it.identity = Some(Identity {
                first_name: f.first_name,
                middle_name: f.middle_name,
                last_name: f.last_name,
                address1,
                address2: f.complement,
                address3: f.neighborhood,
                city: f.city,
                state: f.state,
                postal_code: f.postal_code,
                country: f.country,
                company: f.company,
                email: f.email,
                phone: f.mobile_phone,
                ssn: f.cpf,
                username: f.username,
                passport_number: f.passport,
                license_number: f.drivers_license,
                ..Default::default()
            });
        }
    }
    it
}

pub(super) fn render(vault: &VaultService) -> Result<Rendered> {
    let mut summary = ExportSummary::default();
    let mut out = Zeroizing::new(Vec::new());
    out.extend_from_slice(br#"{"encrypted":false,"folders":[],"items":["#);
    let mut first = true;
    for_each_item(vault, |ov, d| {
        let Some(d) = d else {
            summary.unreadable += 1;
            return Ok(());
        };
        count(&mut summary, ExportFormat::BitwardenJson, ov, &d);
        if !first {
            out.push(b',');
        }
        first = false;
        serde_json::to_writer(&mut *out, &item(ov, d)).map_err(|_| Error::Encryption)
    })?;
    out.write_all(b"]}").map_err(|_| Error::Encryption)?;
    Ok(Rendered {
        bytes: out,
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{CardBrand, CardFields};
    use crate::model::ItemType;

    fn overview(item_type: ItemType, title: &str) -> ItemOverview {
        let input = crate::model::ItemInput {
            content: crate::model::SecretUpdate::Set("x".into()),
            ..crate::model::ItemInput::blank(ItemType::SecureNote, "t".into())
        };
        let (mut ov, _) = crate::vault::build_item(uuid::Uuid::nil(), input, None, 0, 0).unwrap();
        ov.item_type = item_type;
        ov.title = title.to_owned();
        ov
    }

    #[test]
    fn an_empty_title_never_becomes_an_empty_name() {
        let identity = ItemDetails::Identity(Box::default());
        assert_eq!(
            item(&overview(ItemType::Identity, ""), identity).name,
            "Identity"
        );
        let card = ItemDetails::Card(Box::new(CardFields {
            brand: Some(CardBrand::Visa),
            ..Default::default()
        }));
        assert_eq!(item(&overview(ItemType::Card, " "), card).name, "Visa");
        let note = ItemDetails::SecureNote {
            content: "x".into(),
        };
        assert_eq!(
            item(&overview(ItemType::SecureNote, ""), note).name,
            "Untitled"
        );
        let note = ItemDetails::SecureNote {
            content: "x".into(),
        };
        assert_eq!(
            item(&overview(ItemType::SecureNote, "Kept"), note).name,
            "Kept"
        );
    }
}
