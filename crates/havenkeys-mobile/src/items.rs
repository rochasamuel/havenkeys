//! The vault viewer's calls. The list and the view are built from overviews
//! (title, username, websites); every sealed value comes from `reveal`, one
//! field per call, so a screen holds a secret only after the user asked.

use crate::autofill::unix_seconds;
use crate::error::MobileResult;
use crate::vault::MobileVault;
use havenkeys_core::card::CardField;
use havenkeys_core::custom_field::FieldValue;
use havenkeys_core::generator;
use havenkeys_core::identity::IdentityField;
use havenkeys_core::model::{ItemOverview, ItemType, SecretField};
use havenkeys_core::Error;
use url::Url;
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq, uniffi::Enum)]
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
    pub created_at: i64,
}

#[derive(Clone, Copy, PartialEq, Eq, uniffi::Enum)]
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
pub(crate) const DOCUMENTS: [&str; 4] = ["cpf", "rg", "passport", "drivers_license"];

pub(crate) fn parse_id(id: &str) -> MobileResult<Uuid> {
    Ok(Uuid::parse_str(id).map_err(|_| Error::NotFound)?)
}

fn host_of(o: &ItemOverview) -> Option<String> {
    o.urls.first().and_then(|r| url_host(&r.url))
}

/// The host (and a non-default port) only: a saved URL may carry userinfo,
/// which must not reach the list.
fn url_host(raw: &str) -> Option<String> {
    let parsed = Url::parse(raw)
        .ok()
        .filter(Url::has_host)
        .or_else(|| Url::parse(&format!("https://{raw}")).ok())?;
    let host = parsed.host_str()?;
    let host = host.strip_prefix("www.").unwrap_or(host);
    Some(match parsed.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    })
}

