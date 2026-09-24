//! Passkeys (WebAuthn credentials) that HavenKeys holds as an authenticator.
//!
//! * `rp` — may this page use this relying-party ID?
//! * `webauthn` — clientDataJSON, authenticator data, key generation, signing.
//! * `cbor` — the few deterministic CBOR shapes WebAuthn needs.
//! * `vault` — `VaultService` operations with origin binding.
//!
//! The private key is generated, stored (inside a login's encrypted details)
//! and used only here. See docs/superpowers/specs/2026-09-23-passkeys-design.md.

use crate::error::{Error, Result};
use crate::model::MAX_USERNAME_CHARS;
use crate::secret::SecretBytes;
use data_encoding::BASE64URL_NOPAD;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

pub const MAX_PASSKEYS_PER_LOGIN: usize = 8;
/// Every credential ID HavenKeys creates has exactly this length.
pub const CREDENTIAL_ID_LEN: usize = 16;
pub const MAX_USER_HANDLE_BYTES: usize = 64;
pub const MIN_CHALLENGE_BYTES: usize = 1;
pub const MAX_CHALLENGE_BYTES: usize = 1024;
pub const MAX_CREDENTIAL_LIST: usize = 64;
/// COSE algorithm identifier for ES256 (ECDSA P-256 with SHA-256).
pub const COSE_ALG_ES256: i64 = -7;

/// Unpadded base64url.
pub fn encode_b64url(bytes: &[u8]) -> String {
    BASE64URL_NOPAD.encode(bytes)
}

/// Public binary data (credential IDs, user handles), stored as unpadded
/// base64url. Not secret, but identifying: `Debug` shows the length only.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct B64Url(pub Vec<u8>);

impl B64Url {
    pub fn encode(&self) -> String {
        encode_b64url(&self.0)
    }

    pub fn decode(s: &str) -> Result<Self> {
        BASE64URL_NOPAD
            .decode(s.as_bytes())
            .map(Self)
            .map_err(|_| Error::InvalidInput("invalid base64url"))
    }
}

impl fmt::Debug for B64Url {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "B64Url({} bytes)", self.0.len())
    }
}

impl Serialize for B64Url {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.encode())
    }
}

impl<'de> Deserialize<'de> for B64Url {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        B64Url::decode(&s).map_err(|_| serde::de::Error::custom("invalid base64url"))
    }
}

/// One passkey, as stored inside a login's encrypted details.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Passkey {
    pub credential_id: B64Url,
    /// Normalized relying-party ID (lowercase, punycode).
    pub rp_id: String,
    pub user_handle: B64Url,
    /// What the site called the account.
    pub user_name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    /// P-256 scalar, 32 bytes.
    pub private_key: SecretBytes,
    /// Unix milliseconds.
    pub created_at: i64,
}

impl fmt::Debug for Passkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Passkey(<redacted>)")
    }
}

/// A site-supplied account name: trimmed, bounded, no control characters.
/// Empty is allowed (some sites send none); `None` input stays `None`.
pub(crate) fn clean_site_name(name: Option<&str>) -> Result<Option<String>> {
    let Some(n) = name.map(str::trim) else {
        return Ok(None);
    };
    if n.chars().count() > MAX_USERNAME_CHARS || n.chars().any(char::is_control) {
        return Err(Error::InvalidInput(
            "account name is too long or contains control characters",
        ));
    }
    Ok(Some(n.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Passkey {
        Passkey {
            credential_id: B64Url(vec![1; CREDENTIAL_ID_LEN]),
            rp_id: "github.com".into(),
            user_handle: B64Url(vec![9, 9]),
            user_name: "octo".into(),
            display_name: None,
            private_key: SecretBytes::new(vec![7; 32]),
            created_at: 1,
        }
    }

    #[test]
    fn passkey_debug_is_redacted() {
        let printed = format!("{:?}", sample());
        assert!(!printed.contains("octo"));
        assert!(!printed.contains("github"));
        assert!(printed.contains("redacted"));
    }

    #[test]
    fn passkey_round_trips_as_base64url() {
        let json = serde_json::to_string(&sample()).unwrap();
        assert!(json.contains(r#""credentialId":"AQEBAQEBAQEBAQEBAQEBAQ""#));
        let back: Passkey = serde_json::from_str(&json).unwrap();
        assert_eq!(back.credential_id, B64Url(vec![1; CREDENTIAL_ID_LEN]));
        assert_eq!(back.private_key.expose(), &[7; 32]);
    }

    #[test]
    fn site_names_are_bounded() {
        assert_eq!(
            clean_site_name(Some("  a  ")).unwrap().as_deref(),
            Some("a")
        );
        assert_eq!(clean_site_name(Some("")).unwrap().as_deref(), Some(""));
        assert!(clean_site_name(Some("a\u{7}")).is_err());
        assert!(clean_site_name(Some(&"x".repeat(MAX_USERNAME_CHARS + 1))).is_err());
    }
}
