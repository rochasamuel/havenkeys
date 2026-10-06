//! "Sign in with …" providers. A closed list: each provider's display name
//! and the exact origins a sign-in run may continue into
//! (docs/superpowers/specs/2026-09-28-sign-in-with-design.md §3.1). The
//! extension holds a mirror for save detection only; parity is tested in
//! packages/protocol/src/sso-parity.test.ts.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use zeroize::Zeroize;

/// Longest account (an email address) accepted, in characters.
pub const MAX_ACCOUNT_CHARS: usize = 254;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SsoProvider {
    Google,
    Microsoft,
    Github,
    Apple,
    Facebook,
    Discord,
    X,
    Linkedin,
    Gitlab,
}

impl SsoProvider {
    /// Alphabetical by display name: the order the desktop lists them in.
    pub const ALL: [SsoProvider; 9] = [
        Self::Apple,
        Self::Discord,
        Self::Facebook,
        Self::Github,
        Self::Gitlab,
        Self::Google,
        Self::Linkedin,
        Self::Microsoft,
        Self::X,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Google => "Google",
            Self::Microsoft => "Microsoft",
            Self::Github => "GitHub",
            Self::Apple => "Apple",
            Self::Facebook => "Facebook",
            Self::Discord => "Discord",
            Self::X => "X",
            Self::Linkedin => "LinkedIn",
            Self::Gitlab => "GitLab",
        }
    }

    // Origins checked against each provider's developer docs (2026-10-05):
    // x.com: /i/oauth2/authorize (OAuth 2.0 authorize)
    // api.x.com: /oauth/authorize (OAuth 1.0a)
    // www.facebook.com: /v25.0/dialog/oauth
    // discord.com: /oauth2/authorize
    // www.linkedin.com: /oauth/v2/authorization
    // gitlab.com: /oauth/authorize
    // Removed: twitter.com and api.twitter.com (not in the docs checked;
    // twitter.com hosts redirect to x.com before any page loads) and
    // m.facebook.com (not in the docs checked).
    /// Exact origins (scheme + host, no port, no trailing slash).
    pub fn origins(self) -> &'static [&'static str] {
        match self {
            Self::Google => &["https://accounts.google.com"],
            Self::Microsoft => &[
                "https://login.microsoftonline.com",
                "https://login.live.com",
            ],
            Self::Github => &["https://github.com"],
            Self::Apple => &["https://appleid.apple.com"],
            Self::Facebook => &["https://www.facebook.com"],
            Self::Discord => &["https://discord.com"],
            Self::X => &["https://x.com", "https://api.x.com"],
            Self::Linkedin => &["https://www.linkedin.com"],
            Self::Gitlab => &["https://gitlab.com"],
        }
    }

    /// Exact string comparison: the caller passes a serialized origin.
    pub fn allows_origin(self, origin: &str) -> bool {
        self.origins().contains(&origin)
    }

    /// A provider named in an export (`"Google"`, `"github"`, `"Twitter"`),
    /// case-insensitive.
    pub fn from_name(name: &str) -> Option<Self> {
        let n = name.trim();
        if n.eq_ignore_ascii_case("twitter") {
            return Some(Self::X);
        }
        Self::ALL
            .into_iter()
            .find(|p| p.name().eq_ignore_ascii_case(n))
    }
}

/// How a login signs in when it has no password of its own (or also has one).
/// Stored in the encrypted overview, like the username.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SignInWith {
    pub provider: SsoProvider,
    #[serde(default)]
    pub account: Option<String>,
}

impl fmt::Debug for SignInWith {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SignInWith")
            .field("provider", &self.provider)
            .finish_non_exhaustive()
    }
}

impl Drop for SignInWith {
    fn drop(&mut self) {
        self.account.zeroize();
    }
}

