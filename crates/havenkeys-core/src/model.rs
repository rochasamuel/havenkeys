//! Vault item model, inputs and validation.
//!
//! Each item is persisted as two encrypted blobs:
//! * [`ItemOverview`] — decrypted at unlock and kept in memory (list/search).
//! * [`ItemDetails`]  — secrets; decrypted per request only.

use crate::card::{CardFields, CardInput, CardSummary};
use crate::custom_field::{FieldSection, SectionInput};
use crate::error::{Error, Result};
use crate::generator::GeneratorOptions;
use crate::identity::IdentityFields;
use crate::passkey::Passkey;
use crate::secret::SecretString;
use crate::sso::SignInWith;
use crate::totp::{self, TotpConfig};
use serde::{Deserialize, Serialize};
use std::fmt;
use url::Url;
use uuid::Uuid;
use zeroize::Zeroize;

pub const MAX_TITLE_CHARS: usize = 256;
pub const MAX_USERNAME_CHARS: usize = 512;
pub const MAX_URLS: usize = 32;
pub const MAX_URL_LEN: usize = 2048;
pub const MAX_PASSWORD_CHARS: usize = 4096;
pub const MAX_NOTES_BYTES: usize = 64 * 1024;
pub const MAX_NOTE_CONTENT_BYTES: usize = 1024 * 1024;
/// Replaced passwords kept per login (newest first).
pub const MAX_PASSWORD_HISTORY: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemType {
    Login,
    SecureNote,
    /// The account's one Identity (spec 2026-09-29-identity-item).
    Identity,
    /// A payment card (spec 2026-09-29-card-item).
    Card,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchType {
    Exact,
    Origin,
    Domain,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UrlRule {
    pub url: String,
    pub match_type: MatchType,
}

/// Non-secret-bearing summary of an item. Still sensitive (reveals which
/// services the user has accounts on), so it is encrypted at rest and its
/// strings are wiped on drop.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemOverview {
    pub id: Uuid,
    pub item_type: ItemType,
    pub title: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub urls: Vec<UrlRule>,
    pub has_password: bool,
    pub has_totp: bool,
    pub has_notes: bool,
    /// The login holds at least one passkey. `default`: overviews written
    /// before passkeys existed.
    #[serde(default)]
    pub has_passkey: bool,
    /// Whether automatic sign-in may press this login's sign-in button and
    /// continue to later steps. `default`: logins saved before this field
    /// existed are on.
    #[serde(default = "default_true")]
    pub auto_sign_in: bool,
    /// "Sign in with <provider>" and the account used there. `default`:
    /// overviews written before this field existed have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sign_in_with: Option<SignInWith>,
    /// A card's brand, last four digits and expiry; `None` for other items.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<CardSummary>,
    /// The user's tags, normalised by [`crate::tags::normalize`]. `default`:
    /// overviews written before tags existed have none.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Fields written by a newer version that this one does not know. Kept
    /// so an edit made here does not erase them (spec 2026-10-07-item-tags §3.4).
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
    /// Unix milliseconds.
    pub created_at: i64,
    pub updated_at: i64,
}

impl fmt::Debug for ItemOverview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ItemOverview")
            .field("id", &self.id)
            .field("item_type", &self.item_type)
            .finish_non_exhaustive()
    }
}

impl Drop for ItemOverview {
    fn drop(&mut self) {
        self.title.zeroize();
        self.username.zeroize();
        for rule in &mut self.urls {
            rule.url.zeroize();
        }
        for tag in &mut self.tags {
            tag.zeroize();
        }
    }
}

/// An Android app this login fills in, confirmed by the user (spec
/// 2026-10-01-android-app §7.2). `cert_sha256` is the SHA-256 of one of the
/// app's signing certificates, lowercase hex.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppBinding {
    pub package: String,
    pub cert_sha256: String,
}

pub const MAX_APP_BINDINGS: usize = 32;

