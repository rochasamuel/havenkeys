use crate::error::{MobileError, MobileResult};
use crate::qr;
use crate::vault::{MobileVault, Status};
use havenkeys_client::kit::parse_kit;
use havenkeys_core::SecretString;

const KIT_PREFIX: &str = "havenkeys://kit/";

#[derive(uniffi::Record)]
pub struct LumaFrame {
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

/// What the app shows after a scan: never the Secret Key.
#[derive(uniffi::Record)]
pub struct KitPreview {
    pub email: String,
    pub server_url: String,
}

/// What the app shows for a typed setup code: the address and the server.
#[derive(uniffi::Record)]
pub struct InvitePreview {
    pub email: String,
    pub server_url: String,
}

#[uniffi::export]
impl MobileVault {
    /// Decode a camera frame. A HavenKeys kit is kept here, in Rust, until
    /// `sign_in_with_kit`; the app only learns the address and the server.
    pub fn scan_kit(&self, frame: LumaFrame) -> MobileResult<Option<KitPreview>> {
        let Some(text) = qr::decode(frame.bytes, frame.width, frame.height) else {
            return Ok(None);
        };
        if !text.starts_with(KIT_PREFIX) {
            return Ok(None);
        }
        let kit = parse_kit(&text)?;
        let preview = KitPreview {
            email: kit.email.clone(),
            server_url: kit.server_url.clone(),
        };
        *self.kit.lock().map_err(|_| MobileError::internal())? = Some(kit);
        Ok(Some(preview))
    }

    /// The kit stays held after a failure, so a mistyped password can be
    /// retried without scanning again.
    pub fn sign_in_with_kit(&self, password: String) -> MobileResult<Status> {
        let (server, email, key) = {
            let kit = self.kit.lock().map_err(|_| MobileError::internal())?;
            let kit = kit
                .as_ref()
                .ok_or(havenkeys_client::ClientError::invalid_kit())?;
            (
                kit.server_url.clone(),
                kit.email.clone(),
                kit.secret_key.clone(),
            )
        };
        let client = self.client.clone();
        self.block_on(client.sign_in(server, email, SecretString::new(password), Some(key)))?;
        self.forget_kit();
        self.status()
    }

    pub fn sign_in(
        &self,
        server_url: String,
        email: String,
        password: String,
        secret_key: String,
    ) -> MobileResult<Status> {
        let client = self.client.clone();
        self.block_on(client.sign_in(
            server_url,
            email,
            SecretString::new(password),
            Some(SecretString::new(secret_key)),
        ))?;
        self.status()
    }

    /// Decodes a setup code without touching the network or the secret in it.
    pub fn preview_invite(&self, invite: String) -> MobileResult<InvitePreview> {
        let p = self.client.preview_invite(&invite)?;
        Ok(InvitePreview {
            email: p.email,
            server_url: p.server_url,
        })
    }

    pub fn activate(&self, invite: String, password: String) -> MobileResult<Status> {
        let client = self.client.clone();
        self.block_on(client.activate(invite, SecretString::new(password)))?;
        self.status()
    }
}

#[cfg(test)]
mod tests {
    use crate::onboarding::LumaFrame;
    use crate::qr::tests::frame_of;
    use crate::vault::tests::mobile;
    use crate::MobileError;

    #[test]
    fn a_kit_code_is_held_and_a_foreign_code_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let (vault, _) = mobile(dir.path());
        let key = havenkeys_core::crypto::secret_key::SecretKey::generate().unwrap();
        let text = format!(
            "havenkeys://kit/v2?account={}&email=a%40example.com&key={}&server=https%3A%2F%2Fv.example.com",
            uuid::Uuid::from_u128(5),
            key.to_text().expose()
        );
        let (bytes, width, height) = frame_of(&text);
        let preview = vault
            .scan_kit(LumaFrame {
                width,
                height,
                bytes,
            })
            .unwrap()
            .unwrap();
        assert_eq!(preview.email, "a@example.com");
        assert_eq!(preview.server_url, "https://v.example.com");
        assert!(vault.kit.lock().unwrap().is_some());

        let (bytes, width, height) = frame_of("https://example.com");
        assert!(vault
            .scan_kit(LumaFrame {
                width,
                height,
                bytes
            })
            .unwrap()
            .is_none());

        let (bytes, width, height) = frame_of("havenkeys://kit/v2?broken");
        match vault
            .scan_kit(LumaFrame {
                width,
                height,
                bytes,
            })
            .err()
            .unwrap()
        {
            MobileError::Failed { code, .. } => assert_eq!(code, "invalid_kit"),
        }
    }

    #[test]
    fn signing_in_with_no_scanned_kit_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let (vault, _) = mobile(dir.path());
        assert!(vault.sign_in_with_kit("whatever password".into()).is_err());
    }
}
