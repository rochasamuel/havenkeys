//! Vault item model, inputs and validation.
//!
//! Each item is persisted as two encrypted blobs:
//! * [`ItemOverview`] — decrypted at unlock and kept in memory (list/search).
//! * [`ItemDetails`]  — secrets; decrypted per request only.

use crate::error::{Error, Result};
use crate::secret::SecretString;
use crate::totp::TotpConfig;
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
    }
}

/// Secret part of an item.
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
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
    },
    SecureNote {
        content: SecretString,
    },
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
    fn is_keep(&self) -> bool {
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
}

pub const AUTO_LOCK_CHOICES: [u32; 5] = [0, 5, 15, 30, 60];

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_lock_minutes: 15,
            clipboard_clear_seconds: 30,
            theme: Theme::Dark,
            browser_integration: false,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<()> {
        if !AUTO_LOCK_CHOICES.contains(&self.auto_lock_minutes) {
            return Err(Error::InvalidInput("unsupported auto-lock interval"));
        }
        if !(10..=300).contains(&self.clipboard_clear_seconds) {
            return Err(Error::InvalidInput(
                "clipboard timeout must be 10-300 seconds",
            ));
        }
        Ok(())
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

/// Reject login-only fields on a secure note and vice versa.
pub(crate) fn check_shape(input: &ItemInput) -> Result<()> {
    match input.item_type {
        ItemType::Login if !input.content.is_keep() => {
            Err(Error::InvalidInput("logins do not have note content"))
        }
        ItemType::SecureNote
            if input
                .username
                .as_deref()
                .is_some_and(|u| !u.trim().is_empty())
                || !input.urls.is_empty()
                || !input.password.is_keep()
                || !input.totp.is_keep()
                || !input.notes.is_keep() =>
        {
            Err(Error::InvalidInput(
                "secure notes only have a title and content",
            ))
        }
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
}