pub(crate) fn summary(o: &ItemOverview) -> ItemSummary {
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
        created_at: o.created_at,
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

#[uniffi::export]
impl MobileVault {
    pub fn list_items(&self) -> MobileResult<Vec<ItemSummary>> {
        // No `touch()` in any read: the app also calls these on its own (a
        // sync's `items_changed`, the TOTP countdown every second), and
        // counting those as activity would keep the vault unlocked forever.
        // Real interaction reaches the timer through `touch()`.
        self.unlocked()?;
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
        self.unlocked()?;
        Ok(self
            .client
            .vault()?
            .search(&query)?
            .iter()
            .map(summary)
            .collect())
    }

    pub fn item_view(&self, id: String) -> MobileResult<ItemView> {
        self.unlocked()?;
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
            // The body is shown as the note opens, as on the desktop; the
            // screen still asks for it with `reveal`, so the view carries none.
            ItemType::SecureNote => fields.push(field("content", "content", FieldKind::Text, None)),
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
        self.unlocked()?;
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
        self.unlocked()?;
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
    use crate::vault::tests::{overdue_refuses, unlocked};
    use havenkeys_core::card::CardInput;
    use havenkeys_core::custom_field::{FieldInput, FieldValueInput, SectionInput};
    use havenkeys_core::identity::IdentityFields;
    use havenkeys_core::model::{ItemInput, ItemType, MatchType, SecretUpdate, UrlRule};
    use havenkeys_core::SecretString;

    fn add_login(v: &MobileVault) -> String {
        let input = ItemInput {
            tags: None,
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

    /// Reads also run when the app refreshes on its own (a sync, the TOTP
    /// countdown): they must not count as the user being there.
    #[test]
    fn reads_do_not_postpone_the_auto_lock() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = add_login(&v);
        v.clock.arm_for(std::time::Duration::from_millis(600));
        std::thread::sleep(std::time::Duration::from_millis(400));
        v.list_items().unwrap();
        v.search("git".into()).unwrap();
        v.item_view(id.clone()).unwrap();
        v.reveal(id.clone(), "password".into()).unwrap();
        v.totp(id).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(v.clock.due().is_some());
    }

    #[test]
    fn an_overdue_vault_locks_before_any_read() {
        let dir = tempfile::tempdir().unwrap();
        let (v, seen) = unlocked(dir.path());
        let id = add_login(&v);
        overdue_refuses(&v, &seen, |v| v.list_items());
        overdue_refuses(&v, &seen, |v| v.search("git".into()));
        overdue_refuses(&v, &seen, |v| v.item_view(id.clone()));
        overdue_refuses(&v, &seen, |v| v.reveal(id.clone(), "password".into()));
        overdue_refuses(&v, &seen, |v| v.totp(id.clone()));
    }

    #[test]
    fn the_website_shown_is_the_host_never_the_userinfo() {
        for (saved, shown) in [
            ("https://user:pw@host.example/", Some("host.example")),
            ("https://www.github.com/login?next=/", Some("github.com")),
            (
                "https://vault.example.com:8443/x",
                Some("vault.example.com:8443"),
            ),
            ("github.com", Some("github.com")),
            ("not a url at all", None),
        ] {
            assert_eq!(url_host(saved).as_deref(), shown, "{saved}");
        }
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

    fn blank(item_type: ItemType, title: &str) -> ItemInput {
        ItemInput {
            tags: None,
            item_type,
            title: title.into(),
            username: None,
            urls: vec![],
            password: SecretUpdate::Keep,
            totp: SecretUpdate::Keep,
            notes: SecretUpdate::Keep,
            content: SecretUpdate::Keep,
            auto_sign_in: None,
            sign_in_with: None,
            identity: None,
            card: None,
            sections: None,
        }
    }

    fn create(v: &MobileVault, input: ItemInput) -> String {
        let mut vault = v.client.vault().unwrap();
        let staged = vault.stage_create(input, 1).unwrap();
        let id = staged.item_id;
        vault.commit_write(staged, 1).unwrap();
        id.to_string()
    }

    fn custom_key(view: &ItemView, n: usize) -> String {
        view.fields
            .iter()
            .filter(|f| f.key.starts_with("custom."))
            .nth(n)
            .unwrap()
            .key
            .clone()
    }

    #[test]
    fn a_custom_otp_field_reveals_its_code_never_its_secret() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let field = |label: &str, value| FieldInput {
            id: None,
            label: SecretString::from(label),
            value,
        };
        let mut input = blank(ItemType::Login, "Bank");
        input.sections = Some(vec![SectionInput {
            id: None,
            title: None,
            fields: vec![
                field(
                    "PIN",
                    FieldValueInput::Password(SecretUpdate::Set(SecretString::from("4821pin"))),
                ),
                field(
                    "2FA",
                    FieldValueInput::Otp(SecretUpdate::Set(SecretString::from("JBSWY3DPEHPK3PXP"))),
                ),
            ],
        }]);
        let id = create(&v, input);
        let view = v.item_view(id.clone()).unwrap();
        let custom: Vec<_> = view
            .fields
            .iter()
            .filter(|f| f.key.starts_with("custom."))
            .collect();
        assert_eq!(custom.len(), 2);
        assert!(custom
            .iter()
            .all(|f| matches!(f.kind, FieldKind::Secret) && f.value.is_none()));
        assert_eq!(
            v.reveal(id.clone(), custom_key(&view, 0)).unwrap(),
            "4821pin"
        );
        let code = v.reveal(id.clone(), custom_key(&view, 1)).unwrap();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
        assert_ne!(code, "JBSWY3DPEHPK3PXP");
        assert!(v
            .reveal(id, format!("custom.{}", uuid::Uuid::new_v4()))
            .is_err());
    }

    #[test]
    fn notes_and_secure_note_content_reveal() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let mut login = blank(ItemType::Login, "Site");
        login.notes = SecretUpdate::Set(SecretString::from("login notes"));
        let login_id = create(&v, login);
        assert_eq!(
            v.reveal(login_id.clone(), "notes".into()).unwrap(),
            "login notes"
        );
        assert!(v.reveal(login_id, "content".into()).is_err());
        let mut note = blank(ItemType::SecureNote, "Memo");
        note.content = SecretUpdate::Set(SecretString::from("the body"));
        let note_id = create(&v, note);
        assert_eq!(
            v.reveal(note_id.clone(), "content".into()).unwrap(),
            "the body"
        );
        let view = v.item_view(note_id).unwrap();
        assert!(view.fields.iter().all(|f| f.value.is_none()));
        // Shown as the note opens (no eye), but still asked for with `reveal`.
        let content = view.fields.iter().find(|f| f.key == "content").unwrap();
        assert!(matches!(content.kind, FieldKind::Text));
    }

    #[test]
    fn card_and_identity_fields_reveal_one_value_each() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let mut card = blank(ItemType::Card, "Visa");
        card.card = Some(CardInput {
            cardholder_name: Some(SecretString::from("ANA SOUZA")),
            number: SecretUpdate::Set(SecretString::from("4111111111111111")),
            verification_number: SecretUpdate::Set(SecretString::from("123")),
            ..Default::default()
        });
        let card_id = create(&v, card);
        assert_eq!(
            v.reveal(card_id.clone(), "card.number".into()).unwrap(),
            "4111111111111111"
        );
        assert_eq!(
            v.reveal(card_id.clone(), "card.code".into()).unwrap(),
            "123"
        );
        assert_eq!(
            v.reveal(card_id.clone(), "card.holder".into()).unwrap(),
            "ANA SOUZA"
        );
        assert!(v.reveal(card_id.clone(), "password".into()).is_err());
        let view = v.item_view(card_id).unwrap();
        assert!(view.fields.iter().all(|f| f.value.is_none()));

        // The one identity is created by the core under a derived id.
        let identity_id = {
            let mut vault = v.client.vault().unwrap();
            let staged = vault
                .stage_identity_if_missing("ana@example.com", 1)
                .unwrap()
                .unwrap();
            let id = staged.item_id;
            vault.commit_write(staged, 1).unwrap();
            let mut update = blank(ItemType::Identity, "");
            update.identity = Some(IdentityFields {
                first_name: Some(SecretString::from("Ana")),
                passport: Some(SecretString::from("AB123456")),
                ..Default::default()
            });
            let staged = vault.stage_update(&id, update, 2).unwrap();
            vault.commit_write(staged, 2).unwrap();
            id.to_string()
        };
        assert_eq!(
            v.reveal(identity_id.clone(), "identity.first_name".into())
                .unwrap(),
            "Ana"
        );
        assert_eq!(
            v.reveal(identity_id.clone(), "identity.passport".into())
                .unwrap(),
            "AB123456"
        );
        assert!(v
            .reveal(identity_id.clone(), "identity.bogus".into())
            .is_err());
        assert!(v
            .reveal(identity_id.clone(), "identity.cpf".into())
            .is_err());
        let view = v.item_view(identity_id).unwrap();
        let passport = view
            .fields
            .iter()
            .find(|f| f.key == "identity.passport")
            .unwrap();
        assert!(matches!(passport.kind, FieldKind::Secret));
        let name = view
            .fields
            .iter()
            .find(|f| f.key == "identity.first_name")
            .unwrap();
        assert!(matches!(name.kind, FieldKind::Text));
        assert!(view.fields.iter().all(|f| f.value.is_none()));
    }
}
