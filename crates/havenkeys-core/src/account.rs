//! Account identity: the email and account ID that key derivation is bound to.
//!
//! The email is part of the key derivation (docs/crypto.md, key scheme 3), so
//! two devices must normalize it identically or they derive different keys.
//! Normalization is therefore a correctness requirement, not cosmetics.

use crate::error::{Error, Result};
use std::fmt;
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

/// Longest email accepted, from the SMTP path limit (RFC 5321 §4.5.3.1.3).
const MAX_EMAIL_LEN: usize = 254;

/// An email address normalized for key derivation: trimmed, NFC, lowercased.
#[derive(Clone, PartialEq, Eq)]
pub struct NormalizedEmail(String);

impl NormalizedEmail {
    pub fn parse(raw: &str) -> Result<Self> {
        let normalized: String = raw.trim().nfc().collect::<String>().to_lowercase();
        if normalized.is_empty() || normalized.len() > MAX_EMAIL_LEN {
            return Err(Error::InvalidInput("email address is not valid"));
        }
        if normalized.chars().any(char::is_whitespace) {
            return Err(Error::InvalidInput("email address is not valid"));
        }
        let mut parts = normalized.split('@');
        let local = parts.next().unwrap_or("");
        let domain = parts.next().unwrap_or("");
        if local.is_empty() || domain.is_empty() || parts.next().is_some() {
            return Err(Error::InvalidInput("email address is not valid"));
        }
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for NormalizedEmail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NormalizedEmail({:?})", self.0)
    }
}

/// The account a vault belongs to. Both fields are bound into the KEK, so a
/// vault cannot be opened under a different account or a different address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountRef {
    pub id: Uuid,
    pub email: NormalizedEmail,
}

impl AccountRef {
    pub fn new(id: Uuid, email: NormalizedEmail) -> Self {
        Self { id, email }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_case_and_surrounding_space() {
        let a = NormalizedEmail::parse("  User@Example.COM ").unwrap();
        let b = NormalizedEmail::parse("user@example.com").unwrap();
        assert_eq!(a.as_str(), "user@example.com");
        assert_eq!(a.as_str(), b.as_str());
    }

    #[test]
    fn normalizes_unicode_to_nfc() {
        // "josé@example.com" with a combining acute accent must equal the
        // precomposed form, or the two spellings derive different keys.
        let decomposed = NormalizedEmail::parse("jose\u{0301}@example.com").unwrap();
        let composed = NormalizedEmail::parse("jos\u{00e9}@example.com").unwrap();
        assert_eq!(decomposed.as_str(), composed.as_str());
    }

    #[test]
    fn rejects_malformed_addresses() {
        for bad in [
            "",
            "   ",
            "no-at-sign",
            "@example.com",
            "user@",
            "user@@example.com",
            "user name@example.com",
            "user@exa mple.com",
            "user\n@example.com",
        ] {
            assert!(
                NormalizedEmail::parse(bad).is_err(),
                "should have rejected {bad:?}"
            );
        }
    }

    #[test]
    fn rejects_overlong_addresses() {
        let long = format!("{}@example.com", "a".repeat(MAX_EMAIL_LEN));
        assert!(NormalizedEmail::parse(&long).is_err());
    }

    #[test]
    fn debug_shows_the_address() {
        // The email is not a secret; it is an identifier, and a redacted
        // Debug here would make support and tests harder for no gain.
        let e = NormalizedEmail::parse("user@example.com").unwrap();
        assert_eq!(format!("{e:?}"), "NormalizedEmail(\"user@example.com\")");
    }
}
