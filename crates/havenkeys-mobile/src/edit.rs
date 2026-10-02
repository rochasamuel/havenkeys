//! Creating, editing and deleting items (spec 2026-10-01-android-app §6.2).
//!
//! The editor sends changes, not items: a field it did not touch is `Keep`
//! and Rust fills it in from the vault. The edit screen never has to load a
//! secret it is not changing, and what the phone cannot edit (custom fields,
//! passkeys, app bindings, "Sign in with", a card's brand) stays as it is.
//! Every write needs the server; nothing is recorded until it accepted it.
//!
//! None of these types derive `Debug`: drafts carry typed secrets.

use crate::error::{MobileError, MobileResult};
use crate::items::{parse_id, FieldKind, ItemKind, DOCUMENTS};
use crate::vault::MobileVault;
use havenkeys_core::card::{CardExpiry, CardInput};
use havenkeys_core::identity::IdentityFields;
use havenkeys_core::model::{normalize_url, ItemInput, ItemType, MatchType, SecretUpdate, UrlRule};
use havenkeys_core::totp::parse_totp_input;
use havenkeys_core::vault::StagedWrite;
use havenkeys_core::{Error, SecretString};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(uniffi::Enum)]
pub enum Change {
    Keep,
    Replace { value: String },
    Remove,
}

#[derive(Clone, Copy, uniffi::Enum)]
pub enum MatchKind {
    Domain,
    Origin,
    Exact,
}

#[derive(uniffi::Record)]
pub struct Website {
    pub url: String,
    pub match_kind: MatchKind,
}

/// One field the editor shows. `value` is set for the username only (it is
/// part of the overview); every other value is read with `reveal`.
#[derive(uniffi::Record)]
pub struct EditField {
    pub key: String,
    pub kind: FieldKind,
    pub present: bool,
    pub value: Option<String>,
}

#[derive(uniffi::Record)]
pub struct ItemEdit {
    pub kind: ItemKind,
    pub title: String,
    pub websites: Vec<Website>,
    pub fields: Vec<EditField>,
    /// Kept as they are by every save; edited on the desktop.
    pub has_custom_fields: bool,
    pub deletable: bool,
    /// The revision the phone holds for the item; `None` for a new one.
    /// A draft carries it back so a save made from an older copy is refused.
    pub revision: Option<i64>,
}

#[derive(uniffi::Record)]
pub struct FieldChange {
    pub key: String,
    pub change: Change,
}

/// `websites` replaces a login's list; it is ignored for other kinds.
/// `base_revision` is the `revision` of the `ItemEdit` the draft was made
/// from.
#[derive(uniffi::Record)]
pub struct ItemDraft {
    pub kind: ItemKind,
    pub title: String,
    pub websites: Vec<Website>,
    pub changes: Vec<FieldChange>,
    pub base_revision: Option<i64>,
}

/// Every identity value, in the order the desktop shows them.
const IDENTITY_FIELDS: [&str; 27] = [
    "first_name",
    "middle_name",
    "last_name",
    "gender",
    "birth_date",
    "occupation",
    "company",
    "job_title",
    "cpf",
    "rg",
    "passport",
    "drivers_license",
    "email",
    "mobile_phone",
    "home_phone",
    "work_phone",
    "street",
    "number",
    "complement",
    "neighborhood",
    "city",
    "state",
    "postal_code",
    "country",
    "username",
    "website",
    "notes",
];

/// The fields the editor shows for `kind`, keyed like `reveal`.
fn fields_of(kind: ItemKind) -> Vec<(String, FieldKind)> {
    let list = |l: &[(&str, FieldKind)]| l.iter().map(|(k, f)| ((*k).to_owned(), *f)).collect();
    match kind {
        ItemKind::Login => list(&[
            ("username", FieldKind::Text),
            ("password", FieldKind::Secret),
            ("totp", FieldKind::Totp),
            ("notes", FieldKind::Secret),
        ]),
        ItemKind::SecureNote => list(&[("content", FieldKind::Secret)]),
        ItemKind::Card => list(&[
            ("card.holder", FieldKind::Text),
            ("card.number", FieldKind::Secret),
            ("card.code", FieldKind::Secret),
            ("card.expiry", FieldKind::Text),
            ("card.notes", FieldKind::Secret),
        ]),
        ItemKind::Identity => IDENTITY_FIELDS
            .iter()
            .map(|n| {
                let kind = if DOCUMENTS.contains(n) {
                    FieldKind::Secret
                } else {
                    FieldKind::Text
                };
                (format!("identity.{n}"), kind)
            })
            .collect(),
    }
}

fn kind_of(t: ItemType) -> ItemKind {
    match t {
        ItemType::Login => ItemKind::Login,
        ItemType::SecureNote => ItemKind::SecureNote,
        ItemType::Card => ItemKind::Card,
        ItemType::Identity => ItemKind::Identity,
    }
}

