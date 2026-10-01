//! The vault viewer's calls. The list and the view are built from overviews
//! (title, username, websites); every sealed value comes from `reveal`, one
//! field per call, so a screen holds a secret only after the user asked.

use crate::error::MobileResult;
use crate::vault::MobileVault;
use havenkeys_core::card::CardField;
use havenkeys_core::custom_field::FieldValue;
use havenkeys_core::generator;
use havenkeys_core::identity::IdentityField;
use havenkeys_core::model::{ItemOverview, ItemType, SecretField};
use havenkeys_core::Error;
use uuid::Uuid;

#[derive(uniffi::Enum)]
pub enum ItemKind {
    Login,
    SecureNote,
    Card,
    Identity,
}

#[derive(uniffi::Record)]
pub struct ItemSummary {
    pub id: String,
    pub kind: ItemKind,
    pub title: String,
    pub subtitle: Option<String>,
    pub website: Option<String>,
    pub has_totp: bool,
    pub has_passkey: bool,
    pub updated_at: i64,
}

#[derive(uniffi::Enum)]
pub enum FieldKind {
    Text,
    Secret,
    Url,
    Totp,
}

#[derive(uniffi::Record)]
pub struct ViewField {
    pub key: String,
    pub label: String,
    pub kind: FieldKind,
    pub value: Option<String>,
}

#[derive(uniffi::Record)]
pub struct ItemView {
    pub summary: ItemSummary,
    pub fields: Vec<ViewField>,
}

#[derive(uniffi::Record)]
pub struct TotpNow {
    pub code: String,
    pub period: u32,
    pub seconds_remaining: u32,
}

#[derive(uniffi::Record)]
pub struct GeneratorOptions {
    pub length: u32,
    pub uppercase: bool,
    pub lowercase: bool,
    pub digits: bool,
    pub symbols: bool,
    pub avoid_ambiguous: bool,
}

#[derive(uniffi::Record)]
pub struct Generated {
    pub password: String,
    pub entropy_bits: f64,
}

/// Masked until revealed. Everything else is read on demand too, but shown
/// as soon as it arrives.
const DOCUMENTS: [&str; 4] = ["cpf", "rg", "passport", "drivers_license"];

pub(crate) fn parse_id(id: &str) -> MobileResult<Uuid> {
    Ok(Uuid::parse_str(id).map_err(|_| Error::NotFound)?)
}

fn host_of(o: &ItemOverview) -> Option<String> {
    o.urls.first().and_then(|r| url_host(&r.url))
}

fn url_host(raw: &str) -> Option<String> {
    let after_scheme = raw.split_once("://").map_or(raw, |(_, rest)| rest);
    let host = after_scheme.split(['/', '?', '#']).next()?;
    Some(host.trim_start_matches("www.").to_owned()).filter(|h| !h.is_empty())
}

fn summary(o: &ItemOverview) -> ItemSummary {
    let (kind, subtitle) = match o.item_type {
        ItemType::Login => (ItemKind::Login, o.username.clone()),
        ItemType::SecureNote => (ItemKind::SecureNote, None),
        ItemType::Card => (
            ItemKind::Card,
            o.card
                .as_ref()
                .and_then(|c| c.last4.clone())
                .map(|l| format!("•••• {l}")),
        ),
        ItemType::Identity => (ItemKind::Identity, None),
    };
    ItemSummary {
        id: o.id.to_string(),
        kind,
        title: o.title.clone(),
        subtitle,
        website: host_of(o),
        has_totp: o.has_totp,
        has_passkey: o.has_passkey,
        updated_at: o.updated_at,
    }
}

fn field(
    key: impl Into<String>,
    label: impl Into<String>,
    kind: FieldKind,
    value: Option<String>,
) -> ViewField {
    ViewField {
        key: key.into(),
        label: label.into(),
        kind,
        value,
    }
}

/// The names `filled_field_names` returns, mapped to the core's fields.
fn identity_field(name: &str) -> Option<IdentityField> {
    use IdentityField as F;
    Some(match name {
        "first_name" => F::FirstName,
        "middle_name" => F::MiddleName,
        "last_name" => F::LastName,
        "gender" => F::Gender,
        "birth_date" => F::BirthDate,
        "occupation" => F::Occupation,
        "company" => F::Company,
        "job_title" => F::JobTitle,
        "cpf" => F::Cpf,
        "rg" => F::Rg,
        "passport" => F::Passport,
        "drivers_license" => F::DriversLicense,
        "email" => F::Email,
        "mobile_phone" => F::MobilePhone,
        "home_phone" => F::HomePhone,
        "work_phone" => F::WorkPhone,
        "street" => F::Street,
        "number" => F::Number,
        "complement" => F::Complement,
        "neighborhood" => F::Neighborhood,
        "city" => F::City,
        "state" => F::State,
        "postal_code" => F::PostalCode,
        "country" => F::Country,
        "username" => F::Username,
        "website" => F::Website,
        "notes" => F::Notes,
        _ => return None,
    })
}