/// Secret part of an item.
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)] // one short-lived value per decrypted item; boxing Login would touch every pattern
pub enum ItemDetails {
    Login {
        #[serde(default)]
        password: Option<SecretString>,
        #[serde(default)]
        totp: Option<TotpConfig>,
        #[serde(default)]
        notes: Option<SecretString>,
        /// Passwords this login used before, newest first. Kept so that a
        /// password changed from the browser (or by mistake) can be recovered.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        password_history: Vec<PreviousPassword>,
        /// Passkeys this login holds. Private keys never leave the core.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        passkeys: Vec<Passkey>,
        /// Custom fields, in sections (spec 2026-09-30-login-custom-fields).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        sections: Vec<FieldSection>,
        /// Android apps this login fills in. Kept in the encrypted details,
        /// like passkeys, and carried over by every edit.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        app_bindings: Vec<AppBinding>,
        /// Vault health checks the user dismissed for this login (spec
        /// 2026-10-07-vault-health §4.8). Carried over by every edit.
        #[serde(
            default,
            skip_serializing_if = "Vec::is_empty",
            deserialize_with = "crate::health::known_checks"
        )]
        health_ignored: Vec<crate::health::HealthCheck>,
    },
    SecureNote {
        content: SecretString,
    },
    Identity(Box<IdentityFields>),
    Card(Box<CardFields>),
}

/// A password that was replaced, and when (Unix milliseconds).
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviousPassword {
    pub password: SecretString,
    pub replaced_at: i64,
}

impl fmt::Debug for PreviousPassword {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreviousPassword")
            .field("replaced_at", &self.replaced_at)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for ItemDetails {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ItemDetails(<redacted>)")
    }
}

impl ItemDetails {
    pub fn item_type(&self) -> ItemType {
        match self {
            ItemDetails::Login { .. } => ItemType::Login,
            ItemDetails::SecureNote { .. } => ItemType::SecureNote,
            ItemDetails::Identity(_) => ItemType::Identity,
            ItemDetails::Card(_) => ItemType::Card,
        }
    }
}

/// How an edit treats a secret field. Lets the UI edit an item without ever
/// fetching the existing password or TOTP secret.
#[derive(Default, Deserialize)]
#[serde(tag = "op", content = "value", rename_all = "snake_case")]
pub enum SecretUpdate {
    #[default]
    Keep,
    Set(SecretString),
    Clear,
}

impl fmt::Debug for SecretUpdate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SecretUpdate::Keep => "Keep",
            SecretUpdate::Set(_) => "Set(<redacted>)",
            SecretUpdate::Clear => "Clear",
        })
    }
}

impl SecretUpdate {
    pub(crate) fn is_keep(&self) -> bool {
        matches!(self, SecretUpdate::Keep)
    }

    /// Apply to an existing value; an empty `Set` is treated as `Clear`.
    pub(crate) fn apply(self, current: Option<SecretString>) -> Option<SecretString> {
        match self {
            SecretUpdate::Keep => current,
            SecretUpdate::Clear => None,
            SecretUpdate::Set(v) if v.is_empty() => None,
            SecretUpdate::Set(v) => Some(v),
        }
    }

    /// Apply to a one-time-password setup: `Set` takes an `otpauth://` URI
    /// or a bare Base32 secret; a blank `Set` clears. The one rule for the
    /// login's TOTP and for OTP custom fields.
    pub(crate) fn apply_totp(self, current: Option<TotpConfig>) -> Result<Option<TotpConfig>> {
        match self {
            SecretUpdate::Keep => Ok(current),
            SecretUpdate::Clear => Ok(None),
            SecretUpdate::Set(v) if v.expose().trim().is_empty() => Ok(None),
            SecretUpdate::Set(v) => totp::parse_totp_input(v.expose()).map(Some),
        }
    }
}

/// Create/update request from the UI.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ItemInput {
    pub item_type: ItemType,
    pub title: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub urls: Vec<UrlRule>,
    #[serde(default)]
    pub password: SecretUpdate,
    /// `Set` accepts an `otpauth://totp/...` URI or a bare Base32 secret.
    #[serde(default)]
    pub totp: SecretUpdate,
    #[serde(default)]
    pub notes: SecretUpdate,
    /// Secure-note body.
    #[serde(default)]
    pub content: SecretUpdate,
    /// Automatic sign-in switch for a login. `None` keeps the current value
    /// on update and means on for a new item.
    #[serde(default)]
    pub auto_sign_in: Option<bool>,
    /// "Sign in with …" for a login. Sent in full like `username`: `None`
    /// on an update clears it.
    #[serde(default)]
    pub sign_in_with: Option<SignInWith>,
    /// An identity's values, sent in full on every save. Required for an
    /// identity, refused for the other types.
    #[serde(default)]
    pub identity: Option<IdentityFields>,
    /// A card's values. Required for a card, refused for the other types.
    #[serde(default)]
    pub card: Option<CardInput>,
    /// A login's custom fields. `None` keeps them as they are (every save
    /// path except the desktop editor); `Some` replaces the whole layout, in
    /// order (`custom_field::apply_sections`).
    #[serde(default)]
    pub sections: Option<Vec<SectionInput>>,
    /// The item's tags. `None` keeps them as they are (every save path that
    /// does not show tags); `Some` replaces the whole set.
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

