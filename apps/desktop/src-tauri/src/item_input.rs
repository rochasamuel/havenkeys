//! The item create/update request as the desktop renderer sends it. The same
//! as the core's `ItemInput`, except that `totp` may also name a scanned QR
//! code by token (design §4.4); the URI is looked up here, in Rust, so the
//! core sees an ordinary `Set` and the renderer never saw the secret.

use crate::scan_slot::ScanSlot;
use crate::state::{CmdError, CmdResult};
use havenkeys_core::model::{ItemInput, ItemType, SecretUpdate, UrlRule};
use havenkeys_core::SecretString;
use serde::Deserialize;
use std::time::Instant;

#[derive(Default, Deserialize)]
#[serde(tag = "op", content = "value", rename_all = "snake_case")]
pub enum TotpUpdate {
    #[default]
    Keep,
    Set(SecretString),
    Clear,
    /// A token from `scan_totp_qr`.
    Scanned(String),
}

/// Mirrors `havenkeys_core::model::ItemInput` field for field; a field the
/// core gains must be added here too (`deny_unknown_fields` makes a UI that
/// sends it fail loudly rather than lose it).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ItemInputWire {
    pub item_type: ItemType,
    pub title: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub urls: Vec<UrlRule>,
    #[serde(default)]
    pub password: SecretUpdate,
    #[serde(default)]
    pub totp: TotpUpdate,
    #[serde(default)]
    pub notes: SecretUpdate,
    #[serde(default)]
    pub content: SecretUpdate,
    #[serde(default)]
    pub auto_sign_in: Option<bool>,
    #[serde(default)]
    pub sign_in_with: Option<havenkeys_core::sso::SignInWith>,
    #[serde(default)]
    pub identity: Option<havenkeys_core::identity::IdentityFields>,
    #[serde(default)]
    pub card: Option<havenkeys_core::card::CardInput>,
}

fn scan_expired() -> CmdError {
    CmdError {
        code: "scan_expired",
        message: "The scanned code expired. Scan it again.".into(),
    }
}

impl ItemInputWire {
    /// Whether saving this uses a scanned code, so the caller can empty the
    /// slot once the save has gone through.
    pub fn uses_scan(&self) -> bool {
        matches!(self.totp, TotpUpdate::Scanned(_))
    }

    pub fn resolve(self, slot: &ScanSlot, now: Instant) -> CmdResult<ItemInput> {
        let totp = match self.totp {
            TotpUpdate::Keep => SecretUpdate::Keep,
            TotpUpdate::Set(value) => SecretUpdate::Set(value),
            TotpUpdate::Clear => SecretUpdate::Clear,
            TotpUpdate::Scanned(token) => {
                SecretUpdate::Set(slot.get(&token, now).ok_or_else(scan_expired)?)
            }
        };
        Ok(ItemInput {
            item_type: self.item_type,
            title: self.title,
            username: self.username,
            urls: self.urls,
            password: self.password,
            totp,
            notes: self.notes,
            content: self.content,
            auto_sign_in: self.auto_sign_in,
            sign_in_with: self.sign_in_with,
            identity: self.identity,
            card: self.card,
            sections: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qr_scan::ScannedCode;
    use serde_json::json;

    fn wire(totp: serde_json::Value) -> ItemInputWire {
        serde_json::from_value(json!({
            "itemType": "login",
            "title": "GitHub",
            "username": "alice",
            "urls": [{ "url": "https://github.com", "matchType": "domain" }],
            "password": { "op": "keep" },
            "totp": totp,
            "autoSignIn": false
        }))
        .unwrap()
    }

    fn slot_with(uri: &str, now: Instant) -> (ScanSlot, String) {
        let mut slot = ScanSlot::default();
        let token = slot
            .replace(
                vec![ScannedCode {
                    uri: SecretString::new(uri.into()),
                    issuer: None,
                    account: None,
                }],
                now,
            )
            .unwrap()
            .remove(0)
            .token;
        (slot, token)
    }

    #[test]
    fn a_scanned_token_becomes_a_set_with_the_scanned_uri() {
        let now = Instant::now();
        let (slot, token) = slot_with("otpauth://totp/x?secret=JBSWY3DPEHPK3PXP", now);
        let input = wire(json!({ "op": "scanned", "value": token }));
        assert!(input.uses_scan());
        let input = input.resolve(&slot, now).unwrap();
        match input.totp {
            SecretUpdate::Set(v) => {
                assert_eq!(v.expose(), "otpauth://totp/x?secret=JBSWY3DPEHPK3PXP")
            }
            _ => panic!("expected Set"),
        }
        assert_eq!(input.title, "GitHub");
        assert_eq!(input.username.as_deref(), Some("alice"));
        assert_eq!(input.urls.len(), 1);
        assert!(matches!(input.password, SecretUpdate::Keep));
        assert_eq!(input.auto_sign_in, Some(false));
    }

    #[test]
    fn an_unknown_or_expired_token_is_refused() {
        let now = Instant::now();
        let (slot, token) = slot_with("otpauth://totp/x?secret=JBSWY3DPEHPK3PXP", now);
        let err = wire(json!({ "op": "scanned", "value": "feed" }))
            .resolve(&slot, now)
            .err()
            .unwrap();
        assert_eq!(err.code, "scan_expired");
        let late = now + crate::scan_slot::SCAN_TTL;
        assert!(wire(json!({ "op": "scanned", "value": token }))
            .resolve(&slot, late)
            .is_err());
    }

    #[test]
    fn the_other_totp_updates_pass_through() {
        let slot = ScanSlot::default();
        let now = Instant::now();
        let keep = wire(json!({ "op": "keep" }));
        assert!(!keep.uses_scan());
        assert!(matches!(
            keep.resolve(&slot, now).unwrap().totp,
            SecretUpdate::Keep
        ));
        assert!(matches!(
            wire(json!({ "op": "clear" }))
                .resolve(&slot, now)
                .unwrap()
                .totp,
            SecretUpdate::Clear
        ));
        match wire(json!({ "op": "set", "value": "JBSWY3DPEHPK3PXP" }))
            .resolve(&slot, now)
            .unwrap()
            .totp
        {
            SecretUpdate::Set(v) => assert_eq!(v.expose(), "JBSWY3DPEHPK3PXP"),
            _ => panic!("expected Set"),
        }
    }

    #[test]
    fn unknown_fields_are_still_refused() {
        let bad = serde_json::from_value::<ItemInputWire>(json!({
            "itemType": "login", "title": "x", "extra": 1
        }));
        assert!(bad.is_err());
    }
}