fn match_kind(t: &MatchType) -> MatchKind {
    match t {
        MatchType::Domain => MatchKind::Domain,
        MatchType::Origin => MatchKind::Origin,
        MatchType::Exact => MatchKind::Exact,
    }
}

fn match_type(k: MatchKind) -> MatchType {
    match k {
        MatchKind::Domain => MatchType::Domain,
        MatchKind::Origin => MatchType::Origin,
        MatchKind::Exact => MatchType::Exact,
    }
}

fn invalid(code: &str, detail: &str) -> MobileError {
    MobileError::Failed {
        code: code.into(),
        detail: detail.into(),
    }
}

fn blank(item_type: ItemType, title: String) -> ItemInput {
    ItemInput {
        item_type,
        title,
        username: None,
        urls: Vec::new(),
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

fn rules(websites: Vec<Website>) -> MobileResult<Vec<UrlRule>> {
    websites
        .into_iter()
        .filter(|w| !w.url.trim().is_empty())
        .map(|w| {
            let url = normalize_url(&w.url)
                .map_err(|_| invalid("invalid_website", "A website address is not valid."))?;
            Ok(UrlRule {
                url,
                match_type: match_type(w.match_kind),
            })
        })
        .collect()
}

fn checked_totp(update: SecretUpdate) -> MobileResult<SecretUpdate> {
    if let SecretUpdate::Set(v) = &update {
        if !v.expose().trim().is_empty() && parse_totp_input(v.expose()).is_err() {
            return Err(invalid(
                "invalid_totp",
                "That is not a setup key or otpauth:// link.",
            ));
        }
    }
    Ok(update)
}

/// `MM/YY`, `MM/YYYY` (as shown) or `YYYY-MM`; blank is no expiry.
pub(crate) fn parse_expiry(text: &str) -> MobileResult<Option<CardExpiry>> {
    let t = text.trim();
    if t.is_empty() {
        return Ok(None);
    }
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let parsed = match t.split_once('/') {
        Some((m, y))
            if digits(m) && m.len() <= 2 && digits(y) && (y.len() == 2 || y.len() == 4) =>
        {
            let month: u8 = m.parse().unwrap_or(0);
            let year: u16 = y.parse().unwrap_or(0);
            CardExpiry::new(if y.len() == 2 { 2000 + year } else { year }, month)
        }
        Some(_) => Err(Error::InvalidInput("bad expiry")),
        None => CardExpiry::parse(t),
    };
    parsed
        .map(Some)
        .map_err(|_| invalid("invalid_expiry", "Enter the expiry as MM/YYYY."))
}

fn identity_slot<'a>(
    f: &'a mut IdentityFields,
    name: &str,
) -> Option<&'a mut Option<SecretString>> {
    Some(match name {
        "first_name" => &mut f.first_name,
        "middle_name" => &mut f.middle_name,
        "last_name" => &mut f.last_name,
        "gender" => &mut f.gender,
        "birth_date" => &mut f.birth_date,
        "occupation" => &mut f.occupation,
        "company" => &mut f.company,
        "job_title" => &mut f.job_title,
        "cpf" => &mut f.cpf,
        "rg" => &mut f.rg,
        "passport" => &mut f.passport,
        "drivers_license" => &mut f.drivers_license,
        "email" => &mut f.email,
        "mobile_phone" => &mut f.mobile_phone,
        "home_phone" => &mut f.home_phone,
        "work_phone" => &mut f.work_phone,
        "street" => &mut f.street,
        "number" => &mut f.number,
        "complement" => &mut f.complement,
        "neighborhood" => &mut f.neighborhood,
        "city" => &mut f.city,
        "state" => &mut f.state,
        "postal_code" => &mut f.postal_code,
        "country" => &mut f.country,
        "username" => &mut f.username,
        "website" => &mut f.website,
        "notes" => &mut f.notes,
        _ => return None,
    })
}

/// A draft's changes by key. A key the kind does not have, or one named
/// twice, refuses the whole draft.
struct Changes(HashMap<String, Change>);

impl Changes {
    fn new(list: Vec<FieldChange>, kind: ItemKind) -> MobileResult<Self> {
        let allowed = fields_of(kind);
        let mut map = HashMap::new();
        for c in list {
            if !allowed.iter().any(|(k, _)| *k == c.key) || map.contains_key(&c.key) {
                return Err(invalid(
                    "invalid_field",
                    "That field cannot be changed on this item.",
                ));
            }
            map.insert(c.key, c.change);
        }
        Ok(Self(map))
    }

    fn take(&mut self, key: &str) -> Option<Change> {
        self.0.remove(key)
    }