impl ItemInput {
    /// `blank` for other crates' tests.
    #[cfg(any(test, feature = "test-util"))]
    #[doc(hidden)]
    pub fn blank_for_tests(item_type: ItemType, title: &str) -> ItemInput {
        Self::blank(item_type, title.to_owned())
    }

    /// An item of `item_type` with nothing set: no username, websites or
    /// secrets, every secret left as it is.
    pub(crate) fn blank(item_type: ItemType, title: String) -> ItemInput {
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
            tags: None,
        }
    }
}

impl fmt::Debug for ItemInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ItemInput")
            .field("item_type", &self.item_type)
            .finish_non_exhaustive()
    }
}

/// Field the UI may explicitly reveal. TOTP secrets are deliberately absent:
/// only generated codes ever leave the core.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretField {
    Password,
    Notes,
    Content,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    Dark,
    Light,
    System,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    /// 0 = never.
    pub auto_lock_minutes: u32,
    pub clipboard_clear_seconds: u32,
    /// UI preference. `default` keeps settings saved before this field existed readable.
    #[serde(default)]
    pub theme: Theme,
    /// Whether the browser extension may query this vault. Opt-in: off by
    /// default, including for settings saved before this field existed.
    #[serde(default)]
    pub browser_integration: bool,
    /// Whether a site's automatic passkey upgrade (a conditional
    /// `create()` right after HavenKeys filled a password there) may save a
    /// passkey without asking. On by default, including for settings saved
    /// before this field existed; when off, the save card asks instead.
    #[serde(default = "default_true")]
    pub auto_passkey_upgrade: bool,
    /// Whether HavenKeys may press a site's sign-in button after filling and
    /// continue through later steps (password page, one-time code). On by
    /// default, including for settings saved before this field existed. Each
    /// login has its own switch too (`ItemOverview::auto_sign_in`).
    #[serde(default = "default_true")]
    pub auto_sign_in: bool,
    /// The password generator's policy, set in the desktop's generator tab.
    /// The browser extension's "Generate strong password" uses it too.
    /// Settings saved before this field existed get the default policy.
    #[serde(default)]
    pub generator: GeneratorOptions,
}

fn default_true() -> bool {
    true
}

pub const AUTO_LOCK_CHOICES: [u32; 5] = [0, 5, 15, 30, 60];

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_lock_minutes: 15,
            clipboard_clear_seconds: 30,
            theme: Theme::Dark,
            browser_integration: false,
            auto_passkey_upgrade: true,
            auto_sign_in: true,
            generator: GeneratorOptions::default(),
        }
    }
}

impl Settings {
    /// What a session uses when a saved settings blob exists but does not
    /// open or validate: every automatic behavior and the extension off,
    /// so a damaged or tampered row can never switch them back on.
    pub fn restrictive() -> Self {
        Self {
            browser_integration: false,
            auto_passkey_upgrade: false,
            auto_sign_in: false,
            ..Self::default()
        }
    }

    pub fn validate(&self) -> Result<()> {
        if !AUTO_LOCK_CHOICES.contains(&self.auto_lock_minutes) {
            return Err(Error::InvalidInput("unsupported auto-lock interval"));
        }
        if !(10..=300).contains(&self.clipboard_clear_seconds) {
            return Err(Error::InvalidInput(
                "clipboard timeout must be 10-300 seconds",
            ));
        }
        self.generator.validate()
    }
}

// ---------------------------------------------------------------- validation

