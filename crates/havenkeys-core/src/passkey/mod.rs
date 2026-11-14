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

mod app;
pub(crate) mod cbor;
mod key;
mod rp;
mod vault;
mod webauthn;

pub use app::{AppCreateQuery, AppPasskeyCreate};
pub(crate) use key::KeyMaterial;
pub use key::{
    choose_algorithm, Algorithm, PrfInput, PrfOutput, COSE_ALG_ED25519, COSE_ALG_EDDSA,
    COSE_ALG_ES256, COSE_ALG_ESP256, MAX_ALGORITHMS, MAX_PRF_SALT_BYTES, PRF_SECRET_LEN,
};
pub(crate) use rp::normalize_rp_id;
pub use rp::{
    android_app_origin, app_rp_id, authorize_rp, authorize_rp_for_app, RpContext, MAX_RP_ID_BYTES,
};
pub use vault::{
    CreateCheck, CreateQuery, PasskeyCreate, PasskeyInfo, PasskeyMatch, StagedPasskey, Upgrade,
};
pub(crate) use webauthn::{assert, register};
pub use webauthn::{
    authenticator_data, client_data_json, Assertion, Ceremony, NewUser, PrfCreate, PrfCreated,
    Registration,
};

pub const MAX_PASSKEYS_PER_LOGIN: usize = 8;
/// Every credential ID HavenKeys creates has exactly this length.
pub const CREDENTIAL_ID_LEN: usize = 16;
pub const MAX_USER_HANDLE_BYTES: usize = 64;
pub const MIN_CHALLENGE_BYTES: usize = 1;
pub const MAX_CHALLENGE_BYTES: usize = 4096;
pub const MAX_CREDENTIAL_LIST: usize = 64;

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
    /// The key material, decoded only by `key::KeyMaterial`: a 32-byte
    /// P-256 scalar (every passkey made before 2026-11-14, and ES256 ones
    /// made without `prf`), or a versioned envelope holding the key type,
    /// key and PRF secret that older versions refuse as damaged but keep
    /// byte for byte (spec 2026-11-14-passkey-parity §4). No new field on
    /// `Passkey` carries key material: older versions would drop it on an
    /// edit.
    pub private_key: SecretBytes,
    /// Unix milliseconds.
    pub created_at: i64,
    /// When a newer passkey for the same account replaced this one (Unix
    /// milliseconds). Kept until the site shows it knows the new one
    /// (security-review PK5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaced_at: Option<i64>,
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
            replaced_at: None,
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

    /// `Passkey` exactly as HavenKeys 0.35 and earlier declare it: no
    /// `deny_unknown_fields`, no catch-all for unknown fields, and
    /// `private_key` read as a P-256 scalar.
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct OldPasskey {
        credential_id: B64Url,
        rp_id: String,
        user_handle: B64Url,
        user_name: String,
        #[serde(default)]
        display_name: Option<String>,
        private_key: SecretBytes,
        created_at: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        replaced_at: Option<i64>,
    }

    fn new_passkey(alg: Algorithm, prf: bool) -> Passkey {
        let ctx = RpContext {
            rp_id: "github.com".into(),
            origin: "https://github.com".into(),
            cross_origin: false,
            top_origin: None,
        };
        let user = NewUser {
            user_handle: &[1, 2, 3],
            user_name: "octo".into(),
            display_name: None,
        };
        let prf = prf.then_some(PrfCreate { eval: None });
        register(&ctx, &[1; 32], user, 5, alg, prf).unwrap().0
    }

    #[test]
    fn older_versions_load_new_passkeys_and_cannot_misuse_them() {
        for (alg, prf) in [
            (Algorithm::Ed25519, false),
            (Algorithm::EdDsa, true),
            (Algorithm::Es256, true),
        ] {
            let new = new_passkey(alg, prf);
            let json = serde_json::to_string(&new).unwrap();
            // The stored shape is unchanged: no new field for an older
            // version to drop.
            let v: serde_json::Value = serde_json::from_str(&json).unwrap();
            let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
            keys.sort_unstable();
            assert_eq!(
                keys,
                [
                    "createdAt",
                    "credentialId",
                    "displayName",
                    "privateKey",
                    "rpId",
                    "userHandle",
                    "userName"
                ]
            );
            // An older version loads it (the login still opens)...
            let old: OldPasskey = serde_json::from_str(&json).unwrap();
            // ...and refuses to sign with it, as its `assert` does.
            assert!(p256::ecdsa::SigningKey::from_slice(old.private_key.expose()).is_err());
            // An edit there writes the passkey back byte for byte, so the
            // key and the PRF secret survive and work here again.
            let rewritten = serde_json::to_string(&old).unwrap();
            let back: Passkey = serde_json::from_str(&rewritten).unwrap();
            assert_eq!(back.private_key.expose(), new.private_key.expose());
            let m = KeyMaterial::decode(&back.private_key).unwrap();
            assert_eq!(m.has_prf(), prf);
        }
    }

    #[test]
    fn plain_es256_passkeys_keep_the_old_format() {
        for alg in [Algorithm::Es256, Algorithm::Esp256] {
            let new = new_passkey(alg, false);
            let old: OldPasskey =
                serde_json::from_str(&serde_json::to_string(&new).unwrap()).unwrap();
            assert_eq!(old.private_key.expose().len(), 32);
            assert!(p256::ecdsa::SigningKey::from_slice(old.private_key.expose()).is_ok());
        }
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