    /// A sealed value the core merges itself: `Keep` reads the vault there.
    fn update(&mut self, key: &str) -> SecretUpdate {
        match self.take(key) {
            None | Some(Change::Keep) => SecretUpdate::Keep,
            Some(Change::Replace { value }) => SecretUpdate::Set(SecretString::new(value)),
            Some(Change::Remove) => SecretUpdate::Clear,
        }
    }

    /// A blank `Replace` keeps the 2FA key: only an explicit `Remove`
    /// clears it, so a stray space never drops the second factor.
    fn totp(&mut self) -> SecretUpdate {
        match self.update("totp") {
            SecretUpdate::Set(v) if v.expose().trim().is_empty() => SecretUpdate::Keep,
            other => other,
        }
    }

    /// A value the core takes in full: the change applied to `current`.
    /// Replacing it with only spaces removes it.
    fn apply(&mut self, key: &str, current: Option<SecretString>) -> Option<SecretString> {
        match self.take(key) {
            None | Some(Change::Keep) => current,
            Some(Change::Replace { value }) if value.trim().is_empty() => None,
            Some(Change::Replace { value }) => Some(SecretString::new(value)),
            Some(Change::Remove) => None,
        }
    }
}

impl MobileVault {
    /// Seal `draft` as a new item (`id` `None`) or as a change to `id`.
    /// Nothing is sent or recorded here.
    pub(crate) fn stage_draft(
        &self,
        id: Option<&Uuid>,
        draft: ItemDraft,
    ) -> MobileResult<StagedWrite> {
        self.unlocked()?;
        let vault = self.client.vault()?;
        let existing = id.map(|id| vault.get_item(id)).transpose()?;
        if existing
            .as_ref()
            .is_some_and(|o| kind_of(o.item_type) != draft.kind)
        {
            return Err(Error::InvalidInput("item type cannot change").into());
        }
        // A pull may have brought another device's change since the editor
        // opened. The title and websites are always sent in full, so saving
        // over it would undo that change without a word.
        if let Some(id) = id {
            if vault.item_revision(id)? != draft.base_revision {
                return Err(Error::ItemChangedElsewhere.into());
            }
        }
        let mut changes = Changes::new(draft.changes, draft.kind)?;
        let title = draft.title.trim().to_owned();
        let input = match draft.kind {
            ItemKind::Login | ItemKind::SecureNote if title.is_empty() => {
                return Err(invalid("title_required", "Add a title."))
            }
            ItemKind::Login => {
                let current = existing
                    .as_ref()
                    .and_then(|o| o.username.clone())
                    .map(SecretString::new);
                ItemInput {
                    username: changes
                        .apply("username", current)
                        .map(|u| u.expose().trim().to_owned()),
                    urls: rules(draft.websites)?,
                    password: changes.update("password"),
                    totp: checked_totp(changes.totp())?,
                    notes: changes.update("notes"),
                    // The core takes it in full: an update without it would erase it.
                    sign_in_with: existing.as_ref().and_then(|o| o.sign_in_with.clone()),
                    ..blank(ItemType::Login, title)
                }
            }
            ItemKind::SecureNote => ItemInput {
                content: changes.update("content"),
                ..blank(ItemType::SecureNote, title)
            },
            ItemKind::Card => {
                let current = id
                    .map(|id| vault.card_fields(*id))
                    .transpose()?
                    .unwrap_or_default();
                let number = changes.update("card.number");
                let has_number = match &number {
                    SecretUpdate::Set(v) => !v.expose().trim().is_empty(),
                    SecretUpdate::Keep => current.number.is_some(),
                    SecretUpdate::Clear => false,
                };
                if title.is_empty() && !has_number {
                    return Err(invalid(
                        "card_title_required",
                        "Add a title or a card number.",
                    ));
                }
                let expiry = match changes.take("card.expiry") {
                    None | Some(Change::Keep) => current.expiry,
                    Some(Change::Remove) => None,
                    Some(Change::Replace { value }) => parse_expiry(&value)?,
                };
                ItemInput {
                    card: Some(CardInput {
                        cardholder_name: changes
                            .apply("card.holder", current.cardholder_name.clone()),
                        brand: current.brand,
                        number,
                        verification_number: changes.update("card.code"),
                        expiry,
                        notes: changes.apply("card.notes", current.notes.clone()),
                    }),
                    ..blank(ItemType::Card, title)
                }
            }
            ItemKind::Identity => {
                // The one identity is created by the client, never here.
                let id = id.ok_or(Error::Denied)?;
                let mut fields = vault.reveal_identity(id)?;
                for name in IDENTITY_FIELDS {
                    if let Some(slot) = identity_slot(&mut fields, name) {
                        let now = std::mem::take(slot);
                        *slot = changes.apply(&format!("identity.{name}"), now);
                    }
                }
                ItemInput {
                    identity: Some(fields),
                    ..blank(ItemType::Identity, String::new())
                }
            }
        };
        let now = havenkeys_client::now_ms();
        Ok(match id {
            Some(id) => vault.stage_update(id, input, now)?,
            None => vault.stage_create(input, now)?,
        })
    }
}