pub(crate) fn clean_sign_in_with(value: Option<SignInWith>) -> Result<Option<SignInWith>> {
    let Some(v) = value else { return Ok(None) };
    let account = match v
        .account
        .as_deref()
        .map(str::trim)
        .filter(|a| !a.is_empty())
    {
        None => None,
        Some(a) if a.chars().count() > MAX_ACCOUNT_CHARS || a.chars().any(char::is_control) => {
            return Err(Error::InvalidInput(
                "account is too long or contains control characters",
            ))
        }
        Some(a) => Some(a.to_owned()),
    };
    Ok(Some(SignInWith {
        provider: v.provider,
        account,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_providers_wire_names_and_origins() {
        for (p, wire, name) in [
            (SsoProvider::Facebook, "facebook", "Facebook"),
            (SsoProvider::Discord, "discord", "Discord"),
            (SsoProvider::X, "x", "X"),
            (SsoProvider::Linkedin, "linkedin", "LinkedIn"),
            (SsoProvider::Gitlab, "gitlab", "GitLab"),
        ] {
            assert_eq!(serde_json::to_string(&p).unwrap(), format!("\"{wire}\""));
            assert_eq!(p.name(), name);
            assert_eq!(SsoProvider::from_name(name), Some(p));
        }
        assert!(SsoProvider::Facebook.allows_origin("https://www.facebook.com"));
        assert!(!SsoProvider::Facebook.allows_origin("https://facebook.com"));
        assert!(!SsoProvider::Facebook.allows_origin("http://www.facebook.com"));
        assert!(!SsoProvider::Facebook.allows_origin("https://www.facebook.com.evil.com"));
        assert!(SsoProvider::Discord.allows_origin("https://discord.com"));
        assert!(!SsoProvider::Discord.allows_origin("https://evildiscord.com"));
        assert!(SsoProvider::X.allows_origin("https://x.com"));
        assert!(SsoProvider::X.allows_origin("https://api.x.com"));
        assert!(!SsoProvider::X.allows_origin("https://twitter.com"));
        assert!(!SsoProvider::Facebook.allows_origin("https://m.facebook.com"));
        assert!(!SsoProvider::X.allows_origin("https://x.com.evil.com"));
        assert!(SsoProvider::Linkedin.allows_origin("https://www.linkedin.com"));
        assert!(!SsoProvider::Linkedin.allows_origin("https://linkedin.com.evil.com"));
        assert!(SsoProvider::Gitlab.allows_origin("https://gitlab.com"));
        assert!(!SsoProvider::Gitlab.allows_origin("https://gitlab.example.com"));
        assert_eq!(SsoProvider::from_name(" twitter "), Some(SsoProvider::X));
    }

    #[test]
    fn all_is_alphabetical_by_name() {
        let names: Vec<&str> = SsoProvider::ALL.iter().map(|p| p.name()).collect();
        assert_eq!(
            names,
            [
                "Apple",
                "Discord",
                "Facebook",
                "GitHub",
                "GitLab",
                "Google",
                "LinkedIn",
                "Microsoft",
                "X"
            ]
        );
    }

    #[test]
    fn provider_wire_names_and_origins() {
        assert_eq!(
            serde_json::to_string(&SsoProvider::Github).unwrap(),
            "\"github\""
        );
        assert_eq!(SsoProvider::Github.name(), "GitHub");
        assert!(SsoProvider::Microsoft.allows_origin("https://login.live.com"));
        assert!(!SsoProvider::Google.allows_origin("https://accounts.google.com.evil.com"));
        assert!(!SsoProvider::Google.allows_origin("http://accounts.google.com"));
        assert_eq!(
            SsoProvider::from_name(" GitHub "),
            Some(SsoProvider::Github)
        );
        assert_eq!(SsoProvider::from_name("Okta"), None);
    }

    #[test]
    fn account_rules() {
        let with = |a: &str| SignInWith {
            provider: SsoProvider::Google,
            account: Some(a.into()),
        };
        assert_eq!(
            clean_sign_in_with(Some(with("  me@x.com ")))
                .unwrap()
                .unwrap()
                .account
                .as_deref(),
            Some("me@x.com")
        );
        assert_eq!(
            clean_sign_in_with(Some(with("   ")))
                .unwrap()
                .unwrap()
                .account,
            None
        );
        assert!(clean_sign_in_with(Some(with("a\u{0007}b"))).is_err());
        assert!(clean_sign_in_with(Some(with(&"x".repeat(MAX_ACCOUNT_CHARS + 1)))).is_err());
        assert!(clean_sign_in_with(None).unwrap().is_none());
    }

    #[test]
    fn unknown_fields_and_providers_are_rejected() {
        assert!(serde_json::from_str::<SignInWith>(r#"{"provider":"okta"}"#).is_err());
        assert!(serde_json::from_str::<SignInWith>(r#"{"provider":"google","x":1}"#).is_err());
        let s: SignInWith = serde_json::from_str(r#"{"provider":"apple"}"#).unwrap();
        assert_eq!(s.account, None);
    }

    #[test]
    fn debug_hides_the_account() {
        let s = SignInWith {
            provider: SsoProvider::Google,
            account: Some("me@x.com".into()),
        };
        assert!(!format!("{s:?}").contains("me@x.com"));
    }
}