pub(crate) fn clean_title(title: &str) -> Result<String> {
    let t = title.trim();
    if t.is_empty() {
        return Err(Error::InvalidInput("title is required"));
    }
    if t.chars().count() > MAX_TITLE_CHARS || t.chars().any(char::is_control) {
        return Err(Error::InvalidInput(
            "title is too long or contains control characters",
        ));
    }
    Ok(t.to_owned())
}

pub(crate) fn clean_username(username: Option<&str>) -> Result<Option<String>> {
    let Some(u) = username.map(str::trim).filter(|u| !u.is_empty()) else {
        return Ok(None);
    };
    if u.chars().count() > MAX_USERNAME_CHARS || u.chars().any(char::is_control) {
        return Err(Error::InvalidInput(
            "username is too long or contains control characters",
        ));
    }
    Ok(Some(u.to_owned()))
}

/// Normalize a user-entered website. Scheme defaults to https; only http(s)
/// is accepted; embedded credentials are rejected; the fragment is dropped.
pub fn normalize_url(input: &str) -> Result<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_URL_LEN {
        return Err(Error::InvalidInput("invalid website address"));
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_owned()
    } else {
        format!("https://{trimmed}")
    };
    let mut url =
        Url::parse(&with_scheme).map_err(|_| Error::InvalidInput("invalid website address"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(Error::InvalidInput(
            "only http and https websites are supported",
        ));
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err(Error::InvalidInput("invalid website address"));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(Error::InvalidInput(
            "website addresses must not contain credentials",
        ));
    }
    url.set_fragment(None);
    let s = url.to_string();
    if s.len() > MAX_URL_LEN {
        return Err(Error::InvalidInput("invalid website address"));
    }
    Ok(s)
}

pub(crate) fn clean_urls(urls: &[UrlRule]) -> Result<Vec<UrlRule>> {
    if urls.len() > MAX_URLS {
        return Err(Error::InvalidInput("too many websites"));
    }
    urls.iter()
        .filter(|r| !r.url.trim().is_empty())
        .map(|r| {
            Ok(UrlRule {
                url: normalize_url(&r.url)?,
                match_type: r.match_type,
            })
        })
        .collect()
}

pub(crate) fn check_password(p: &SecretString) -> Result<()> {
    if p.char_len() > MAX_PASSWORD_CHARS {
        return Err(Error::InvalidInput("password is too long"));
    }
    Ok(())
}

pub(crate) fn check_notes(n: &SecretString) -> Result<()> {
    if n.expose().len() > MAX_NOTES_BYTES {
        return Err(Error::InvalidInput("notes are too long"));
    }
    Ok(())
}

pub(crate) fn check_note_content(n: &SecretString) -> Result<()> {
    if n.expose().len() > MAX_NOTE_CONTENT_BYTES {
        return Err(Error::InvalidInput("note is too long"));
    }
    Ok(())
}