#[uniffi::export]
impl MobileVault {
    /// What the editor needs to open `id`: names and presence, the title,
    /// websites and username. No secret.
    pub fn item_edit(&self, id: String) -> MobileResult<ItemEdit> {
        self.unlocked()?;
        let id = parse_id(&id)?;
        let vault = self.client.vault()?;
        let o = vault.get_item(&id)?;
        let kind = kind_of(o.item_type);
        let mut title = o.title.clone();
        let present: Vec<String> = match kind {
            ItemKind::Login => [
                ("username", o.username.is_some()),
                ("password", o.has_password),
                ("totp", o.has_totp),
                ("notes", o.has_notes),
            ]
            .into_iter()
            .filter(|(_, p)| *p)
            .map(|(k, _)| k.to_owned())
            .collect(),
            ItemKind::SecureNote => vec!["content".to_owned()],
            ItemKind::Card => {
                let c = vault.card_fields(id)?;
                // A card named after its brand is named again from its
                // number on every save: a blank title lets the core do it,
                // so a new number of another brand renames it.
                if c.number.is_some() && title == c.effective_brand().display_name() {
                    title.clear();
                }
                [
                    ("card.holder", c.cardholder_name.is_some()),
                    ("card.number", c.number.is_some()),
                    ("card.code", c.verification_number.is_some()),
                    ("card.expiry", c.expiry.is_some()),
                    ("card.notes", c.notes.is_some()),
                ]
                .into_iter()
                .filter(|(_, p)| *p)
                .map(|(k, _)| k.to_owned())
                .collect()
            }
            ItemKind::Identity => vault
                .reveal_identity(&id)?
                .filled_field_names()
                .into_iter()
                .map(|n| format!("identity.{n}"))
                .collect(),
        };
        let has_custom_fields = kind == ItemKind::Login && !vault.login_sections(&id)?.is_empty();
        Ok(ItemEdit {
            kind,
            title,
            websites: o
                .urls
                .iter()
                .map(|r| Website {
                    url: r.url.clone(),
                    match_kind: match_kind(&r.match_type),
                })
                .collect(),
            fields: fields_of(kind)
                .into_iter()
                .map(|(key, field_kind)| EditField {
                    present: present.contains(&key),
                    value: if key == "username" {
                        o.username.clone()
                    } else {
                        None
                    },
                    key,
                    kind: field_kind,
                })
                .collect(),
            has_custom_fields,
            deletable: kind != ItemKind::Identity,
            revision: vault.item_revision(&id)?,
        })
    }

    /// The editor for a new item of `kind`. The identity cannot be created.
    pub fn item_template(&self, kind: ItemKind) -> MobileResult<ItemEdit> {
        self.unlocked()?;
        if kind == ItemKind::Identity {
            return Err(Error::Denied.into());
        }
        Ok(ItemEdit {
            kind,
            title: String::new(),
            websites: Vec::new(),
            fields: fields_of(kind)
                .into_iter()
                .map(|(key, field_kind)| EditField {
                    key,
                    kind: field_kind,
                    present: false,
                    value: None,
                })
                .collect(),
            has_custom_fields: false,
            deletable: true,
            revision: None,
        })
    }

    /// Returns the new item's ID.
    pub fn create_item(&self, draft: ItemDraft) -> MobileResult<String> {
        self.unlocked()?;
        self.client.require_online()?;
        let staged = self.stage_draft(None, draft)?;
        let id = staged.item_id.to_string();
        self.send(staged)?;
        Ok(id)
    }

    pub fn update_item(&self, id: String, draft: ItemDraft) -> MobileResult<()> {
        let id = parse_id(&id)?;
        self.unlocked()?;
        self.client.require_online()?;
        let staged = self.stage_draft(Some(&id), draft)?;
        self.send(staged)
    }

