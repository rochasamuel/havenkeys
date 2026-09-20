//! The single-use string that authorizes activation.
//!
//! The secret is 128 bits from the CSPRNG and is stored only as its SHA-256:
//! the input is already high-entropy, so a password hash would buy nothing.
//! It authorizes *creating* a vault for an account; it protects no data, and
//! it stops being accepted the moment the account activates.
//!
//! The string carries the account id because the client needs it before it
//! can derive the KEK (docs/crypto.md, key scheme 3), and looking it up by
//! email first would turn activation into an account-enumeration oracle.

use crate::error::ApiError;
use data_encoding::BASE64URL_NOPAD;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use zeroize::Zeroizing;

const PREFIX: &str = "HKINV1-";
const SECRET_BYTES: usize = 16;
const MAX_INVITE_CHARS: usize = 2048;

pub const INVITE_TTL_DAYS: i64 = 7;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Invite {
    pub server: String,
    pub email: String,
    pub account: Uuid,
    pub secret: String,
}

pub fn generate_secret() -> Zeroizing<String> {
    let mut bytes = Zeroizing::new([0u8; SECRET_BYTES]);
    rand::thread_rng().fill_bytes(bytes.as_mut());
    Zeroizing::new(BASE64URL_NOPAD.encode(bytes.as_ref()))
}

pub fn encode(invite: &Invite) -> String {
    let json = serde_json::to_vec(invite).expect("an invite always serializes");
    format!("{PREFIX}{}", BASE64URL_NOPAD.encode(&json))
}

/// Parse an invite string. Every failure is the same error: a prober must not
/// learn which part of a guess was wrong.
pub fn decode(raw: &str) -> Result<Invite, ApiError> {
    const BAD: ApiError = ApiError::InvalidRequest("invite is not valid");
    let raw = raw.trim();
    if raw.len() > MAX_INVITE_CHARS {
        return Err(BAD);
    }
    let body = raw.strip_prefix(PREFIX).ok_or(BAD)?;
    let json = BASE64URL_NOPAD.decode(body.as_bytes()).map_err(|_| BAD)?;
    serde_json::from_slice(&json).map_err(|_| BAD)
}

pub fn hash(secret: &str) -> [u8; 32] {
    Sha256::digest(secret.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_invite_round_trips_through_its_string() {
        let invite = Invite {
            server: "https://vault.example.com".into(),
            email: "user@example.com".into(),
            account: Uuid::from_u128(1),
            secret: "AAAAAAAAAAAAAAAAAAAAAA".into(),
        };
        let encoded = encode(&invite);
        assert!(encoded.starts_with("HKINV1-"));
        assert_eq!(decode(&encoded).unwrap(), invite);
    }

    #[test]
    fn a_mangled_invite_is_refused_rather_than_guessed() {
        for bad in ["", "HKINV1-", "HKINV2-abc", "HKINV1-!!!", "not-an-invite"] {
            assert!(decode(bad).is_err(), "should have refused {bad:?}");
        }
    }

    #[test]
    fn an_invite_with_extra_fields_is_refused() {
        let json = br#"{"server":"s","email":"e@x.com","account":"00000000-0000-0000-0000-000000000001","secret":"s","extra":1}"#;
        let raw = format!("{PREFIX}{}", BASE64URL_NOPAD.encode(json));
        assert!(decode(&raw).is_err());
    }

    #[test]
    fn two_generated_secrets_differ() {
        assert_ne!(*generate_secret(), *generate_secret());
    }
}