pub(crate) fn unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[uniffi::export]
impl MobileVault {
    pub fn list_items(&self) -> MobileResult<Vec<ItemSummary>> {
        self.touch();
        let mut items: Vec<ItemSummary> = self
            .client
            .vault()?
            .list_items()?
            .iter()
            .map(summary)
            .collect();
        items.sort_by_key(|s| s.title.to_lowercase());
        Ok(items)
    }

    pub fn search(&self, query: String) -> MobileResult<Vec<ItemSummary>> {
        self.touch();
        Ok(self
            .client
            .vault()?
            .search(&query)?
            .iter()
            .map(summary)
            .collect())
    }

    pub fn item_view(&self, id: String) -> MobileResult<ItemView> {
        self.touch();
        let id = parse_id(&id)?;
        let vault = self.client.vault()?;
        let o = vault.get_item(&id)?;
        let mut fields = Vec::new();
        match o.item_type {
            ItemType::Login => {
                if let Some(u) = &o.username {
                    fields.push(field(
                        "username",
                        "username",
                        FieldKind::Text,
                        Some(u.clone()),
                    ));
                }
                if o.has_password {
                    fields.push(field("password", "password", FieldKind::Secret, None));
                }
                if o.has_totp {
                    fields.push(field("totp", "totp", FieldKind::Totp, None));
                }
                for (i, rule) in o.urls.iter().enumerate() {
                    fields.push(field(
                        format!("website.{i}"),
                        "website",
                        FieldKind::Url,
                        Some(rule.url.clone()),
                    ));
                }
                for section in vault.login_sections(&id)? {
                    for f in section.fields {
                        let kind = if f.value.is_concealed() {
                            FieldKind::Secret
                        } else {
                            FieldKind::Text
                        };
                        fields.push(field(format!("custom.{}", f.id), "", kind, None));
                    }
                }
                if o.has_notes {
                    fields.push(field("notes", "notes", FieldKind::Secret, None));
                }
            }
            ItemType::SecureNote => {
                fields.push(field("content", "content", FieldKind::Secret, None))
            }
            ItemType::Card => {
                for (key, kind) in [
                    ("card.holder", FieldKind::Text),
                    ("card.number", FieldKind::Secret),
                    ("card.code", FieldKind::Secret),
                    ("card.expiry", FieldKind::Text),
                    ("card.notes", FieldKind::Secret),
                ] {
                    fields.push(field(key, key, kind, None));
                }
            }
            ItemType::Identity => {
                let present = vault.reveal_identity(&id)?;
                for name in present.filled_field_names() {
                    let kind = if DOCUMENTS.contains(&name) {
                        FieldKind::Secret
                    } else {
                        FieldKind::Text
                    };
                    fields.push(field(
                        format!("identity.{name}"),
                        format!("identity.{name}"),
                        kind,
                        None,
                    ));
                }
            }
        }
        Ok(ItemView {
            summary: summary(&o),
            fields,
        })
    }

    /// Exactly one field of one item.
    pub fn reveal(&self, id: String, key: String) -> MobileResult<String> {
        self.touch();
        let id = parse_id(&id)?;
        let vault = self.client.vault()?;
        let wrong = || Error::InvalidInput("field does not exist on this item");
        let value = match key.as_str() {
            "password" => vault.reveal(&id, SecretField::Password)?,
            "notes" => vault.reveal(&id, SecretField::Notes)?,
            "content" => vault.reveal(&id, SecretField::Content)?,
            "card.holder" => vault.card_value(&id, CardField::CardholderName)?,
            "card.number" => vault.card_value(&id, CardField::Number)?,
            "card.code" => vault.card_value(&id, CardField::VerificationNumber)?,
            "card.expiry" => vault.card_value(&id, CardField::Expiry)?,
            "card.notes" => vault.card_fields(id)?.notes.ok_or(Error::NotFound)?,
            k if k.starts_with("identity.") => {
                let f = identity_field(&k["identity.".len()..]).ok_or_else(wrong)?;
                vault.identity_value(&id, f)?
            }
            k if k.starts_with("custom.") => {
                let field_id = Uuid::parse_str(&k["custom.".len()..]).map_err(|_| wrong())?;
                let custom = vault.login_field(&id, &field_id)?;
                // An OTP field's secret never leaves the core: its code does.
                if matches!(custom.value, FieldValue::Otp(_)) {
                    custom.totp_code(unix_seconds())?.code
                } else {
                    custom.value.into_text()
                }
            }
            _ => return Err(wrong().into()),
        };
        Ok(value.expose().to_owned())
    }