/// Reject login-only fields on a secure note and vice versa, anything but
/// identity values on an identity, and anything but card values on a card.
pub(crate) fn check_shape(input: &ItemInput) -> Result<()> {
    let login_fields = input
        .username
        .as_deref()
        .is_some_and(|u| !u.trim().is_empty())
        || !input.urls.is_empty()
        || !input.password.is_keep()
        || !input.totp.is_keep()
        || !input.notes.is_keep()
        || input.sign_in_with.is_some();
    // What a login or a secure note may hold, and an identity or card not.
    let identity_only = login_fields || !input.content.is_keep();
    match input.item_type {
        t if t != ItemType::Login && input.sections.is_some() => {
            Err(Error::InvalidInput("only a login has custom fields"))
        }
        ItemType::Card if input.card.is_none() => {
            Err(Error::InvalidInput("a card needs its values"))
        }
        ItemType::Card if identity_only || input.identity.is_some() => {
            Err(Error::InvalidInput("a card only has card values"))
        }
        ItemType::Login | ItemType::SecureNote | ItemType::Identity if input.card.is_some() => {
            Err(Error::InvalidInput("only a card has card values"))
        }
        ItemType::Identity if input.identity.is_none() => {
            Err(Error::InvalidInput("an identity needs its values"))
        }
        ItemType::Identity if identity_only => {
            Err(Error::InvalidInput("an identity only has identity values"))
        }
        ItemType::Login | ItemType::SecureNote if input.identity.is_some() => {
            Err(Error::InvalidInput("only an identity has identity values"))
        }
        ItemType::Login if !input.content.is_keep() => {
            Err(Error::InvalidInput("logins do not have note content"))
        }
        ItemType::SecureNote if login_fields => Err(Error::InvalidInput(
            "secure notes only have a title and content",
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_normalization() {
        assert_eq!(normalize_url("github.com").unwrap(), "https://github.com/");
        assert_eq!(
            normalize_url(" https://GitHub.com/login#x ").unwrap(),
            "https://github.com/login"
        );
        assert_eq!(
            normalize_url("http://localhost:8080/a").unwrap(),
            "http://localhost:8080/a"
        );
        assert_eq!(
            normalize_url("https://bücher.de").unwrap(),
            "https://xn--bcher-kva.de/"
        );
        for bad in [
            "",
            "javascript:alert(1)",
            "file:///etc/passwd",
            "ftp://example.com",
            "https://user:pw@example.com",
            "https://",
            "data:text/html,hi",
        ] {
            assert!(normalize_url(bad).is_err(), "accepted {bad:?}");
        }
        assert!(normalize_url(&format!("https://a.com/{}", "a".repeat(3000))).is_err());
    }

    #[test]
    fn title_and_username_rules() {
        assert!(clean_title("  ").is_err());
        assert!(clean_title("a\u{0007}b").is_err());
        assert!(clean_title(&"x".repeat(MAX_TITLE_CHARS + 1)).is_err());
        assert_eq!(clean_title(" GitHub ").unwrap(), "GitHub");
        assert_eq!(clean_username(Some("  ")).unwrap(), None);
        assert_eq!(
            clean_username(Some(" me@x.com ")).unwrap().as_deref(),
            Some("me@x.com")
        );
    }

    #[test]
    fn settings_without_theme_still_parse() {
        // Settings blobs written before the theme field existed.
        let old: Settings =
            serde_json::from_str(r#"{"autoLockMinutes":5,"clipboardClearSeconds":30}"#).unwrap();
        assert_eq!(old.theme, Theme::Dark);
        assert!(!old.browser_integration);
    }

    #[test]
    fn auto_passkey_upgrade_defaults_on() {
        assert!(Settings::default().auto_passkey_upgrade);
        // Settings saved before the field existed.
        let old: Settings = serde_json::from_str(
            r#"{"autoLockMinutes":5,"clipboardClearSeconds":30,"theme":"dark","browserIntegration":true}"#,
        )
        .unwrap();
        assert!(old.auto_passkey_upgrade);
        let off: Settings = serde_json::from_str(
            r#"{"autoLockMinutes":5,"clipboardClearSeconds":30,"autoPasskeyUpgrade":false}"#,
        )
        .unwrap();
        assert!(!off.auto_passkey_upgrade);
    }

    #[test]
    fn auto_sign_in_defaults_on() {
        assert!(Settings::default().auto_sign_in);
        let old: Settings =
            serde_json::from_str(r#"{"autoLockMinutes":5,"clipboardClearSeconds":30}"#).unwrap();
        assert!(old.auto_sign_in);
        let off: Settings = serde_json::from_str(
            r#"{"autoLockMinutes":5,"clipboardClearSeconds":30,"autoSignIn":false}"#,
        )
        .unwrap();
        assert!(!off.auto_sign_in);
    }

    #[test]
    fn overviews_saved_before_auto_sign_in_are_on() {
        let json = r#"{"id":"7c9e6679-7425-40de-944b-e07fc1f90ae7","itemType":"login","title":"t",
            "hasPassword":true,"hasTotp":false,"hasNotes":false,"createdAt":1,"updatedAt":1}"#;
        let o: ItemOverview = serde_json::from_str(json).unwrap();
        assert!(o.auto_sign_in);
    }

    #[test]
    fn settings_validation() {
        assert!(Settings::default().validate().is_ok());
        assert!(Settings {
            auto_lock_minutes: 7,
            clipboard_clear_seconds: 30,
            ..Default::default()
        }
        .validate()
        .is_err());
        assert!(Settings {
            auto_lock_minutes: 0,
            clipboard_clear_seconds: 5,
            ..Default::default()
        }
        .validate()
        .is_err());
    }

    #[test]
    fn secret_update_deserialization() {
        let keep: SecretUpdate = serde_json::from_str(r#"{"op":"keep"}"#).unwrap();
        assert!(keep.is_keep());
        let set: SecretUpdate = serde_json::from_str(r#"{"op":"set","value":"pw"}"#).unwrap();
        assert_eq!(set.apply(None).unwrap().expose(), "pw");
        let clear: SecretUpdate = serde_json::from_str(r#"{"op":"clear"}"#).unwrap();
        assert!(clear.apply(Some("old".into())).is_none());
        assert!(format!("{:?}", SecretUpdate::Set("pw".into())).contains("redacted"));
    }

    #[test]
    fn item_input_rejects_unknown_fields() {
        let json = r#"{"itemType":"login","title":"x","isAdmin":true}"#;
        assert!(serde_json::from_str::<ItemInput>(json).is_err());
    }

    #[test]
    fn logins_saved_before_passkeys_still_parse() {
        let d: ItemDetails = serde_json::from_str(r#"{"type":"login","password":"pw"}"#).unwrap();
        match d {
            ItemDetails::Login { passkeys, .. } => assert!(passkeys.is_empty()),
            _ => panic!("wrong type"),
        }
        // An empty passkey list is not written, so existing blobs are unchanged.
        let json = serde_json::to_string(&ItemDetails::Login {
            password: None,
            totp: None,
            notes: None,
            password_history: Vec::new(),
            passkeys: Vec::new(),
            sections: Vec::new(),
            app_bindings: Vec::new(),
            health_ignored: Vec::new(),
        })
        .unwrap();
        assert!(!json.contains("passkeys"));
        assert!(!json.contains("app_bindings") && !json.contains("health_ignored"));
        let ov: ItemOverview = serde_json::from_str(
            r#"{"id":"7c9e6679-7425-40de-944b-e07fc1f90ae7","itemType":"login","title":"t",
                "hasPassword":true,"hasTotp":false,"hasNotes":false,"createdAt":1,"updatedAt":1}"#,
        )
        .unwrap();
        assert!(!ov.has_passkey);
    }

    /// A newer app may add a check kind; an older one keeps the kinds it
    /// knows and still opens the login.
    #[test]
    fn unknown_dismissed_checks_are_dropped_not_fatal() {
        use crate::health::HealthCheck;
        let d: ItemDetails = serde_json::from_str(
            r#"{"type":"login","password":"pw","health_ignored":["weak","leaked",7,"two_factor"]}"#,
        )
        .unwrap();
        let ItemDetails::Login {
            password,
            health_ignored,
            ..
        } = d
        else {
            panic!("wrong type")
        };
        assert_eq!(password.unwrap().expose(), "pw");
        assert_eq!(
            health_ignored,
            vec![HealthCheck::Weak, HealthCheck::TwoFactor]
        );
        let d: ItemDetails =
            serde_json::from_str(r#"{"type":"login","health_ignored":["weak","leaked"]}"#).unwrap();
        let ItemDetails::Login { health_ignored, .. } = &d else {
            panic!("wrong type")
        };
        assert_eq!(health_ignored, &vec![HealthCheck::Weak]);
        // Serialization is unchanged.
        assert!(serde_json::to_string(&d)
            .unwrap()
            .contains(r#""health_ignored":["weak"]"#));
        // Not a list at all is still an error (a corrupted item).
        assert!(
            serde_json::from_str::<ItemDetails>(r#"{"type":"login","health_ignored":"weak"}"#)
                .is_err()
        );
    }

    #[test]
    fn overviews_without_sign_in_with_parse_and_omit_it() {
        let json = r#"{"id":"7c9e6679-7425-40de-944b-e07fc1f90ae7","itemType":"login","title":"t",
            "hasPassword":false,"hasTotp":false,"hasNotes":false,"createdAt":1,"updatedAt":1}"#;
        let o: ItemOverview = serde_json::from_str(json).unwrap();
        assert!(o.sign_in_with.is_none());
        assert!(!serde_json::to_string(&o).unwrap().contains("signInWith"));
    }

    /// Which error each kind of mismatched input gets, and that matching
    /// inputs pass. Arm order matters: the first rule that applies wins.
    #[test]
    fn check_shape_answers() {
        let shape = |json: serde_json::Value| {
            let input: ItemInput = serde_json::from_value(json).unwrap();
            match check_shape(&input) {
                Ok(()) => "ok",
                Err(Error::InvalidInput(m)) => m,
                Err(_) => "other",
            }
        };
        let card = serde_json::json!({});
        let identity = serde_json::json!({});
        let set = serde_json::json!({"op": "set", "value": "x"});
        let cases = [
            (serde_json::json!({"itemType": "login", "title": "t"}), "ok"),
            (
                serde_json::json!({"itemType": "login", "title": "t", "username": "u", "password": set, "totp": set, "notes": set}),
                "ok",
            ),
            (
                serde_json::json!({"itemType": "login", "title": "t", "content": set}),
                "logins do not have note content",
            ),
            (
                serde_json::json!({"itemType": "login", "title": "t", "card": card}),
                "only a card has card values",
            ),
            (
                serde_json::json!({"itemType": "login", "title": "t", "identity": identity}),
                "only an identity has identity values",
            ),
            (
                serde_json::json!({"itemType": "secure_note", "title": "t", "content": set}),
                "ok",
            ),
            (
                serde_json::json!({"itemType": "secure_note", "title": "t", "username": "  "}),
                "ok",
            ),
            (
                serde_json::json!({"itemType": "secure_note", "title": "t", "username": "u"}),
                "secure notes only have a title and content",
            ),
            (
                serde_json::json!({"itemType": "secure_note", "title": "t", "urls": [{"url": "https://a.com", "matchType": "domain"}]}),
                "secure notes only have a title and content",
            ),
            (
                serde_json::json!({"itemType": "secure_note", "title": "t", "password": set}),
                "secure notes only have a title and content",
            ),
            (
                serde_json::json!({"itemType": "secure_note", "title": "t", "totp": set}),
                "secure notes only have a title and content",
            ),
            (
                serde_json::json!({"itemType": "secure_note", "title": "t", "notes": set}),
                "secure notes only have a title and content",
            ),
            (
                serde_json::json!({"itemType": "secure_note", "title": "t", "card": card}),
                "only a card has card values",
            ),
            (
                serde_json::json!({"itemType": "secure_note", "title": "t", "identity": identity}),
                "only an identity has identity values",
            ),
            (
                serde_json::json!({"itemType": "identity", "title": "t", "identity": identity}),
                "ok",
            ),
            (
                serde_json::json!({"itemType": "identity", "title": "t"}),
                "an identity needs its values",
            ),
            (
                serde_json::json!({"itemType": "identity", "title": "t", "identity": identity, "content": set}),
                "an identity only has identity values",
            ),
            (
                serde_json::json!({"itemType": "identity", "title": "t", "identity": identity, "username": "u"}),
                "an identity only has identity values",
            ),
            (
                serde_json::json!({"itemType": "identity", "title": "t", "identity": identity, "signInWith": {"provider": "google"}}),
                "an identity only has identity values",
            ),
            (
                serde_json::json!({"itemType": "identity", "title": "t", "identity": identity, "card": card}),
                "only a card has card values",
            ),
            (
                serde_json::json!({"itemType": "card", "title": "t", "card": card}),
                "ok",
            ),
            (
                serde_json::json!({"itemType": "card", "title": "t"}),
                "a card needs its values",
            ),
            (
                serde_json::json!({"itemType": "card", "title": "t", "card": card, "notes": set}),
                "a card only has card values",
            ),
            (
                serde_json::json!({"itemType": "card", "title": "t", "card": card, "content": set}),
                "a card only has card values",
            ),
            (
                serde_json::json!({"itemType": "card", "title": "t", "card": card, "identity": identity}),
                "a card only has card values",
            ),
        ];
        for (json, expected) in cases {
            assert_eq!(shape(json.clone()), expected, "{json}");
        }
    }

    #[test]
    fn secure_notes_cannot_sign_in_with() {
        let input: ItemInput = serde_json::from_str(
            r#"{"itemType":"secure_note","title":"n","signInWith":{"provider":"google"}}"#,
        )
        .unwrap();
        assert!(check_shape(&input).is_err());
    }
}
