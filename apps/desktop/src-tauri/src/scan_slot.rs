//! The QR scans whose codes are waiting to be saved (design §4.2). The
//! renderer only ever holds a token and the labels; the `otpauth://` URI
//! stays here until an item is saved with it, the vault locks, a new scan
//! is added beside it, or five minutes pass.

use crate::qr_scan::ScannedCode;
use havenkeys_core::crypto::fill_random;
use havenkeys_core::SecretString;
use serde::Serialize;
use std::time::{Duration, Instant};

pub const SCAN_TTL: Duration = Duration::from_secs(5 * 60);

/// What the renderer gets for each code found: never the URI.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannedTotp {
    pub token: String,
    pub issuer: Option<String>,
    pub account: Option<String>,
}

/// The most scanned codes held at once; the oldest go first.
pub const MAX_HELD: usize = 16;

#[derive(Default)]
pub struct ScanSlot {
    entries: Vec<Entry>,
}

struct Entry {
    created: Instant,
    token: String,
    uri: SecretString,
}

impl ScanSlot {
    /// Hold `codes` beside any earlier scans (the editor can scan the
    /// login's TOTP and several one-time-password fields before one save),
    /// each under a fresh 128-bit token and its own expiry. Past
    /// `MAX_HELD` the oldest are dropped.
    pub fn add(
        &mut self,
        codes: Vec<ScannedCode>,
        now: Instant,
    ) -> havenkeys_core::Result<Vec<ScannedTotp>> {
        let mut previews = Vec::with_capacity(codes.len());
        for code in codes {
            let mut raw = [0u8; 16];
            fill_random(&mut raw)?;
            let token: String = raw.iter().map(|b| format!("{b:02x}")).collect();
            previews.push(ScannedTotp {
                token: token.clone(),
                issuer: code.issuer,
                account: code.account,
            });
            self.entries.push(Entry {
                created: now,
                token,
                uri: code.uri,
            });
        }
        if self.entries.len() > MAX_HELD {
            let extra = self.entries.len() - MAX_HELD;
            self.entries.drain(..extra);
        }
        Ok(previews)
    }

    /// The URI for `token` while its scan is fresh. Looking it up does not
    /// use it up, so a save that fails can be retried; `clear` after a
    /// successful save.
    pub fn get(&self, token: &str, now: Instant) -> Option<SecretString> {
        self.entries
            .iter()
            .find(|e| e.token == token)
            .filter(|e| now.saturating_duration_since(e.created) < SCAN_TTL)
            .map(|e| e.uri.clone())
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(issuer: &str, uri: &str) -> ScannedCode {
        ScannedCode {
            uri: SecretString::new(uri.to_owned()),
            issuer: Some(issuer.to_owned()),
            account: None,
        }
    }

    #[test]
    fn a_token_finds_its_code_until_cleared() {
        let t0 = Instant::now();
        let mut slot = ScanSlot::default();
        let out = slot
            .add(vec![code("A", "uri-a"), code("B", "uri-b")], t0)
            .unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[1].issuer.as_deref(), Some("B"));
        assert_eq!(out[0].token.len(), 32);
        assert_ne!(out[0].token, out[1].token);
        // Looking a token up does not use it: a failed save can be retried.
        assert_eq!(slot.get(&out[1].token, t0).unwrap().expose(), "uri-b");
        assert_eq!(slot.get(&out[1].token, t0).unwrap().expose(), "uri-b");
        slot.clear();
        assert!(slot.get(&out[1].token, t0).is_none());
    }

    #[test]
    fn unknown_and_expired_tokens_find_nothing() {
        let t0 = Instant::now();
        let mut slot = ScanSlot::default();
        assert!(slot.get("00", t0).is_none());
        let out = slot.add(vec![code("A", "uri-a")], t0).unwrap();
        assert!(slot.get("not-a-token", t0).is_none());
        assert!(slot
            .get(&out[0].token, t0 + SCAN_TTL - Duration::from_secs(1))
            .is_some());
        assert!(slot.get(&out[0].token, t0 + SCAN_TTL).is_none());
    }

    #[test]
    fn a_new_scan_keeps_the_earlier_ones() {
        let t0 = Instant::now();
        let mut slot = ScanSlot::default();
        let old = slot.add(vec![code("A", "uri-a")], t0).unwrap();
        let new = slot.add(vec![code("B", "uri-b")], t0).unwrap();
        assert_eq!(slot.get(&old[0].token, t0).unwrap().expose(), "uri-a");
        assert_eq!(slot.get(&new[0].token, t0).unwrap().expose(), "uri-b");
    }

    #[test]
    fn each_scan_expires_on_its_own() {
        let t0 = Instant::now();
        let mut slot = ScanSlot::default();
        let old = slot.add(vec![code("A", "uri-a")], t0).unwrap();
        let t1 = t0 + Duration::from_secs(200);
        let new = slot.add(vec![code("B", "uri-b")], t1).unwrap();
        let later = t0 + SCAN_TTL;
        assert!(slot.get(&old[0].token, later).is_none());
        assert!(slot.get(&new[0].token, later).is_some());
    }

    #[test]
    fn the_cap_drops_the_oldest_first() {
        let t0 = Instant::now();
        let mut slot = ScanSlot::default();
        let mut tokens = Vec::new();
        for i in 0..MAX_HELD + 2 {
            let out = slot.add(vec![code("A", &format!("uri-{i}"))], t0).unwrap();
            tokens.push(out[0].token.clone());
        }
        assert!(slot.get(&tokens[0], t0).is_none());
        assert!(slot.get(&tokens[1], t0).is_none());
        assert!(slot.get(&tokens[2], t0).is_some());
        assert!(slot.get(&tokens[MAX_HELD + 1], t0).is_some());
    }

    #[test]
    fn the_preview_never_carries_the_uri() {
        let mut slot = ScanSlot::default();
        let out = slot
            .add(vec![code("A", "otpauth://totp/x?secret=S")], Instant::now())
            .unwrap();
        let json = serde_json::to_string(&out).unwrap();
        assert!(!json.contains("otpauth"));
        assert!(json.contains("\"token\""));
        assert!(json.contains("\"issuer\":\"A\""));
    }
}