    pub fn totp(&self, id: String) -> MobileResult<TotpNow> {
        self.touch();
        let id = parse_id(&id)?;
        let code = self.client.vault()?.totp_code(&id, unix_seconds())?;
        Ok(TotpNow {
            code: code.code.expose().to_owned(),
            period: code.period,
            seconds_remaining: code.seconds_remaining,
        })
    }

    pub fn generate_password(&self, o: GeneratorOptions) -> MobileResult<Generated> {
        let g = generator::generate(&generator::GeneratorOptions {
            length: o.length as usize,
            uppercase: o.uppercase,
            lowercase: o.lowercase,
            digits: o.digits,
            symbols: o.symbols,
            avoid_ambiguous: o.avoid_ambiguous,
        })?;
        Ok(Generated {
            password: g.password.expose().to_owned(),
            entropy_bits: g.entropy_bits,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::unlocked;
    use havenkeys_core::model::{ItemInput, ItemType, MatchType, SecretUpdate, UrlRule};
    use havenkeys_core::SecretString;

    fn add_login(v: &MobileVault) -> String {
        let input = ItemInput {
            item_type: ItemType::Login,
            title: "GitHub".into(),
            username: Some("octo".into()),
            urls: vec![UrlRule {
                url: "https://github.com".into(),
                match_type: MatchType::Domain,
            }],
            password: SecretUpdate::Set(SecretString::from("hunter2hunter2")),
            totp: SecretUpdate::Set(SecretString::from("JBSWY3DPEHPK3PXP")),
            notes: SecretUpdate::Keep,
            content: SecretUpdate::Keep,
            auto_sign_in: None,
            sign_in_with: None,
            identity: None,
            card: None,
            sections: None,
        };
        let mut vault = v.client.vault().unwrap();
        let staged = vault.stage_create(input, 1).unwrap();
        let id = staged.item_id;
        vault.commit_write(staged, 1).unwrap();
        id.to_string()
    }

    #[test]
    fn the_list_and_the_view_carry_no_secrets() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = add_login(&v);
        let list = v.list_items().unwrap();
        let login = list.iter().find(|s| s.id == id).unwrap();
        assert_eq!(login.subtitle.as_deref(), Some("octo"));
        assert_eq!(login.website.as_deref(), Some("github.com"));
        let view = v.item_view(id.clone()).unwrap();
        let password = view.fields.iter().find(|f| f.key == "password").unwrap();
        assert!(matches!(password.kind, FieldKind::Secret));
        assert!(password.value.is_none());
        assert!(view
            .fields
            .iter()
            .all(|f| f.value.as_deref() != Some("hunter2hunter2")));
    }

    #[test]
    fn reveal_returns_exactly_one_field() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = add_login(&v);
        assert_eq!(
            v.reveal(id.clone(), "password".into()).unwrap(),
            "hunter2hunter2"
        );
        assert!(v.reveal(id.clone(), "card.number".into()).is_err());
        assert!(v.reveal(id, "../../etc".into()).is_err());
        assert_eq!(v.totp(add_login(&v)).unwrap().code.len(), 6);
    }

    #[test]
    fn a_locked_vault_reveals_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = add_login(&v);
        v.lock();
        assert!(v.list_items().is_err());
        assert!(v.reveal(id.clone(), "password".into()).is_err());
        assert!(v.totp(id).is_err());
    }

    #[test]
    fn search_finds_by_title() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        add_login(&v);
        assert_eq!(v.search("git".into()).unwrap().len(), 1);
        assert!(v.search("zzz".into()).unwrap().is_empty());
    }

    #[test]
    fn an_invalid_id_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        assert!(v.item_view("not-a-uuid".into()).is_err());
        assert!(v
            .reveal(uuid::Uuid::new_v4().to_string(), "password".into())
            .is_err());
    }

    #[test]
    fn the_generator_honours_its_options() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let g = v
            .generate_password(GeneratorOptions {
                length: 32,
                uppercase: false,
                lowercase: true,
                digits: false,
                symbols: false,
                avoid_ambiguous: false,
            })
            .unwrap();
        assert_eq!(g.password.len(), 32);
        assert!(g.password.chars().all(|c| c.is_ascii_lowercase()));
        assert!(v
            .generate_password(GeneratorOptions {
                length: 32,
                uppercase: false,
                lowercase: false,
                digits: false,
                symbols: false,
                avoid_ambiguous: false
            })
            .is_err());
    }
}