    /// The identity cannot be deleted (the core refuses).
    pub fn delete_item(&self, id: String) -> MobileResult<()> {
        let id = parse_id(&id)?;
        self.unlocked()?;
        self.client.require_online()?;
        let staged = self.client.vault()?.stage_delete(&id)?;
        self.send(staged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autofill::TargetFacts;
    use crate::vault::tests::{code, overdue_refuses, unlocked};
    use havenkeys_core::app_target::AppIdentity;
    use havenkeys_core::custom_field::{FieldInput, FieldValueInput, SectionInput};
    use havenkeys_core::passkey::PasskeyCreate;
    use havenkeys_core::sso::{SignInWith, SsoProvider};

    fn commit(v: &MobileVault, staged: StagedWrite) {
        v.client.vault().unwrap().commit_write(staged, 1).unwrap();
    }

    fn replace(key: &str, value: &str) -> FieldChange {
        FieldChange {
            key: key.into(),
            change: Change::Replace {
                value: value.into(),
            },
        }
    }

    fn remove(key: &str) -> FieldChange {
        FieldChange {
            key: key.into(),
            change: Change::Remove,
        }
    }

    fn github() -> Website {
        Website {
            url: "github.com".into(),
            match_kind: MatchKind::Domain,
        }
    }

    fn login_draft(title: &str, changes: Vec<FieldChange>) -> ItemDraft {
        ItemDraft {
            kind: ItemKind::Login,
            title: title.into(),
            websites: vec![github()],
            changes,
            base_revision: None,
        }
    }

    fn create(v: &MobileVault, draft: ItemDraft) -> String {
        let staged = v.stage_draft(None, draft).unwrap();
        let id = staged.item_id.to_string();
        commit(v, staged);
        id
    }

    /// Saves `draft` as an edit opened now: it carries the item's revision.
    fn update(v: &MobileVault, id: &str, mut draft: ItemDraft) {
        draft.base_revision = v.item_edit(id.into()).unwrap().revision;
        let id = Uuid::parse_str(id).unwrap();
        let staged = v.stage_draft(Some(&id), draft).unwrap();
        commit(v, staged);
    }

    fn stage_error(v: &MobileVault, id: Option<&str>, draft: ItemDraft) -> String {
        let id = id.map(|i| Uuid::parse_str(i).unwrap());
        let Err(e) = v.stage_draft(id.as_ref(), draft) else {
            panic!("refused")
        };
        code(e)
    }

    fn chrome_github() -> TargetFacts {
        TargetFacts {
            package_name: "com.android.chrome".into(),
            signing_certs: vec![havenkeys_core::app_target::parse_fingerprint(
                "F0:FD:6C:5B:41:0F:25:CB:25:C3:B5:33:46:C8:97:2F:AE:30:F8:EE:74:11:DF:91:04:80:AD:6B:2D:60:DB:83",
            )
            .unwrap()
            .to_vec()],
            web_domain: Some("github.com".into()),
            web_scheme: Some("https".into()),
        }
    }

    fn full_login(v: &MobileVault) -> String {
        create(
            v,
            login_draft(
                "GitHub",
                vec![
                    replace("username", "octo"),
                    replace("password", "hunter2hunter2"),
                    replace("totp", "JBSWY3DPEHPK3PXP"),
                    replace("notes", "recovery codes in the safe"),
                ],
            ),
        )
    }

    #[test]
    fn a_new_login_holds_what_was_typed_and_fills_its_site() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = full_login(&v);
        assert_eq!(
            v.reveal(id.clone(), "password".into()).unwrap(),
            "hunter2hunter2"
        );
        assert_eq!(
            v.reveal(id.clone(), "notes".into()).unwrap(),
            "recovery codes in the safe"
        );
        assert_eq!(v.totp(id.clone()).unwrap().code.len(), 6);
        let m = v.autofill_matches(chrome_github()).unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].id, id);
        assert_eq!(m[0].username.as_deref(), Some("octo"));
    }

    #[test]
    fn untouched_fields_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = full_login(&v);
        update(&v, &id, login_draft("GitHub (work)", vec![]));
        let view = v.item_view(id.clone()).unwrap();
        assert_eq!(view.summary.title, "GitHub (work)");
        assert_eq!(view.summary.subtitle.as_deref(), Some("octo"));
        assert_eq!(
            v.reveal(id.clone(), "password".into()).unwrap(),
            "hunter2hunter2"
        );
        assert_eq!(
            v.reveal(id.clone(), "notes".into()).unwrap(),
            "recovery codes in the safe"
        );
        assert!(view.summary.has_totp);
    }

    #[test]
    fn replace_and_remove_change_one_value_each() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = full_login(&v);
        update(
            &v,
            &id,
            login_draft(
                "GitHub",
                vec![
                    replace("password", "a-new-password"),
                    remove("totp"),
                    remove("notes"),
                ],
            ),
        );
        assert_eq!(
            v.reveal(id.clone(), "password".into()).unwrap(),
            "a-new-password"
        );
        assert!(v.reveal(id.clone(), "notes".into()).is_err());
        assert!(!v.item_view(id.clone()).unwrap().summary.has_totp);
        update(&v, &id, login_draft("GitHub", vec![remove("username")]));
        assert_eq!(v.item_view(id).unwrap().summary.subtitle, None);
    }

    #[test]
    fn an_edit_keeps_what_the_phone_does_not_edit() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let mut s = v.settings().unwrap();
        s.asset_links = false;
        v.update_settings(s).unwrap();
        let id = {
            let mut vault = v.client.vault().unwrap();
            let input = ItemInput {
                username: Some("octo".into()),
                urls: vec![UrlRule {
                    url: "https://github.com".into(),
                    match_type: MatchType::Domain,
                }],
                password: SecretUpdate::Set(SecretString::from("hunter2hunter2")),
                sign_in_with: Some(SignInWith {
                    provider: SsoProvider::Google,
                    account: Some("ana@example.com".into()),
                }),
                sections: Some(vec![SectionInput {
                    id: None,
                    title: None,
                    fields: vec![FieldInput {
                        id: None,
                        label: SecretString::from("PIN"),
                        value: FieldValueInput::Text(SecretString::from("4821")),
                    }],
                }]),
                ..blank(ItemType::Login, "GitHub".into())
            };
            let staged = vault.stage_create(input, 1).unwrap();
            let id = staged.item_id;
            vault.commit_write(staged, 1).unwrap();
            let app = AppIdentity::new("com.github.android", &[vec![1; 32]]).unwrap();
            let staged = vault.stage_bind_app(&id, &app, 2).unwrap();
            vault.commit_write(staged, 2).unwrap();
            let passkey = vault
                .stage_passkey_create(
                    PasskeyCreate {
                        rp_id: "github.com",
                        page_url: "https://github.com/login",
                        top_url: None,
                        challenge: &[7; 32],
                        user_handle: &[1],
                        user_name: "octo",
                        display_name: None,
                        item_id: Some(id),
                        conditional: false,
                    },
                    3,
                )
                .unwrap();
            vault.commit_write(passkey.write, 3).unwrap();
            id
        };
        assert!(v.client.vault().unwrap().get_item(&id).unwrap().has_passkey);
        let app = TargetFacts {
            package_name: "com.github.android".into(),
            signing_certs: vec![vec![1; 32]],
            web_domain: None,
            web_scheme: None,
        };
        assert_eq!(v.autofill_matches(app.clone()).unwrap().len(), 1);

        update(
            &v,
            &id.to_string(),
            login_draft("Renamed", vec![replace("password", "a-new-password")]),
        );

        let o = v.client.vault().unwrap().get_item(&id).unwrap();
        assert_eq!(o.title, "Renamed");
        assert!(o.has_passkey);
        assert_eq!(
            v.client.vault().unwrap().list_passkeys(&id).unwrap().len(),
            1
        );
        assert_eq!(
            o.sign_in_with.as_ref().unwrap().account.as_deref(),
            Some("ana@example.com")
        );
        let view = v.item_view(id.to_string()).unwrap();
        let custom: Vec<_> = view
            .fields
            .iter()
            .filter(|f| f.key.starts_with("custom."))
            .collect();
        assert_eq!(custom.len(), 1);
        assert_eq!(
            v.reveal(id.to_string(), custom[0].key.clone()).unwrap(),
            "4821"
        );
        assert_eq!(v.autofill_matches(app).unwrap().len(), 1);
        assert!(v.item_edit(id.to_string()).unwrap().has_custom_fields);
    }

    #[test]
    fn a_card_edit_keeps_what_was_not_changed() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = create(
            &v,
            ItemDraft {
                kind: ItemKind::Card,
                title: String::new(),
                websites: vec![],
                base_revision: None,
                changes: vec![
                    replace("card.holder", "ANA SOUZA"),
                    replace("card.number", "4111111111111111"),
                    replace("card.code", "123"),
                    replace("card.expiry", "4/30"),
                ],
            },
        );
        assert_eq!(v.item_view(id.clone()).unwrap().summary.title, "Visa");
        update(
            &v,
            &id,
            ItemDraft {
                kind: ItemKind::Card,
                title: "Visa".into(),
                websites: vec![],
                base_revision: None,
                changes: vec![replace("card.code", "999")],
            },
        );
        assert_eq!(v.reveal(id.clone(), "card.code".into()).unwrap(), "999");
        assert_eq!(
            v.reveal(id.clone(), "card.number".into()).unwrap(),
            "4111111111111111"
        );
        assert_eq!(
            v.reveal(id.clone(), "card.holder".into()).unwrap(),
            "ANA SOUZA"
        );
        assert_eq!(
            v.reveal(id.clone(), "card.expiry".into()).unwrap(),
            "04/2030"
        );
        update(
            &v,
            &id,
            ItemDraft {
                kind: ItemKind::Card,
                title: "Visa".into(),
                websites: vec![],
                base_revision: None,
                changes: vec![replace("card.expiry", "2031-05"), remove("card.holder")],
            },
        );
        assert_eq!(
            v.reveal(id.clone(), "card.expiry".into()).unwrap(),
            "05/2031"
        );
        assert!(v.reveal(id, "card.holder".into()).is_err());
    }

    #[test]
    fn identity_values_change_one_by_one_and_it_is_never_created_here() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let identity = {
            let mut vault = v.client.vault().unwrap();
            let staged = vault
                .stage_identity_if_missing("ana@example.com", 1)
                .unwrap()
                .unwrap();
            let id = staged.item_id;
            vault.commit_write(staged, 1).unwrap();
            id.to_string()
        };
        let draft = |changes| ItemDraft {
            kind: ItemKind::Identity,
            title: String::new(),
            websites: vec![],
            changes,
            base_revision: None,
        };
        update(
            &v,
            &identity,
            draft(vec![
                replace("identity.first_name", "Ana"),
                replace("identity.passport", "AB123456"),
            ]),
        );
        update(&v, &identity, draft(vec![remove("identity.passport")]));
        assert_eq!(
            v.reveal(identity.clone(), "identity.first_name".into())
                .unwrap(),
            "Ana"
        );
        assert_eq!(
            v.reveal(identity.clone(), "identity.email".into()).unwrap(),
            "ana@example.com"
        );
        assert!(v
            .reveal(identity.clone(), "identity.passport".into())
            .is_err());
        assert_eq!(stage_error(&v, None, draft(vec![])), "denied");
        assert!(!v.item_edit(identity).unwrap().deletable);
        assert_eq!(
            code(v.item_template(ItemKind::Identity).err().unwrap()),
            "denied"
        );
    }

    #[test]
    fn bad_input_gets_a_code_the_app_can_name() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        assert_eq!(
            stage_error(&v, None, login_draft("  ", vec![])),
            "title_required"
        );
        assert_eq!(
            stage_error(
                &v,
                None,
                login_draft("x", vec![replace("totp", "not a key!!")])
            ),
            "invalid_totp"
        );
        let mut bad_site = login_draft("x", vec![]);
        bad_site.websites = vec![Website {
            url: "http://exa mple.com".into(),
            match_kind: MatchKind::Domain,
        }];
        assert_eq!(stage_error(&v, None, bad_site), "invalid_website");
        assert_eq!(
            stage_error(
                &v,
                None,
                login_draft("x", vec![replace("card.number", "4111111111111111")])
            ),
            "invalid_field"
        );
        assert_eq!(
            stage_error(
                &v,
                None,
                login_draft(
                    "x",
                    vec![replace("password", "a"), replace("password", "b")]
                )
            ),
            "invalid_field"
        );
        let card = |changes| ItemDraft {
            kind: ItemKind::Card,
            title: String::new(),
            websites: vec![],
            changes,
            base_revision: None,
        };
        assert_eq!(stage_error(&v, None, card(vec![])), "card_title_required");
        assert_eq!(
            stage_error(
                &v,
                None,
                card(vec![
                    replace("card.number", "4111111111111111"),
                    replace("card.expiry", "13/2030")
                ])
            ),
            "invalid_expiry"
        );
        let id = full_login(&v);
        let note = ItemDraft {
            kind: ItemKind::SecureNote,
            title: "x".into(),
            websites: vec![],
            changes: vec![],
            base_revision: None,
        };
        assert_eq!(stage_error(&v, Some(&id), note), "invalid_input");
    }

    #[test]
    fn item_edit_names_fields_and_hands_out_no_secret() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = full_login(&v);
        let edit = v.item_edit(id).unwrap();
        assert!(edit.kind == ItemKind::Login);
        assert_eq!(edit.title, "GitHub");
        assert_eq!(edit.websites.len(), 1);
        assert!(edit.deletable);
        let keys: Vec<_> = edit.fields.iter().map(|f| f.key.as_str()).collect();
        assert_eq!(keys, ["username", "password", "totp", "notes"]);
        assert!(edit.fields.iter().all(|f| f.present));
        let username = edit.fields.iter().find(|f| f.key == "username").unwrap();
        assert_eq!(username.value.as_deref(), Some("octo"));
        assert!(edit
            .fields
            .iter()
            .filter(|f| f.key != "username")
            .all(|f| f.value.is_none()));

        let template = v.item_template(ItemKind::Card).unwrap();
        assert!(template
            .fields
            .iter()
            .all(|f| !f.present && f.value.is_none()));
        assert_eq!(template.fields.len(), 5);
    }

    #[test]
    fn edits_need_the_server() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = full_login(&v);
        assert_eq!(
            code(v.create_item(login_draft("New", vec![])).err().unwrap()),
            "offline"
        );
        assert_eq!(
            code(
                v.update_item(id.clone(), login_draft("Changed", vec![]))
                    .err()
                    .unwrap()
            ),
            "offline"
        );
        assert_eq!(code(v.delete_item(id.clone()).err().unwrap()), "offline");
        assert_eq!(v.list_items().unwrap().len(), 1);
        assert_eq!(v.item_view(id).unwrap().summary.title, "GitHub");
    }

    #[test]
    fn a_locked_or_overdue_vault_edits_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (v, seen) = unlocked(dir.path());
        let id = full_login(&v);
        overdue_refuses(&v, &seen, |v| v.item_edit(id.clone()));
        overdue_refuses(&v, &seen, |v| v.item_template(ItemKind::Login));
        overdue_refuses(&v, &seen, |v| v.create_item(login_draft("New", vec![])));
        overdue_refuses(&v, &seen, |v| {
            v.update_item(id.clone(), login_draft("x", vec![]))
        });
        overdue_refuses(&v, &seen, |v| v.delete_item(id.clone()));
        assert_eq!(
            code(v.stage_draft(None, login_draft("x", vec![])).err().unwrap()),
            "locked"
        );
    }

    #[test]
    fn an_edit_from_an_older_copy_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = full_login(&v);
        let uuid = Uuid::parse_str(&id).unwrap();
        let opened = v.item_edit(id.clone()).unwrap();
        assert_eq!(opened.revision, Some(1));

        // Another device's change arrives in a pull while the editor is open.
        {
            let mut vault = v.client.vault().unwrap();
            let input = ItemInput {
                urls: vec![UrlRule {
                    url: "https://github.com".into(),
                    match_type: MatchType::Domain,
                }],
                ..blank(ItemType::Login, "GitHub (elsewhere)".into())
            };
            let staged = vault.stage_update(&uuid, input, 2).unwrap();
            vault.commit_write(staged, 2).unwrap();
        }
        let stale = ItemDraft {
            base_revision: opened.revision,
            ..login_draft("GitHub", vec![])
        };
        assert_eq!(stage_error(&v, Some(&id), stale), "item_changed_elsewhere");
        assert_eq!(
            stage_error(&v, Some(&id), login_draft("GitHub", vec![])),
            "item_changed_elsewhere"
        );
        assert_eq!(
            v.item_view(id.clone()).unwrap().summary.title,
            "GitHub (elsewhere)"
        );

        let fresh = v.item_edit(id.clone()).unwrap();
        assert_eq!(fresh.revision, Some(2));
        let draft = ItemDraft {
            base_revision: fresh.revision,
            ..login_draft("GitHub", vec![])
        };
        assert!(v.stage_draft(Some(&uuid), draft).is_ok());
        assert_eq!(v.item_template(ItemKind::Login).unwrap().revision, None);
    }

    #[test]
    fn a_blank_totp_replace_keeps_the_key() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let id = full_login(&v);
        update(&v, &id, login_draft("GitHub", vec![replace("totp", "   ")]));
        assert!(v.item_view(id.clone()).unwrap().summary.has_totp);
        update(&v, &id, login_draft("GitHub", vec![remove("totp")]));
        assert!(!v.item_view(id).unwrap().summary.has_totp);
    }

    #[test]
    fn a_card_named_after_its_brand_is_renamed_by_a_new_number() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let card = |title: &str, changes| ItemDraft {
            kind: ItemKind::Card,
            title: title.into(),
            websites: vec![],
            changes,
            base_revision: None,
        };
        let id = create(
            &v,
            card("", vec![replace("card.number", "4111111111111111")]),
        );
        assert_eq!(v.item_view(id.clone()).unwrap().summary.title, "Visa");
        let edit = v.item_edit(id.clone()).unwrap();
        assert_eq!(edit.title, "");
        update(
            &v,
            &id,
            card(
                &edit.title,
                vec![replace("card.number", "5555555555554444")],
            ),
        );
        assert_eq!(v.item_view(id.clone()).unwrap().summary.title, "Mastercard");

        // A title the user typed stays, whatever the number.
        update(&v, &id, card("Travel card", vec![]));
        assert_eq!(v.item_edit(id.clone()).unwrap().title, "Travel card");
        update(
            &v,
            &id,
            card(
                "Travel card",
                vec![replace("card.number", "4111111111111111")],
            ),
        );
        assert_eq!(v.item_view(id).unwrap().summary.title, "Travel card");
    }

    #[test]
    fn short_and_long_expiry_forms_are_read() {
        for (typed, shown) in [
            ("4/30", "04/2030"),
            ("04/2030", "04/2030"),
            ("2030-04", "04/2030"),
            (" 12/31 ", "12/2031"),
        ] {
            assert_eq!(
                parse_expiry(typed).unwrap().unwrap().display(),
                shown,
                "{typed}"
            );
        }
        assert!(parse_expiry("").unwrap().is_none());
        for bad in ["0/30", "13/30", "4/3", "4/203", "aa/bb", "4-30"] {
            assert!(parse_expiry(bad).is_err(), "{bad}");
        }
    }
}
