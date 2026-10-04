//! The phone's side of signing a new device in
//! (spec 2026-10-03-phone-approved-sign-in §3.2). Kotlin scans and shows;
//! Rust reads the code, talks to the server and seals the keys.

use crate::error::MobileResult;
use crate::onboarding::LumaFrame;
use crate::qr;
use crate::vault::MobileVault;
use havenkeys_core::pairing::PairingLink;

#[derive(uniffi::Record)]
pub struct PairingRequestView {
    /// The code as scanned, for `approve_pairing` / `deny_pairing`.
    pub link: String,
    pub device_name: String,
    pub ip: String,
    pub location: Option<String>,
    pub created_at: String,
}

#[uniffi::export]
impl MobileVault {
    /// A camera frame on the "Sign in a new device" screen. Only a
    /// `havenkeys://pair/v1` link comes back; any other code is dropped.
    pub fn scan_pairing(&self, frame: LumaFrame) -> MobileResult<Option<String>> {
        self.unlocked()?;
        let Some(text) = qr::decode(frame.bytes, frame.width, frame.height) else {
            return Ok(None);
        };
        Ok(PairingLink::parse(&text).ok().map(|_| text.to_string()))
    }

    pub fn pairing_request(&self, link: String) -> MobileResult<PairingRequestView> {
        self.touch();
        let client = self.client.clone();
        let r = self.block_on(async move { client.pairing_request(&link).await })?;
        Ok(PairingRequestView {
            link: r.link,
            device_name: r.device_name,
            ip: r.ip,
            location: r.location,
            created_at: r.created_at,
        })
    }

    /// Kotlin asks for biometrics before calling this.
    pub fn approve_pairing(&self, link: String) -> MobileResult<()> {
        self.touch();
        let client = self.client.clone();
        Ok(self.block_on(async move { client.approve_pairing(&link).await })?)
    }

    pub fn deny_pairing(&self, link: String) -> MobileResult<()> {
        self.touch();
        let client = self.client.clone();
        Ok(self.block_on(async move { client.deny_pairing(&link).await })?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::unlocked;

    fn frame(text: &str) -> LumaFrame {
        let (bytes, width, height) = crate::qr::tests::frame_of(text);
        LumaFrame {
            width,
            height,
            bytes,
        }
    }

    #[test]
    fn scan_pairing_keeps_only_pair_links() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        let keys = havenkeys_core::pairing::PairingKeys::generate();
        let link = havenkeys_core::pairing::PairingLink {
            server_url: "https://vault.example.com".into(),
            pairing_id: "AAAAAAAAAAAAAAAAAAAAAA".into(),
            public_key: keys.public_key(),
        }
        .to_text();
        assert_eq!(
            v.scan_pairing(frame(&link)).unwrap().as_deref(),
            Some(link.as_str())
        );
        assert_eq!(
            v.scan_pairing(frame("otpauth://totp/A?secret=JBSWY3DPEHPK3PXP"))
                .unwrap(),
            None
        );
        assert_eq!(
            v.scan_pairing(frame("havenkeys://kit/v2?x=1")).unwrap(),
            None
        );
        assert_eq!(v.scan_pairing(frame("https://example.com")).unwrap(), None);
    }

    #[test]
    fn a_locked_vault_scans_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        v.lock();
        assert!(v.scan_pairing(frame("https://example.com")).is_err());
    }
}
