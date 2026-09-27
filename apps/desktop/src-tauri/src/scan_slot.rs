//! The one QR scan whose codes are waiting to be saved (design §4.2). The
//! renderer only ever holds a token and the labels; the `otpauth://` URI
//! stays here until an item is saved with it, the vault locks, a new scan
//! replaces it, or five minutes pass.

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

#[derive(Default)]
pub struct ScanSlot {
    batch: Option<Batch>,
}

struct Batch {
    created: Instant,
    entries: Vec<(String, SecretString)>,
}

impl ScanSlot {
    /// Forget the previous scan and hold `codes`, each under a fresh
    /// 128-bit token.
    pub fn replace(
        &mut self,
        codes: Vec<ScannedCode>,
        now: Instant,
    ) -> havenkeys_core::Result<Vec<ScannedTotp>> {
        self.batch = None;
        let mut entries = Vec::with_capacity(codes.len());
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
            entries.push((token, code.uri));
        }
        self.batch = Some(Batch { created: now, entries });
        Ok(previews)
    }

    /// The URI for `token` while the scan is fresh. Looking it up does not
    /// use it up, so a save that fails can be retried; `clear` after a
    /// successful save.
    pub fn get(&self, token: &str, now: Instant) -> Option<SecretString> {
        let batch = self.batch.as_ref()?;
        if now.saturating_duration_since(batch.created) >= SCAN_TTL {
            return None;
        }
        batch
            .entries
            .iter()
            .find(|(t, _)| t == token)
            .map(|(_, uri)| uri.clone())
    }

    pub fn clear(&mut self) {
        self.batch = None;
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
        let out = slot.replace(vec![code("A", "uri-a"), code("B", "uri-b")], t0).unwrap();
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
        let out = slot.replace(vec![code("A", "uri-a")], t0).unwrap();
        assert!(slot.get("not-a-token", t0).is_none());
        assert!(slot.get(&out[0].token, t0 + SCAN_TTL - Duration::from_secs(1)).is_some());
        assert!(slot.get(&out[0].token, t0 + SCAN_TTL).is_none());
    }

    #[test]
    fn a_new_scan_replaces_the_old_one() {
        let t0 = Instant::now();
        let mut slot = ScanSlot::default();
        let old = slot.replace(vec![code("A", "uri-a")], t0).unwrap();
        let new = slot.replace(vec![code("B", "uri-b")], t0).unwrap();
        assert!(slot.get(&old[0].token, t0).is_none());
        assert_eq!(slot.get(&new[0].token, t0).unwrap().expose(), "uri-b");
    }

    #[test]
    fn the_preview_never_carries_the_uri() {
        let mut slot = ScanSlot::default();
        let out = slot.replace(vec![code("A", "otpauth://totp/x?secret=S")], Instant::now()).unwrap();
        let json = serde_json::to_string(&out).unwrap();
        assert!(!json.contains("otpauth"));
        assert!(json.contains("\"token\""));
        assert!(json.contains("\"issuer\":\"A\""));
    }
}
