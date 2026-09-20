//! Reading the invite string a server operator handed the user.
//!
//! The format is restated here rather than shared with `havenkeys-server`,
//! because the two sides must not depend on each other: the server is the
//! authority that issues and burns an invite, and the client only needs to
//! read the three things it carries — which server to talk to, which account
//! to derive keys for, and the secret that authorizes activation.
//!
//! `havenkeys-server`'s `invite.rs` and this file describe the same format;
//! `tests/round_trip.rs` fails if they ever disagree.

use crate::error::{Result, SyncError};
use data_encoding::BASE64URL_NOPAD;
use serde::Deserialize;
use uuid::Uuid;
use zeroize::Zeroize;

const PREFIX: &str = "HKINV1-";
const MAX_INVITE_CHARS: usize = 2048;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Invite {
    /// Where the account lives.
    pub server: String,
    pub email: String,
    pub account: Uuid,
    /// Single-use, and only ever sent back to `server`.
    pub secret: String,
}

impl Drop for Invite {
    fn drop(&mut self) {
        self.secret.zeroize();
    }
}

/// Parse an invite. Every failure is the same error: a user who mistyped one
/// character and a user who pasted something else entirely get the same
/// answer, and nothing about the expected shape is revealed.
pub fn decode(raw: &str) -> Result<Invite> {
    const BAD: SyncError = SyncError::Refused("that invite is not valid");
    let raw = raw.trim();
    if raw.len() > MAX_INVITE_CHARS {
        return Err(BAD);
    }
    let body = raw.strip_prefix(PREFIX).ok_or(BAD)?;
    let json = BASE64URL_NOPAD.decode(body.as_bytes()).map_err(|_| BAD)?;
    let invite: Invite = serde_json::from_slice(&json).map_err(|_| BAD)?;
    if invite.email.trim().is_empty() || invite.secret.is_empty() {
        return Err(BAD);
    }
    Ok(invite)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_well_formed_invite_is_read() {
        let json = br#"{"server":"https://vault.example.com","email":"user@example.com","account":"00000000-0000-0000-0000-000000000001","secret":"s3cret"}"#;
        let raw = format!("{PREFIX}{}", BASE64URL_NOPAD.encode(json));
        let invite = decode(&raw).unwrap();
        assert_eq!(invite.server, "https://vault.example.com");
        assert_eq!(invite.email, "user@example.com");
        assert_eq!(invite.account, Uuid::from_u128(1));
    }

    #[test]
    fn anything_else_is_refused_the_same_way() {
        for bad in [
            "",
            "HKINV1-",
            "HKINV2-abc",
            "HKINV1-!!!",
            "not-an-invite",
            &format!("{PREFIX}{}", BASE64URL_NOPAD.encode(b"{}")),
        ] {
            assert!(decode(bad).is_err(), "should have refused {bad:?}");
        }
    }
}
